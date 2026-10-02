//! 身份基礎建設（M1-WP-C4）——Principal 的兩個**唯一**構造入口（I4／D6）。
//!
//! 1. **operator**：bootstrap human principal（對應共用 Bearer token 的單人版；
//!    於 store 冪等建立，見 [`ensure_operator_principal`]）。
//! 2. **ai_employee**：由 `Employee` **讀時推導**（不儲存——Q6 裁決，無同步問題）。
//!
//! `AccessContext` 只能經本模組或 bootstrap 產生；**不存在**「呼叫端字串自稱
//! principal」的 API 路徑（Test 8 的結構性保證）。部門/角色屬性 M1 恆空——
//! 屬性條件與正規化屬 WP-C7+。

use anyhow::Result;

use super::types::{AccessContext, Principal, PrincipalAttrs, PrincipalType, SecurityLevel};
use crate::domain::models::Employee;
use crate::domain::store::Store;

/// bootstrap human principal 的固定 id。
pub const OPERATOR_PRINCIPAL_ID: &str = "principal-operator";

/// operator principal 的權威構造（冪等——同參數永遠同一身份）。
pub fn operator_principal() -> Principal {
    Principal {
        id: OPERATOR_PRINCIPAL_ID.to_string(),
        principal_type: PrincipalType::Human,
        employee_id: None,
        display_name: "operator".to_string(),
        // C13a（D-C13c）：operator＝bootstrap 最高管理者，clearance 結構性＝Secret。
        attrs: PrincipalAttrs {
            roles: vec![],
            departments: vec![],
            projects: vec![],
            clearance: Some(SecurityLevel::Secret),
        },
        token_hash: None,
    }
}

/// C12a（D-C12a-1）：token 明文 → SHA-256 hex。
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    format!("{:x}", h.finalize())
}

/// C12a-2：建立 principal 的管理入口（service 型別不經 API——Q6；Rule 8 留痕）。
pub fn create_principal(store: &dyn Store, principal: Principal) -> Result<()> {
    if store.get_principal(&principal.id)?.is_some() {
        return Err(anyhow::anyhow!("principal 已存在：{}", principal.id));
    }
    store.put_principal(&principal)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_created",
        &principal.id,
    );
    Ok(())
}

/// C12a：簽發 principal 的 API token（明文**只在此回傳一次**；存 SHA-256；Rule 8 留痕）。
/// 輪替＝對同一 principal 重簽（舊 token 立即失效——hash 被覆寫）。
pub fn issue_principal_token(store: &dyn Store, principal_id: &str) -> Result<String> {
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    let token = format!(
        "okt-{}-{}",
        principal_id.trim_start_matches("ai:").trim_start_matches("principal-"),
        uuid_like(),
    );
    p.token_hash = Some(hash_token(&token));
    store.put_principal(&p)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_token_issued",
        principal_id,
    );
    Ok(token)
}

/// C12a：撤銷 principal 的 API token（即時失效；Rule 8 留痕）。冪等。
pub fn revoke_principal_token(store: &dyn Store, principal_id: &str) -> Result<()> {
    let mut p = match store.get_principal(principal_id)? {
        Some(p) => p,
        None => return Ok(()),
    };
    if p.token_hash.is_none() {
        return Ok(());
    }
    p.token_hash = None;
    store.put_principal(&p)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_token_revoked",
        principal_id,
    );
    Ok(())
}

/// C12a：以 token 雜湊查找 principal（authn 用；明文比對永不出現在 store）。
pub fn find_principal_by_token(store: &dyn Store, token: &str) -> Result<Option<Principal>> {
    let hash = hash_token(token);
    Ok(store
        .list_principals()?
        .into_iter()
        .find(|p| p.token_hash.as_deref() == Some(hash.as_str())))
}

/// 32 hex（token 尾段）：時間＋pid＋進程內計數器經 SHA-256——本機服務的務實取捨；
/// C12b（IdP）改 CSPRNG。
fn uuid_like() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos().to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(SEQ.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    format!("{:x}", h.finalize())
}

/// 於 store 冪等確保 operator principal 存在（oserver 啟動與殼層初始化呼叫）。
pub fn ensure_operator_principal(store: &dyn Store) -> Result<()> {
    match store.get_principal(OPERATOR_PRINCIPAL_ID)? {
        Some(mut p) => {
            // C13a（D-C13c）：既有列冪等升級 clearance 至 Secret（管理員明示降級過則尊重）。
            if p.attrs.clearance.is_none() {
                p.attrs.clearance = Some(SecurityLevel::Secret);
                store.put_principal(&p)?;
            }
            Ok(())
        }
        None => {
            store.put_principal(&operator_principal())?;
            // C9（Rule 8）：身份建立留稽核（冪等——僅首次建立時記）。
            crate::runtime::record_event(
                store,
                crate::runtime::AGENT_WS,
                "knowledge",
                "principal_created",
                OPERATOR_PRINCIPAL_ID,
            );
            Ok(())
        }
    }
}

/// C9（Rule 8）：管理員賦/改 principal 屬性——唯一入口，一律留稽核。
/// 身份 id/型別不經此函式變更（防冒名；I4）。
pub fn set_principal_attrs(
    store: &dyn Store,
    principal_id: &str,
    attrs: super::types::PrincipalAttrs,
) -> Result<()> {
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    p.attrs = attrs;
    store.put_principal(&p)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_attrs_changed",
        format!(
            "{principal_id} departments={:?} projects={:?} roles={:?}",
            p.attrs.departments,
            p.attrs.projects,
            p.attrs.roles,
        ),
    );
    Ok(())
}

/// ai_employee principal——由 Employee **讀時推導**（Q6：不儲存）。
pub fn principal_for_employee(emp: &Employee) -> Principal {
    Principal {
        id: format!("ai:{}", emp.id),
        principal_type: PrincipalType::AiEmployee,
        employee_id: Some(emp.id.clone()),
        display_name: emp.name.clone(),
        attrs: Default::default(),
        token_hash: None,
    }
}

/// 員工的檢索授權脈絡（`build_tool_ctx` 的 AccessContext 來源）。
///
/// `task_id`/`purpose` 由呼叫端依 run 語境帶入（receipt 的 Who→Task 鏈）；
/// roles/departments/projects M1 恆空（正規化屬 C7+）。
pub fn access_context_for_employee(
    emp: &Employee,
    task_id: Option<String>,
    purpose: Option<String>,
) -> AccessContext {
    AccessContext {
        principal_id: format!("ai:{}", emp.id),
        principal_type: PrincipalType::AiEmployee,
        employee_id: Some(emp.id.clone()),
        workspace_id: emp.workspace_id.clone(),
        roles: vec![],
        departments: vec![],
        projects: vec![],
        task_id,
        purpose,
        clearance: None, // C13a：AI 員工預設無 clearance＝Internal
    }
}

/// C7b：富集版——推導基底＋store 內 `ai:{id}` principal 列的 attrs 疊加（D-C7b）。
/// 列不存在＝行為同 M1（attrs 空）；列存在時**僅取 attrs**（身份仍是推導的，防冒名）。
pub fn access_context_for_employee_enriched(
    store: &dyn Store,
    emp: &Employee,
    task_id: Option<String>,
    purpose: Option<String>,
) -> Result<AccessContext> {
    let mut ctx = access_context_for_employee(emp, task_id, purpose);
    if let Some(p) = store.get_principal(&format!("ai:{}", emp.id))? {
        ctx.roles = p.attrs.roles;
        ctx.departments = p.attrs.departments;
        ctx.projects = p.attrs.projects;
        // C13a：store 列有 clearance（管理員明示設定/降級過）以列為準。
        if p.attrs.clearance.is_some() {
            ctx.clearance = p.attrs.clearance;
        }
    }
    Ok(ctx)
}

/// operator 的檢索授權脈絡（管理面／op_run 改道用；M1 單人版＝全權 bootstrap policy）。
pub fn operator_access_context(workspace_id: &str) -> AccessContext {
    AccessContext {
        principal_id: OPERATOR_PRINCIPAL_ID.to_string(),
        principal_type: PrincipalType::Human,
        employee_id: None,
        workspace_id: workspace_id.to_string(),
        roles: vec![],
        departments: vec![],
        projects: vec![],
        task_id: None,
        purpose: None,
        // C13a（D-C13c）：operator＝bootstrap 最高管理者。
        clearance: Some(SecurityLevel::Secret),
    }
}

/// C7b：operator 富集版（operator principal 列的 attrs 疊加；冪等建立由 bootstrap 保證）。
pub fn operator_access_context_enriched(
    store: &dyn Store,
    workspace_id: &str,
) -> Result<AccessContext> {
    let mut ctx = operator_access_context(workspace_id);
    if let Some(p) = store.get_principal(OPERATOR_PRINCIPAL_ID)? {
        ctx.roles = p.attrs.roles;
        ctx.departments = p.attrs.departments;
        ctx.projects = p.attrs.projects;
        // C13a：store 列有 clearance（管理員明示設定/降級過）以列為準。
        if p.attrs.clearance.is_some() {
            ctx.clearance = p.attrs.clearance;
        }
    }
    Ok(ctx)
}

/// C12a（D-C12a-3）：**HTTP 面的身份→AccessContext**——Identity.name 即 principal id，
/// 由 token 鏈伺服器端裁定（Test 8 完整版）。查無 → Err（401 語意）。
pub fn access_context_for_principal(
    store: &dyn Store,
    principal_id: &str,
    workspace_id: &str,
) -> Result<AccessContext> {
    let p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    Ok(AccessContext {
        principal_id: p.id,
        principal_type: p.principal_type,
        employee_id: p.employee_id,
        workspace_id: workspace_id.to_string(),
        roles: p.attrs.roles,
        departments: p.attrs.departments,
        projects: p.attrs.projects,
        task_id: None,
        purpose: None,
        clearance: p.attrs.clearance, // C13a：攜 attrs.clearance
    })
}

/// 測試用預設脈絡（既有工具測試 helpers 的身份填充；C4 前的 ToolCtx 無身份欄位）。
#[cfg(test)]
pub(crate) fn test_default() -> AccessContext {
    operator_access_context(crate::runtime::AGENT_WS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{BrainRef, Employee, EmployeeState};

    fn emp(id: &str) -> Employee {
        Employee {
            id: id.to_string(),
            workspace_id: crate::runtime::AGENT_WS.to_string(),
            name: format!("員工{id}"),
            brain: BrainRef { brain_id: "__default__".into() },
            role: None,
            template_id: None,
            state: EmployeeState::Sleeping,
            archived: false,
            tools: None,
            created_at: "2026-10-02T00:00:00Z".into(),
        }
    }

    #[test]
    fn derivation_maps_employee_fields() {
        let e = emp("alice");
        let p = principal_for_employee(&e);
        assert_eq!(p.id, "ai:alice");
        assert_eq!(p.principal_type, PrincipalType::AiEmployee);
        assert_eq!(p.employee_id.as_deref(), Some("alice"));
        assert_eq!(p.display_name, e.name);
        let ctx = access_context_for_employee(&e, Some("task-1".into()), Some("test".into()));
        assert_eq!(ctx.principal_id, "ai:alice");
        assert_eq!(ctx.workspace_id, e.workspace_id);
        assert_eq!(ctx.task_id.as_deref(), Some("task-1"));
        assert!(ctx.roles.is_empty() && ctx.departments.is_empty() && ctx.projects.is_empty());
    }

    #[test]
    fn operator_principal_is_stable() {
        let p = operator_principal();
        assert_eq!(p.id, "principal-operator");
        assert_eq!(p.principal_type, PrincipalType::Human);
        assert_eq!(p, operator_principal(), "operator 身份冪等");
    }

    #[test]
    fn ensure_operator_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("m1c4-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = crate::domain::SqliteStore::open(&dir.join("test.db")).unwrap();
        ensure_operator_principal(&store).unwrap();
        ensure_operator_principal(&store).unwrap(); // 冪等：重複呼叫不重複建也不報錯
        let got = store.get_principal(OPERATOR_PRINCIPAL_ID).unwrap();
        assert_eq!(got, Some(operator_principal()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
