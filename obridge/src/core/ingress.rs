//! Ingress client——把 InboundEvent POST 給 Operoid 的 `POST /event`。
//!
//! 冪等重推安全：Operoid 端以 `(source, external_ref)` 去重（契約 §七），故 obridge 端
//! 失敗重試（下一輪 poll 重投）不會造成重複喚醒。這裡不自行重試——交給通道的 poll 迴圈。

use ocontract::InboundEvent;

use super::config::OperoidCfg;

/// 投遞一則事件。2xx → Ok；其他 → Err（呼叫端記 log，事件隨下輪 poll 重投）。
pub async fn post_event(cfg: &OperoidCfg, ev: &InboundEvent) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client
        .post(&cfg.ingress_url)
        .bearer_auth(&cfg.ingress_secret)
        .json(ev)
        .send()
        .await?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        anyhow::bail!("Operoid ingress 回 HTTP {status}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocontract::EventKind;

    /// 管線（loopback）：事件 → stub ingress 收到完整 JSON（Bearer 帶上）。
    #[tokio::test]
    async fn posts_event_to_stub_ingress() {
        use axum::http::StatusCode;
        use std::sync::{Arc, Mutex};

        let received: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let rx = Arc::clone(&received);
        let app = axum::Router::new().route(
            "/event",
            axum::routing::post(
                move |headers: axum::http::HeaderMap, body: String| {
                    let rx = Arc::clone(&rx);
                    async move {
                        assert_eq!(
                            headers.get("authorization").unwrap(),
                            "Bearer s1",
                            "應帶 ingress_secret Bearer"
                        );
                        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
                        rx.lock().unwrap().push(v);
                        StatusCode::ACCEPTED
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let cfg = crate::core::config::OperoidCfg {
            ingress_url: format!("http://{addr}/event"),
            ingress_secret: "s1".into(),
        };
        let ev = ocontract::InboundEvent {
            kind: EventKind::ExternalMessage,
            source: "email".into(),
            brain_id: None,
            employee_id: Some("Steve-TW".into()),
            title: "測試信".into(),
            content: "From: x\n\n本體".into(),
            external_ref: Some("m1".into()),
            occurred_at: None,
            reply_to: Some("email:msg:%3Cm1%3E?to=x%40corp.com".into()),
            category: None,
        };
        post_event(&cfg, &ev).await.unwrap();
        let got = received.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0]["title"], "測試信");
        assert_eq!(got[0]["source"], "email");
        assert_eq!(got[0]["employee_id"], "Steve-TW");
    }
}
