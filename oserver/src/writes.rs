//! 寫入面 handlers（P3）——與殼 `#[tauri::command]` 同一 ocore 核心（單一實作）。
//!
//! 紀律：SQLite 走 `spawn_blocking`；busy-lock 快速回絕（`agent_os.employeeBusy` → 409）；
//! 背景喚醒語意（create/approve commitment）由 ocore 核心 `tokio::spawn` 處理，
//! handler 直接回 200/202。

use std::sync::Arc;

use axum::extract::{Path as AxPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, patch, post};
use axum::{Json, Router};
use crate::routes::cors_layer;
use serde::Deserialize;
use serde_json::json;

use ocore::agent_state::AppState;
use ocore::domain::Store as _;
use ocore::i18n::AppError;
use ocore::runtime::{
    approve_commitment_core, archive_commitment_core, archive_employee_core,
    cancel_task_core, clear_messages_core,
    create_commitment_core, create_template_core, delete_template_core,
    hard_delete_employee_core, reject_commitment_core, registry_drill_core, registry_save_core,
    review_commitment_core,
    deploy_instance, ensure_workspace_core, rename_employee_core, rename_template_core,
    send_message_core, stop_employee_core, unarchive_employee_core,
    update_employee_description_core, update_employee_email_core, update_employee_tools_core,
};

use crate::routes::{err_response, open_store, require_auth, require_identity, ServerState};
use crate::auth::Identity;

/// R2「限自身」：user 僅能操作自己名下的員工（manager/admin 不限；既有員工＝operator 歸屬）。
fn ensure_employee_access(
    identity: &Identity,
    store: &ocore::domain::SqliteStore,
    employee_id: &str,
) -> Result<(), AppError> {
    if let Some(emp) = store.get_employee(employee_id)? {
        if !crate::rbac::can_access_employee(identity, emp.owner_principal.as_deref()) {
            return Err(AppError::new("auth.forbidden"));
        }
    }
    Ok(())
}

/// R2「限自身」：承諾 → 其員工 → 歸屬判定。
fn ensure_commitment_access(
    identity: &Identity,
    store: &ocore::domain::SqliteStore,
    commitment_id: &str,
) -> Result<(), AppError> {
    if let Some(c) = store.get_commitment(commitment_id)? {
        ensure_employee_access(identity, store, &c.owner_employee_id)?;
    }
    Ok(())
}

pub fn write_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/workspace/ensure", post(api_ensure_workspace))
        .route("/api/templates", post(api_create_template))
        .route("/api/templates/{id}", delete(api_delete_template).patch(api_rename_template))
        .route("/api/employees/{id}", delete(api_delete_employee).patch(api_patch_employee))
        .route("/api/employees/{id}/tools", patch(api_update_employee_tools))
        .route("/api/employees/deploy", post(api_deploy))
        .route("/api/employees/{id}/stop", post(api_stop_employee))
        .route("/api/employees/{id}/unarchive", post(api_unarchive_employee))
        .route("/api/employees/{id}/messages", post(api_send_message).delete(api_clear_messages))
        .route("/api/commitments", post(api_create_commitment))
        .route("/api/commitments/{id}/approve", post(api_approve))
        .route("/api/commitments/{id}/reject", post(api_reject))
        .route("/api/commitments/{id}/archive", post(api_archive))
        .route("/api/commitments/{id}/review", post(api_review_commitment))
        .route("/api/commitments/{id}/satisfy", post(api_satisfy_commitment))
        .route("/api/skill-proposals/{id}/approve", post(api_skill_proposal_approve))
        .route("/api/skill-proposals/{id}/reject", post(api_skill_proposal_reject))
        .route("/api/skills/promote", post(api_skill_promote))
        .route("/api/registry/drill", post(api_registry_drill))
        .route("/api/tasks/{id}/cancel", post(api_cancel_task))
        .route("/api/registry", post(api_registry_save))
        .layer(cors_layer())
}

#[derive(Deserialize)]
struct RegistrySaveBody {
    raw_json: String,
}

#[derive(Deserialize)]
struct ReviewBody {
    misclassified: bool,
}

async fn api_review_commitment(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<ReviewBody>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let mis = body.0.misclassified;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_commitment_access(ident, &store, &id)?;
        }
        let data_dir = st
            .db_path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf);
        review_commitment_core(&data_dir, &store, &id, mis)
    })
    .await;
    match res {
        Ok(Ok(())) => Json(json!({ "ok": true })).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

// ── 技能提案（三區發佈流）：核可權限＝委任主體本人或 manager/admin（handler 細化，
// 比照「限自身」承諾存取）；晉升（組織級）由 RBAC 矩陣收 Admin。──

async fn api_skill_proposal_approve(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let Some(ident) = require_identity(&state, &headers, identity) else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"code": "auth.unauthorized"}))).into_response();
    };
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let p = store
            .get_skill_proposal(&id)?
            .ok_or_else(|| AppError::new("agent_os.skillProposalNotFound").p("id", &id))?;
        if !crate::rbac::satisfies(&ident, crate::rbac::Req::Manager)
            && p.owner_principal.as_deref() != Some(ident.name.as_str())
        {
            return Err(AppError::new("auth.forbidden"));
        }
        ocore::runtime::approve_skill_proposal_core(&st.cfg, &st.db_path, &store, &id, &ident.name)
    })
    .await;
    match res {
        Ok(Ok(p)) => Json(serde_json::to_value(&p).unwrap_or(json!({ "ok": true }))).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SkillRejectBody {
    /// 退回理由（回饋提名員工；可省）。
    reason: Option<String>,
}

async fn api_skill_proposal_reject(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<SkillRejectBody>,
) -> Response {
    let Some(ident) = require_identity(&state, &headers, identity) else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"code": "auth.unauthorized"}))).into_response();
    };
    let st = state.clone();
    let reason = body.0.reason.unwrap_or_else(|| "未附理由（主體退回）".into());
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let p = store
            .get_skill_proposal(&id)?
            .ok_or_else(|| AppError::new("agent_os.skillProposalNotFound").p("id", &id))?;
        if !crate::rbac::satisfies(&ident, crate::rbac::Req::Manager)
            && p.owner_principal.as_deref() != Some(ident.name.as_str())
        {
            return Err(AppError::new("auth.forbidden"));
        }
        ocore::runtime::reject_skill_proposal_core(&store, &id, &reason, &ident.name)
    })
    .await;
    match res {
        Ok(Ok(p)) => Json(serde_json::to_value(&p).unwrap_or(json!({ "ok": true }))).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SkillPromoteBody {
    /// 技能目前所在的主體個人區（principal id）。
    principal: String,
    /// 技能目錄名。
    skill: String,
}

/// 組織級晉升（admin——RBAC 矩陣收口）：主體個人區 → 公司共用庫。
async fn api_skill_promote(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    body: Json<SkillPromoteBody>,
) -> Response {
    let Some(ident) = require_identity(&state, &headers, identity) else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"code": "auth.unauthorized"}))).into_response();
    };
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::runtime::promote_skill_core(
            &st.cfg,
            &st.db_path,
            &store,
            &b.principal,
            &b.skill,
            &ident.name,
        )
    })
    .await;
    match res {
        Ok(Ok(())) => Json(json!({ "ok": true })).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_registry_drill(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let data_dir = st
            .db_path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf);
        registry_drill_core(&data_dir, &store)
    })
    .await;
    match res {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_registry_save(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<RegistrySaveBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let raw = body.0.raw_json;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let data_dir = st
            .db_path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf);
        registry_save_core(&data_dir, &store, &raw)
    })
    .await;
    match res {
        Ok(Ok(reg)) => Json(json!({ "registry": reg })).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
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
    /// 職責描述（自由文字；部署時抄襲到 Instance，實例可個別修改）。
    description: Option<String>,
    /// W3（D-H3）：工具 allowlist（如 ["write-note"]；None＝預設集）。
    tools: Option<Vec<String>>,
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
        let tool_refs: Option<Vec<&str>> =
            b.tools.as_deref().map(|v| v.iter().map(String::as_str).collect());
        create_template_core(
            &st.cfg,
            &store,
            &ws,
            &b.name,
            b.brain_id.as_deref(),
            b.role.as_deref(),
            b.description.as_deref(),
            tool_refs.as_deref(),
        )
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

/// W1（E13）語意變更：DELETE 預設＝**封存**（軟刪除，歷史保留）；`?hard=true`＝
/// 串聯硬刪除（僅開發／測試情境——正式產品語意是封存）。
async fn api_delete_employee(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let hard = q.get("hard").map(String::as_str) == Some("true");
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if hard {
            hard_delete_employee_core(&store, &id)
        } else {
            let as_ = app_state(&st)?.clone();
            archive_employee_core(&as_, &store, &id)
        }
    })
    .await;
    finish(res)
}

/// W1（E13）：解除封存（恢復可喚醒身分；歷史本就保留）。
async fn api_unarchive_employee(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_employee_access(ident, &store, &id)?;
        }
        unarchive_employee_core(&store, &id)
    })
    .await;
    finish(res)
}

/// D-M7：更新員工工具 allowlist（升權／降權）——治理事件 `tools_changed` 留痕。
#[derive(Deserialize)]
struct UpdateToolsBody {
    /// `None`＝回預設集；`Some([])`＝清空（僅核心工具）。
    tools: Option<Vec<String>>,
}

async fn api_update_employee_tools(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<UpdateToolsBody>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let tools = body.0.tools;
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_employee_access(ident, &store, &id)?;
        }
        update_employee_tools_core(&store, &id, tools)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct PatchEmployeeBody {
    /// 有帶才改（改名）。
    name: Option<String>,
    /// 有帶才改（職責描述；空字串＝清除）。空白 trim 後不落地。
    description: Option<String>,
    /// 有帶才改（對外 email；空字串＝清除）。變更後同步 obridge.toml routes/senders。
    email: Option<String>,
}

async fn api_patch_employee(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<PatchEmployeeBody>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let b = body.0;
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_employee_access(ident, &store, &id)?;
        }
        if let Some(name) = b.name {
            rename_employee_core(&store, &id, &name)?;
        }
        if let Some(desc) = b.description {
            update_employee_description_core(&store, &id, Some(&desc))?;
        }
        if let Some(email) = b.email {
            let emp = update_employee_email_core(&store, &id, Some(&email))?;
            // obridge.toml 同步（best-effort）：有值→加/改 routes+senders；清除→移除。
            if let Err(e) = crate::obridge_admin::sync_employee_email(
                &st.settings_dir,
                &id,
                &emp.name,
                emp.email.as_deref(),
            ) {
                eprintln!("[oserver] email 同步 obridge.toml 失敗（{id}）：{e}");
            }
        }
        store
            .get_employee(&id)?
            .ok_or_else(|| AppError::new("agent_os.employeeNotFound").p("id", id))
    })
    .await;
    finish(res)
}

/// W1 合作式停止：對執行中員工設停止旗標（runner 於步驟邊界優雅中止）。
async fn api_stop_employee(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?.clone();
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_employee_access(ident, &store, &id)?;
        }
        stop_employee_core(&as_, &store, &id)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct DeployBody {
    template_id: String,
    instance_name: String,
    /// 對外 email（選填）。單一真相在 Employee.email——server 於部署後同步
    /// obridge.toml 的 routes/senders（best-effort：設定檔缺漏僅記事件，不擋部署）。
    email: Option<String>,
}

/// R2：部署記錄部署者（`owner_principal`）——「限自身」歸屬的資料源。
async fn api_deploy(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    body: Json<DeployBody>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        let employee_id = deploy_instance(
            &store,
            &b.template_id,
            &b.instance_name,
            identity.as_ref().map(|i| i.name.as_str()),
            b.email.as_deref(),
        )?;
        // email 通道綁定同步（best-effort——obridge.toml 缺漏不擋部署）。
        if let Some(email) = b.email.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if let Err(e) = crate::obridge_admin::sync_employee_email(
                &st.settings_dir,
                &employee_id,
                &b.instance_name,
                Some(email),
            ) {
                eprintln!("[oserver] email 同步 obridge.toml 失敗（{employee_id}）：{e}");
            }
        }
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
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<SendMessageBody>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?;
        let store = open_store(&st)?;
        // D-M1：寄件者歸因——identity.name（principal id）＝該員工 owner → 職權指示；
        // 其他人類 → 諮詢或請求（硬約束仍由 registry＋核可 RBAC 承擔）。
        // 歸因名用 principals.display_name（查無 → 原 id）——提示詞「來自 X」可讀可辨識。
        let from = match &identity {
            Some(ident) => {
                let is_owner = store
                    .get_employee(&id)
                    .ok()
                    .flatten()
                    .and_then(|e| e.owner_principal)
                    .map(|owner| owner == ident.name)
                    .unwrap_or(false);
                let display = ocore::runtime::principal_display_name(&store, &ident.name)
                    .unwrap_or_else(|| ident.name.clone());
                ocore::domain::SenderAttribution::new(
                    if is_owner {
                        ocore::domain::SenderKind::Owner
                    } else {
                        ocore::domain::SenderKind::Human
                    },
                    Some(ident.name.clone()),
                    Some(display),
                )
            }
            None => ocore::domain::SenderAttribution::new(
                ocore::domain::SenderKind::Human,
                None,
                None,
            ),
        };
        // R2「聊天為主」開放：任何已認證者可對在職員工送訊息（同事對話——與 email 通道對稱）；
        // 封存員工由 send_message_core 拒收；保密語意由提示詞 need-to-know 條款＋脫敏 watch 承擔。
        send_message_core(as_, &store, &id, &b.text, b.commitment_id.as_deref(), from)
    })
    .await;
    finish(res)
}

async fn api_clear_messages(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_employee_access(ident, &store, &id)?;
        }
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
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let as_ = app_state(&st)?.clone();
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_commitment_access(ident, &store, &id)?;
        }
        approve_commitment_core(&as_, &st.cfg, &st.db_path, &store, &id)
    })
    .await;
    finish(res)
}

async fn api_reject(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_commitment_access(ident, &store, &id)?;
        }
        reject_commitment_core(&store, &id)
    })
    .await;
    finish(res)
}

async fn api_archive(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_commitment_access(ident, &store, &id)?;
        }
        archive_commitment_core(&store, &id)
    })
    .await;
    finish(res)
}

/// R3：人工覆寫承諾為 Satisfied（原殼專屬 invoke——API 覆蓋缺口補齊；「限自身」歸屬檢查）。
async fn api_satisfy_commitment(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if let Some(ident) = &identity {
            ensure_commitment_access(ident, &store, &id)?;
        }
        ocore::runtime::satisfy_commitment_core(&store, &id)
    })
    .await;
    match res {
        Ok(Ok(())) => Json(json!({ "ok": true })).into_response(),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
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
