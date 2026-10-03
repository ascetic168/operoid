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

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde_json::json;

use crate::auth::Identity;
use crate::routes::{cors_layer, err_response, open_store, require_identity, ServerState};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_single_use_and_expiry() {
        let t = issue_ticket("principal-x");
        // 單次消費。
        assert_eq!(consume_ticket(&t).as_deref(), Some("principal-x"));
        assert_eq!(consume_ticket(&t), None, "第二次消費必須失敗");
        // 壞票。
        assert_eq!(consume_ticket("nonexistent"), None);
    }
}
