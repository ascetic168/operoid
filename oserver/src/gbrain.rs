//! GBrain 能力域 endpoints（P4）——設定／腦管理／來源／長跑操作／工廠／分類／前置檢查。
//!
//! 長跑操作（op_run／sync／bind）走 `OpRegistry` ring buffer：POST 回 202+`operation_id`，
//! 前端輪詢 `GET /api/operations/{id}?since=n` 取增量行與最終結果。
//! 需持久化的操作（brains CRUD）直接寫回 `app-settings.json`（與桌面殼同一檔）。

use std::sync::Arc;

use axum::extract::{Multipart, Path as AxPath, Query, State};
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
use crate::routes::{err_response, require_auth, require_identity, ServerState};

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
            "/api/brains/defaults",
            get(api_brain_defaults_get).put(api_brain_defaults_put),
        )
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
        // 使用者級知識檢索（Req::User；僅 ask/query/think——同步回應，不進 op registry）
        .route("/api/knowledge/ask", post(api_knowledge_ask))
        // 工廠（使用者級——RBAC Req::User；寫入面由 handler 寫入端天花板細化）
        .route("/api/factories/types", get(api_factory_types))
        .route("/api/factories/run", post(api_factory_run))
        .route("/api/factories/write-pages", post(api_factory_write_pages))
        .route("/api/factories/extract-companies", post(api_extract_companies))
        .route("/api/factories/save-authored", post(api_factory_save_authored))
        .route("/api/factories/classify", post(api_factory_classify))
        // 瀏覽器前端無本機路徑——檔案先上傳暫存，再以回傳路徑交 run/classify
        .route(
            "/api/factories/upload",
            post(api_factory_upload).layer(axum::extract::DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .route("/api/factories/upload/cleanup", post(api_factory_upload_cleanup))
        // 前置檢查
        .route("/api/prereq", get(api_prereq))
        // K6 知識管線健康（admin——路由未列於 RBAC 表 → fail-closed 預設 admin-only；
        // 會跑網路／spawn 探測，勿掛啟動路徑）
        .route("/api/knowledge/health", get(api_knowledge_health))
        // K6 精簡能力狀態（Req::User——user 上傳面提示；便宜探測：無 spawn、只打 /v1/models）
        .route("/api/knowledge/caps", get(api_knowledge_caps))
        // K5/P1：PDF 知識入庫（Req::User——企業 user 上傳面；非同步 202＋狀態查詢）
        .route("/api/knowledge/ingest-pdf", post(api_knowledge_ingest_pdf))
        .route(
            "/api/knowledge/ingest-pdf/{id}",
            get(api_knowledge_ingest_status),
        )
        // K5/P1：檢索命中圖片的安全服務（Req::User——只服務 sidecar 登記過的路徑）
        .route("/api/knowledge/figure-image", get(api_knowledge_figure_image))
        // K5/P1：媒體簽名（POST=Req::User，RBAC 表明列；GET=Public——簽名即驗證，
        // <img> 標籤帶不了 Bearer，is_public 放行後由 handler 查驗 HMAC＋sidecar）
        .route("/api/media/figure-urls", post(api_media_sign_figure_urls))
        .route("/api/media/figure", get(api_media_figure))
        .layer(crate::routes::cors_layer())
}

// ── 輔助 ─────────────────────────────────────────────────────────────

fn load_cfg(st: &ServerState) -> Result<AppConfig, AppError> {
    Ok(load_config(&st.settings_dir))
}

fn save_cfg(st: &ServerState, cfg: &AppConfig) -> Result<(), AppError> {
    save_config(&st.settings_dir, cfg)
        .map_err(|e| AppError::new("server.cfgSaveFail").p("detail", e.to_string()))
}

/// 由 settings_dir（<home>/AppData/Roaming/com.operoid.studio 或 --data-dir 覆寫）
/// 反推使用者 home；非預設版面（覆寫）回 None（不做 fallback 探測）。
fn derive_user_home(settings_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // …/AppData/Roaming/com.operoid.studio → 往上三層
    settings_dir.ancestors().nth(3).map(std::path::PathBuf::from).filter(|p| p.join(".bun").exists())
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

// ── 新腦預設 embedding（AppConfig 層；RBAC 未列即 Admin）──
// 只影響「新建腦」的初始值（優先序：此設定 > GBrain config > 內建常數）；
// 既有腦換模型走 `gbrain migrate embeddings`（破壞性全量重嵌，不提供 API）。

#[derive(Deserialize)]
struct BrainDefaultsReq {
    #[serde(default)]
    default_embedding_model: Option<String>,
    #[serde(default)]
    default_embedding_dimensions: Option<i64>,
}

/// 把輸入正規化成可儲存值：空字串／非正維度視同未設定（None）；
/// 模型缺 provider 前綴（無 `:`）拒絕。
fn normalize_brain_defaults(
    model: Option<String>,
    dim: Option<i64>,
) -> Result<(Option<String>, Option<i64>), AppError> {
    let model = model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(m) = &model {
        if !m.contains(':') {
            return Err(AppError::new("brains.defEmbedInvalid"));
        }
    }
    let dim = dim.filter(|d| *d > 0);
    Ok((model, dim))
}

async fn api_brain_defaults_get(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let res = tokio::task::spawn_blocking(move || {
        let c = load_cfg(&state)?;
        Ok(json!({
            "default_embedding_model": c.default_embedding_model,
            "default_embedding_dimensions": c.default_embedding_dimensions,
        }))
    })
    .await;
    finish(res)
}

async fn api_brain_defaults_put(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<BrainDefaultsReq>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let b = body.0;
    let res = tokio::task::spawn_blocking(move || {
        let (model, dim) = normalize_brain_defaults(
            b.default_embedding_model,
            b.default_embedding_dimensions,
        )?;
        let mut c = load_cfg(&st)?;
        c.default_embedding_model = model;
        c.default_embedding_dimensions = dim;
        save_cfg(&st, &c)?;
        Ok(json!({
            "default_embedding_model": c.default_embedding_model,
            "default_embedding_dimensions": c.default_embedding_dimensions,
        }))
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
    // C12a（D-C12a-3）：身份出自 token 鏈（Test 8 完整版）——檢索改道用它構造 AccessContext。
    let Some(requestor) = require_identity(&state, &headers, None).map(|i| i.name) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
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
        // C6（Q7）：檢索 op 改道 KnowledgeService——單人版行為不變、一律留 receipt。
        // C12a-2（Q7 企業面）：`ops_retrieval_enabled=false` 時管理面不繞檢索（403）。
        if matches!(b.op.as_str(), "ask" | "query" | "think") {
            if !cfg.0.ops_retrieval_enabled {
                st2.ops.finish_err(
                    &op_id,
                    &ocore::i18n::AppError::new("op.retrievalDisabled"),
                );
                return;
            }
            // Test 8 完整版：AccessContext 出自請求身份（token 鏈），不出自參數。
            let access = match tokio::task::spawn_blocking({
                let db = st2.db_path.clone();
                let requestor = requestor.clone();
                move || {
                    let store = ocore::domain::SqliteStore::open(&db)?;
                    ocore::knowledge::identity::access_context_for_principal(
                        &store,
                        &requestor,
                        ocore::runtime::AGENT_WS,
                    )
                }
            })
            .await
            {
                Ok(Ok(a)) => a,
                Ok(Err(e)) => {
                    st2.ops.finish_err(
                        &op_id,
                        &ocore::i18n::AppError::new("auth.unauthorized").p("detail", e.to_string()),
                    );
                    return;
                }
                Err(e) => {
                    st2.ops.finish_err(
                        &op_id,
                        &ocore::i18n::AppError::new("server.internal").p("detail", e.to_string()),
                    );
                    return;
                }
            };
            let kind = if b.op == "think" {
                ocore::knowledge::backend::RetrieveKind::Think
            } else {
                ocore::knowledge::backend::RetrieveKind::Search
            };
            let svc = ocore::knowledge::service::service_for_config(&st2.db_path, &cfg.0);
            let tctx = ocore::domain::tools::ToolCtx {
                gbrain_exe: cfg.1.clone(),
                gbrain_home: cfg.0.active_env_home().map(str::to_string),
                chat_model: None,
                mcp: if cfg.0.gbrain_transport == "mcp" {
                    Some(std::sync::Arc::new(ocore::gbrain_mcp::GbrainMcpClient::new(
                        cfg.1.clone(),
                        cfg.0.active_env_home().map(str::to_string),
                    )))
                } else {
                    None
                },
                allowed_tools: Default::default(),
                employee_output_root: std::path::PathBuf::from(&cfg.0.employee_output_path),
                registry: None,
                knowledge: None,
                access,
                turn_max_steps: cfg.0.turn_max_steps,
                tool_result_max_chars: cfg.0.tool_result_max_chars,
            };
            let q = b.arg.clone().unwrap_or_default();
            let res = match svc.retrieve(&tctx.access, kind, &q, None, 10, &tctx).await {
                Ok(o) => {
                    sink(ocore::gbrain_cli::CliLine { stream: "stdout".into(), text: o.text });
                    Ok(ocore::gbrain_cli::OpResult::from_code(0))
                }
                Err(e) => Err(ocore::i18n::AppError::new("op.runFailed").p("detail", e.to_string())),
            };
            match res {
                Ok(res) => st2.ops.finish(&op_id, serde_json::to_value(&res).unwrap_or_default()),
                Err(e) => st2.ops.finish_err(&op_id, &e),
            }
            return;
        }
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

/// 使用者級知識檢索（Req::User；C12a）：**僅** ask／query／think，同步回應（不進
/// op registry——registry id 為流水號，輪詢面不開給 user）。檢索與 manager 面同一
/// KnowledgeService 路徑：身份出自 token 鏈（I4：source 集合只由 policy 推導）、
/// I1 檢索前授權＋I2 fail-closed、一律留 receipt；`ops_retrieval_enabled=false` 時
/// 一併停用。維運／診斷 ops 不在此端點——manager 面 `/api/operations`。
#[derive(serde::Deserialize)]
struct KnowledgeAskBody {
    op: String,
    arg: String,
}

async fn api_knowledge_ask(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<KnowledgeAskBody>,
) -> Response {
    let Some(requestor) = require_identity(&state, &headers, None).map(|i| i.name) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    let b = body.0;
    let kind = match b.op.as_str() {
        "think" => ocore::knowledge::backend::RetrieveKind::Think,
        "ask" | "query" => ocore::knowledge::backend::RetrieveKind::Search,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"code": "auth.forbidden"})),
            )
                .into_response()
        }
    };
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
    if !cfg.0.ops_retrieval_enabled {
        return err_response(&ocore::i18n::AppError::new("op.retrievalDisabled"));
    }
    let access = match tokio::task::spawn_blocking({
        let db = state.db_path.clone();
        let requestor = requestor.clone();
        move || {
            let store = ocore::domain::SqliteStore::open(&db)?;
            ocore::knowledge::identity::access_context_for_principal(
                &store,
                &requestor,
                ocore::runtime::AGENT_WS,
            )
        }
    })
    .await
    {
        Ok(Ok(a)) => a,
        Ok(Err(e)) => {
            return err_response(
                &ocore::i18n::AppError::new("auth.unauthorized").p("detail", e.to_string()),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"code": "server.internal", "detail": e.to_string()})),
            )
                .into_response()
        }
    };
    let tctx = ocore::domain::tools::ToolCtx {
        gbrain_exe: cfg.1.clone(),
        gbrain_home: cfg.0.active_env_home().map(str::to_string),
        chat_model: None,
        mcp: if cfg.0.gbrain_transport == "mcp" {
            Some(std::sync::Arc::new(ocore::gbrain_mcp::GbrainMcpClient::new(
                cfg.1.clone(),
                cfg.0.active_env_home().map(str::to_string),
            )))
        } else {
            None
        },
        allowed_tools: Default::default(),
        employee_output_root: std::path::PathBuf::from(&cfg.0.employee_output_path),
        registry: None,
        knowledge: None,
        access,
        turn_max_steps: cfg.0.turn_max_steps,
        tool_result_max_chars: cfg.0.tool_result_max_chars,
    };
    let svc = ocore::knowledge::service::service_for_config(&state.db_path, &cfg.0);
    match svc.retrieve(&tctx.access, kind, &b.arg, None, 10, &tctx).await {
        Ok(o) => {
            let denied = o.meta.get("denied").and_then(|v| v.as_bool()).unwrap_or(false);
            ok_json(json!({ "text": o.text, "denied": denied }))
        }
        Err(e) => err_response(&ocore::i18n::AppError::new("op.runFailed").p("detail", e.to_string())),
    }
}

// ── 工廠 ─────────────────────────────────────────────────────────────

/// 作用中 schema pack 的工廠類型清單（前端動態渲染用）。
#[derive(serde::Serialize)]
struct FactoryTypesResult {
    pack_name: Option<String>,
    pack_effective: String,
    is_v2: bool,
    types: Vec<ocore::factory_types::FactoryTypeInfo>,
    v2_hint: Option<ocore::i18n::L10n>,
}

async fn api_factory_types(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let st = state.clone();
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        let (pack, name) = ocore::factory_types::active_pack(&cfg);
        Ok::<FactoryTypesResult, AppError>(FactoryTypesResult {
            pack_name: name.clone(),
            pack_effective: pack.name.to_string(),
            is_v2: pack.name.contains("v2"),
            types: ocore::factory_types::type_infos(pack),
            v2_hint: ocore::factory_types::v2_hint(name.clone().as_deref()),
        })
    })
    .await;
    finish(res)
}

#[derive(Deserialize)]
struct FactoryRunBody {
    factory: String,
    paths: Vec<String>,
    target_repo: Option<String>,
    /// C13c：轉換寫入目標（圈子×等級）——缺省＝企業預設來源（target_repo 流）。
    target: Option<AuthoredTarget>,
}

async fn api_factory_run(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<FactoryRunBody>,
) -> Response {
    // C13c（D-C13i）：run 是寫入路徑（pages 落盤 repo）——作者身份出自 token 鏈。
    // 無 target：寫入端天花板按有效 repo（target_repo 缺省＝cfg.notes_repo_path）裁定；
    // 有 target：供給目錄路徑（run_to_scope_core 內建等級天花板）。capture 管線不落盤
    // repo（gbrain capture）→ 兩者皆不適用，target 靜默忽略；未知工廠交 run_core 回錯。
    let Some(owner) = require_identity(&state, &headers, None).map(|i| i.name) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    let st = state.clone();
    let b = body.0;
    // run_core 含 LLM 子行程——非 SQLite，直接 async。
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        let (pack, _) = ocore::factory_types::active_pack(&cfg);
        let writes_repo = pack.spec(&b.factory).map(|s| !s.is_capture()).unwrap_or(false);
        let scoped = writes_repo && b.target.is_some();
        if !writes_repo {
            let preview = run_core(&cfg, &b.factory, &b.paths, None).await?;
            return serde_json::to_value(preview)
                .map_err(|e| AppError::new("server.internal").p("detail", e.to_string()));
        }
        if scoped {
            let t = b.target.expect("scoped ⇒ target");
            let store = ocore::domain::SqliteStore::open(&st.db_path)?;
            let (kind, circle, level) = parse_authored_target(&t)?;
            let (preview, target) = ocore::factories::run_to_scope_core(
                &cfg, &store, &b.factory, &b.paths, kind, &circle, level, &owner,
            )
            .await?;
            let mut v = serde_json::to_value(preview)
                .unwrap_or_else(|_| serde_json::Value::Null);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("scope".into(), json!(target.scope_id));
                obj.insert("source".into(), json!(target.source_id));
            }
            return Ok::<_, AppError>(v);
        }
        // 預設來源流：天花板按有效 repo。
        let repo = b.target_repo.clone().unwrap_or_else(|| cfg.notes_repo_path.clone());
        {
            let store = ocore::domain::SqliteStore::open(&st.db_path)?;
            ocore::knowledge::provision::enforce_write_ceiling(&cfg, &store, &owner, &repo).await?;
        }
        let preview = run_core(&cfg, &b.factory, &b.paths, Some(&repo)).await?;
        serde_json::to_value(preview).map_err(|e| AppError::new("server.internal").p("detail", e.to_string()))
    })
    .await;
    finish(res)
}

/// C13c target 解析（save-authored／run／write-pages 共用）：kind 字串→CircleKind、
/// level 字串→SecurityLevel；circle 空 → "company"。
fn parse_authored_target(
    t: &AuthoredTarget,
) -> Result<(ocore::knowledge::provision::CircleKind, String, ocore::knowledge::types::SecurityLevel), AppError> {
    let kind = match t.kind.as_str() {
        "department" => ocore::knowledge::provision::CircleKind::Department,
        "project" => ocore::knowledge::provision::CircleKind::Project,
        _ => ocore::knowledge::provision::CircleKind::Company,
    };
    let level = serde_json::from_value::<ocore::knowledge::types::SecurityLevel>(json!(t.level))
        .map_err(|e| AppError::new("knowledge.writeFailed").p("detail", e.to_string()))?;
    let circle = if t.circle.trim().is_empty() { "company".into() } else { t.circle.trim().to_string() };
    Ok((kind, circle, level))
}

#[derive(Deserialize)]
struct WritePagesBody {
    pages: Vec<WritePage>,
    target_repo: Option<String>,
    /// C13c：覆蓋寫回轉換時的同一圈子×等級供給目錄——缺省＝企業預設來源。
    target: Option<AuthoredTarget>,
}

async fn api_factory_write_pages(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<WritePagesBody>,
) -> Response {
    // C13c（D-C13i）：批次路徑受寫入端天花板管制——無 target 按有效 repo（缺省＝
    // 預設 notes repo）；有 target 走供給目錄（內建等級天花板，冪等回轉換時的 scope）。
    let Some(owner) = require_identity(&state, &headers, None).map(|i| i.name) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    let st = state.clone();
    let b = body.0;
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        if let Some(t) = &b.target {
            let store = ocore::domain::SqliteStore::open(&st.db_path)?;
            let (kind, circle, level) = parse_authored_target(t)?;
            let (result, _) = ocore::factories::write_pages_to_scope_core(
                &cfg, &store, &b.pages, kind, &circle, level, &owner,
            )
            .await?;
            if let Some(as_) = &st.agent_state {
                ocore::factories::emit_factory_events(as_, &cfg, &b.pages);
            }
            return Ok::<_, AppError>(result);
        }
        let repo = b.target_repo.clone().unwrap_or_else(|| cfg.notes_repo_path.clone());
        {
            let store = ocore::domain::SqliteStore::open(&st.db_path)?;
            ocore::knowledge::provision::enforce_write_ceiling(&cfg, &store, &owner, &repo).await?;
        }
        let notes = std::path::PathBuf::from(&repo);
        let result = write_pages_core(&notes, &b.pages);
        // 事件 emit（AppState 有則 emit）
        if let Some(as_) = &st.agent_state {
            ocore::factories::emit_factory_events(as_, &cfg, &b.pages);
        }
        Ok(result)
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
struct AuthoredTarget {
    /// company | department | project
    kind: String,
    circle: String,
    /// public | internal | confidential | secret
    level: String,
}

#[derive(Deserialize)]
struct SaveAuthoredBody {
    factory: String,
    markdown: String,
    existing_slug: Option<String>,
    target_repo: Option<String>,
    /// C13c（Q12）：可選寫入目標（圈子×等級）——缺省＝co-common 既有行為。
    target: Option<AuthoredTarget>,
}

async fn api_factory_save_authored(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Json<SaveAuthoredBody>,
) -> Response {
    // C13c：身份即作者（token 鏈裁定——D-C13i owner provenance）。
    let Some(owner) = require_identity(&state, &headers, None).map(|i| i.name) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    let st = state.clone();
    let b = body.0;
    let agent_state = st.agent_state.clone();
    let res = tokio::spawn(async move {
        let cfg = load_cfg(&st)?;
        // C13c（D-C13i）：寫入端天花板——所選來源（targetRepo）的等級 ≤ 作者 clearance；
        // target_repo 缺省＝預設 notes repo，同樣受管。指定 target（圈子×等級）時落點是
        // 供給目錄而非此 repo——由 authored_to_scope_core 內建的等級天花板裁定，不重複檢。
        if b.target.is_none() {
            let repo = b.target_repo.clone().unwrap_or_else(|| cfg.notes_repo_path.clone());
            let store = ocore::domain::SqliteStore::open(&st.db_path)?;
            ocore::knowledge::provision::enforce_write_ceiling(&cfg, &store, &owner, &repo).await?;
        }
        match &b.target {
            Some(t) => {
                let store = ocore::domain::SqliteStore::open(&st.db_path)?;
                let (kind, circle, level) = parse_authored_target(t)?;
                let (res, target) = ocore::factories::authored_to_scope_core(
                    &cfg,
                    agent_state.as_ref(),
                    &store,
                    &b.factory,
                    &b.markdown,
                    b.existing_slug.as_deref(),
                    kind,
                    &circle,
                    level,
                    &owner,
                )
                .await?;
                Ok(json!({
                    "slug": res.slug,
                    "target_dir": res.target_dir,
                    "path": res.path,
                    "used_fallback": res.used_fallback,
                    "enriched_markdown": res.enriched_markdown,
                    "names_count": res.names_count,
                    "scope": target.scope_id,
                    "source": target.source_id,
                }))
            }
            None => {
                let r = save_authored_core(
                    &cfg,
                    agent_state.as_ref(),
                    &b.factory,
                    &b.markdown,
                    b.existing_slug.as_deref(),
                    b.target_repo.as_deref(),
                )
                .await?;
                Ok(serde_json::to_value(r).unwrap_or_default())
            }
        }
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

// ── 工廠上傳暫存（企業模式）──────────────────────────────────────────
// 個人版 GUI 交本機路徑給 run/classify；企業前端在瀏覽器裡——檔案先 multipart 上傳到
// OS temp 下的暫存區（每身份一層＋每批次隨機 id 一層），以回傳的伺服器路徑交後續端點。
// 生命週期：前端流程結束呼叫 cleanup；逾時由上傳時的隨手清道夫掃掉（24h）。

/// 上傳 route 的 body 上限（整批；axum 預設 2MB 裝不下 PDF 批次）。
const UPLOAD_BODY_LIMIT: usize = 64 * 1024 * 1024;
/// 單檔上限（超過整檔拒收）與單批檔數上限。
const UPLOAD_MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
const UPLOAD_MAX_FILES: usize = 50;
/// 暫存目錄壽命（清道夫掃描門檻）。
const STAGING_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

fn staging_root() -> std::path::PathBuf {
    std::env::temp_dir().join("operoid-factory-uploads")
}

/// 暫存批次 id（CSPRNG hex——與 SSE 短票同源；同時是 cleanup 的路由鍵）。
fn new_staging_id() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("OS 熵源不可用");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 檔名消毒：取 basename、白名單字元（英數．._-），其餘→`_`；空／點開頭→前綴 file。
fn sanitize_file_name(raw: &str) -> String {
    let base = std::path::Path::new(raw)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() || cleaned.starts_with('.') {
        format!("file{cleaned}")
    } else {
        cleaned
    }
}

/// 目前身分包支援的副檔名（各管線 extensions 聯集，小寫）——上傳白名單。
fn supported_extensions(cfg: &AppConfig) -> Vec<String> {
    let (pack, _) = ocore::factory_types::active_pack(cfg);
    let mut exts: Vec<String> = pack
        .types
        .iter()
        .flat_map(|t| t.pipeline.extensions())
        .map(|e| e.to_ascii_lowercase())
        .collect();
    exts.sort();
    exts.dedup();
    exts
}

/// 隨手清道夫：上傳時順手掃掉逾時暫存目錄（best-effort，失敗不擋上傳）。
fn staging_sweep() {
    let Ok(owners) = std::fs::read_dir(staging_root()) else {
        return;
    };
    for owner in owners.flatten() {
        let Ok(batches) = std::fs::read_dir(owner.path()) else {
            continue;
        };
        for batch in batches.flatten() {
            let stale = batch
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| t.elapsed().unwrap_or_default() >= STAGING_TTL)
                .unwrap_or(false);
            if stale {
                std::fs::remove_dir_all(batch.path()).ok();
            }
        }
    }
}

async fn api_factory_upload(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, None) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    let st = state.clone();
    let owner = sanitize_file_name(&identity.name);
    // 暫存目錄＋白名單在 blocking 域備妥（設定檔讀取沿既有紀律）。
    let prepared = tokio::task::spawn_blocking(move || -> Result<(std::path::PathBuf, String, Vec<String>), AppError> {
        let cfg = load_cfg(&st)?;
        let exts = supported_extensions(&cfg);
        staging_sweep();
        let staging = new_staging_id();
        let dir = staging_root().join(owner).join(&staging);
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::new("factory.uploadFailed").p("detail", e.to_string()))?;
        Ok((dir, staging, exts))
    })
    .await;
    let (dir, staging, exts) = match prepared {
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

    let mut saved: Vec<String> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    loop {
        match mp.next_field().await {
            Ok(Some(field)) => {
                // 非檔案欄位（無 filename）略過——契約只收 files。
                let Some(raw_name) = field.file_name().map(str::to_owned) else {
                    continue;
                };
                if saved.len() + skipped.len() >= UPLOAD_MAX_FILES {
                    skipped.push(json!({"name": raw_name, "reason": "factory.tooManyFiles"}));
                    continue;
                }
                let name = sanitize_file_name(&raw_name);
                let ext_ok = name
                    .rsplit('.')
                    .next()
                    .map(|e| exts.iter().any(|x| x == &e.to_ascii_lowercase()))
                    .unwrap_or(false);
                if !ext_ok {
                    skipped.push(json!({"name": raw_name, "reason": "factory.unsupportedExt"}));
                    continue;
                }
                let bytes = match field.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        skipped.push(json!({"name": raw_name, "reason": "factory.uploadFailed",
                            "detail": e.to_string()}));
                        continue;
                    }
                };
                if bytes.len() > UPLOAD_MAX_FILE_BYTES {
                    skipped.push(json!({"name": raw_name, "reason": "factory.tooLarge"}));
                    continue;
                }
                let path = dir.join(&name);
                // 同批同名 → 序號前綴並存（001-name.ext）。
                let path = if path.exists() {
                    dir.join(format!("{:03}-{}", saved.len() + skipped.len(), name))
                } else {
                    path
                };
                match std::fs::write(&path, &bytes) {
                    Ok(()) => saved.push(path.to_string_lossy().into_owned()),
                    Err(e) => skipped.push(json!({"name": raw_name, "reason": "factory.uploadFailed",
                        "detail": e.to_string()})),
                }
            }
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"code": "factory.uploadFailed", "params": {"detail": e.to_string()}})),
                )
                    .into_response()
            }
        }
    }
    ok_json(json!({ "paths": saved, "staging": staging, "skipped": skipped }))
}

#[derive(Deserialize)]
struct UploadCleanupBody {
    staging: String,
}

async fn api_factory_upload_cleanup(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(body): Json<UploadCleanupBody>,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, None) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()
    };
    // staging id 只可能是 CSPRNG hex——格式不符直接 400（防路徑探測）。
    if body.staging.len() != 32 || !body.staging.chars().all(|c| c.is_ascii_hexdigit()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "factory.stagingInvalid"})),
        )
            .into_response()
    }
    let dir = staging_root()
        .join(sanitize_file_name(&identity.name))
        .join(&body.staging);
    tokio::task::spawn_blocking(move || std::fs::remove_dir_all(dir).ok())
        .await
        .ok();
    ok_json(json!({ "ok": true }))
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
        let cache = cfg.prereq_cache.clone().unwrap_or_default();
        let user_home = derive_user_home(&st.settings_dir);
        let needs_refresh = cache.bun.is_none() || cache.gbrain.is_none();
        let deps = check_all(&cfg.gbrain_exe_path, user_home.as_deref(), &cache);
        Ok((deps, needs_refresh, user_home, cfg.gbrain_exe_path))
    })
    .await;
    match res {
        Ok(Ok((deps, needs_refresh, user_home, gbrain_exe))) => {
            // 版本快取缺漏 → 背景刷新（spawn gbrain 是 bun 冷啟——絕不掛 API 回應）。
            if needs_refresh {
                let st2 = state.clone();
                tokio::task::spawn_blocking(move || {
                    let fresh = ocore::prereq::refresh_details(&gbrain_exe, user_home.as_deref());
                    if let Ok(mut cfg) = load_cfg(&st2) {
                        cfg.prereq_cache = Some(fresh);
                        let _ = save_cfg(&st2, &cfg);
                    }
                });
            }
            ok_json(serde_json::to_value(&deps).unwrap_or_default())
        }
        Ok(Err(e)) => err_response(&e),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "server.internal", "detail": e.to_string()})),
        )
            .into_response(),
    }
}


// ── K6 知識管線健康檢查 ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct KnowledgeHealthQuery {
    /// 嵌入端點 base（預設本地 llama-server http://127.0.0.1:8080/v1）。
    embedding_base: Option<String>,
    /// chat 端點 base＋model 都給時才探 VLM 能力（K5 讀圖/定位模式的判準）。
    chat_base: Option<String>,
    chat_model: Option<String>,
}

async fn api_knowledge_health(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<KnowledgeHealthQuery>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let cfg = match load_cfg(&state) {
        Ok(c) => c,
        Err(e) => return err_response(&e),
    };
    let embed_base = q
        .embedding_base
        .unwrap_or_else(|| ocore::knowledge::doctor::DEFAULT_EMBEDDING_BASE.into());
    let chat = q.chat_base.as_deref().zip(q.chat_model.as_deref());
    let health =
        ocore::knowledge::doctor::check(&embed_base, chat, &cfg.convert_config()).await;
    ok_json(serde_json::to_value(&health).unwrap_or_default())
}


/// `GET /api/knowledge/caps`——使用者層級的精簡能力狀態（企業版 user 前端上傳面）。
/// 只回答「複雜 PDF 能否完整轉換／圖片索引有無多模態」，不含管理資訊。
async fn api_knowledge_caps(State(state): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let cfg = match load_cfg(&state) {
        Ok(c) => c,
        Err(e) => return err_response(&e),
    };
    let caps = ocore::knowledge::doctor::quick_status(
        ocore::knowledge::doctor::DEFAULT_EMBEDDING_BASE,
        &cfg.convert_config(),
    )
    .await;
    ok_json(serde_json::to_value(&caps).unwrap_or_default())
}


// ── K5/P1：PDF 知識入庫（非同步；狀態表行程內）──────────────────────────

static INGESTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[derive(Deserialize)]
struct IngestPdfBody {
    /// PDF 絕對路徑（企業端上傳暫存路徑或伺服器本機路徑）。
    pdf: String,
    /// 授權歸檔 source id；缺省自動以 notes repo 比對 `sources list`。
    source_id: Option<String>,
    /// 生產 sidecar 路徑；缺省 cfg.figures_db_path，再缺省 notes repo 伴隨檔。
    figures_db: Option<String>,
}

/// `POST /api/knowledge/ingest-pdf`——PDF → 知識庫入庫（非同步；MinerU 解析為
/// 長操作，立即回 202＋`ingest_id`，以 GET 查詢狀態與報告）。
async fn api_knowledge_ingest_pdf(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(body): Json<IngestPdfBody>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let cfg = match load_cfg(&state) {
        Ok(c) => c,
        Err(e) => return err_response(&e),
    };
    let pdf = std::path::PathBuf::from(&body.pdf);
    if !pdf.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "ingest.pdfNotFound", "detail": body.pdf})),
        )
            .into_response();
    }
    let notes = std::path::PathBuf::from(cfg.notes_repo_path.trim_end_matches('/'));
    let source_id = match body.source_id.clone() {
        Some(s) if !s.trim().is_empty() => s,
        _ => match ocore::knowledge::ingest::resolve_source_id(
            &cfg.gbrain_exe_path,
            cfg.active_env_home(),
            &notes,
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"code": "ingest.noSource", "detail": e.to_string()})),
                )
                    .into_response()
            }
        },
    };
    let figures_db = body
        .figures_db
        .clone()
        .or_else(|| cfg.figures_db_path.clone())
        .unwrap_or_else(|| {
            ocore::knowledge::ingest::default_figures_db(&notes).to_string_lossy().into_owned()
        });

    let id = format!(
        "ing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    INGESTS
        .lock()
        .expect("ingests")
        .insert(id.clone(), json!({ "state": "running", "pdf": body.pdf }));
    let ingest_id = id.clone();

    let gbrain_exe = cfg.gbrain_exe_path.clone();
    let home = cfg.active_env_home().map(str::to_string);
    let convert_cfg = cfg.convert_config();
    tokio::spawn(async move {
        let result = ocore::knowledge::ingest::ingest_pdf(
            &pdf,
            &convert_cfg,
            std::path::Path::new(&figures_db),
            std::path::Path::new(&notes),
            &source_id,
            &gbrain_exe,
            home.as_deref(),
        )
        .await;
        let mut st = INGESTS.lock().expect("ingests");
        match result {
            Ok(r) => {
                let entry = st.get_mut(&id).expect("ingest entry");
                *entry = json!({
                    "state": "done",
                    "report": {
                        "doc_id": r.doc_id,
                        "notes_written": r.notes_written,
                        "figure_rows": r.figure_rows,
                        "vectors_carried": r.vectors_carried,
                        "vectors_embedded": r.vectors_embedded,
                        "synced": r.synced,
                        "warnings": r.warnings,
                    }
                });
            }
            Err(e) => {
                let entry = st.get_mut(&id).expect("ingest entry");
                *entry = json!({ "state": "error", "error": e.to_string() });
            }
        }
    });
    ok_json(json!({ "ingest_id": ingest_id, "state": "running" }))
}

/// `GET /api/knowledge/ingest-pdf/{id}`——入庫狀態／報告。
async fn api_knowledge_ingest_status(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    match INGESTS.lock().expect("ingests").get(&id) {
        Some(v) => ok_json(v.clone()),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"code": "ingest.notFound", "detail": id})),
        )
            .into_response(),
    }
}


/// `GET /api/knowledge/figure-image?path=…`——檢索命中圖片的安全服務。
/// **allowlist 鐵律**：只服務 sidecar `figures` 表中登記過 `image_path` 的檔案
/// （路徑攻擊天然被擋：未入庫＝未授權）。回圖位元組＋Content-Type。
async fn api_knowledge_figure_image(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let Some(path) = q.get("path").cloned() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "figure.pathRequired"})),
        )
            .into_response();
    };
    serve_registered_figure(&state, &path).await
}


// ── K5/P1：媒體簽名 URL（企業瀏覽器讀圖的認證面擴充）────────────────────

static MEDIA_SIGNER: std::sync::LazyLock<ocore::knowledge::media::MediaSigner> =
    std::sync::LazyLock::new(|| {
        ocore::knowledge::media::MediaSigner::from_random().expect("os entropy")
    });

const MEDIA_URL_TTL_SECS: u64 = 300;

/// `POST /api/media/figure-urls`——批量簽發圖片短時 URL。
/// **簽名前先過知識織網授權**（M1 鐵律）：圖的 source 必須在請求者 principal
/// 的 authorized_sources 內，否則拒簽。
async fn api_media_sign_figure_urls(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, None) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response();
    };
    let Some(paths) = body["paths"].as_array() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "media.pathsRequired"})),
        )
            .into_response();
    };
    let cfg = match load_cfg(&state) {
        Ok(c) => c,
        Err(e) => return err_response(&e),
    };
    let figures_db = cfg
        .figures_db_path
        .clone()
        .unwrap_or_else(|| {
            ocore::knowledge::ingest::default_figures_db(std::path::Path::new(
                cfg.notes_repo_path.trim_end_matches('/'),
            ))
            .to_string_lossy()
            .into_owned()
        });

    // M1 授權面：以請求者 principal 評估 authorized_sources（複用 KnowledgeService 鐵律）。
    let store = match ocore::domain::SqliteStore::open(&state.db_path) {
        Ok(s) => s,
        Err(e) => return err_response(&AppError::new("server.internal").p("detail", e.to_string())),
    };
    let access = ocore::knowledge::types::AccessContext {
        principal_id: identity.name.clone(),
        principal_type: ocore::knowledge::types::PrincipalType::Human,
        employee_id: None,
        workspace_id: ocore::runtime::AGENT_WS.into(),
        roles: identity.roles.clone(),
        departments: vec![],
        projects: vec![],
        task_id: None,
        purpose: Some("media-sign".into()),
        clearance: None,
    };
    let svc = ocore::knowledge::service::KnowledgeService::new(&state.db_path);
    let plan = match svc.plan(&store, &access) {
        Ok(p) => p,
        Err(e) => return err_response(&AppError::new("server.internal").p("detail", e.to_string())),
    };

    let sidecar = ocore::knowledge::figures::Sidecar::open(&figures_db).ok();
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + MEDIA_URL_TTL_SECS;
    let mut out: Vec<serde_json::Value> = Vec::new();
    for p in paths {
        let Some(path) = p.as_str() else { continue };
        // 授權：圖的 source ∈ 請求者 authorized_sources（未登記/未授權 → 拒簽）。
        let authorized = sidecar
            .as_ref()
            .and_then(|s| s.source_of_path(path).ok().flatten())
            .map(|src| plan.source_ids.iter().any(|a| a == &src))
            .unwrap_or(false);
        if !authorized {
            out.push(json!({ "path": path, "authorized": false }));
            continue;
        }
        let sig = MEDIA_SIGNER.sign(path, &identity.name, exp);
        let url = format!(
            "/api/media/figure?path={}&principal={}&exp={}&sig={}",
            ocore::knowledge::media::percent_encode(path),
            ocore::knowledge::media::percent_encode(&identity.name),
            exp,
            sig
        );
        out.push(json!({ "path": path, "authorized": true, "url": url, "exp": exp }));
    }
    ok_json(json!({ "urls": out }))
}

/// `GET /api/media/figure`——帶簽圖片服務：簽名驗證＋sidecar allowlist
/// （路徑必須登記過）雙重查驗後回圖位元組。路由為 Public——簽名即驗證
/// （綁 path＋principal＋過期，開機隨機金鑰）。
async fn api_media_figure(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    // 帶 Bearer 者（個人 GUI／程式化客戶端）直接走既有認證。
    if headers.get("authorization").is_some() {
        if let Err(r) = require_auth(&state, &headers) {
            return r;
        }
    }
    let (Some(path), Some(principal), Some(exp_s), Some(sig)) = (
        q.get("path").cloned(),
        q.get("principal").cloned(),
        q.get("exp").and_then(|v| v.parse::<u64>().ok()),
        q.get("sig").cloned(),
    ) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "media.paramsRequired"})),
        )
            .into_response();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if !MEDIA_SIGNER.verify(&path, &principal, exp_s, now, &sig) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "media.badSignature"})),
        )
            .into_response();
    }
    serve_registered_figure(&state, &path).await
}

/// 共用：sidecar allowlist 查驗＋讀檔回圖。
async fn serve_registered_figure(state: &Arc<ServerState>, path: &str) -> Response {
    let cfg = match load_cfg(state) {
        Ok(c) => c,
        Err(e) => return err_response(&e),
    };
    let figures_db = cfg
        .figures_db_path
        .clone()
        .unwrap_or_else(|| {
            ocore::knowledge::ingest::default_figures_db(std::path::Path::new(
                cfg.notes_repo_path.trim_end_matches('/'),
            ))
            .to_string_lossy()
            .into_owned()
        });
    let registered = std::path::Path::new(&figures_db).is_file()
        && rusqlite::Connection::open(&figures_db)
            .and_then(|db| {
                db.query_row(
                    "SELECT COUNT(*) FROM figures WHERE image_path = ?1",
                    [path],
                    |r| r.get::<_, i64>(0),
                )
            })
            .map(|n| n > 0)
            .unwrap_or(false);
    if !registered {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"code": "figure.notRegistered"})),
        )
            .into_response();
    }
    let p = std::path::Path::new(path);
    let mime = match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    match tokio::fs::read(p).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (axum::http::header::CONTENT_TYPE, mime.to_string()),
                (
                    axum::http::header::CACHE_CONTROL,
                    "private, max-age=86400".to_string(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"code": "figure.fileMissing"})),
        )
            .into_response(),
    }
}
