//! RBAC（遠端化 R2，DR-E4）——端點級授權中介層。
//!
//! **矩陣為單一真相**：`requirement(method, matched_path)` 回答「這條端點至少要什麼
//! 角色」。角色層級 admin ⊃ manager ⊃ user；operator（master token）結構性＝admin。
//! **fail-closed**：矩陣未列的端點一律 Admin——新增路由必須同步本表（矩陣測試會抓）。
//! 「限自身」歸屬（employee.owner_principal）屬 handler 層細化，見 [`ensure_employee_access`]。
//!
//! 中介層職責：authn（插入 `Identity` 進 extensions）→ must_change_password 閘 →
//! 矩陣裁定 → 403 `auth.forbidden`。OPTIONS preflight 與公開路徑直接放行。
//! 授權拒絕記稽核事件（best-effort，不擋回應）。

use std::sync::Arc;

use axum::extract::{MatchedPath, State};
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use ocore::knowledge::identity::OPERATOR_PRINCIPAL_ID;

use crate::auth::Identity;
use crate::routes::ServerState;

/// 端點的角色要求（層級：Public < Authenticated < User < Manager < Admin）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Req {
    /// 免認證（healthz／登入／靜態頁）。
    Public,
    /// 任何通過認證的身份（「限自身」由 handler 細化）。
    Authenticated,
    /// 一般使用者（manager/admin 經層級涵蓋）。
    User,
    /// 高階經理人。
    Manager,
    /// 系統管理者。
    Admin,
}

/// 角色矩陣——(method, 路由 pattern) → 最低要求。
/// 語意依《Operoid-計畫-遠端化.md》§四；未列端點 → Admin（fail-closed）。
pub fn requirement(method: &str, path: Option<&str>) -> Req {
    match (method, path) {
        // 公開（免認證；靜態頁另在 is_public 以字首放行）
        ("GET", Some("/healthz")) | ("POST", Some("/api/auth/login")) => Req::Public,

        // 通用認證面（所有角色可讀；「限自身」在 handler 內裁定）
        (_, Some("/event")) => Req::Authenticated, // obridge ingress（master token→operator）
        ("GET", Some("/api/state"))
        | ("GET", Some("/api/employees"))
        | ("GET", Some("/api/templates"))
        | ("GET", Some("/api/inbox"))
        | ("GET", Some("/api/events"))
        | ("GET", Some("/api/employees/{id}/watch"))
        | ("GET", Some("/api/artifacts/{id}"))
        | ("POST", Some("/api/commitments")) => Req::Authenticated,
        // 自身會話面（登出／refresh／改自身密碼——所有已認證角色）
        ("POST", Some("/api/auth/logout"))
        | ("POST", Some("/api/auth/refresh"))
        | ("POST", Some("/api/auth/password")) => Req::Authenticated,
        // R3：SSE 推送（短票或 Bearer 皆可）
        ("GET", Some("/api/stream")) | ("POST", Some("/api/stream/ticket")) => Req::Authenticated,
        // R3：承諾人工覆寫（「限自身」由 handler 裁定）
        ("POST", Some("/api/commitments/{id}/satisfy")) => Req::Manager,
        // R3：服務狀態唯讀（顯示列出——語意即 admin）
        ("GET", Some("/api/service/status")) => Req::Admin,
        // K6：知識管線精簡能力狀態（user 上傳面提示——精簡面，不含內部路徑／端點等管理資訊）
        ("GET", Some("/api/knowledge/caps")) => Req::User,

        // 一般使用者可操作（限自身——handler 裁定）
        ("POST", Some("/api/employees/deploy"))
        | ("POST", Some("/api/employees/{id}/stop"))
        | ("POST", Some("/api/employees/{id}/unarchive"))
        | ("PATCH", Some("/api/employees/{id}"))
        | ("POST", Some("/api/employees/{id}/messages"))
        | ("DELETE", Some("/api/employees/{id}/messages")) => Req::User,

        // 使用者級工廠（個人版 GUI「工廠」頁的企業對應）：類型清單／上傳暫存／轉換／
        // 覆蓋寫入／撰寫器／自動分類。寫入面由 handler 的寫入端天花板（C13c D-C13i）細化；
        // extract-companies（批次公司重建）屬維運批次，不在工廠 UI 面——留 Admin fail-closed。
        ("GET", Some("/api/factories/types"))
        | ("POST", Some("/api/factories/upload"))
        | ("POST", Some("/api/factories/upload/cleanup"))
        | ("POST", Some("/api/factories/run"))
        | ("POST", Some("/api/factories/write-pages"))
        | ("POST", Some("/api/factories/save-authored"))
        | ("POST", Some("/api/factories/classify")) => Req::User,

        // 使用者級知識檢索（C12a：身份出自 token 鏈，policy 依序過濾——僅 ask/query/think；
        // 維運／診斷 ops 留在 Req::Manager 的 /api/operations）
        ("POST", Some("/api/knowledge/ask")) => Req::User,

        // 高階經理人（營運視圖＋治理動作；承諾核可「限自身」由 handler 細化）
        ("GET", Some("/api/registry"))
        | ("POST", Some("/api/operations"))
        | ("GET", Some("/api/operations/{id}"))
        | ("GET", Some("/api/knowledge/overview"))
        | ("POST", Some("/api/knowledge/grants"))
        | ("POST", Some("/api/knowledge/grants/{id}/revoke"))
        | ("POST", Some("/api/commitments/{id}/approve"))
        | ("POST", Some("/api/commitments/{id}/reject"))
        | ("POST", Some("/api/commitments/{id}/archive"))
        | ("POST", Some("/api/commitments/{id}/review")) => Req::Manager,

        // 其餘全部 → Admin（fail-closed：templates CRUD、employees 封存/硬刪、registry 寫、
        // tasks cancel、brains/gbrain/prereq、factories extract-companies、knowledge
        // policy/scopes/principals/tokens、accounts 管理、config、operoid.toml 相關……未列即 Admin）
        _ => Req::Admin,
    }
}

/// 公開路徑（免 authn）——OPTIONS preflight 另於中介層放行。
pub fn is_public(path: &str) -> bool {
    path == "/healthz"
        || path == "/"
        || path == "/api/auth/login"
        || path.starts_with("/admin")
        || path.starts_with("/manager")
        || path.starts_with("/user")
}

/// 身份的有效角色集合（operator 結構性＝admin；其餘取 attrs.roles）。
pub fn effective_roles(identity: &Identity) -> Vec<String> {
    if identity.name == OPERATOR_PRINCIPAL_ID {
        return vec!["admin".to_string()];
    }
    identity.roles.clone()
}

/// 角色層級檢查：admin ⊃ manager ⊃ user。
pub fn satisfies(identity: &Identity, req: Req) -> bool {
    let roles = effective_roles(identity);
    let has = |r: &str| roles.iter().any(|x| x == r);
    match req {
        Req::Public => true,
        Req::Authenticated => true,
        Req::User => has("user") || has("manager") || has("admin"),
        Req::Manager => has("manager") || has("admin"),
        Req::Admin => has("admin"),
    }
}

/// handler 層細化：「限自身」——user 僅能操作 `owner_principal` 等於自己的員工；
/// None（既有員工／AI 招募）＝operator 歸屬 → 僅 manager/admin 可動。
pub fn can_access_employee(identity: &Identity, owner_principal: Option<&str>) -> bool {
    if satisfies(identity, Req::Manager) {
        return true;
    }
    owner_principal == Some(identity.name.as_str())
}

/// RBAC 中介層（掛最外層）。
pub async fn rbac_middleware(
    State(state): State<Arc<ServerState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // CORS preflight 不帶 Authorization——先放行（cors layer 在內層處理）。
    if req.method() == Method::OPTIONS {
        return next.run(req).await;
    }
    let path = req.uri().path().to_string();
    if is_public(&path) {
        return next.run(req).await;
    }

    // R3：SSE 短票——EventSource 無法帶 header。一次性票消費後由 store 構造完整
    // Identity；壞票／無票 → 落到一般 Bearer 檢查（非 EventSource 客戶端用 header 連）。
    if path == "/api/stream" {
        let ticket = req.uri().query().and_then(|q| {
            q.split('&').find_map(|kv| kv.strip_prefix("ticket="))
        });
        if let Some(principal_id) = ticket.and_then(crate::sse::consume_ticket) {
            if let Ok(store) = ocore::domain::SqliteStore::open(&state.db_path) {
                if let Ok(Some(p)) = ocore::domain::Store::get_principal(&store, &principal_id) {
                    if !p.disabled {
                        req.extensions_mut().insert(Identity {
                            name: p.id,
                            token_id: None,
                            roles: p.attrs.roles,
                            must_change_password: p.must_change_password,
                        });
                        return next.run(req).await;
                    }
                }
            }
        }
    }

    let headers = req.headers().get("authorization").and_then(|v| v.to_str().ok());
    let identity = match state.auth.check(headers) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"code": "auth.unauthorized"})),
            )
                .into_response()
        }
    };

    // 首次登入強制改密碼：只放行改密碼與登出。
    if identity.must_change_password
        && path != "/api/auth/password"
        && path != "/api/auth/logout"
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"code": "auth.mustChangePassword"})),
        )
            .into_response();
    }

    let matched = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string());
    let req_kind = requirement(req.method().as_str(), matched.as_deref());
    if !satisfies(&identity, req_kind) {
        audit_authz_denied(&state, &identity, &path);
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"code": "auth.forbidden"})),
        )
            .into_response();
    }

    req.extensions_mut().insert(identity);
    next.run(req).await
}

/// 授權拒絕稽核（fire-and-forget；開 store 失敗靜默——不擋 403 回應）。
fn audit_authz_denied(state: &Arc<ServerState>, identity: &Identity, path: &str) {
    let db_path = state.db_path.clone();
    let who = identity.name.clone();
    let path = path.to_string();
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(store) = ocore::domain::SqliteStore::open(&db_path) {
                ocore::runtime::record_event(
                    &store,
                    ocore::runtime::AGENT_WS,
                    "auth",
                    "authz_denied",
                    &format!("{who} → {path}"),
                );
            }
        })
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_admin_implies_all() {
        let admin = Identity::new("p1", &["admin"]);
        let manager = Identity::new("p2", &["manager"]);
        let user = Identity::new("p3", &["user"]);
        assert!(satisfies(&admin, Req::Admin));
        assert!(satisfies(&admin, Req::User));
        assert!(satisfies(&manager, Req::Manager));
        assert!(!satisfies(&manager, Req::Admin));
        assert!(satisfies(&user, Req::User));
        assert!(!satisfies(&user, Req::Manager));
        // operator 結構性 admin（attrs 空）。
        let op = Identity::new(OPERATOR_PRINCIPAL_ID, &[]);
        assert!(satisfies(&op, Req::Admin));
        // 無角色 → 僅 Authenticated。
        let nobody = Identity::new("p4", &[]);
        assert!(satisfies(&nobody, Req::Authenticated));
        assert!(!satisfies(&nobody, Req::User));
    }

    #[test]
    fn ownership_user_scoped_others_operator_owned() {
        let user = Identity::new("principal-alice", &["user"]);
        let manager = Identity::new("principal-mgr", &["manager"]);
        // 自己部署的員工。
        assert!(can_access_employee(&user, Some("principal-alice")));
        // 他人／既有（operator 歸屬）→ user 擋、manager 過。
        assert!(!can_access_employee(&user, Some("principal-bob")));
        assert!(!can_access_employee(&user, None));
        assert!(can_access_employee(&manager, None));
    }

    #[test]
    fn fail_closed_unknown_route_is_admin() {
        assert_eq!(requirement("GET", Some("/api/never-heard-of")), Req::Admin);
        assert_eq!(requirement("GET", None), Req::Admin);
    }

    #[test]
    fn public_routes() {
        assert!(is_public("/healthz"));
        assert!(is_public("/"));
        assert!(is_public("/api/auth/login"));
        assert!(is_public("/admin"));
        assert!(is_public("/admin/assets/x.js"));
        assert!(!is_public("/api/state"));
    }
}
