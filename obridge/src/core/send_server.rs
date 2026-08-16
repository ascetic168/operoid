//! Send endpoint（出氣方向）——Operoid `event_outbound_url` 指向這裡。
//!
//! `POST /send`（Bearer）→ 解 [`SendPayload`] → 依 `source` 查 [`Registry`] 分派給通道。
//! 鏡像 Operoid `ingress_server` 的 check_auth 模式（127.0.0.1 only）。

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use ocontract::SendPayload;

use super::channel::Registry;

/// 啟動 send endpoint（綁 127.0.0.1，常駐 serve）。
///
/// `registry` 為 RwLock 共享——熱重載時 run() 會整組替換註冊表內容，這裡每次請求重讀。
pub async fn serve(
    port: u16,
    secret: String,
    registry: std::sync::Arc<std::sync::RwLock<Registry>>,
) {
    let app = Router::new()
        .route("/send", post(send_handler))
        .with_state((secret, registry));
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[obridge] send endpoint 綁定 127.0.0.1:{port} 失敗：{e}");
            return;
        }
    };
    eprintln!("[obridge] send endpoint 就緒：127.0.0.1:{port}/send");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[obridge] send endpoint 結束：{e}");
    }
}

async fn send_handler(
    State((secret, registry)): State<(
        String,
        std::sync::Arc<std::sync::RwLock<Registry>>,
    )>,
    headers: HeaderMap,
    Json(p): Json<SendPayload>,
) -> Response {
    if !check_auth(&headers, &secret) {
        return (StatusCode::UNAUTHORIZED, "bad credentials").into_response();
    }
    let ch = registry
        .read()
        .ok()
        .and_then(|r| r.get(&p.source));
    let Some(ch) = ch else {
        return (
            StatusCode::NOT_FOUND,
            format!("unknown source: {}", p.source),
        )
            .into_response();
    };
    match ch.send(&p.to, &p.employee_id, &p.text).await {
        Ok(()) => (StatusCode::OK, "sent").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("send failed: {e}"),
        )
            .into_response(),
    }
}

/// Bearer 認證（鏡像 Operoid ingress_server::check_auth）。
pub(crate) fn check_auth(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| !expected.is_empty() && t.trim() == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::sync::Arc;

    use crate::core::channel::{Channel, Registry};

    struct EchoChannel {
        sent: std::sync::Mutex<Vec<(String, String, String)>>,
    }

    #[async_trait::async_trait]
    impl Channel for EchoChannel {
        fn source(&self) -> &str {
            "email"
        }
        async fn run_inbound(&self, _tx: tokio::sync::mpsc::Sender<ocontract::InboundEvent>) {}
        async fn send(&self, to: &str, employee_id: &str, text: &str) -> anyhow::Result<()> {
            self.sent
                .lock()
                .unwrap()
                .push((to.into(), employee_id.into(), text.into()));
            Ok(())
        }
    }

    #[test]
    fn auth_checks_bearer() {
        let mut h = HeaderMap::new();
        assert!(!check_auth(&h, "s"));
        h.insert(axum::http::header::AUTHORIZATION, HeaderValue::from_static("Bearer s"));
        assert!(check_auth(&h, "s"));
        assert!(!check_auth(&h, "other"));
        h.insert(axum::http::header::AUTHORIZATION, HeaderValue::from_static("Bearer x"));
        assert!(!check_auth(&h, "s"));
    }

    /// e2e（loopback）：POST /send → 分派正確／unknown source 404／壞認證 401。
    #[tokio::test]
    async fn send_endpoint_dispatches_by_source() {
        let echo = Arc::new(EchoChannel {
            sent: std::sync::Mutex::new(Vec::new()),
        });
        let mut reg = Registry::new();
        reg.register(echo.clone()).unwrap();
        tokio::spawn(serve(
            41041,
            "s3cret".into(),
            Arc::new(std::sync::RwLock::new(reg)),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let client = reqwest::Client::new();
        let url = "http://127.0.0.1:41041/send";
        // 1) 有效認證＋已知 source → 200 sent、通道收到三欄。
        let r = client
            .post(url)
            .bearer_auth("s3cret")
            .json(&SendPayload {
                source: "email".into(),
                to: "email:msg:%3Ca%40b%3E".into(),
                employee_id: "Steve-TW".into(),
                text: "回覆".into(),
            })
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(echo.sent.lock().unwrap().len(), 1);
        // 2) unknown source → 404。
        let r = client
            .post(url)
            .bearer_auth("s3cret")
            .json(&SendPayload {
                source: "nope".into(),
                to: "x".into(),
                employee_id: "e".into(),
                text: "t".into(),
            })
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404);
        // 3) 壞認證 → 401。
        let r = client
            .post(url)
            .json(&SendPayload {
                source: "email".into(),
                to: "x".into(),
                employee_id: "e".into(),
                text: "t".into(),
            })
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
        assert_eq!(echo.sent.lock().unwrap().len(), 1, "401 不應分派");
    }
}
