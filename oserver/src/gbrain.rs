//! GBrain 能力域 endpoints（P4）——設定／腦管理／來源／長跑操作／工廠／分類／前置檢查。
//!
//! 長跑操作（op_run／sync／bind）走 `OpRegistry` ring buffer：POST 回 202+`operation_id`，
//! 前端輪詢 `GET /api/operations/{id}?since=n` 取增量行與最終結果。
//! 需持久化的操作（brains CRUD）直接寫回 `app-settings.json`（與桌面殼同一檔）。

use std::sync::Arc;

use axum::extract::{Path as AxPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use ocore::app_config::AppConfig;
use ocore::brains::{
    add_brain_core, add_source_core, bind_source_path_core, list_sources, remove_source_core,
    sync_brain_core, AddBrainReq,
};
use ocore::classifier::classify_one;
use ocore::factories::{
    extract_companies_core, run_core, save_authored_core, write_pages_core, WritePage,
};
use ocore::gbrain_cfg;
use ocore::gbrain_cli::op_run_core;
use ocore::i18n::AppError;
use ocore::prereq::check_all;

use crate::config::{load_config, save_config};
use crate::routes::{err_response, require_auth, ServerState};

pub fn gbrain_routes() -> Router<Arc<ServerState>> {
    Router::new()
        // gbrain 設定
        .route("/api/gbrain/config", get(api_gbrain_config))
        .route("/api/gbrain/model", post(api_set_model))
        .route("/api/gbrain/models-all", post(api_set_models_all))
        .route("/api/gbrain/model/unset", post(api_unset_model))
        .route("/api/gbrain/db-overrides/clear", post(api_clear_db_overrides))
        .route("/api/gbrain/provider-base-url", post(api_set_provider_base_url))
        .route("/api/gbrain/config-raw", post(api_save_config_raw))
        // 腦管理
        .route("/api/brains", get(api_brains_list).post(api_brains_add))
        .route("/api/brains/{id}", delete(api_brains_remove))
        .route("/api/brains/{id}/active", post(api_brains_set_active))
        .route("/api/brains/active-source", post(api_brains_set_active_source))
        .route(
            "/api/brains/{id}/sources",
            get(api_brain_sources).post(api_brain_source_add),
        )
        .route("/api/brains/{id}/sources/{source_id}", delete(api_brain_source_remove))
        .route("/api/brains/{id}/sync", post(api_brain_sync))
        .route("/api/brains/{id}/bind-path", post(api_brain_bind_path))
        // 長跑操作
        .route("/api/operations", post(api_op_run))
        .route("/api/operations/{id}", get(api_op_snapshot))
        // 工廠
        .route("/api/factories/run", post(api_factory_run))
        .route("/api/factories/write-pages", post(api_factory_write_pages))
        .route("/api/factories/extract-companies", post(api_extract_companies))
        .route("/api/factories/save-authored", post(api_factory_save_authored))
        .route("/api/factories/classify", post(api_factory_classify))
        // 前置檢查
        .route("/api/prereq", get(api_prereq))
        .layer(tower_http::cors::CorsLayer::very_permissive())
}

// ── 輔助 ─────────────────────────────────────────────────────────────

fn load_cfg(st: &ServerState) -> Result<AppConfig, AppError> {
    Ok(load_config(&st.settings_dir))
}

fn save_cfg(st: &ServerState, cfg: &AppConfig) -> Result<(), AppError> {
    save_config(&st.settings_dir, cfg)
        .map_err(|e| AppError::new("server.cfgSaveFail").p("detail", e.to_string()))
}

fn exe_of(cfg: &AppConfig) -> Result<String, AppError> {
    if std::path::Path::new(&cfg.gbrain_exe_path).exists() {
        Ok(cfg.gbrain_exe_path.clone())
    } else {
        Err(AppError::new("gbrain.exeNotFound").p("path", &cfg.gbrain_exe_path))
    }
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

fn ok_json(v: serde_json::Value) -> Response {
    Json(v).into_response()
}

// ── gbrain 設定 ──────────────────────────────────────────────────────

async fn api_gbrain_config(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || load_cfg(&st).map(|cfg| (cfg, ())))
        .await
        .map(|r| r.map(|(cfg, ())| {
            let exe = if std::path::Path::new(&cfg.gbrain_exe_path).exists() {
                Some(cfg.gbrain_exe_path.clone())
            } else {
                None
            };
            (exe, cfg.active_env_home().map(|s| s.to_string()))
        }));
    match res {
        Ok(Ok((exe, home))) => {
            match gbrain_cfg::build_config_view(exe.as_deref(), home.as_deref()).await {
                Ok(v) => ok_json(serde_json::to_value(v).unwrap_or_default()),
                Err(e) => err_response(&e),
            }
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SetModelBody {
    key: String,
    value: String,
}

async fn api_set_model(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<SetModelBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let exe = exe_of(&cfg)?;
        Ok((cfg, exe))
    })
    .await;
    match res {
        Ok(Ok((cfg, exe))) => {
            let home = cfg.active_env_home().map(|s| s.to_string());
            let r = gbrain_cfg::set_model(&exe, home.as_deref(), &b.key, &b.value).await;
            match r {
                Ok(()) => ok_json(json!({"ok": true})),
                Err(e) => err_response(&e),
            }
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SetModelsAllBody {
    model: String,
}

async fn api_set_models_all(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<SetModelsAllBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let exe = exe_of(&cfg)?;
        Ok((cfg, exe))
    })
    .await;
    match res {
        Ok(Ok((cfg, exe))) => {
            let home = cfg.active_env_home().map(|s| s.to_string());
            match gbrain_cfg::set_models_all(&exe, home.as_deref(), &b.model).await {
                Ok(()) => ok_json(json!({"ok": true})),
                Err(e) => err_response(&e),
            }
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

async fn api_unset_model(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<KeyBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let exe = exe_of(&cfg)?;
        Ok((cfg, exe))
    })
    .await;
    match res {
        Ok(Ok((cfg, exe))) => {
            let home = cfg.active_env_home().map(|s| s.to_string());
            match gbrain_cfg::unset_model(&exe, home.as_deref(), &b.key).await {
                Ok(()) => ok_json(json!({"ok": true})),
                Err(e) => err_response(&e),
            }
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_clear_db_overrides(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let exe = exe_of(&cfg)?;
        Ok((cfg, exe))
    })
    .await;
    match res {
        Ok(Ok((cfg, exe))) => {
            let home = cfg.active_env_home().map(|s| s.to_string());
            match gbrain_cfg::clear_db_overrides(&exe, home.as_deref()).await {
                Ok(()) => ok_json(json!({"ok": true})),
                Err(e) => err_response(&e),
            }
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ProviderBaseUrlBody {
    provider: String,
    base_url: Option<String>,
}

async fn api_set_provider_base_url(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<ProviderBaseUrlBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let home = cfg.active_env_home().map(|s| s.to_string());
        gbrain_cfg::set_provider_base_url(home.as_deref(), &b.provider, b.base_url.as_deref())
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct RawJsonBody {
    raw_json: serde_json::Value,
}

async fn api_save_config_raw(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<RawJsonBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let home = cfg.active_env_home().map(|s| s.to_string());
        gbrain_cfg::save_raw(home.as_deref(), &b.raw_json)
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

// ── 腦管理 ───────────────────────────────────────────────────────────

async fn api_brains_list(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let c = load_cfg(&st)?;
        Ok(json!({
            "brains": c.brains,
            "active_id": c.active_brain_id,
            "active_dot_gbrain": c.active_brain().map(|b| b.dot_gbrain_path().to_string_lossy().into_owned()),
        }))
    })
    .await;
    finish(res)
}

async fn api_brains_add(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<AddBrainReq>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    // add_brain_core 跑 gbrain init（子行程）——非 SQLite，直接 async。
    let res = tokio::spawn(async move {
        let c = load_cfg(&st)?;
        let (c2, entry) = add_brain_core(&c, &b).await?;
        save_cfg(&st, &c2)?;
        Ok(entry)
    })
    .await;
    finish(res)
}

async fn api_brains_remove(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        use ocore::app_config::DEFAULT_BRAIN_ID;
        if id == DEFAULT_BRAIN_ID {
            return Err(AppError::new("brain.cannotRemoveDefault"));
        }
        let mut c = load_cfg(&st)?;
        let before = c.brains.len();
        c.brains.retain(|b| b.id != id);
        if c.brains.len() == before {
            return Err(AppError::new("brain.notFound").p("id", &id));
        }
        if c.active_brain_id.as_deref() == Some(id.as_str()) {
            c.active_brain_id = Some(DEFAULT_BRAIN_ID.into());
            c.active_source_id = None;
        }
        save_cfg(&st, &c)
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_brains_set_active(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let mut c = load_cfg(&st)?;
        if !c.brains.iter().any(|b| b.id == id) {
            return Err(AppError::new("brain.notFound").p("id", &id));
        }
        c.active_brain_id = Some(id);
        c.active_source_id = None;
        save_cfg(&st, &c)
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ActiveSourceBody {
    source_id: Option<String>,
}

async fn api_brains_set_active_source(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<ActiveSourceBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let mut c = load_cfg(&st)?;
        c.active_source_id = b.source_id;
        save_cfg(&st, &c)
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_brain_sources(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        list_sources(&cfg, &id).await
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct SourceAddBody {
    source_id: String,
    path: String,
}

async fn api_brain_source_add(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<SourceAddBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let req = ocore::brains::SourceAdd {
        brain_id: id,
        source_id: b.source_id,
        path: b.path,
    };
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        add_source_core(&cfg, &req).await
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_brain_source_remove(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath((id, source_id)): AxPath<(String, String)>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let req = ocore::brains::SourceRef { brain_id: id, source_id };
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        remove_source_core(&cfg, &req).await
    })
    .await;
    match res {
        Ok(Ok(())) => ok_json(json!({"ok": true})),
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}

// ── 長跑操作（ring buffer 輪詢）──────────────────────────────────────

#[derive(Deserialize)]
struct BrainSyncBody {
    scope: String,
    source_id: Option<String>,
}

async fn api_brain_sync(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<BrainSyncBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let cfg = match tokio::task::spawn_blocking(move || load_cfg(&st)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return err_response(&e),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"code": "server.internal", "detail": e.to_string()})),
            )
                .into_response()
        }
    };
    let (op_id, sink) = state.ops.create();
    let op_id_resp = op_id.clone();
    let st2 = state.clone();
    let b = body.0;
    tokio::spawn(async move {
        let r = sync_brain_core(&cfg, &sink, &id, &b.scope, b.source_id.as_deref()).await;
        match r {
            Ok(res) => st2.ops.finish(
                &op_id,
                serde_json::to_value(&res).unwrap_or_default(),
            ),
            Err(e) => st2.ops.finish_err(&op_id, &e),
        }
    });
    (StatusCode::ACCEPTED, Json(json!({"operation_id": op_id_resp}))).into_response()
}

#[derive(Deserialize)]
struct BindPathBody {
    path: String,
}

async fn api_brain_bind_path(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    body: Json<BindPathBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let cfg = match tokio::task::spawn_blocking(move || load_cfg(&st)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return err_response(&e),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"code": "server.internal", "detail": e.to_string()})),
            )
                .into_response()
        }
    };
    let (op_id, sink) = state.ops.create();
    let op_id_resp = op_id.clone();
    let st2 = state.clone();
    let path = body.0.path;
    tokio::spawn(async move {
        let r = bind_source_path_core(&cfg, &sink, &id, &path).await;
        match r {
            Ok(res) => st2.ops.finish(
                &op_id,
                serde_json::to_value(&res).unwrap_or_default(),
            ),
            Err(e) => st2.ops.finish_err(&op_id, &e),
        }
    });
    (StatusCode::ACCEPTED, Json(json!({"operation_id": op_id_resp}))).into_response()
}

#[derive(Deserialize)]
struct OpRunBody {
    op: String,
    arg: Option<String>,
}

/// 跑一個 gbrain 操作（stats/sync/think/ask/...）：202 + operation_id，
/// 結果以 `GET /api/operations/{id}` 輪詢。
async fn api_op_run(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<OpRunBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let cfg = match tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        exe_of(&cfg).map(|exe| (cfg, exe))
    })
    .await
    {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return err_response(&e),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"code": "server.internal", "detail": e.to_string()})),
            )
                .into_response()
        }
    };
    let (op_id, sink) = state.ops.create();
    let op_id_resp = op_id.clone();
    let st2 = state.clone();
    let b = body.0;
    tokio::spawn(async move {
        let r = op_run_core(&cfg.0, &cfg.1, &sink, &b.op, b.arg.as_deref()).await;
        match r {
            Ok(res) => st2.ops.finish(
                &op_id,
                serde_json::to_value(&res).unwrap_or_default(),
            ),
            Err(e) => st2.ops.finish_err(&op_id, &e),
        }
    });
    (StatusCode::ACCEPTED, Json(json!({"operation_id": op_id_resp}))).into_response()
}

async fn api_op_snapshot(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let since = q.get("since").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    match state.ops.snapshot(&id, since) {
        Some(snap) => ok_json(serde_json::to_value(&snap).unwrap_or_default()),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"code": "server.opNotFound", "params": {"id": id}})),
        )
            .into_response(),
    }
}

// ── 工廠 ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FactoryRunBody {
    factory: String,
    paths: Vec<String>,
    target_repo: Option<String>,
}

async fn api_factory_run(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<FactoryRunBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    // run_core 含 LLM 子行程——非 SQLite，直接 async。
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        run_core(&cfg, &b.factory, &b.paths, b.target_repo.as_deref()).await
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct WritePagesBody {
    pages: Vec<WritePage>,
    target_repo: Option<String>,
}

async fn api_factory_write_pages(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<WritePagesBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        let notes = std::path::PathBuf::from(
            b.target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()),
        );
        let result = write_pages_core(&notes, &b.pages);
        // 事件 emit（AppState 有則 emit）
        if let Some(as_) = &st.agent_state {
            ocore::factories::emit_factory_events(as_, &cfg, &b.pages);
        }
        Ok::<_, AppError>(result)
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct ExtractCompaniesBody {
    clean: bool,
    target_repo: Option<String>,
}

async fn api_extract_companies(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<ExtractCompaniesBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        extract_companies_core(&cfg, b.clean, b.target_repo.as_deref())
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct SaveAuthoredBody {
    factory: String,
    markdown: String,
    existing_slug: Option<String>,
    target_repo: Option<String>,
}

async fn api_factory_save_authored(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<SaveAuthoredBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let agent_state = st.agent_state.clone();
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        save_authored_core(
            &cfg,
            agent_state.as_ref(),
            &b.factory,
            &b.markdown,
            b.existing_slug.as_deref(),
            b.target_repo.as_deref(),
        )
        .await
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct ClassifyBody {
    paths: Vec<String>,
}

async fn api_factory_classify(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<ClassifyBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    // classify_one 是 async——直接在 handler 跑（無 SQLite）。
    let cfg = match tokio::task::spawn_blocking(move || load_cfg(&st)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return err_response(&e),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"code": "server.internal", "detail": e.to_string()})),
            )
                .into_response()
        }
    };
    let endpoint = ocore::gbrain_config::load_for(cfg.active_env_home())
        .ok()
        .and_then(|loaded| ocore::gbrain_config::resolve_endpoint(&loaded.config).ok())
        .filter(|ep| ep.has_api_key || ep.provider == "ollama");
    let mut out = Vec::with_capacity(b.paths.len());
    for p in &b.paths {
        out.push(classify_one(std::path::Path::new(p), &cfg, endpoint.as_ref()).await);
    }
    ok_json(serde_json::to_value(&out).unwrap_or_default())
}

// ── 前置檢查 ─────────────────────────────────────────────────────────

async fn api_prereq(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_cfg(&st)?;
        Ok(check_all(&cfg.gbrain_exe_path))
    })
    .await;
    finish(res)
}
