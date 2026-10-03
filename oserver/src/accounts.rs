//! 帳號與認證 API（遠端化 R2，C12b 最小版／DR-E3）。
//!
//! 認證面：`POST /api/auth/login`（帳密 → principal token；連續失敗鎖定）、
//! `logout`（逐 token 撤銷）、`refresh`（輪替換發）、`password`（改自身密碼）。
//! 管理面（admin）：`/api/accounts` 建號／停用／啟用／重設密碼／角色指派——全稽核。
//!
//! 鎖定為**行程內**狀態（重啟即清）——v1 取捨；跨實例共用鎖定隨 C12b 完整版。
//! 人類會話 token 預設 TTL 24h；refresh 輪替換發。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::json;

use ocore::domain::Store as _;

use crate::auth::Identity;
use crate::routes::{cors_layer, err_response, open_store, require_identity, ServerState};

/// 人類會話 token 預設 TTL。
const SESSION_TTL_SECS: i64 = 24 * 3600;
/// 防暴：連續失敗 N 次 → 鎖定 15 分鐘。
const MAX_LOGIN_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(900);

/// 登入防暴（行程內；測試可注入）。
#[derive(Default)]
pub struct LoginGuard {
    failures: HashMap<String, (u32, Option<Instant>)>,
}

impl LoginGuard {
    pub fn is_locked(&self, login_name: &str) -> bool {
        match self.failures.get(login_name) {
            Some((n, Some(until))) => *n >= MAX_LOGIN_FAILURES && Instant::now() < *until,
            _ => false,
        }
    }
    pub fn record_failure(&mut self, login_name: &str) {
        let e = self.failures.entry(login_name.to_string()).or_default();
        e.0 += 1;
        if e.0 >= MAX_LOGIN_FAILURES {
            e.1 = Some(Instant::now() + LOCKOUT);
        }
    }
    pub fn record_success(&mut self, login_name: &str) {
        self.failures.remove(login_name);
    }
}

static LOGIN_GUARD: Mutex<Option<LoginGuard>> = Mutex::new(None);

fn with_guard<T>(f: impl FnOnce(&mut LoginGuard) -> T) -> T {
    let mut g = LOGIN_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    let g = g.get_or_insert_with(LoginGuard::default);
    f(g)
}

pub fn account_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/auth/login", post(api_login))
        .route("/api/auth/logout", post(api_logout))
        .route("/api/auth/refresh", post(api_refresh))
        .route("/api/auth/password", post(api_change_password))
        .route("/api/accounts", post(api_account_create))
        .route("/api/accounts/{id}/disable", post(api_account_disable))
        .route("/api/accounts/{id}/enable", post(api_account_enable))
        .route("/api/accounts/{id}/password", post(api_account_reset_password))
        .route("/api/accounts/{id}/roles", post(api_account_roles))
        .layer(cors_layer())
}

// ── 認證面 ──

#[derive(Deserialize)]
struct LoginBody {
    login_name: String,
    password: String,
}

/// 登入：帳密 → 24h 會話 token。失敗統一 `auth.invalidCredentials`（不洩漏帳號存在性）；
/// 鎖定 → 429 `auth.accountLocked`。成功/失敗皆稽核。
async fn api_login(State(state): State<Arc<ServerState>>, body: Json<LoginBody>) -> Response {
    if with_guard(|g| g.is_locked(&body.login_name)) {
        return err_response(&ocore::i18n::AppError::new("auth.accountLocked"));
    }
    let st = state.clone();
    let login_name = body.login_name.clone();
    let password = body.password.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        match ocore::knowledge::identity::verify_login(&store, &login_name, &password) {
            Ok(p) => {
                let (token, rec) = ocore::knowledge::identity::issue_principal_token_v2(
                    &store,
                    &p.id,
                    Some(SESSION_TTL_SECS),
                    Some("web-session"),
                )?;
                ocore::runtime::record_event(
                    &store,
                    ocore::runtime::AGENT_WS,
                    "auth",
                    "login_success",
                    &p.id,
                );
                Ok((p, token, rec))
            }
            Err(e) => {
                ocore::runtime::record_event(
                    &store,
                    ocore::runtime::AGENT_WS,
                    "auth",
                    "login_failed",
                    &login_name,
                );
                Err(anyhow::anyhow!(e.to_string()))
            }
        }
    })
    .await;
    match res {
        Ok(Ok((p, token, rec))) => {
            with_guard(|g| g.record_success(&body.login_name));
            (
                StatusCode::OK,
                Json(json!({
                    "token": token,
                    "token_id": rec.id,
                    "expires_at": rec.expires_at,
                    "principal": {"id": p.id, "display_name": p.display_name, "roles": p.attrs.roles},
                    "must_change_password": p.must_change_password,
                })),
            )
                .into_response()
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            // 停用帳號：登入一律拒（403 accountDisabled）——不計入密碼防暴計數。
            if msg == "disabled" {
                return err_response(&ocore::i18n::AppError::new("auth.accountDisabled"));
            }
            with_guard(|g| g.record_failure(&body.login_name));
            if with_guard(|g| g.is_locked(&body.login_name)) {
                err_response(&ocore::i18n::AppError::new("auth.accountLocked"))
            } else {
                err_response(&ocore::i18n::AppError::new("auth.invalidCredentials"))
            }
        }
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

/// 登出：撤銷本次請求的 token（master token 無 token_id → 冪等 ok）。
async fn api_logout(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    if let Some(token_id) = &identity.token_id {
        let st = state.clone();
        let token_id = token_id.clone();
        let res = tokio::task::spawn_blocking(move || {
            let store = open_store(&st)?;
            ocore::knowledge::identity::revoke_token_by_id(&store, &token_id)
        })
        .await;
        if let Ok(Err(e)) = res {
            return err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string()));
        }
    }
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

/// refresh：輪替換發——撤舊 token、簽新 24h token（label 沿用）。
async fn api_refresh(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    let Some(token_id) = identity.token_id.clone() else {
        // master token 無 session 記錄可輪替——客戶端錯誤（400），非授權拒絕。
        return err_response(&ocore::i18n::AppError::new("auth.noSession").p("detail", "master token 無 session 可輪替"));
    };
    let st = state.clone();
    let who = identity.name.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let label = store
            .list_tokens_for_principal(&who)?
            .into_iter()
            .find(|t| t.id == token_id)
            .and_then(|t| t.label);
        ocore::knowledge::identity::revoke_token_by_id(&store, &token_id)?;
        ocore::knowledge::identity::issue_principal_token_v2(&store, &who, Some(SESSION_TTL_SECS), label.as_deref())
    })
    .await;
    match res {
        Ok(Ok((token, rec))) => (
            StatusCode::OK,
            Json(json!({"token": token, "token_id": rec.id, "expires_at": rec.expires_at})),
        )
            .into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    old_password: String,
    new_password: String,
}

/// 改自身密碼（驗舊密碼；清 must_change_password）。
async fn api_change_password(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
    body: Json<ChangePasswordBody>,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    let st = state.clone();
    let who = identity.name.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::change_password(&store, &who, &b.old_password, &b.new_password)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            let code = if msg.contains("密碼長度不足") {
                "auth.weakPassword"
            } else {
                "auth.invalidCredentials"
            };
            err_response(&ocore::i18n::AppError::new(code).p("detail", msg))
        }
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

// ── 管理面（admin；RBAC 中介層已擋非 admin）──

#[derive(Deserialize)]
struct AccountCreateBody {
    login_name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    roles: Vec<String>,
    /// 可省 → 自動產生臨時密碼（明文僅本次回應出現）。
    #[serde(default)]
    temp_password: Option<String>,
}

/// 建帳號（含角色；臨時密碼明文僅一次；首次登入強制改密）。
async fn api_account_create(
    State(state): State<Arc<ServerState>>,
    body: Json<AccountCreateBody>,
) -> Response {
    let st = state.clone();
    let b = body.0;
    let id = format!("principal-{}", b.login_name);
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::create_account(
            &store,
            ocore::knowledge::identity::AccountSpec {
                id: id.clone(),
                login_name: b.login_name.clone(),
                display_name: if b.display_name.is_empty() { b.login_name.clone() } else { b.display_name },
                roles: b.roles.clone(),
                temp_password: b.temp_password.clone(),
            },
        )
    })
    .await;
    match res {
        Ok(Ok((p, temp))) => (
            StatusCode::OK,
            Json(json!({
                "id": p.id,
                "login_name": p.login_name,
                "roles": p.attrs.roles,
                // ⚠️ 明文僅此一次。
                "temporary_password": temp,
                "must_change_password": true,
            })),
        )
            .into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("auth.accountCreateFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

async fn api_account_disable(State(state): State<Arc<ServerState>>, AxPath(id): AxPath<String>) -> Response {
    account_flag(state, id, true).await
}
async fn api_account_enable(State(state): State<Arc<ServerState>>, AxPath(id): AxPath<String>) -> Response {
    account_flag(state, id, false).await
}

async fn account_flag(state: Arc<ServerState>, id: String, disabled: bool) -> Response {
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&state)?;
        ocore::knowledge::identity::set_account_disabled(&store, &id, disabled)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("auth.accountCreateFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

#[derive(Deserialize)]
struct ResetPasswordBody {
    new_password: String,
}

/// admin 重設密碼：must_change_password=true＋撤銷全部 token。
async fn api_account_reset_password(
    State(state): State<Arc<ServerState>>,
    AxPath(id): AxPath<String>,
    body: Json<ResetPasswordBody>,
) -> Response {
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::admin_reset_password(&store, &id, &b.new_password)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            let code = if msg.contains("密碼長度不足") {
                "auth.weakPassword"
            } else {
                "auth.accountCreateFailed"
            };
            err_response(&ocore::i18n::AppError::new(code).p("detail", msg))
        }
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

#[derive(Deserialize)]
struct RolesBody {
    roles: Vec<String>,
}

async fn api_account_roles(
    State(state): State<Arc<ServerState>>,
    AxPath(id): AxPath<String>,
    body: Json<RolesBody>,
) -> Response {
    let st = state.clone();
    let roles = body.0.roles;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::set_account_roles(&store, &id, roles)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("auth.accountCreateFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_guard_locks_after_max_failures_and_clears_on_success() {
        let mut g = LoginGuard::default();
        assert!(!g.is_locked("alice"));
        for _ in 0..(MAX_LOGIN_FAILURES - 1) {
            g.record_failure("alice");
        }
        assert!(!g.is_locked("alice"), "未達上限不鎖");
        g.record_failure("alice");
        assert!(g.is_locked("alice"), "達上限即鎖");
        g.record_success("alice");
        assert!(!g.is_locked("alice"), "成功登入清空計數");
        // 其他帳號不受牽連。
        assert!(!g.is_locked("bob"));
    }
}
