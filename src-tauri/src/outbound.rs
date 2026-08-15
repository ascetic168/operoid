//! Outbound（E7/E12 外發）——把員工的外發訊息 POST 給 bridge 的 send endpoint。
//!
//! v2（2026-08-16，E12）：外發**統一走 [`SendTool`]**（tool-choice 編排）——所有外發都是
//! 員工的行動（Principle 10：送什麼、送誰由員工決定；Tool 不決策，只執行）。v1 的
//! 「回覆式自動觸發」（寫完 Out message 由 runtime 自動外發）已移除；唯一例外是
//! **無 Reasoner 的退化路徑**（無 Reasoner 即無「員工」可言，runtime 代發是既有退化語意，
//! 見 `runtime.rs` 的 fallback）。
//!
//! 目標表達（薄外殼決策）：payload 的 `to` 是**自由字串**——缺省回退進站事件的 `reply_to`
//! 不透明錨點（回到原 thread）；員工也可明示新目標（如 email 地址），由 bridge 解讀路由。
//! Operoid 不建模收件人。
//!
//! 職責邊界：Operoid 只負責把 `{source, to, employee_id, text}` POST 給 bridge；
//! **通道設定（SMTP／Slack token／ERP endpoint）全在 bridge 端**，bridge 依 `to` 定位
//! 送達目標，並以 `source`/`employee_id` 選擇寄件身分。
//!
//! 啟用條件：`AppConfig.event_outbound_url` 有設（opt-in）。未設 → [`SendTool`] 回明確的
//! 「外發未啟用」說明給員工（進對話上下文，員工可改只留內部回覆），不靜默。

use serde_json::{json, Value};
use tauri::{AppHandle, Runtime};

use crate::config::app_config;
use crate::domain::tools::{Tool, ToolCtx, ToolFuture, ToolInput, ToolOutput, ToolSpec};

/// 外發組態（`AppConfig` 的 outbound 切片，便宜 Clone，沿 runtime 傳遞鏈下傳）。
///
/// `url` 為 None → 外發停用（[`send_external`] 回 `Skipped`）。
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
/// - `to`：送達目標——進站事件附的不透明錨點（回原 thread），或員工明示的新目標（自由字串，
///   bridge 解讀路由）。
/// - `employee_id`：發送的員工——bridge 選寄件身分（如不同員工不同寄件地址）。
/// - `text`：外發全文。
pub fn build_outbound_payload(source: &str, to: &str, employee_id: &str, text: &str) -> Value {
    json!({
        "source": source,
        "to": to,
        "employee_id": employee_id,
        "text": text,
    })
}

/// 把一則外發訊息 POST 給 bridge。`cfg.url` 未設 → `Skipped`（不視為錯誤）。
///
/// `event_outbound_secret` 有設才帶 `Authorization: Bearer`。timeout 10s——外發不該久等。
pub async fn send_external(
    cfg: &OutboundConfig,
    source: &str,
    to: &str,
    employee_id: &str,
    text: &str,
) -> SendOutcome {
    let Some(url) = &cfg.url else {
        return SendOutcome::Skipped;
    };
    let payload = build_outbound_payload(source, to, employee_id, text);
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

// ───────────────── SendTool（E12 tool-choice）─────────────────

/// 外發 Tool（`send-external-message`）：把員工的訊息送給 bridge。
///
/// 每個回合建一個 instance——capture 該回合的外發組態、來源（`Task.external_source`）、
/// 回覆錨點（`Task.external_reply_to`）與員工 id。Tool 只執行：`to` 缺省回退錨點（回到
/// 員工被喚醒的那個 thread）；無錨點且無明示 `to`、或無來源（非外部事件且未明示 `source`，
/// 如自主循環的主動通知）→ 回明確錯誤訊息給員工。
///
/// 輸入（`ToolInput.params`）：`{"to"?: string, "text": string, "source"?: string}`。
/// `source`（通道標籤，如 "email"）缺省用進站事件的來源；**主動發送**（自主循環等無進站
/// 來源的情境）必須明示——bridge 依此選通道（薄外殼：Operoid 不建模通道）。
/// 輸出：送達結果的**人類可讀描述**（成功／未啟用／失敗原因）——進對話上下文讓員工知道
/// 外發是否可用；`meta` 帶機讀結果（`{"outcome": "sent"|"skipped"|"failed"|"error", ...}`），
/// 供 Runtime 記 `outbound_sent`/`outbound_failed` 事件。失敗不回 `Err`（不炸對話循環）。
pub struct SendTool {
    spec: ToolSpec,
    cfg: OutboundConfig,
    /// 進站事件的來源標籤（`Task.external_source`）；None＝非外部事件（需明示 `source`）。
    source: Option<String>,
    /// 進站事件的不透明錨點（`Task.external_reply_to`）；`to` 缺省時回退到此。
    reply_to: Option<String>,
    employee_id: String,
}

impl SendTool {
    pub fn new(
        cfg: OutboundConfig,
        source: Option<String>,
        reply_to: Option<String>,
        employee_id: impl Into<String>,
    ) -> Self {
        Self {
            spec: ToolSpec {
                id: "send-external-message".into(),
                description:
                    "Send a message to an external channel (email/IM) via the bridge. Params: {to?: string (default: reply to the thread that woke you; required for proactive sends), text: string, source?: string (channel tag like 'email'; default: source of the inbound event; required for proactive sends)}."
                        .into(),
            },
            cfg,
            source,
            reply_to,
            employee_id: employee_id.into(),
        }
    }

    /// 解析送達目標：明示 `to` > 回覆錨點。兩者皆無 → 明確錯誤訊息（給員工看）。
    fn resolve_to(&self, to: Option<&str>) -> Result<String, String> {
        if let Some(t) = to.filter(|s| !s.trim().is_empty()) {
            return Ok(t.trim().to_string());
        }
        if let Some(anchor) = &self.reply_to {
            return Ok(anchor.clone());
        }
        Err("沒有可用的送達目標：本訊息非源自外部事件（無回覆錨點），且未提供 to。".into())
    }
}

impl Tool for SendTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn invoke<'a>(&'a self, input: ToolInput, _ctx: &'a ToolCtx) -> ToolFuture<'a> {
        Box::pin(async move {
            let params = input.params.unwrap_or_default();
            let text = params
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| input.query.clone());
            let to_opt = params.get("to").and_then(|v| v.as_str());
            // 來源解析：明示 source > 進站事件來源。兩者皆無（主動發送未指明通道）→ 回報員工。
            let source = match params
                .get("source")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| self.source.clone())
            {
                Some(s) => s,
                None => {
                    return Ok(ToolOutput {
                        text: "外發失敗：沒有可用的來源通道——主動發送請在 params 提供 source（如 \"email\"）。"
                            .into(),
                        meta: json!({"outcome": "error", "reason": "no source"}),
                    })
                }
            };
            let to = match self.resolve_to(to_opt) {
                Ok(t) => t,
                Err(e) => {
                    return Ok(ToolOutput {
                        text: format!("外發失敗：{e}"),
                        meta: json!({"outcome": "error", "reason": e}),
                    })
                }
            };
            let outcome = send_external(&self.cfg, &source, &to, &self.employee_id, &text).await;
            let (desc, meta) = match &outcome {
                SendOutcome::Sent => (
                    format!("已外發給 {to}。"),
                    json!({"outcome": "sent", "to": to}),
                ),
                SendOutcome::Skipped => (
                    "外發未啟用（event_outbound_url 未設定）——訊息僅留在 Operoid 內部對話歷史。".into(),
                    json!({"outcome": "skipped", "to": to}),
                ),
                SendOutcome::Failed(e) => (
                    format!("外發失敗（{to}）：{e}"),
                    json!({"outcome": "failed", "to": to, "error": e}),
                ),
            };
            Ok(ToolOutput {
                text: desc,
                meta,
            })
        })
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
        assert_eq!(v["to"], "email:msg-42");
        assert_eq!(v["employee_id"], "Steve-TW");
        assert_eq!(v["text"], "收到，已追蹤。");
    }

    fn ctx() -> ToolCtx {
        ToolCtx {
            gbrain_exe: "gbrain".into(),
            gbrain_home: None,
            chat_model: None,
        }
    }

    fn params(p: &[(&str, Value)]) -> ToolInput {
        let mut m = serde_json::Map::new();
        for (k, v) in p {
            m.insert((*k).into(), v.clone());
        }
        ToolInput {
            query: String::new(),
            anchor: None,
            params: Some(m),
        }
    }

    /// `to` 缺省 → 回退進站錨點；未啟用（url 未設）→ 明確回報員工（非靜默）。
    #[tokio::test]
    async fn send_tool_defaults_to_anchor_and_reports_disabled() {
        let tool = SendTool::new(
            OutboundConfig::default(),
            Some("email".into()),
            Some("email:msg-9".into()),
            "Steve-TW",
        );
        let out = tool
            .invoke(
                params(&[("text", json!("收到"))]),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.text.contains("外發未啟用"));
        assert_eq!(out.meta["outcome"], "skipped");
        assert_eq!(out.meta["to"], "email:msg-9");
    }

    /// 明示 `to`（新目標）優先於錨點。
    #[tokio::test]
    async fn send_tool_explicit_to_overrides_anchor() {
        let tool = SendTool::new(
            OutboundConfig::default(),
            Some("email".into()),
            Some("email:msg-9".into()),
            "Steve-TW",
        );
        let out = tool
            .invoke(
                params(&[("to", json!("boss@corp.com")), ("text", json!("hi"))]),
                &ctx(),
            )
            .await
            .unwrap();
        assert_eq!(out.meta["to"], "boss@corp.com");
    }

    /// 無來源（非外部事件＋未明示 source）→ 明確錯誤訊息給員工，不是 Tool 硬錯。
    #[tokio::test]
    async fn send_tool_without_external_source_reports_error() {
        let tool = SendTool::new(OutboundConfig::default(), None, None, "Steve-TW");
        let out = tool
            .invoke(
                params(&[("text", json!("hi"))]),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.text.contains("外發失敗"));
        assert_eq!(out.meta["outcome"], "error");
    }

    /// 主動發送（無進站來源）：明示 source + to 即可（未啟用 → 回報 skipped）。
    #[tokio::test]
    async fn send_tool_proactive_send_with_explicit_source() {
        let tool = SendTool::new(OutboundConfig::default(), None, None, "Steve-TW");
        let out = tool
            .invoke(
                params(&[
                    ("to", json!("boss@corp.com")),
                    ("text", json!("報告：承諾已完成")),
                    ("source", json!("email")),
                ]),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.text.contains("外發未啟用"));
        assert_eq!(out.meta["outcome"], "skipped");
        assert_eq!(out.meta["to"], "boss@corp.com");
    }
}
