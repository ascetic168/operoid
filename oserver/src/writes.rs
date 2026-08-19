//! 寫入面 handlers（P3）——與殼 `#[tauri::command]` 同一 ocore 核心（單一實作）。
//!
//! 紀律：SQLite 走 `spawn_blocking`；busy-lock 快速回絕（`agent_os.employeeBusy` → 409）；
//! 背景喚醒語意（create/approve commitment）由 ocore 核心 `tokio::spawn` 處理，
//! handler 直接回 200/202。

use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use ocore::agent_state::AppState;
use ocore::i18n::AppError;
use ocore::runtime::{
    approve_commitment_core, archive_commitment_core, cancel_task_core, clear_messages_core,
    create_commitment_core, create_template_core, delete_employee_core, delete_template_core,
    reject_commitment_core,
    deploy_instance, ensure_workspace_core, rename_employee_core, rename_template_core,
    send_message_core,
};

use crate::routes::{err_response, open_store, require_auth, ServerState};

pub fn write_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/workspace/ensure", post(api_ensure_workspace))
        .route("/api/templates", post(api_create_template))
        .route("/api/templates/{id}", delete(api_delete_template).patch(api_rename_template))
        .route("/api/employees/{id}", delete(api_delete_employee).patch(api_rename_employee))
        .route("/api/employees/deploy", post(api_deploy))
        .route("/api/employees/{id}/messages", post(api_send_message).delete(api_clear_messages))
        .route("/api/commitments", post(api_create_commitment))
        .route("/api/commitments/{id}/approve", post(api_approve))
        .route("/api/commitments/{id}/reject", post(api_reject))
        .route("/api/commitments/{id}/archive", post(api_archive))
        .route("/api/tasks/{id}/cancel", post(api_cancel_task))
}

/// ServerState 需要 AppState（寫入面喚醒）——routes::router 建立時注入。
/// 放在 crate::routes::ServerState 的擴充：見 main.rs 組裝（agent_state 欄位）。
fn app_state(st: &ServerState) -> Result<&AppState, AppError> {
    st.agent_state
        .as_ref()
        .ok_or_else(|| AppError::new("server.notReady"))
}

async fn api_ensure_workspace(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ensure_workspace_core(&store)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct CreateTemplateBody {
    workspace_id: Option<String>,
    name: String,
    brain_id: Option<String>,
    role: Option<String>,
}

async fn api_create_template(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<CreateTemplateBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let ws = body.workspace_id.clone().unwrap_or_else(|| "ws-default".into());
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        create_template_core(&st.cfg, &store, &ws, &b.name, b.brain_id.as_deref(), b.role.as_deref())
    })
    .await;
    finish(res)
}

async fn api_delete_template(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        delete_template_core(&store, &id)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct RenameBody {
    name: String,
}

async fn api_rename_template(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<RenameBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let name = body.0.name;
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        rename_template_core(&store, &id, &name)
    })
    .await;
    finish(res)
}

async fn api_delete_employee(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        delete_employee_core(&store, &id)
    })
    .await;
    finish(res)
}

async fn api_rename_employee(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<RenameBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let name = body.0.name;
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        rename_employee_core(&store, &id, &name)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct DeployBody {
    template_id: String,
    instance_name: String,
}

async fn api_deploy(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<DeployBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let employee_id = deploy_instance(&store, &b.template_id, &b.instance_name)?;
        Ok(json!({ "employee_id": employee_id }))
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct SendMessageBody {
    text: String,
    commitment_id: Option<String>,
}

async fn api_send_message(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<SendMessageBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?;
        let store = open_store(&st)?;
        send_message_core(as_, &store, &id, &b.text, b.commitment_id.as_deref())
    })
    .await;
    finish(res)
}

async fn api_clear_messages(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        clear_messages_core(&store, &id)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct CreateCommitmentBody {
    employee_id: String,
    title: String,
    completion_condition: String,
}

/// 交辦承諾：**202 Accepted**——背景喚醒（ocore 核心 spawn），結果以 watch 輪詢觀察。
async fn api_create_commitment(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<CreateCommitmentBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?.clone();
        let store = open_store(&st)?;
        create_commitment_core(
            &as_,
            &st.cfg,
            &st.db_path,
            &store,
            &b.employee_id,
            &b.title,
            &b.completion_condition,
        )
    })
    .await;
    match res {
        Ok(Ok(r)) => (StatusCode::ACCEPTED, Json(json!({"commitment_id": r.commitment_id}))).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_approve(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?.clone();
        let store = open_store(&st)?;
        approve_commitment_core(&as_, &st.cfg, &st.db_path, &store, &id)
    })
    .await;
    finish(res)
}

async fn api_reject(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        reject_commitment_core(&store, &id)
    })
    .await;
    finish(res)
}

async fn api_archive(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        archive_commitment_core(&store, &id)
    })
    .await;
    finish(res)
}

async fn api_cancel_task(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        cancel_task_core(&store, &id)
    })
    .await;
    finish(res)
}

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
