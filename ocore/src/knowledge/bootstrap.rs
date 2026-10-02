//! 知識授權 bootstrap（M1-WP-C5）——單人版行為不變保證的落點。
//!
//! Q3 裁決的落地：**不搬頁**——既有 brain 的全部 sources 映射進 `co-common`
//! scope（`visibility: Company`），並寫入明示的 allow-all bootstrap policy
//! （operator 全權；企業模式由管理員增補條件式規則，C7+）。
//!
//! fail closed（I2）只在「policy 無法判定」時生效：缺／損壞 → 空規則＝全 DENY
//! ＋invalid 旗標（呼叫端記 `knowledge_policy_invalid` 事件——C6 服務消費）。
//! 正常流程因 bootstrap 先行，不會觸發。

use anyhow::Result;

use super::types::{Effect, KnowledgePolicy, KnowledgeScope, PolicyRule, SecurityLevel, Visibility};
use crate::domain::store::Store;

/// bootstrap 建立的唯一 scope id（Q2 命名慣例）。
pub const CO_COMMON_SCOPE_ID: &str = "co-common";

/// bootstrap 結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// 已有 scope／policy——冪等跳過。
    Unchanged,
    /// 本次完成初始化（帶映射的 source 數）。
    Bootstrapped { sources: usize },
}

/// 同步核心（可直測）：以給定的 source id 集合完成 bootstrap。
///
/// 冪等：scopes 或 policy 任一已存在即 `Unchanged`（避免覆寫管理員的後續調整）。
pub fn bootstrap_with_sources(store: &dyn Store, source_ids: &[String]) -> Result<BootstrapOutcome> {
    if !store.list_scopes()?.is_empty() || store.get_policy()?.is_some() {
        return Ok(BootstrapOutcome::Unchanged);
    }
    store.put_scope(&KnowledgeScope {
        id: CO_COMMON_SCOPE_ID.to_string(),
        visibility: Visibility::Company,
        classification: SecurityLevel::Internal,
        source_ids: source_ids.to_vec(),
        owner: None,
        department: None,
        project: None,
    })?;
    save_policy_new_version(
        store,
        vec![PolicyRule {
            id: "bootstrap-allow-all".to_string(),
            priority: 1,
            effect: Effect::Allow,
            principals: None,
            principal_types: None,
            scopes: None,
            departments: None,
            projects: None,
            classifications: None,
            department_membership: false,
            project_membership: false,
        }],
    )?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "knowledge_bootstrapped",
        format!("co-common ← {} 個既有 source（Q3：不搬頁）", source_ids.len()),
    );
    Ok(BootstrapOutcome::Bootstrapped { sources: source_ids.len() })
}

/// 實際入口：列舉 active brain 的 sources（live：`gbrain sources list --json`）後
/// bootstrap。gbrain 不可用時回 Err——呼叫端記錄後續行（檢索將 fail closed，
/// 但 gbrain 不可用時檢索本就無法進行）。
pub async fn ensure_knowledge_bootstrap(
    store: &dyn Store,
    cfg: &crate::app_config::AppConfig,
) -> Result<BootstrapOutcome> {
    if !store.list_scopes()?.is_empty() || store.get_policy()?.is_some() {
        return Ok(BootstrapOutcome::Unchanged);
    }
    let brain_id = cfg.active_brain_id.as_deref().unwrap_or(crate::app_config::DEFAULT_BRAIN_ID);
    let sources = crate::brains::list_sources(cfg, brain_id)
        .await
        .map_err(|e| anyhow::anyhow!("列舉既有 sources 失敗：{e}"))?
        .into_iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();
    bootstrap_with_sources(store, &sources)
}

/// C13b：scope 建立／更新（管理面唯一入口；Rule 8 留痕）。
pub fn save_scope_with_event(store: &dyn Store, scope: &KnowledgeScope) -> Result<()> {
    let existed = store.list_scopes()?.iter().any(|s| s.id == scope.id);
    store.put_scope(scope)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "knowledge_scope_changed",
        format!(
            "{} {}等級 sources={:?} dept={:?} proj={:?}",
            if existed { "更新" } else { "建立" },
            format!("{:?}", scope.classification).to_lowercase(),
            scope.source_ids,
            scope.department,
            scope.project,
        ),
    );
    Ok(())
}

/// 寫入新版 policy（version+1＋事件——D9：無快取，下一次檢索即時生效）。
pub fn save_policy_new_version(
    store: &dyn Store,
    rules: Vec<PolicyRule>,
) -> Result<KnowledgePolicy> {
    let prev = store.get_policy()?.map(|p| p.version).unwrap_or(0);
    let policy = KnowledgePolicy { version: prev + 1, rules };
    store.put_policy(&policy)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "knowledge_policy_changed",
        format!("v{}（{} 條規則）", policy.version, policy.rules.len()),
    );
    Ok(policy)
}

/// fail closed 載入（I2）：缺／損壞 → 空規則 policy＋invalid 旗標。
/// 第二個回傳值為 `true` 時，呼叫端應記 `knowledge_policy_invalid` 事件。
pub fn load_policy_fail_closed(store: &dyn Store) -> (KnowledgePolicy, bool) {
    match store.get_policy() {
        Ok(Some(p)) => (p, false),
        _ => (KnowledgePolicy { version: 0, rules: vec![] }, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::policy::authorized_sources;
    use crate::knowledge::types::AccessContext;

    fn store() -> crate::domain::SqliteStore {
        let dir = std::env::temp_dir().join(format!(
            "m1c5-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::domain::SqliteStore::open(&dir.join("test.db")).unwrap()
    }

    fn operator() -> AccessContext {
        crate::knowledge::identity::operator_access_context(crate::runtime::AGENT_WS)
    }

    #[test]
    fn bootstrap_is_idempotent_and_maps_all_sources() {
        let s = store();
        let out1 = bootstrap_with_sources(&s, &["src-a".into(), "src-b".into()]).unwrap();
        assert_eq!(out1, BootstrapOutcome::Bootstrapped { sources: 2 });
        let out2 = bootstrap_with_sources(&s, &["src-a".into(), "src-b".into()]).unwrap();
        assert_eq!(out2, BootstrapOutcome::Unchanged, "重跑不覆寫");

        let scopes = s.list_scopes().unwrap();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].id, "co-common");
        assert_eq!(scopes[0].source_ids, vec!["src-a".to_string(), "src-b".to_string()]);
        assert_eq!(scopes[0].visibility, Visibility::Company);

        // operator 全權（單人行為不變保證）：co-common 與未來 scope 都放行。
        let (policy, invalid) = load_policy_fail_closed(&s);
        assert!(!invalid);
        assert_eq!(policy.version, 1);
        assert_eq!(evaluate_allow(&policy, &operator(), "co-common"), true);
        assert_eq!(evaluate_allow(&policy, &operator(), "任何未來 scope"), true);
    }

    #[test]
    fn missing_policy_fails_closed() {
        let s = store();
        // 未 bootstrap：policy 缺 → 空規則＋invalid 旗標 → 授權集為空。
        s.put_scope(&KnowledgeScope {
            id: "co-common".into(),
            visibility: Visibility::Company,
            classification: SecurityLevel::Internal,
            source_ids: vec!["src-a".into()],
            owner: None,
            department: None,
            project: None,
        })
        .unwrap();
        let (policy, invalid) = load_policy_fail_closed(&s);
        assert!(invalid, "policy 缺＝無法判定＝fail closed");
        assert!(authorized_sources(&policy, &operator(), &s.list_scopes().unwrap()).is_empty());
    }

    #[test]
    fn corrupted_policy_fails_closed() {
        let dir = std::env::temp_dir().join(format!(
            "m1c5bad-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.db");
        let s = crate::domain::SqliteStore::open(&db).unwrap();
        bootstrap_with_sources(&s, &["src-a".into()]).unwrap();
        drop(s);
        // 直接破壞 data 欄（模擬磁碟損壞）。
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE knowledge_policies SET data = '{bad json' WHERE id = 'active'",
            [],
        )
        .unwrap();
        drop(conn);

        let s = crate::domain::SqliteStore::open(&db).unwrap();
        let (policy, invalid) = load_policy_fail_closed(&s);
        assert!(invalid, "損壞＝無法判定＝fail closed");
        assert!(policy.rules.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn evaluate_allow(p: &KnowledgePolicy, ctx: &AccessContext, scope: &str) -> bool {
        let s = super::super::types::KnowledgeScope {
            id: scope.into(),
            visibility: super::super::types::Visibility::Company,
            classification: SecurityLevel::Internal,
            source_ids: vec![],
            owner: None,
            department: None,
            project: None,
        };
        matches!(super::super::policy::evaluate(p, ctx, &s), super::super::types::Decision::Allow)
    }
}
