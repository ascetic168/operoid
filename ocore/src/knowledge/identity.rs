//! 身份基礎建設（M1-WP-C4 → R2 遠端化擴充）——Principal 的兩個**唯一**構造入口（I4／D6）。
//!
//! 1. **operator**：bootstrap human principal（對應共用 Bearer token 的單人版；
//!    於 store 冪等建立，見 [`ensure_operator_principal`]）。
//! 2. **ai_employee**：由 `Employee` **讀時推導**（不儲存——Q6 裁決，無同步問題）。
//!
//! `AccessContext` 只能經本模組或 bootstrap 產生；**不存在**「呼叫端字串自稱
//! principal」的 API 路徑（Test 8 的結構性保證）。
//! R2（遠端化）：CSPRNG token 生命週期（多 token／TTL／逐 token 撤銷）＋
//! 帳號密碼（Argon2id、登入名、停用、首次登入強改）——C12b 最小版（DR-E3）。

use anyhow::Result;

use super::types::{
    AccessContext, Principal, PrincipalAttrs, PrincipalToken, PrincipalType, SecurityLevel,
};
use crate::domain::store::Store;
use crate::domain::models::Employee;

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
        login_name: None,
        password_hash: None,
        disabled: false,
        must_change_password: false,
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

// ── R2（遠端化）：CSPRNG、帳號密碼、token 生命週期（C12b 最小版，DR-E3）──

/// 密碼長度下限（密碼政策）。
pub const MIN_PASSWORD_LEN: usize = 8;

/// CSPRNG → hex（遠端化缺口 8：取代 M1「時間＋pid」務實取捨——token 可預測性）。
fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).expect("OS 熵源不可用");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Argon2id PHC 字串（明文永不落 store）。
pub fn hash_password(password: &str) -> Result<String> {
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    // 鹽：自家 CSPRNG 16 bytes → b64（不依賴 argon2 的 rand feature）。
    let mut salt_bytes = [0u8; 16];
    getrandom::getrandom(&mut salt_bytes).expect("OS 熵源不可用");
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!("salt: {e}"))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("argon2 hash: {e}"))
}

/// 驗證密碼（對 PHC 字串）；PHC 格式壞 → false（fail closed）。
pub fn verify_password(password: &str, phc: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    match PasswordHash::new(phc) {
        Ok(h) => Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok(),
        Err(_) => false,
    }
}

/// 密碼政策（R2：長度下限；複雜度規則隨 C12b 完整版）。
pub fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        anyhow::bail!("密碼長度不足：至少 {MIN_PASSWORD_LEN} 碼");
    }
    Ok(())
}

/// 產生 token 明文——`okt2-` 前綴＝CSPRNG 新制（舊制 `okt-` 已退役，一律重簽）。
fn mint_token_plain(principal_id: &str) -> String {
    let name = principal_id
        .trim_start_matches("ai:")
        .trim_start_matches("principal-");
    format!("okt2-{name}-{}", random_hex(32))
}

/// 簽發 principal token（R2 版）：**不覆蓋既有 token**（一 principal 多 token＝
/// 裝置／session 粒度），可帶 TTL（人類會話短 TTL；None＝長期服務 token）與標籤。
/// 明文**只在此回傳一次**；稽核留痕（Rule 8）。
pub fn issue_principal_token_v2(
    store: &dyn Store,
    principal_id: &str,
    ttl_secs: Option<i64>,
    label: Option<&str>,
) -> Result<(String, PrincipalToken)> {
    if store.get_principal(principal_id)?.is_none() {
        anyhow::bail!("principal 不存在：{principal_id}");
    }
    let plain = mint_token_plain(principal_id);
    let token = PrincipalToken {
        id: format!("tok-{}", random_hex(8)),
        principal_id: principal_id.to_string(),
        token_hash: hash_token(&plain),
        created_at: crate::domain::now_rfc3339(),
        expires_at: ttl_secs
            .map(|s| (chrono::Utc::now() + chrono::Duration::seconds(s)).to_rfc3339()),
        last_used_at: None,
        label: label.map(|l| l.to_string()),
    };
    store.put_token(&token)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_token_issued",
        &format!("{principal_id}/{}", token.id),
    );
    Ok((plain, token))
}

/// authn 查找：明文 → (token 記錄, principal)。過期／principal 停用 → None（401 語意）。
pub fn find_valid_token(
    store: &dyn Store,
    plain: &str,
) -> Result<Option<(PrincipalToken, Principal)>> {
    let Some(tok) = store.get_token_by_hash(&hash_token(plain))? else {
        return Ok(None);
    };
    if let Some(exp) = &tok.expires_at {
        let expired = chrono::DateTime::parse_from_rfc3339(exp)
            .map(|d| d <= chrono::Utc::now())
            .unwrap_or(true); // 壞時間戳＝fail closed
        if expired {
            return Ok(None);
        }
    }
    let Some(p) = store.get_principal(&tok.principal_id)? else {
        return Ok(None);
    };
    if p.disabled {
        return Ok(None);
    }
    Ok(Some((tok, p)))
}

/// last_used 節流更新（60s 內不重寫——請求路徑寫入放大防護）。
pub fn touch_token_last_used(store: &dyn Store, token: &PrincipalToken) -> Result<()> {
    let due = token
        .last_used_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| {
            chrono::Utc::now().signed_duration_since(d) > chrono::Duration::seconds(60)
        })
        .unwrap_or(true);
    if !due {
        return Ok(());
    }
    let mut t = token.clone();
    t.last_used_at = Some(crate::domain::now_rfc3339());
    store.put_token(&t)
}

/// 逐 token 撤銷（冪等）。稽核留痕。
pub fn revoke_token_by_id(store: &dyn Store, token_id: &str) -> Result<bool> {
    let removed = store.delete_token(token_id)?;
    if removed {
        crate::runtime::record_event(
            store,
            crate::runtime::AGENT_WS,
            "knowledge",
            "principal_token_revoked",
            token_id,
        );
    }
    Ok(removed)
}

/// 撤銷某 principal 全部 token（停用帳號／admin 重設密碼）。稽核留痕。
pub fn revoke_all_tokens(store: &dyn Store, principal_id: &str) -> Result<usize> {
    let n = store.delete_tokens_for_principal(principal_id)?;
    if n > 0 {
        crate::runtime::record_event(
            store,
            crate::runtime::AGENT_WS,
            "knowledge",
            "principal_tokens_revoked_all",
            &format!("{principal_id}/{n}"),
        );
    }
    Ok(n)
}

// ── 帳號（C12b 最小版：本機帳號密碼登入）──

/// 建人類帳號的規格（oserver 帳號管理 API 的輸入）。
pub struct AccountSpec {
    pub id: String,
    pub login_name: String,
    pub display_name: String,
    pub roles: Vec<String>,
    /// None → 自動產生臨時密碼（明文隨回傳值**僅出現一次**）。
    pub temp_password: Option<String>,
}

/// 建人類帳號：登入名唯一、Argon2id、`must_change_password=true`（稽核留痕）。
/// 回傳 (Principal, 臨時密碼明文)。
pub fn create_account(store: &dyn Store, spec: AccountSpec) -> Result<(Principal, String)> {
    if spec.id.is_empty() || spec.login_name.is_empty() {
        anyhow::bail!("id 與 login_name 不可空");
    }
    if store.get_principal(&spec.id)?.is_some() {
        anyhow::bail!("principal 已存在：{}", spec.id);
    }
    if store
        .list_principals()?
        .iter()
        .any(|p| p.login_name.as_deref() == Some(spec.login_name.as_str()))
    {
        anyhow::bail!("login_name 已存在：{}", spec.login_name);
    }
    let temp = match spec.temp_password {
        Some(p) => {
            validate_password(&p)?;
            p
        }
        None => random_hex(8), // 16 hex 臨時密碼（≥8 碼）
    };
    let p = Principal {
        id: spec.id.clone(),
        principal_type: PrincipalType::Human,
        employee_id: None,
        display_name: spec.display_name,
        attrs: PrincipalAttrs {
            roles: spec.roles.clone(),
            departments: vec![],
            projects: vec![],
            clearance: None,
        },
        token_hash: None,
        login_name: Some(spec.login_name.clone()),
        password_hash: Some(hash_password(&temp)?),
        disabled: false,
        must_change_password: true,
    };
    create_principal(store, p.clone())?; // 既有入口（稽核 principal_created）
    Ok((p, temp))
}

/// 帳密驗證（登入用）。失敗回傳原因代碼（no_such_account／disabled／no_password／
/// bad_credentials）——鎖定計數在 oserver 層（ocore 保持無狀態）。
pub fn verify_login(store: &dyn Store, login_name: &str, password: &str) -> Result<Principal> {
    let p = store
        .list_principals()?
        .into_iter()
        .find(|p| p.login_name.as_deref() == Some(login_name))
        .ok_or_else(|| anyhow::anyhow!("no_such_account"))?;
    if p.disabled {
        anyhow::bail!("disabled");
    }
    let Some(hash) = &p.password_hash else {
        anyhow::bail!("no_password");
    };
    if !verify_password(password, hash) {
        anyhow::bail!("bad_credentials");
    }
    Ok(p)
}

/// 改自身密碼（驗舊密碼；清 must_change_password）。稽核留痕。
pub fn change_password(store: &dyn Store, principal_id: &str, old: &str, new: &str) -> Result<()> {
    validate_password(new)?;
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    let Some(hash) = &p.password_hash else {
        anyhow::bail!("no_password");
    };
    if !verify_password(old, hash) {
        anyhow::bail!("bad_credentials");
    }
    p.password_hash = Some(hash_password(new)?);
    p.must_change_password = false;
    store.put_principal(&p)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_password_changed",
        principal_id,
    );
    Ok(())
}

/// admin 重設密碼：設臨時密碼＋must_change_password=true＋撤銷全部 token。稽核留痕。
pub fn admin_reset_password(store: &dyn Store, principal_id: &str, new: &str) -> Result<()> {
    validate_password(new)?;
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    p.password_hash = Some(hash_password(new)?);
    p.must_change_password = true;
    store.put_principal(&p)?;
    revoke_all_tokens(store, principal_id)?;
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        "principal_password_reset",
        principal_id,
    );
    Ok(())
}

/// 停用／啟用帳號：停用即撤銷全部 token（登入與既有 session 立即失效）。稽核留痕。
pub fn set_account_disabled(store: &dyn Store, principal_id: &str, disabled: bool) -> Result<()> {
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    p.disabled = disabled;
    store.put_principal(&p)?;
    if disabled {
        revoke_all_tokens(store, principal_id)?;
    }
    crate::runtime::record_event(
        store,
        crate::runtime::AGENT_WS,
        "knowledge",
        if disabled {
            "principal_disabled"
        } else {
            "principal_enabled"
        },
        principal_id,
    );
    Ok(())
}

/// admin 設定角色（僅 roles，其餘 attrs 不動——複用 set_principal_attrs 留痕）。
pub fn set_account_roles(store: &dyn Store, principal_id: &str, roles: Vec<String>) -> Result<()> {
    let mut p = store
        .get_principal(principal_id)?
        .ok_or_else(|| anyhow::anyhow!("principal 不存在：{principal_id}"))?;
    p.attrs.roles = roles;
    set_principal_attrs(store, principal_id, p.attrs)
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
        login_name: None,
        password_hash: None,
        disabled: false,
        must_change_password: false,
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
            owner_principal: None,
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
