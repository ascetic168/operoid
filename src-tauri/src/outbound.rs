//! Outbound（E7 外發）——把員工對外部事件的回覆 POST 給 bridge 的 send endpoint。
//!
//! 設計（見 `docs/Operoid-設計-統一事件ingress契約.md`）：outbound 走 **回覆式自動觸發**——
//! 源自外部事件（`Task.external_reply_to` 有值）的對話回合產生 Out message 後，runtime 自動
//! 呼叫 [`send_reply`]。**免人類核可**（使用者決策 2026-08-15：outbound 是通用通道——Email／IM／
//! 未來 ERP/MES——逐一核可會使自動化失去效益）。
//!
//! 職責邊界：Operoid 只負責把 `{source, reply_to, employee_id, text}` POST 給 bridge；
//! **通道設定（SMTP／Slack token／ERP endpoint）全在 bridge 端**，bridge 依 `reply_to`（自己
//! 定義的不透明錨點）定位回覆目標（Email thread／IM channel+thread），並以
//! `source`/`employee_id` 選擇寄件身分。多員工並發回覆不會交叉——每個進站事件各自帶
//! `reply_to`、各自產生獨立 Inbox Task，回覆帶回自己 Task 的錨點。
//!
//! 啟用條件：`AppConfig.event_outbound_url` 有設（opt-in）。未設 → [`send_reply`] 回
//! `Skipped`（回覆僅留在 Operoid 對話歷史）。

use serde_json::{json, Value};
use tauri::{AppHandle, Runtime};

use crate::config::app_config;

/// 外發組態（`AppConfig` 的 outbound 切片，便宜 Clone，沿 runtime 傳遞鏈下傳）。
///
/// `url` 為 None → 外發停用（[`send_reply`] 回 `Skipped`）。
#[derive(Debug, Clone, Default)]
pub struct OutboundConfig {
    pub url: Option<String>,
    pub secret: Option<String>,
}

impl OutboundConfig {
    /// 從 App 設定載入（呼叫端已 load AppConfig 者可直接組；此處供 scheduler 等便利使用）。
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Self {
        match app_config::load(app) {
            Ok(c) => Self {
                url: c.event_outbound_url,
                secret: c.event_outbound_secret,
            },
            Err(_) => Self::default(),
        }
    }
}

/// 外發結果：成功、未啟用（skip）、失敗（payload 已組好但 HTTP 失敗）。
#[derive(Debug, PartialEq, Eq)]
pub enum SendOutcome {
    Sent,
    Skipped,
    Failed(String),
}

/// 組 outbound payload（bridge send endpoint 的 JSON body）。
///
/// - `source`：來源類型標籤（與進站事件相同，如 "email"／"slack"）——bridge 選通道。
/// - `reply_to`：進站事件附的不透明錨點——bridge 定位回覆目標。
/// - `employee_id`：回覆的員工——bridge 選寄件身分（如不同員工不同寄件地址）。
/// - `text`：Out message 全文。
pub fn build_outbound_payload(
    source: &str,
    reply_to: &str,
    employee_id: &str,
    text: &str,
) -> Value {
    json!({
        "source": source,
        "reply_to": reply_to,
        "employee_id": employee_id,
        "text": text,
    })
}

/// 把一則回覆外發給 bridge。`cfg.url` 未設 → `Skipped`（不視為錯誤）。
///
/// `event_outbound_secret` 有設才帶 `Authorization: Bearer`。timeout 10s——外發是對話回合的
/// 尾端附加動作，不該久等。
pub async fn send_reply(
    cfg: &OutboundConfig,
    source: &str,
    reply_to: &str,
    employee_id: &str,
    text: &str,
) -> SendOutcome {
    let Some(url) = &cfg.url else {
        return SendOutcome::Skipped;
    };
    let payload = build_outbound_payload(source, reply_to, employee_id, text);
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => return SendOutcome::Failed(format!("建 client 失敗：{e}")),
    };
    let mut req = client.post(url).json(&payload);
    if let Some(secret) = &cfg.secret {
        req = req.bearer_auth(secret);
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => SendOutcome::Sent,
        Ok(resp) => SendOutcome::Failed(format!("bridge 回 HTTP {}", resp.status())),
        Err(e) => SendOutcome::Failed(format!("連線失敗：{e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// payload 四欄各就位（bridge 契約的最小保證）。
    #[test]
    fn outbound_payload_carries_routing_fields() {
        let v = build_outbound_payload("email", "email:msg-42", "Steve-TW", "收到，已追蹤。");
        assert_eq!(v["source"], "email");
        assert_eq!(v["reply_to"], "email:msg-42");
        assert_eq!(v["employee_id"], "Steve-TW");
        assert_eq!(v["text"], "收到，已追蹤。");
    }
}
