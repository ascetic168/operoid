//! 知識授權管理 API（C12a-2）——grants／principals／tokens 的管理面。
//!
//! 唯一管理入口：全部走 ocore `knowledge::{grants, identity}` 的函式（稽核事件在
//! 函式內記，Rule 8）。讀寫皆 `require_auth`（master 或 principal token）＋spawn_blocking。

use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::routes::{err_response, open_store, require_auth, ServerState};
use ocore::domain::Store as _;

pub fn knowledge_admin_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/knowledge/overview", get(api_overview))
        .route("/api/knowledge/policy", post(api_policy_save))
        .route("/api/knowledge/scopes", post(api_scope_upsert))
        .route("/api/knowledge/principals/{id}/attrs", post(api_attrs_set))
        .route("/api/knowledge/grants", post(api_grant_create))
        .route("/api/knowledge/grants/{id}/revoke", post(api_grant_revoke))
        .route("/api/knowledge/principals", post(api_principal_create))
        .route(
            "/api/knowledge/principals/{id}/token",
            post(api_token_issue).delete(api_token_revoke),
        )
        .layer(CorsLayer::very_permissive())
}

/// 一次抓齊管理面三集合（scopes 唯讀參考＋principals＋grants）。
async fn api_overview(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        Ok::<_, anyhow::Error>(json!({
            "principals": store.list_principals()?,
            "grants": store.list_grants()?,
            "scopes": store.list_scopes()?,
            "policy": store.get_policy()?,
        }))
    })
    .await;
    match res {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

    /// C13b：儲存整份政策（version+1；D9 無快取即時生效）。
    async fn api_policy_save(
        State(state): State<Arc<ServerState>>,
        headers: HeaderMap,
        body: Json<Vec<ocore::knowledge::types::PolicyRule>>,
    ) -> Response {
        if let Err(r) = require_auth(&state, &headers) {
            return r;
        }
        let st = state;
        let rules = body.0;
        let res = tokio::task::spawn_blocking(move || {
            let store = open_store(&st)?;
            ocore::knowledge::bootstrap::save_policy_new_version(&store, rules)
        })
        .await;
        match res {
            Ok(Ok(p)) => (StatusCode::OK, Json(p)).into_response(),
            Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.policySaveFailed").p("detail", e.to_string())),
            Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
        }
    }

    /// C13b：scope 建立／更新（upsert；一 source 一等級——D4）。
    async fn api_scope_upsert(
        State(state): State<Arc<ServerState>>,
        headers: HeaderMap,
        body: Json<ocore::knowledge::types::KnowledgeScope>,
    ) -> Response {
        if let Err(r) = require_auth(&state, &headers) {
            return r;
        }
        let st = state;
        let scope = body.0;
        if scope.id.is_empty() || scope.source_ids.is_empty() {
            return err_response(&ocore::i18n::AppError::new("knowledge.scopeFailed").p("detail", "id 與 source_ids 不可空"));
        }
        let echo = scope.clone();
        let res = tokio::task::spawn_blocking(move || {
            let store = open_store(&st)?;
            ocore::knowledge::bootstrap::save_scope_with_event(&store, &scope)
        })
        .await;
        match res {
            Ok(Ok(())) => (StatusCode::OK, Json(echo)).into_response(),
            Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.scopeFailed").p("detail", e.to_string())),
            Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
        }
    }

    #[derive(Deserialize)]
    struct AttrsBody {
        /// null＝降回 Internal 基準線。
        clearance: Option<ocore::knowledge::types::SecurityLevel>,
        #[serde(default)]
        departments: Vec<String>,
        #[serde(default)]
        projects: Vec<String>,
        #[serde(default)]
        roles: Vec<String>,
    }

    /// C13b：賦/改 principal 屬性（clearance 等；identity::set_principal_attrs 留痕）。
    async fn api_attrs_set(
        State(state): State<Arc<ServerState>>,
        headers: HeaderMap,
        AxPath(id): AxPath<String>,
        body: Json<AttrsBody>,
    ) -> Response {
        if let Err(r) = require_auth(&state, &headers) {
            return r;
        }
        let st = state;
        let b = body.0;
        let res = tokio::task::spawn_blocking(move || {
            let store = open_store(&st)?;
            ocore::knowledge::identity::set_principal_attrs(
                &store,
                &id,
                ocore::knowledge::types::PrincipalAttrs {
                    roles: b.roles,
                    departments: b.departments,
                    projects: b.projects,
                    clearance: b.clearance,
                },
            )
        })
        .await;
        match res {
            Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
            Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.principalFailed").p("detail", e.to_string())),
            Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
        }
    }

#[derive(Deserialize)]
struct GrantCreateBody {
    principal_id: String,
    scope_id: String,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    purpose: Option<String>,
    ttl_secs: i64,
}

async fn api_grant_create(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<GrantCreateBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::grants::create_grant(
            &store,
            &b.principal_id,
            &b.scope_id,
            b.task_id,
            b.purpose,
            b.ttl_secs,
        )
    })
    .await;
    match res {
        Ok(Ok(g)) => (StatusCode::OK, Json(g)).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.grantFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

async fn api_grant_revoke(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::grants::revoke_grant(&store, &id)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.grantFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

#[derive(Deserialize)]
struct PrincipalCreateBody {
    id: String,
    #[serde(default)]
    display_name: String,
    /// human（預設）| ai_employee；service 不經 API 建立（Q6）。
    #[serde(default)]
    principal_type: Option<String>,
    #[serde(default)]
    employee_id: Option<String>,
}

async fn api_principal_create(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<PrincipalCreateBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let b = body.0;
    if b.id.is_empty() {
        return err_response(&ocore::i18n::AppError::new("knowledge.principalFailed").p("detail", "id 不可空"));
    }
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        if store.get_principal(&b.id)?.is_some() {
            return Err(anyhow::anyhow!("principal 已存在：{}", b.id));
        }
        let principal_type = match b.principal_type.as_deref() {
            Some("ai_employee") => ocore::knowledge::types::PrincipalType::AiEmployee,
            _ => ocore::knowledge::types::PrincipalType::Human,
        };
        let p = ocore::knowledge::types::Principal {
            id: b.id.clone(),
            principal_type,
            employee_id: b.employee_id,
            display_name: b.display_name,
            attrs: Default::default(),
            token_hash: None,
        };
        ocore::knowledge::identity::create_principal(&store, p.clone())?;
        Ok(p)
    })
    .await;
    match res {
        Ok(Ok(p)) => (StatusCode::OK, Json(p)).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.principalFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

/// 簽發/輪替 principal token——**明文只在此回應出現一次**。
async fn api_token_issue(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::issue_principal_token(&store, &id)
    })
    .await;
    match res {
        Ok(Ok(token)) => (StatusCode::OK, Json(json!({"token": token}))).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.principalFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}

async fn api_token_revoke(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state;
    let res = tokio::task::spawn_blocking(move || {
        let store = open_store(&st)?;
        ocore::knowledge::identity::revoke_principal_token(&store, &id)
    })
    .await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Ok(Err(e)) => err_response(&ocore::i18n::AppError::new("knowledge.principalFailed").p("detail", e.to_string())),
        Err(e) => err_response(&ocore::i18n::AppError::new("server.internal").p("detail", e.to_string())),
    }
}
