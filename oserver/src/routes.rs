//! HTTP 路由與 handlers（P2 讀取面）。
//!
//! 紀律：**所有 SQLite 存取走 `spawn_blocking`**（rusqlite 是同步 API，直接在
//! async handler 跑會餓死 tokio worker）。錯誤統一 `AppError` → JSON
//! `{"code", "params"}`＋status 映射（401 認證／404 找不到／503 agent-os 未啟用／500 其他）。

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use ocore::app_config::AppConfig;
use ocore::domain::{SqliteStore, Store};
use ocore::i18n::AppError;
use ocore::runtime::{
    inbox_summary_payload, list_state_payload, recent_events_payload, watch_payload,
};

use crate::auth::{AuthProvider, AuthError};

/// 服務共享狀態。
pub struct ServerState {
    pub auth: Arc<dyn AuthProvider>,
    pub cfg: AppConfig,
    pub db_path: PathBuf,
    pub ready: Arc<std::sync::atomic::AtomicBool>,
}

/// 統一錯誕回應：`AppError` → JSON＋status 映射。
fn err_response(e: &AppError) -> Response {
    // AppError 序列化形狀與桌面 IPC 一致（code＋params），前端錯誤鍵可直接沿用。
    let body = serde_json::to_value(e).unwrap_or_else(|_| json!({"code": "server.internal"}));
    let status = match e.code.as_str() {
        "agent_os.employeeNotFound" | "agent_os.templateNotFound" => StatusCode::NOT_FOUND,
        "agent_os.disabled" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(body)).into_response()
}

/// 認證中介：所有 /api/* 走此處（/healthz 在 router 層免認證）。
fn require_auth(state: &ServerState, headers: &HeaderMap) -> Result<(), Response> {
    let h = headers.get("authorization").and_then(|v| v.to_str().ok());
    match state.auth.check(h) {
        Ok(_) => Ok(()),
        Err(AuthError) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()),
    }
}

/// 開 store（handler 內先認證、再進 spawn_blocking 開連線）。
fn open_store(state: &ServerState) -> Result<SqliteStore, AppError> {
    SqliteStore::open(&state.db_path).map_err(|e| {
        AppError::new("server.dbOpenFail").p("detail", e.to_string())
    })
}

pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/state", get(api_state))
        .route("/api/employees", get(api_employees))
        .route("/api/templates", get(api_templates))
        .route("/api/employees/{id}/watch", get(api_watch))
        .route("/api/inbox", get(api_inbox))
        .route("/api/events", get(api_events))
        .with_state(state)
}

/// 免認證健康檢查：`warming`（初始化中）→ `ready`。
async fn healthz(State(state): State<Arc<ServerState>>) -> Response {
    let ready = state.ready.load(std::sync::atomic::Ordering::SeqCst);
    Json(json!({"status": if ready { "ready" } else { "warming" }})).into_response()
}

async fn api_state(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let ws = q.get("workspace").cloned().unwrap_or_else(|| "ws-default".into());
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        list_state_payload(&store, &ws)
    })
    .await;
    finish(res)
}

async fn api_employees(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        let employees = store.list_all_employees()?;
        Ok(json!(employees))
    })
    .await;
    finish(res)
}

async fn api_templates(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let ws = q.get("workspace").cloned().unwrap_or_else(|| "ws-default".into());
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        let templates = store.list_templates(&ws)?;
        Ok(json!(templates))
    })
    .await;
    finish(res)
}

async fn api_watch(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        watch_payload(&st.cfg, &store, &id)
    })
    .await;
    finish(res)
}

async fn api_inbox(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        inbox_summary_payload(&store)
    })
    .await;
    finish(res)
}

async fn api_events(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        recent_events_payload(&store, limit)
    })
    .await;
    finish(res)
}

// ── 輔助 ─────────────────────────────────────────────────────────────

fn check_enabled(st: &ServerState) -> Result<(), AppError> {
    if !st.cfg.agent_os_enabled {
        return Err(AppError::new("agent_os.disabled"));
    }
    Ok(())
}

/// JoinHandle 結果 → Response（JoinError 視為內部錯誤；T 泛型序列化）。
fn finish<T: serde::Serialize>(
    res: Result<Result<T, AppError>, tokio::task::JoinError>,
) -> Response {
    match res {
        Ok(Ok(v)) => Json(serde_json::to_value(v).unwrap_or_else(|_| serde_json::Value::Null)).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}
