//! SSE 事件推送（遠端化 R3，DR-E8）——event bus 的推送面。
//!
//! 認證：EventSource **無法帶 Authorization header** → 一次性短票：
//! 1. 已認證客戶端 `POST /api/stream/ticket`（Bearer）→ 60s 一次性票（CSPRNG）；
//! 2. `GET /api/stream?ticket=...` → RBAC 中介層驗票（單次消費）→ 構造 Identity；
//!    亦接受一般 Bearer header（非 EventSource 客戶端）。
//! 3. handler 內每秒輪詢 store 的新事件（`created_at >` 上次所見）→ SSE `data:` 推送；
//!    user 身份過濾到自身相關（同 inbox/events 語意）；無事件時推註解心跳。
//!
//! 輪詢橋接而非 event bus 訂閱——零架構變更（事件匯流排的訂閱面屬 H8 另案）。
//! 票為**行程內**狀態（重啟即失效——客戶端重取即可；跨實例共用隨 C12b 完整版）。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde_json::json;

use crate::auth::Identity;
use crate::routes::{
    check_enabled, cors_layer, err_response, open_store, require_identity, ServerState,
};
use ocore::domain::Store;

const TICKET_TTL: Duration = Duration::from_secs(60);
/// 輪詢週期。
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// 每次輪詢最多取回的事件數。
const POLL_LIMIT: usize = 100;

static TICKETS: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

fn with_tickets<T>(f: impl FnOnce(&mut HashMap<String, (String, Instant)>) -> T) -> T {
    let mut g = TICKETS.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    // 順手清過期票。
    let now = Instant::now();
    m.retain(|_, (_, exp)| *exp > now);
    f(m)
}

/// 簽發一次性短票（60s、單次消費、CSPRNG）。
pub fn issue_ticket(principal_id: &str) -> String {
    let mut b = [0u8; 24];
    getrandom::getrandom(&mut b).expect("OS 熵源不可用");
    let ticket: String = b.iter().map(|x| format!("{x:02x}")).collect();
    with_tickets(|m| {
        m.insert(ticket.clone(), (principal_id.to_string(), Instant::now() + TICKET_TTL));
    });
    ticket
}

/// 驗票（單次消費——取出即失效）；過期／不存在 → None。
pub fn consume_ticket(ticket: &str) -> Option<String> {
    with_tickets(|m| {
        match m.remove(ticket) {
            Some((principal, exp)) if Instant::now() <= exp => Some(principal),
            _ => None,
        }
    })
}

pub fn sse_routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/stream/ticket", post(api_stream_ticket))
        .route("/api/stream", axum::routing::get(api_stream))
        .route("/api/employees/{id}/stream", get(api_employee_stream))
        .layer(cors_layer())
}

/// 簽發 SSE 短票（任何已認證身份；RBAC 中介層擋未認證）。
async fn api_stream_ticket(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    let ticket = issue_ticket(&identity.name);
    (StatusCode::OK, Json(json!({"ticket": ticket, "ttl_secs": TICKET_TTL.as_secs()}))).into_response()
}

/// SSE 事件流。查詢參數 `ticket`（一次性）或一般 Bearer header 均可（中介層裁定）。
async fn api_stream(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    let user_scoped = !crate::rbac::satisfies(&identity, crate::rbac::Req::Manager);
    let who = identity.name.clone();
    let st = state.clone();

    // user 身份：起流時抓一次歸屬集（變更罕見；v1 取捨）。
    let st_for_owned = st.clone();
    let owned: std::collections::HashSet<String> = tokio::task::spawn_blocking(move || {
        let store = open_store(&st_for_owned)?;
        crate::routes::owned_employee_ids(&store, &who)
    })
    .await
    .unwrap_or_else(|_| Err(ocore::i18n::AppError::new("server.internal")))
    .unwrap_or_default();

    // 水位＝連線當下的 RFC3339（只推連線後的新事件；RFC3339 同格式字串序＝時間序）。
    let last_seen = Some(ocore::domain::now_rfc3339());

    let stream = futures::stream::unfold(
        (st, owned, user_scoped, last_seen, VecDeque::new()),
        |(st, owned, user_scoped, mut last_seen, mut pending)| async move {
            loop {
                if let Some(ev) = pending.pop_front() {
                    let data = serde_json::to_string(&ev).unwrap_or_default();
                    return Some((
                        Ok::<_, std::convert::Infallible>(
                            SseEvent::default().event("event").data(data),
                        ),
                        (st, owned, user_scoped, last_seen, pending),
                    ));
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                let st2 = st.clone();
                let last = last_seen.clone();
                let owned2 = owned.clone();
                let res = tokio::task::spawn_blocking(move || {
                    let store = open_store(&st2)?;
                    let events = ocore::runtime::recent_events_payload(&store, POLL_LIMIT)?;
                    let owned_ref = &owned2;
                    Ok::<_, ocore::i18n::AppError>(
                        events
                            .into_iter()
                            .filter(|e| match &last {
                                Some(l) => e.created_at.as_str() > l.as_str(),
                                None => false,
                            })
                            .filter(|e| !user_scoped || owned_ref.contains(&e.employee_id))
                            .collect::<Vec<_>>(),
                    )
                })
                .await;
                if let Ok(Ok(events)) = res {
                    if events.is_empty() {
                        // 心跳（註解行——客戶端可藉此判活）。
                        return Some((
                            Ok(SseEvent::default().comment("ping")),
                            (st, owned, user_scoped, last_seen, pending),
                        ));
                    }
                    let mut q = VecDeque::new();
                    for e in &events {
                        if let Ok(v) = serde_json::to_value(e) {
                            q.push_back(v);
                        }
                    }
                    // 更新水位（RFC3339 字串序＝時間序——chrono to_rfc3339 格式一致）。
                    if let Some(e) = events.iter().max_by(|a, b| a.created_at.cmp(&b.created_at)) {
                        last_seen = Some(e.created_at.clone());
                    }
                    pending = q;
                    continue;
                }
                // 查詢失敗 → 心跳（不中斷流）。
                return Some((
                    Ok(SseEvent::default().comment("ping")),
                    (st, owned, user_scoped, last_seen, pending),
                ));
            }
        },
    );

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"))
        .into_response()
}

/// TurnDelta 的 employee_id 投影（匯流排過濾用——單通道廣播、接收端按員工分流）。
fn delta_employee(ev: &ocore::agent_state::TurnDelta) -> &str {
    use ocore::agent_state::TurnDelta as TD;
    match ev {
        TD::StepStart { employee_id, .. }
        | TD::TextDelta { employee_id, .. }
        | TD::TextEnd { employee_id, .. }
        | TD::ThinkingDelta { employee_id, .. }
        | TD::ToolCallStart { employee_id, .. }
        | TD::ToolCallEnd { employee_id, .. }
        | TD::TurnEnd { employee_id, .. } => employee_id,
    }
}

/// 員工級回合 delta 流（F1 串流）：訂閱行程內 broadcast 匯流排**直推**——與 `/api/stream`
/// 的 DB 輪詢橋不同，delta 為 ephemeral（不落 DB），只能來自匯流排（`TurnDelta` 文檔）。
/// 認證：短票（EventSource 無法帶 header；rbac 中介層特例消費）或一般 Bearer。
/// 「限自身」：user 僅能訂閱名下員工（同 watch 慣例——`can_access_employee`，
/// manager/admin 不限；不存在／非名下一律 forbidden，不洩漏存在性）。
/// 幀：`event: delta`，data 為 TurnDelta JSON（kind 標籤）。Lagged（廣播溢出）→
/// `{"kind":"resync"}`（客戶端丟草稿回退輪詢）；發佈端全關（AppState drop）→ 流結束。
async fn api_employee_stream(
    State(state): State<Arc<ServerState>>,
    identity: Option<Extension<Identity>>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let Some(identity) = require_identity(&state, &headers, identity) else {
        return err_response(&ocore::i18n::AppError::new("auth.unauthorized"));
    };
    if let Err(e) = check_enabled(&state) {
        return err_response(&e);
    }
    // 限自身（訂閱期間歸屬視為不變——與 /api/stream 的 owned 集快照同取捨）。
    let ident = identity.clone();
    let st = state.clone();
    let emp_id = id.clone();
    let allowed = tokio::task::spawn_blocking(move || -> bool {
        let Ok(store) = open_store(&st) else { return false };
        match store.get_employee(&emp_id) {
            Ok(Some(emp)) => {
                crate::rbac::can_access_employee(&ident, emp.owner_principal.as_deref())
            }
            _ => false,
        }
    })
    .await
    .unwrap_or(false);
    if !allowed {
        return err_response(&ocore::i18n::AppError::new("auth.forbidden"));
    }
    // 訂閱先於回應建構（開流前的 delta 不漏）。
    let Some(app_state) = state.agent_state.as_ref() else {
        return err_response(&ocore::i18n::AppError::new("agent_os.disabled"));
    };
    let rx = app_state.subscribe_turn_deltas();

    let stream = futures::stream::unfold((rx, id), move |(mut rx, id)| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if delta_employee(&ev) != id {
                        continue; // 其他員工的 delta——過濾。
                    }
                    let data = serde_json::to_string(&ev).unwrap_or_default();
                    return Some((
                        Ok::<_, std::convert::Infallible>(
                            SseEvent::default().event("delta").data(data),
                        ),
                        (rx, id),
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[sse] 員工 delta 流 Lagged（丟失 {n} 幀）——通知客戶端 resync");
                    let data = json!({"kind": "resync", "lost": n}).to_string();
                    return Some((
                        Ok(SseEvent::default().event("delta").data(data)),
                        (rx, id),
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthError, AuthProvider};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use std::sync::atomic::AtomicBool;
    use tower::ServiceExt;

    #[test]
    fn ticket_single_use_and_expiry() {
        let t = issue_ticket("principal-x");
        // 單次消費。
        assert_eq!(consume_ticket(&t).as_deref(), Some("principal-x"));
        assert_eq!(consume_ticket(&t), None, "第二次消費必須失敗");
        // 壞票。
        assert_eq!(consume_ticket("nonexistent"), None);
    }

    /// F1 端點測試用 stub auth：token → 固定身份（usr＝principal-user）。
    struct StubAuth;
    impl AuthProvider for StubAuth {
        fn check(&self, auth_header: Option<&str>) -> Result<Identity, AuthError> {
            match auth_header.and_then(|h| h.strip_prefix("Bearer ").map(str::trim)) {
                Some("usr") => Ok(Identity {
                    name: "principal-user".into(),
                    token_id: None,
                    roles: vec!["user".into()],
                    must_change_password: false,
                }),
                _ => Err(AuthError),
            }
        }
    }

    fn stream_test_state(db: &std::path::Path) -> Arc<ServerState> {
        let (wtx, _wrx) = tokio::sync::mpsc::channel(8);
        let (etx, _erx) = tokio::sync::mpsc::channel::<ocore::agent_state::InboundEvent>(8);
        let mut cfg = ocore::app_config::AppConfig::default();
        cfg.agent_os_enabled = true;
        Arc::new(ServerState {
            auth: Arc::new(StubAuth),
            cfg,
            db_path: db.to_path_buf(),
            ready: Arc::new(AtomicBool::new(true)),
            agent_state: Some(ocore::agent_state::AppState::new(wtx, etx, 4)),
            ops: Arc::new(crate::operations::OpRegistry::new()),
            settings_dir: db.parent().unwrap().to_path_buf(),
            server_token: Some("master-token".into()),
            server_port: 7340,
            factory_gate: Arc::new(ocore::factory_sched::FactoryGate::new()),
            media_sources: Arc::new(tokio::sync::RwLock::new(
                crate::gbrain::MediaSourcesCache::default(),
            )),
            update_check_enabled: true,
        })
    }

    /// seed：workspace＋兩名員工（emp-mine＝principal-user 名下；emp-other＝他人名下）。
    fn seed_employees(db: &std::path::Path) {
        use ocore::domain::{
            BrainRef, Employee, EmployeeState, SqliteStore, Store, Workspace, WorkspaceStatus,
        };
        let store = SqliteStore::open(db).unwrap();
        store
            .put_workspace(&Workspace {
                id: "ws".into(),
                name: "W".into(),
                description: None,
                status: WorkspaceStatus::Active,
                created_at: "t".into(),
            })
            .unwrap();
        for (id, owner) in [("emp-mine", Some("principal-user")), ("emp-other", Some("principal-other"))] {
            store
                .put_employee(&Employee {
                    id: id.into(),
                    workspace_id: "ws".into(),
                    name: id.into(),
                    brain: BrainRef { brain_id: "b1".into() },
                    role: None,
                    description: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    archived: false,
                    tools: None,
                    created_at: "t".into(),
                    owner_principal: owner.map(str::to_string),
                })
                .unwrap();
        }
    }

    /// F1：員工級 delta 流直推——名下 user 訂閱即達（非輪詢）；連線隨發佈端關閉而收尾。
    #[tokio::test]
    async fn employee_stream_pushes_deltas_to_owned_user() {
        let dir = std::env::temp_dir().join(format!("operoid-sse-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.db");
        seed_employees(&db);
        let state = stream_test_state(&db);
        let app = sse_routes().with_state(state.clone());

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/employees/emp-mine/stream")
            .header("authorization", "Bearer usr")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "text/event-stream",
            "SSE content-type"
        );

        // 訂閱已建立（headers 回傳前 handler 已 subscribe）→ 發佈即達。
        let sender = state
            .agent_state
            .as_ref()
            .unwrap()
            .turn_delta_sender();
        sender
            .send(ocore::agent_state::TurnDelta::TextDelta {
                employee_id: "emp-mine".into(),
                task_id: "task-1".into(),
                step: 1,
                text: "主因是接點氧化".into(),
            })
            .unwrap();
        // v0.5.8：過程幀（ToolCallStart）同樣經 `event: delta` 直推。
        sender
            .send(ocore::agent_state::TurnDelta::ToolCallStart {
                employee_id: "emp-mine".into(),
                task_id: "task-1".into(),
                step: 1,
                name: "gbrain_think".into(),
                args: "{\"query\":\"蝕刻良率\"}".into(),
            })
            .unwrap();
        drop(sender);
        // 丟掉全部 ServerState（含內含的發佈端）→ 匯流排 Closed → 流自然收尾。
        drop(app);
        drop(state);
        let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(s.contains("event: delta"), "幀名：{s}");
        assert!(s.contains("\"kind\":\"text_delta\""), "payload：{s}");
        assert!(s.contains("主因是接點氧化"), "文字：{s}");
        assert!(s.contains("\"kind\":\"tool_call_start\""), "過程幀 payload：{s}");
        assert!(s.contains("gbrain_think"), "工具名：{s}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F1：user 訂閱**非名下**員工 → 403（不洩漏存在性）；未認證 → 401。
    #[tokio::test]
    async fn employee_stream_scopes_to_owned_employees() {
        let dir = std::env::temp_dir().join(format!("operoid-sse-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.db");
        seed_employees(&db);
        let state = stream_test_state(&db);
        let app = sse_routes().with_state(state.clone());

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/employees/emp-other/stream")
            .header("authorization", "Bearer usr")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "他人員工不得訂閱");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/employees/emp-mine/stream")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
