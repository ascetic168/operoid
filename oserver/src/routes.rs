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
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;
use serde_json::json;

use ocore::app_config::AppConfig;
use ocore::domain::{SqliteStore, Store};
use ocore::i18n::AppError;
use ocore::runtime::{
    inbox_summary_payload, list_state_payload, recent_events_payload, registry_divergence_payload,
    registry_load_payload, watch_payload,
};

use crate::auth::{AuthProvider, Identity};

/// 服務共享狀態。
pub struct ServerState {
    pub auth: Arc<dyn AuthProvider>,
    pub cfg: AppConfig,
    pub db_path: PathBuf,
    pub ready: Arc<std::sync::atomic::AtomicBool>,
    /// scheduler 的 AppState（寫入面喚醒用；初始化完成後注入——warming 期 None → 503）。
    pub agent_state: Option<ocore::agent_state::AppState>,
    /// 長跑操作 ring buffer（P4：op_run／sync／bind 的輪詢主控台）。
    pub ops: Arc<crate::operations::OpRegistry>,
    /// 桌面設定目錄（app-settings.json 讀寫——與殼同一檔）。
    pub settings_dir: std::path::PathBuf,
    /// server token（啟動期解析結果）——obridge managed 模式寫入 ingress secret 用；
    /// 僅伺服器端使用，不出任何 API。
    pub server_token: Option<String>,
    /// 本服務實際 bind 的 port（obridge managed 模式推導 ingress_url 用）。
    pub server_port: u16,
}

/// 統一錯誕回應：`AppError` → JSON＋status 映射。
pub(crate) fn err_response(e: &AppError) -> Response {
    // AppError 序列化形狀與桌面 IPC 一致（code＋params），前端錯誤鍵可直接沿用。
    let body = serde_json::to_value(e).unwrap_or_else(|_| json!({"code": "server.internal"}));
    let status = match e.code.as_str() {
        "agent_os.employeeNotFound" | "agent_os.templateNotFound" | "agent_os.commitmentNotFound"
        | "agent_os.taskNotFound" => StatusCode::NOT_FOUND,
        "agent_os.employeeBusy" => StatusCode::CONFLICT, // busy-lock 快速回絕（API 契約）
        "agent_os.employeeNotRunning" | "agent_os.employeeArchived" => StatusCode::CONFLICT,
        "agent_os.disabled" | "server.notReady" | "server.dbOpenFail" => StatusCode::SERVICE_UNAVAILABLE,
        "agent_os.invalidTransition" => StatusCode::CONFLICT,
        // R2：認證／授權語意（401 未認證、403 無權、429 鎖定、400 政策不合）
        "auth.unauthorized" | "auth.invalidCredentials" => StatusCode::UNAUTHORIZED,
        "auth.forbidden" | "auth.accountDisabled" | "auth.mustChangePassword" => StatusCode::FORBIDDEN,
        "auth.accountLocked" => StatusCode::TOO_MANY_REQUESTS,
        "auth.weakPassword" | "auth.accountCreateFailed" | "auth.noSession" => StatusCode::BAD_REQUEST,
        // obridge 管理（企業模式）：驗證失敗／未啟用／非代管模式 → 400；重複啟動 → 409；exe 缺 → 404
        "obridge.configInvalid" | "obridge.notEnabled" | "obridge.notManaged" => StatusCode::BAD_REQUEST,
        "obridge.alreadyRunning" => StatusCode::CONFLICT,
        "obridge.exeNotFound" => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(body)).into_response()
}

/// 認證中介：所有 /api/* 走此處（/healthz 在 router 層免認證）。
/// R2：RBAC 中介層已做 authn 並把 `Identity` 放進 extensions——此處為縱深防禦
/// 的第二次檢查（未來最佳化可讀 extensions，先不動 60 個呼叫點）。
pub(crate) fn require_auth(state: &ServerState, headers: &HeaderMap) -> Result<(), Response> {
    match require_identity(state, headers, None) {
        Some(_) => Ok(()),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "auth.unauthorized"})),
        )
            .into_response()),
    }
}

/// R2：認證並回傳請求身份——優先讀 RBAC 中介層放入 extensions 的 `Identity`，
/// 沒有才走 header 檢查。身份一律出自 token 鏈，不出自呼叫端參數（Test 8 保證）。
pub(crate) fn require_identity(
    state: &ServerState,
    headers: &HeaderMap,
    ext: Option<axum::Extension<Identity>>,
) -> Option<Identity> {
    if let Some(axum::Extension(id)) = ext {
        return Some(id);
    }
    let h = headers.get("authorization").and_then(|v| v.to_str().ok());
    state.auth.check(h).ok()
}

/// R2：「限自身」過濾的資料源——某 principal 名下（owner_principal）的員工 id 集。
pub(crate) fn owned_employee_ids(
    store: &SqliteStore,
    principal_id: &str,
) -> Result<std::collections::HashSet<String>, AppError> {
    Ok(store
        .list_all_employees()?
        .into_iter()
        .filter(|e| e.owner_principal.as_deref() == Some(principal_id))
        .map(|e| e.id)
        .collect())
}

/// 開 store（handler 內先認證、再進 spawn_blocking 開連線）。
pub(crate) fn open_store(state: &ServerState) -> Result<SqliteStore, AppError> {
    SqliteStore::open(&state.db_path).map_err(|e| {
        AppError::new("server.dbOpenFail").p("detail", e.to_string())
    })
}

/// CORS 允許清單（遠端化 R1 收緊——取代 `very_permissive`，差距清單第 1 條：
/// Bearer 過網路後不得再配 permissive CORS）：
/// - Tauri webview（個人模式）：macOS/Linux `tauri://localhost`、Windows `http(s)://tauri.localhost`；
/// - vite dev（開發期）：`http://localhost:1420`／`http://127.0.0.1:1420`；
/// - 企業模式三前端由 oserver 同 origin 服務（`/{app}` 靜態檔）——瀏覽器請求不經 CORS。
pub fn cors_layer() -> CorsLayer {
    use axum::http::{header, HeaderValue, Method};
    CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("tauri://localhost"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("https://tauri.localhost"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://127.0.0.1:1420"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        // E7 ingress（P5 併入）：外部事件投遞口——Bearer＝server token、
        // (source, external_ref) 去重、dispatch_event 喚醒腦匹配員工。
        .route("/event", post(api_event))
        .route("/api/state", get(api_state))
        .route("/api/employees", get(api_employees))
        .route("/api/templates", get(api_templates))
        .route("/api/employees/{id}/watch", get(api_watch))
        .route("/api/inbox", get(api_inbox))
        .route("/api/events", get(api_events))
        .route("/api/registry", get(api_registry))
        .route("/api/service/status", get(api_service_status))
        .layer(cors_layer())
        .with_state(state)
}

/// 免認證健康檢查：`warming`（初始化中）→ `ready`。
/// R3 加驗：`version`——GUI/前端可比對新舊（遠端化診斷教訓：舊版服務被 healthz
/// 探測誤沿用，造成「部分端點 404」的隱性故障）。
async fn healthz(State(state): State<Arc<ServerState>>) -> Response {
    let ready = state.ready.load(std::sync::atomic::Ordering::SeqCst);
    Json(json!({
        "status": if ready { "ready" } else { "warming" },
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
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
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        // R2「限自身」：user 僅能監看自己名下的員工（manager/admin 不限；既有員工＝operator 歸屬）。
        if let Some(ident) = &identity {
            if let Some(emp) = store.get_employee(&id)? {
                if !crate::rbac::can_access_employee(ident, emp.owner_principal.as_deref()) {
                    return Err(AppError::new("auth.forbidden"));
                }
            }
        }
        watch_payload(&st.cfg, &store, &id)
    })
    .await;
    finish(res)
}

async fn api_inbox(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        let summary = inbox_summary_payload(&store)?;
        match &identity {
            // R2：user 的收件匣過濾到「自身相關」（員工歸屬）。
            Some(id) if !crate::rbac::satisfies(id, crate::rbac::Req::Manager) => {
                let owned = owned_employee_ids(&store, &id.name)?;
                let v = serde_json::to_value(&summary)
                    .unwrap_or_else(|_| serde_json::Value::Null);
                Ok(filter_payload_by_employees(
                    v,
                    &[("proposals", "employee_id"), ("flagged_employees", "employee_id")],
                    &owned,
                ))
            }
            _ => Ok(serde_json::to_value(&summary).unwrap_or_else(|_| serde_json::Value::Null)),
        }
    })
    .await;
    finish(res)
}

async fn api_events(
    State(state): State<Arc<ServerState>>,
    identity: Option<axum::Extension<Identity>>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let identity = require_identity(&state, &headers, identity);
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    let st = state.clone();
    let res = tokio::task::spawn_blocking(move || {
        check_enabled(&st)?;
        let store = open_store(&st)?;
        let events = recent_events_payload(&store, limit)?;
        match &identity {
            // R2：user 的事件流過濾到「自身相關」。
            Some(id) if !crate::rbac::satisfies(id, crate::rbac::Req::Manager) => {
                let owned = owned_employee_ids(&store, &id.name)?;
                Ok(serde_json::json!(events
                    .into_iter()
                    .filter(|e| owned.contains(&e.employee_id))
                    .collect::<Vec<_>>()))
            }
            _ => Ok(serde_json::json!(events)),
        }
    })
    .await;
    finish(res)
}

/// R2：依員工歸屬過濾 payload 內的陣列（`[(欄位名, 員工id欄)]`）；缺欄位的項目保留。
pub(crate) fn filter_payload_by_employees(
    mut v: serde_json::Value,
    arrays: &[(&str, &str)],
    owned: &std::collections::HashSet<String>,
) -> serde_json::Value {
    if let Some(obj) = v.as_object_mut() {
        for (key, field) in arrays {
            if let Some(arr) = obj.get_mut(*key).and_then(|x| x.as_array_mut()) {
                arr.retain(|item| {
                    item.get(*field)
                        .and_then(|f| f.as_str())
                        .map_or(true, |eid| owned.contains(eid))
                });
            }
        }
    }
    v
}

async fn api_registry(
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
        let data_dir = st
            .db_path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf);
        let mut payload = registry_load_payload(&data_dir);
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("divergence".into(), registry_divergence_payload(&store));
        }
        Ok(payload)
    })
    .await;
    finish(res)
}

// ── 輔助 ─────────────────────────────────────────────────────────────

/// R3：服務狀態唯讀（admin——矩陣明示 Admin）。`running` 以本行程存在為準
/// （服務在跑才答得到）；`installed` 查 OS 服務註冊。install/uninstall 不開遠端
/// Web 操作（克制清單——管理面不開遠端破壞性動作，留伺服器 CLI）。
async fn api_service_status(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let installed = crate::service::is_installed().unwrap_or(false);
    Json(json!({"installed": installed, "running": true})).into_response()
}

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


/// 外部事件投遞（原殼層 ingress_server，P5 併入）：
/// 認證（server token）→ 去重（session 內 (source, external_ref) 首見）→ dispatch。
/// 重複→`200 duplicate; ignored`；首見→`202 accepted`（喚醒為非同步，結果見 events）。
async fn api_event(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    use ocore::agent_state::InboundEvent;
    if let Err(r) = require_auth(&state, &headers) {
        return r;
    }
    let ev: InboundEvent = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"code": "ingress.badRequest", "params": {"detail": e.to_string()}})),
            )
                .into_response()
        }
    };
    let Some(app_state) = state.agent_state.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"code": "server.notReady"}))).into_response();
    };
    // 去重（session 內；重啟清空——bridge 應自追 last-seen）。
    if let Some(ext_ref) = ev.external_ref.as_deref() {
        if !app_state.is_new_external_ref(&ev.source, ext_ref) {
            return (StatusCode::OK, Json(json!({"status": "duplicate; ignored"}))).into_response();
        }
    }
    // dispatch（cfg 即時載——熱生效；operoid.toml 覆寫與 app-settings 同批）。
    let toml_cfg = crate::operoid_toml::load(&state.settings_dir).unwrap_or(None);
    let cfg = crate::config::load_effective(&state.settings_dir, toml_cfg.as_ref())
        .unwrap_or_else(|_| crate::config::load_config(&state.settings_dir));
    let db_path = state.db_path.clone();
    match ocore::event_bus::dispatch_event(app_state, &cfg, &db_path, ev).await {
        Ok(()) => (StatusCode::ACCEPTED, Json(json!({"status": "accepted"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code": "ingress.dispatchFail", "params": {"detail": e.to_string()}})),
        )
            .into_response(),
    }
}
