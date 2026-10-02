//! C9 稽核測試——跨域標記、Who→Employee→Task→Policy→Scope 鏈重構、保留策略、身份稽核。

use crate::domain::models::{Task, TaskStatus};
use crate::domain::{SqliteStore, Store as _};
use crate::knowledge::bootstrap::{bootstrap_with_sources, load_policy_fail_closed};
use crate::knowledge::identity::{
    ensure_operator_principal, operator_access_context, set_principal_attrs,
    OPERATOR_PRINCIPAL_ID,
};
use crate::knowledge::service::{prune_receipts, KnowledgeService, RECEIPT_RETENTION_DAYS};
use crate::knowledge::types::{AccessContext, KnowledgeScope, Principal, PrincipalAttrs, Visibility};
use crate::runtime::AGENT_WS;

fn store() -> SqliteStore {
    let dir = std::env::temp_dir().join(format!(
        "m2c9-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    SqliteStore::open(&dir.join("test.db")).unwrap()
}

/// **Test 9（C9 完整版）**：跨域標記＋Who→Employee→Task→Policy→Scope 鏈重構。
#[test]
fn c9_cross_domain_and_chain_reconstruction() {
    let s = store();
    bootstrap_with_sources(&s, &["src-a".into()]).unwrap(); // co-common←src-a
    s.put_scope(&KnowledgeScope {
        id: "proj-x".into(),
        visibility: Visibility::Project,
        classification: "internal".into(),
        source_ids: vec!["src-x".into()],
        owner: None,
        department: None,
        project: Some("proj-x".into()),
    })
    .unwrap();
    s.put_task(&Task {
        id: "t1".into(),
        workspace_id: AGENT_WS.into(),
        owner_employee_id: "alice".into(),
        objective: "o".into(),
        input: "i".into(),
        status: TaskStatus::InProgress,
        output_artifact_id: None,
        commitment_id: None,
        project_id: Some("proj-x".into()),
        external_reply_to: None,
        external_source: None,
        created_at: "2026-10-02T00:00:00Z".into(),
    })
    .unwrap();
    let svc = KnowledgeService::new("unused.db");

    // 跨域（2 scopes）：operator 全權 → co-common＋proj-x。
    let access = AccessContext {
        task_id: Some("t1".into()),
        purpose: Some("調查 PO 逾期".into()),
        ..operator_access_context(AGENT_WS)
    };
    let plan = svc.plan(&s, &access).unwrap();
    assert_eq!(plan.scope_ids.len(), 2);
    let rid = svc.record(&s, &access, "search", &plan, false, 2).unwrap();
    let receipts = s.list_recent_receipts(10).unwrap();
    let r = receipts.iter().find(|r| r.id == rid).expect("receipt 落表");
    // 鏈重構：Who→Employee→Task→Policy→Scope。
    assert_eq!(r.principal_id, "principal-operator");
    assert_eq!(r.employee_id, None);
    assert_eq!(r.task_id.as_deref(), Some("t1"));
    assert_eq!(r.purpose.as_deref(), Some("調查 PO 逾期"));
    assert!(r.cross_domain, "兩個 scope＝跨域檢索（Rule 8）");
    assert_eq!(r.returned_sources, 2);
    assert_eq!(r.authorized_scopes, vec!["co-common".to_string(), "proj-x".to_string()]);
    assert_eq!(r.policy_version, 1);

    // 單域（1 scope）：跨域標記為 false。
    let (policy, _) = load_policy_fail_closed(&s);
    let _ = policy;
    crate::knowledge::bootstrap::save_policy_new_version(
        &s,
        vec![crate::knowledge::types::PolicyRule {
            id: "common-only".into(),
            priority: 10,
            effect: crate::knowledge::types::Effect::Allow,
            principals: None,
            principal_types: None,
            scopes: Some(vec!["co-common".into()]),
            departments: None,
            projects: None,
            classifications: None,
            department_membership: false,
            project_membership: false,
        }],
    )
    .unwrap();
    let plan2 = svc.plan(&s, &access).unwrap();
    let rid2 = svc.record(&s, &access, "search", &plan2, false, 1).unwrap();
    let r2 = s.list_recent_receipts(10).unwrap().into_iter().find(|r| r.id == rid2).unwrap();
    assert!(!r2.cross_domain, "單一 scope＝非跨域");
    assert_eq!(r2.policy_version, 2, "policy 版本隨變更遞增（鏈可重構）");
}

/// **C9 保留策略**：90 天前的 receipts 被清、新的保留、清除留事件。
#[test]
fn c9_prune_retention() {
    let s = store();
    let old = crate::knowledge::service::RetrievalReceipt {
        id: "rcpt-old".into(),
        principal_id: "principal-operator".into(),
        employee_id: None,
        kind: "search".into(),
        denied: false,
        authorized_sources: vec![],
        authorized_scopes: vec![],
        cross_domain: false,
        returned_sources: 0,
        policy_version: 1,
        workspace_id: AGENT_WS.into(),
        task_id: None,
        purpose: None,
        created_at: (chrono::Utc::now() - chrono::Duration::days(100)).to_rfc3339(),
    };
    let fresh = crate::knowledge::service::RetrievalReceipt { created_at: chrono::Utc::now().to_rfc3339(), id: "rcpt-new".into(), ..old.clone() };
    s.put_receipt(&old).unwrap();
    s.put_receipt(&fresh).unwrap();

    let n = prune_receipts(&s, RECEIPT_RETENTION_DAYS).unwrap();
    assert_eq!(n, 1, "清除 1 筆逾期 receipt");
    let left = s.list_recent_receipts(10).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, "rcpt-new");
    let events = s.list_recent_events(10).unwrap();
    assert!(events.iter().any(|e| e.kind == "receipts_pruned"));
}

/// **C9 身份稽核**：principal_created 冪等（僅首次）、attrs 變更留事件、缺列拒絕。
#[test]
fn c9_identity_audit() {
    let s = store();
    ensure_operator_principal(&s).unwrap();
    ensure_operator_principal(&s).unwrap(); // 冪等
    let events = s.list_recent_events(20).unwrap();
    assert_eq!(
        events.iter().filter(|e| e.kind == "principal_created").count(),
        1,
        "principal_created 只記一次"
    );

    set_principal_attrs(
        &s,
        OPERATOR_PRINCIPAL_ID,
        PrincipalAttrs { roles: vec![], departments: vec!["quality".into()], projects: vec![] },
    )
    .unwrap();
    let p = s.get_principal(OPERATOR_PRINCIPAL_ID).unwrap().unwrap();
    assert_eq!(p.attrs.departments, vec!["quality".to_string()]);
    let events = s.list_recent_events(20).unwrap();
    assert!(events.iter().any(|e| e.kind == "principal_attrs_changed"));

    // 缺列 → Err（不會靜默建立身份——建立只有 bootstrap 一條路）。
    assert!(set_principal_attrs(
        &s,
        "ai:ghost",
        PrincipalAttrs::default(),
    )
    .is_err());
    let _ = Principal { id: String::new(), principal_type: crate::knowledge::types::PrincipalType::Human, employee_id: None, display_name: String::new(), attrs: PrincipalAttrs::default(), token_hash: None };
}
