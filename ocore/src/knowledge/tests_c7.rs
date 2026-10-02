//! C7 條件式擴充的測試（Test 2 完整版＋attrs 富集）。

use crate::domain::tools::ToolOutput;
use crate::knowledge::backend::{BackendQuery, KnowledgeBackend, RetrieveKind};
use crate::knowledge::fake::{FakeBackend, FakeDoc};
use crate::knowledge::policy::authorized_sources;
use crate::knowledge::tests_m1::tool_ctx;
use crate::knowledge::types::{
    AccessContext, Effect, KnowledgePolicy, KnowledgeScope, PolicyRule, PrincipalType, Visibility,
};

/// **Test 2（C7 完整版）**：跨部門成員制——允許與限制並存。
///
/// 佈局：co-common（全公司）＋dept-quality/dept-mfg（成員制）＋restricted-hr（hr 白名單）。
/// quality 成員：co-common＋dept-quality（允許的跨域）；R&D 成員：僅 co-common（限制）；
/// hr 成員：經優先序白名單取 restricted；其他人被全域 deny 擋住（先到先得）。
#[tokio::test]
async fn m1_t2_cross_department_allow_and_restrict() {
    let mut b = FakeBackend::new();
    b.add_doc("src-c", FakeDoc { title: "Common".into(), body: "Company wide common protocol knowledge.".into() });
    b.add_doc("src-q", FakeDoc { title: "Quality".into(), body: "Quality NCR protocol for Q.".into() });
    b.add_doc("src-m", FakeDoc { title: "Mfg".into(), body: "Manufacturing yield protocol for M.".into() });
    b.add_doc("src-hr", FakeDoc { title: "HR".into(), body: "Restricted HR protocol material.".into() });
    let scopes = vec![
        KnowledgeScope { id: "co-common".into(), visibility: Visibility::Company, classification: "internal".into(), source_ids: vec!["src-c".into()], owner: None, department: None, project: None },
        KnowledgeScope { id: "dept-quality".into(), visibility: Visibility::Department, classification: "internal".into(), source_ids: vec!["src-q".into()], owner: None, department: Some("quality".into()), project: None },
        KnowledgeScope { id: "dept-mfg".into(), visibility: Visibility::Department, classification: "internal".into(), source_ids: vec!["src-m".into()], owner: None, department: Some("manufacturing".into()), project: None },
        KnowledgeScope { id: "restricted-hr".into(), visibility: Visibility::Restricted, classification: "confidential".into(), source_ids: vec!["src-hr".into()], owner: None, department: None, project: None },
    ];
    let member_ctx = |pid: &str, departments: &[&str]| AccessContext {
        principal_id: pid.into(),
        principal_type: PrincipalType::AiEmployee,
        employee_id: Some(pid.trim_start_matches("ai:").to_string()),
        workspace_id: "ws-default".into(),
        roles: vec![],
        departments: departments.iter().map(|s| s.to_string()).collect(),
        projects: vec![],
        task_id: None,
        purpose: None,
    };
    // 規則（先到先得）：p1 hr 白名單 → p20 全域 deny restricted → p10 co-common 全體 → p10 部門成員制。
    let mut allow_hr = PolicyRule {
        id: "allow-hr".into(),
        priority: 1,
        effect: Effect::Allow,
        principals: Some(vec!["ai:dave".into()]),
        principal_types: None,
        scopes: Some(vec!["restricted-hr".into()]),
        departments: None,
        projects: None,
        classifications: None,
        department_membership: false,
        project_membership: false,
    };
    allow_hr.priority = 1;
    let deny_restricted = PolicyRule {
        id: "deny-restricted".into(),
        priority: 20,
        effect: Effect::Deny,
        principals: None,
        principal_types: None,
        scopes: Some(vec!["restricted-hr".into()]),
        departments: None,
        projects: None,
        classifications: None,
        department_membership: false,
        project_membership: false,
    };
    let allow_common = PolicyRule {
        id: "common".into(),
        priority: 10,
        effect: Effect::Allow,
        principals: None,
        principal_types: None,
        scopes: Some(vec!["co-common".into()]),
        departments: None,
        projects: None,
        classifications: None,
        department_membership: false,
        project_membership: false,
    };
    let member = PolicyRule {
        id: "member".into(),
        priority: 10,
        effect: Effect::Allow,
        principals: None,
        principal_types: None,
        scopes: None,
        departments: None,
        projects: None,
        classifications: None,
        department_membership: true,
        project_membership: false,
    };
    let policy = KnowledgePolicy { version: 1, rules: vec![allow_hr, deny_restricted, allow_common, member] };

    async fn run(
        b: &FakeBackend,
        policy: &KnowledgePolicy,
        scopes: &[KnowledgeScope],
        access: &AccessContext,
        q: &str,
    ) -> ToolOutput {
        let source_ids = authorized_sources(policy, access, scopes);
        let tctx = tool_ctx();
        b.retrieve(
            access,
            BackendQuery { kind: RetrieveKind::Search, query: q.to_string(), anchor: None, source_ids, limit: 10 },
            &tctx,
        )
        .await
        .expect("fake ok")
    }

    let alice = run(&b, &policy, &scopes, &member_ctx("ai:alice", &["rd"]), "protocol").await;
    let carol = run(&b, &policy, &scopes, &member_ctx("ai:carol", &["quality"]), "protocol").await;
    let dave = run(&b, &policy, &scopes, &member_ctx("ai:dave", &["hr"]), "protocol").await;

    // R&D 成員：co-common 可達；quality/mfg/restricted 皆被擋（限制的跨域）。
    assert!(alice.text.contains("Common"));
    assert!(!alice.text.contains("Quality"), "R&D 不得見 quality：{}", alice.text);
    assert!(!alice.text.contains("Mfg"));
    assert!(!alice.text.contains("HR"));
    // quality 成員：co-common＋dept-quality（允許的跨域）；mfg/restricted 被擋。
    assert!(carol.text.contains("Common"));
    assert!(carol.text.contains("Quality"), "quality 成員經成員制取部門 scope：{}", carol.text);
    assert!(!carol.text.contains("Mfg"));
    assert!(!carol.text.contains("HR"), "受限內容永不入 context");
    // hr 成員：白名單優先於全域 deny（priority 先到先得）。
    assert!(dave.text.contains("HR"), "hr 白名單：{}", dave.text);
    assert!(dave.text.contains("Common"));
    assert!(!dave.text.contains("Quality"));
    let _ = ToolOutput { text: String::new(), meta: serde_json::json!({}) };
}

/// **C7b**：attrs 富集——store 內 principal 列的屬性疊加進 AccessContext（D-C7b）；
/// 無列者行為同 M1（推導基底不變、身份仍為推導——列只供 attrs，防冒名）。
#[test]
fn c7b_attrs_enrichment() {
    use crate::domain::{SqliteStore, Store as _};
    use crate::knowledge::identity::{
        access_context_for_employee_enriched, ensure_operator_principal,
    };
    use crate::knowledge::types::{Principal, PrincipalAttrs};

    let dir = std::env::temp_dir().join(format!(
        "m1c7b-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = SqliteStore::open(&dir.join("test.db")).unwrap();
    ensure_operator_principal(&store).unwrap();

    let mk_emp = |id: &str| crate::domain::models::Employee {
        id: id.into(),
        workspace_id: "ws-default".into(),
        name: id.into(),
        brain: crate::domain::models::BrainRef { brain_id: "__default__".into() },
        role: None,
        template_id: None,
        state: crate::domain::models::EmployeeState::Sleeping,
        archived: false,
        tools: None,
        created_at: "2026-10-02T00:00:00Z".into(),
    };

    // bob 有 attrs 列 → 富集；carol 無列 → M1 行為（空屬性）。
    store
        .put_principal(&Principal {
            id: "ai:bob".into(),
            principal_type: PrincipalType::AiEmployee,
            employee_id: Some("bob".into()),
            display_name: String::new(),
            attrs: PrincipalAttrs {
                roles: vec![],
                departments: vec!["quality".into()],
                projects: vec![],
            },
        })
        .unwrap();
    let bob = access_context_for_employee_enriched(&store, &mk_emp("bob"), None, None).unwrap();
    assert_eq!(bob.departments, vec!["quality".to_string()]);
    assert_eq!(bob.principal_id, "ai:bob", "身份仍是推導的——列只供 attrs（防冒名）");
    let carol = access_context_for_employee_enriched(&store, &mk_emp("carol"), None, None).unwrap();
    assert!(carol.departments.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
