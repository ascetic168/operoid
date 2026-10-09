//! LLM 呼叫層 — 重用 GBrain config 解析出的端點（chat_model + provider_base_urls + env key）。
//!
//! 採 OpenAI 相容 `/chat/completions`（groq/openai/ollama/deepseek/together/zhipu/... 皆相容）。
//! anthropic 的 schema 不同，未支援（resolve_endpoint 的 default_base_url 不含 anthropic）。
//!
//! 兩個入口：
//! - [`complete`]：system+user 單發取文字（PLAN／EVAL／文字 JSON 協議）。
//! - [`chat`]：多訊息歷史＋**原生 function calling**（對話回合 tool-loop 的 native 路徑）。
//!   `tools` 為空時請求體與 `complete` 完全相同。provider 不支援 tools（400 且訊息提及
//!   tool/function）→ 回 [`ToolsUnsupported`]，呼叫端可降級文字 JSON 協議。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::gbrain_config::{self, LlmEndpoint};

/// LLM 採樣參數（`AppConfig` 的 `llm_temperature`／`llm_max_tokens` 切片，由呼叫端
/// 組好傳入——ocore 不依賴 Tauri 側的 `AppConfig`；桌面殼以 `AppConfig::llm_sampling()`
/// 轉換）。P1a 前的簽名直接收 `&AppConfig`。
#[derive(Debug, Clone, Copy)]
pub struct SamplingParams {
    pub temperature: f64,
    pub max_tokens: u32,
}

// ───────────────── native tool calling（M1：對話回合 native 路徑）─────────────────

/// 提供給 LLM 的工具定義（OpenAI function calling 形狀的薄包裝）。
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema（`parameters` 欄位）。
    pub parameters: serde_json::Value,
}

/// 模型回傳的一次工具呼叫（`arguments` 已從 JSON 字串解析成 Value）。
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 訊息角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 多訊息歷史的一則訊息（native tool-loop 的歷史載體）。
///
/// - `System`／`User`：只有 `content`。
/// - `Assistant`：`content` 可為空字串，`tool_calls` 帶本步的工具呼叫。
/// - `Tool`：`content` 帶工具結果，`tool_call_id` 對應呼叫 id（協議要求逐條回）。
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    /// K5：附帶圖片（data URI 或 http URL；OpenAI content-parts 形）。
    /// 非空時 wire content 以 parts 陣列序列化；provider 拒收（400）時 `chat`
    /// 自動降級為純文字＋定位者指示重試一次。
    pub images: Vec<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: ChatRole::System, content: content.into(), tool_calls: Vec::new(), tool_call_id: None, images: Vec::new() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: ChatRole::User, content: content.into(), tool_calls: Vec::new(), tool_call_id: None, images: Vec::new() }
    }
    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self { role: ChatRole::Assistant, content: content.into(), tool_calls, tool_call_id: None, images: Vec::new() }
    }
    pub fn tool(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: ChatRole::Tool, content: content.into(), tool_calls: Vec::new(), tool_call_id: Some(call_id.into()), images: Vec::new() }
    }
    /// K5：帶圖片的工具結果（生成端讀圖；provider 拒收時呼叫層自動降級定位者）。
    pub fn tool_with_images(
        call_id: impl Into<String>,
        content: impl Into<String>,
        images: Vec<String>,
    ) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id.into()),
            images,
        }
    }
}

/// K5 定位者指示——provider 拒收多模態 content（或端點無 VLM）時，附加在
/// 工具結果之後：誠實降級為「定位者」（指向圖，不推測像素內容；C7/§K5）。
pub const LOCATOR_DIRECTIVE: &str =
    "
[圖片提示｜visual_unreadable] 本環境無法讀取圖片像素：若答案需要圖中內容，     請指出「文件／頁碼／圖號／圖檔路徑」引導使用者開圖確認，勿推測圖中數值。";

/// 一次 `chat` 的結果：文字（無工具呼叫時）＋工具呼叫＋finish 原因＋usage。
#[derive(Debug, Clone)]
pub struct LlmTurn {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

/// provider 不支援 function calling（400 且錯誤訊息提及 tool/function）。
/// 呼叫端以 `e.downcast_ref::<ToolsUnsupported>()` 判別 → 降級文字 JSON 協議。
#[derive(Debug)]
pub struct ToolsUnsupported;

impl std::fmt::Display for ToolsUnsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "provider 不支援原生工具呼叫（tools unsupported）")
    }
}

impl std::error::Error for ToolsUnsupported {}

// ───────────────── wire types ─────────────────

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    temperature: f64,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireTool<'a>>>,
}

/// 助理訊息上的 tool_calls 之 wire 形（`arguments` 須為 JSON **字串**）。
#[derive(Serialize)]
struct WireToolCallRef<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    typ: &'static str,
    function: WireToolCallFn<'a>,
}
#[derive(Serialize)]
struct WireToolCallFn<'a> {
    name: &'a str,
    arguments: String,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<WireContent<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<WireToolCallRef<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
}

/// K5：訊息內容的兩種 wire 形——純文字（回溯相容）或 OpenAI content-parts
/// （文字＋圖片；`untagged` 讓兩者都序列化為正確形狀）。
#[derive(Serialize)]
#[serde(untagged)]
enum WireContent<'a> {
    Text(&'a str),
    Parts(Vec<WirePart<'a>>),
}

#[derive(Serialize)]
struct WirePart<'a> {
    #[serde(rename = "type")]
    typ: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<WireImageUrl<'a>>,
}

#[derive(Serialize)]
struct WireImageUrl<'a> {
    url: &'a str,
}

#[derive(Clone, Serialize)]
struct WireTool<'a> {
    #[serde(rename = "type")]
    typ: &'static str,
    function: WireToolFn<'a>,
}
#[derive(Clone, Serialize)]
struct WireToolFn<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    /// W2：OpenAI 相容回應的 usage 區塊（成本記錄 lite；provider 沒給 → None）。
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<RespToolCall>>,
}

#[derive(Deserialize)]
struct RespToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<RespFn>,
}

/// `arguments` 在 wire 上是 JSON 字串（有的 provider 直接給 `{}` 物件——兩者皆容）。
#[derive(Deserialize)]
struct RespFn {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "flexible_arguments")]
    arguments: Option<serde_json::Value>,
}

/// `arguments` 容忍三種形：JSON 字串（規範形）、JSON 物件（少數 provider）、缺省。
fn flexible_arguments<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => Some(serde_json::json!({})),
        Some(serde_json::Value::String(s)) => Some(if s.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
        }),
        Some(other) => Some(other),
    })
}

/// token 用量（prompt／completion／total；個別欄位可能缺）。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
}

/// 一次 chat completion 的結果：內容＋token 用量（W2）。
#[derive(Debug, Clone)]
pub struct ChatCompletion {
    pub content: String,
    pub usage: Option<Usage>,
}

/// 從環境變數取該 provider 的 API key（ollama 等回 None → 不帶 Authorization）。
fn env_key_for(endpoint: &LlmEndpoint) -> Option<String> {
    gbrain_config::env_key(&endpoint.provider)
        .and_then(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// 暫態失敗（網路錯誤／429／5xx）的重試退避：指數 5s·2^attempt（5/10/20s）。
/// 測試建置改 10/20/40ms——重試路徑可在毫秒級驗證（不可用 `start_paused`：暫停時鐘的
/// 自動推進會與 reqwest 的 120s 請求逾時計時器互相干擾，把正常回應變成網路錯誤）。
#[cfg(not(test))]
fn retry_backoff(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_secs((5u64 << attempt).min(20))
}
#[cfg(test)]
fn retry_backoff(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis((10u64 << attempt).min(40))
}

/// 送出請求並處理 HTTP 層（暫態重試、4xx 分類）；成功回傳解析後的 JSON。
/// `sent_tools` 只影響 400 的錯誤分類（provider 不支援 tools → [`ToolsUnsupported`]）。
async fn post_chat(
    endpoint: &LlmEndpoint,
    body: &ChatRequest<'_>,
    sent_tools: bool,
) -> Result<serde_json::Value> {
    let url = format!(
        "{}/chat/completions",
        endpoint.base_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let key = env_key_for(endpoint);
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..3u32 {
        let backoff = retry_backoff(attempt);
        let mut req = client.post(&url).json(body);
        if let Some(k) = &key {
            req = req.header("Authorization", format!("Bearer {k}"));
        }
        let resp = match req.send().await.context("LLM 請求失敗") {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(backoff).await;
                continue;
            }
        };
        let status = resp.status();
        if status.is_success() {
            return resp.json().await.context("LLM 回應非預期 JSON");
        }
        let retryable = status.as_u16() == 429 || status.is_server_error();
        if retryable && attempt < 2 {
            eprintln!(
                "[llm] {status} 暫態失敗，{backoff:?} 後重試（attempt {}）",
                attempt + 1
            );
            tokio::time::sleep(backoff).await;
            continue;
        }
        let text = resp.text().await.unwrap_or_default();
        // 400 且訊息提及 tool/function → 判定 provider 不支援原生工具呼叫
        // （ollama 舊模型、少數相容端點）。啟發式判定：僅在真的帶了 tools 時成立，
        // 讓呼叫端降級文字 JSON 協議（見 ToolsUnsupported 文檔）。
        if sent_tools
            && status.as_u16() == 400
            && (text.to_ascii_lowercase().contains("tool")
                || text.to_ascii_lowercase().contains("function"))
        {
            return Err(anyhow::Error::new(ToolsUnsupported).context(format!(
                "LLM 回應 400（tools）：{text}"
            )));
        }
        return Err(anyhow!("LLM 回應非 2xx（{status}）：{text}"));
    }
    Err(last_err.unwrap_or_else(|| anyhow!("LLM 重試耗盡")))
}

/// 呼叫一次 chat completion，回傳內容＋token 用量（[`ChatCompletion`]）。
///
/// W2 暫態失敗重試：**網路錯誤／429／5xx**——指數退避 5s·2^attempt（5/10/20s），
/// 共 3 次嘗試；4xx（如 401／400）屬永久失敗，立即回錯不重試。
pub async fn complete(
    endpoint: &LlmEndpoint,
    sampling: &SamplingParams,
    system: &str,
    user: &str,
) -> Result<ChatCompletion> {
    let body = ChatRequest {
        model: &endpoint.model,
        messages: vec![
            WireMessage { role: "system", content: Some(WireContent::Text(system)), tool_calls: None, tool_call_id: None },
            WireMessage { role: "user", content: Some(WireContent::Text(user)), tool_calls: None, tool_call_id: None },
        ],
        temperature: sampling.temperature,
        max_tokens: sampling.max_tokens,
        tools: None,
    };
    let v = post_chat(endpoint, &body, false).await?;
    let chat: ChatResponse =
        serde_json::from_value(v).map_err(|e| anyhow!("LLM 回應解析失敗：{e}"))?;
    let content = chat
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| anyhow!("LLM 回應沒有 content"))?;
    Ok(ChatCompletion { content, usage: chat.usage })
}

/// 多訊息歷史＋原生 function calling 的一步呼叫（對話回合 native 路徑）。
///
/// `tools` 為空 → 請求體不含 `tools` 欄位（與 [`complete`] 等價的單發語意）。
/// 模型回 tool_calls 時 `content` 常為 None／空；呼叫端以 tool_calls 優先。
/// 單一訊息 → wire 形。帶圖時 content 以 parts 陣列呈現（text＋image_url）。
fn build_wire_message(m: &ChatMessage) -> WireMessage<'_> {
    let role = match m.role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    };
    let content = if !m.images.is_empty() {
        let mut parts: Vec<WirePart<'_>> = Vec::with_capacity(m.images.len() + 1);
        if !m.content.is_empty() {
            parts.push(WirePart {
                typ: "text",
                text: Some(m.content.as_str()),
                image_url: None,
            });
        }
        for uri in &m.images {
            parts.push(WirePart {
                typ: "image_url",
                text: None,
                image_url: Some(WireImageUrl { url: uri }),
            });
        }
        Some(WireContent::Parts(parts))
    } else if m.content.is_empty() && m.role == ChatRole::Assistant {
        None
    } else {
        Some(WireContent::Text(m.content.as_str()))
    };
    WireMessage {
        role,
        content,
        tool_calls: if m.tool_calls.is_empty() {
            None
        } else {
            Some(
                m.tool_calls
                    .iter()
                    .map(|tc| WireToolCallRef {
                        id: tc.id.as_str(),
                        typ: "function",
                        function: WireToolCallFn {
                            name: tc.name.as_str(),
                            arguments: tc.arguments.to_string(),
                        },
                    })
                    .collect(),
            )
        },
        tool_call_id: m.tool_call_id.as_deref(),
    }
}

pub async fn chat(
    endpoint: &LlmEndpoint,
    sampling: &SamplingParams,
    messages: &[ChatMessage],
    tools: &[ToolDef],
) -> Result<LlmTurn> {
    let has_images = messages.iter().any(|m| !m.images.is_empty());
    let wire_msgs: Vec<WireMessage<'_>> = messages.iter().map(build_wire_message).collect();
    let wire_tools = if tools.is_empty() {
        None
    } else {
        Some(
            tools
                .iter()
                .map(|t| WireTool {
                    typ: "function",
                    function: WireToolFn {
                        name: t.name.as_str(),
                        description: t.description.as_str(),
                        parameters: &t.parameters,
                    },
                })
                .collect(),
        )
    };
    let body = ChatRequest {
        model: &endpoint.model,
        messages: wire_msgs,
        temperature: sampling.temperature,
        max_tokens: sampling.max_tokens,
        tools: wire_tools.clone(),
    };
    let v = match post_chat(endpoint, &body, !tools.is_empty()).await {
        Ok(v) => v,
        // K5 定位者降級：provider 拒收多模態 content（400）→ 剝除圖片＋注入
        // 定位者指示，重試一次。管線任何階段不因「無 VLM／不支援讀圖」失效。
        Err(e) if has_images && e.to_string().contains("非 2xx（400") => {
            eprintln!(
                "[llm] provider 拒收多模態 content（400）——降級為文字＋定位者指示重試"
            );
            let stripped: Vec<ChatMessage> = messages
                .iter()
                .map(|m| {
                    let mut m = m.clone();
                    if !m.images.is_empty() {
                        m.images.clear();
                        if !m.content.ends_with(LOCATOR_DIRECTIVE) {
                            m.content.push_str(LOCATOR_DIRECTIVE);
                        }
                    }
                    m
                })
                .collect();
            let wire_msgs: Vec<WireMessage<'_>> =
                stripped.iter().map(build_wire_message).collect();
            let body = ChatRequest {
                model: &endpoint.model,
                messages: wire_msgs,
                temperature: sampling.temperature,
                max_tokens: sampling.max_tokens,
                tools: wire_tools.clone(),
            };
            post_chat(endpoint, &body, !tools.is_empty()).await?
        }
        Err(e) => return Err(e),
    };
    let chat: ChatResponse =
        serde_json::from_value(v).map_err(|e| anyhow!("LLM 回應解析失敗：{e}"))?;
    let choice = chat
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("LLM 回應沒有 choices"))?;
    let tool_calls = choice
        .message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, tc)| {
            let f = tc.function.unwrap_or(RespFn { name: None, arguments: None });
            Ok(ToolCall {
                id: tc.id.unwrap_or_else(|| format!("call_{i}")),
                name: f.name.ok_or_else(|| anyhow!("tool_call 缺 function.name"))?,
                arguments: f.arguments.unwrap_or(serde_json::json!({})),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let content = choice.message.content.filter(|c| !c.trim().is_empty());
    Ok(LlmTurn {
        content,
        tool_calls,
        finish_reason: choice.finish_reason,
        usage: chat.usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::routing::post;
    use axum::Json;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// 起一個本機 stub：依序回應指定狀態碼（200 時回帶 usage 的 OpenAI 相容 JSON）。
    /// 回 (base_url, 尚未消耗的狀態佇列——供斷言實際打了幾次)。
    async fn spawn_stub(statuses: Vec<u16>) -> (String, Arc<Mutex<VecDeque<u16>>>) {
        let q: Arc<Mutex<VecDeque<u16>>> =
            Arc::new(Mutex::new(statuses.into_iter().collect()));
        let q2 = Arc::clone(&q);
        let app = axum::Router::new().route(
            "/chat/completions",
            post(move || {
                let q = Arc::clone(&q2);
                async move {
                    let code = q.lock().unwrap().pop_front().unwrap_or(200);
                    if code == 200 {
                        (
                            axum::http::StatusCode::OK,
                            Json(serde_json::json!({
                                "choices": [{"message": {"content": "{\"done\": true}"}}],
                                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
                            })),
                        )
                    } else {
                        (
                            axum::http::StatusCode::from_u16(code).unwrap(),
                            Json(serde_json::json!({"error": "stub"})),
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), q)
    }

    /// native 測試用：記錄請求體、依腳本回應（content 或 tool_calls）。
    struct StubScript(Vec<serde_json::Value>);
    async fn spawn_script_stub(
        script: Vec<serde_json::Value>,
    ) -> (String, Arc<Mutex<VecDeque<serde_json::Value>>>) {
        let seen: Arc<Mutex<VecDeque<serde_json::Value>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let seen2 = Arc::clone(&seen);
        let script = Arc::new(Mutex::new(VecDeque::from(script)));
        let script2 = Arc::clone(&script);
        let app = axum::Router::new().route(
            "/chat/completions",
            post(
                move |State((seen, script)): State<(Arc<Mutex<VecDeque<serde_json::Value>>>, Arc<Mutex<VecDeque<serde_json::Value>>>)>,
                      Json(body): Json<serde_json::Value>| {
                    seen.lock().unwrap().push_back(body);
                    let next = script.lock().unwrap().pop_front().unwrap_or(
                        serde_json::json!({"choices": [{"message": {"content": "done"}, "finish_reason": "stop"}]}),
                    );
                    async move { (axum::http::StatusCode::OK, Json(next)) }
                },
            ),
        )
        .with_state((seen2, script2));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), seen)
    }

    /// K5：捕獲請求體＋依腳本回狀態碼（供 400 降級斷言）。
    async fn spawn_capture_stub(responses: Vec<u16>) -> (String, Arc<Mutex<Vec<serde_json::Value>>>) {
        let bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let q: Arc<Mutex<VecDeque<u16>>> =
            Arc::new(Mutex::new(responses.into_iter().collect()));
        let app = axum::Router::new().route(
            "/chat/completions",
            post(
                move |State((bodies, q)): State<(Arc<Mutex<Vec<serde_json::Value>>>, Arc<Mutex<VecDeque<u16>>>)>,
                      Json(body): Json<serde_json::Value>| {
                    bodies.lock().unwrap().push(body);
                    let code = q.lock().unwrap().pop_front().unwrap_or(200);
                    async move {
                        if code == 200 {
                            (
                                axum::http::StatusCode::OK,
                                Json(serde_json::json!({
                                    "choices": [{"message": {"content": "done"}, "finish_reason": "stop"}],
                                    "usage": {"prompt_tokens": 1, "completion_tokens": 1}
                                })),
                            )
                        } else {
                            (
                                axum::http::StatusCode::from_u16(code).unwrap(),
                                Json(serde_json::json!({"error": "bad content parts"})),
                            )
                        }
                    }
                },
            )
            .with_state((Arc::clone(&bodies), q)),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), bodies)
    }

    /// K5：帶圖訊息 → content-parts wire（text＋image_url）；純文字訊息保持字串形。
    #[tokio::test]
    async fn k5_images_serialize_as_content_parts() {
        let (base, bodies) = spawn_capture_stub(vec![200]).await;
        let ep = endpoint(&base);
        let msgs = vec![
            ChatMessage::system("s"),
            ChatMessage::tool_with_images(
                "call_1",
                "檢索結果含圖",
                vec!["data:image/jpeg;base64,QUJD".into()],
            ),
        ];
        let res = chat(&ep, &sampling(), &msgs, &[]).await.unwrap();
        assert_eq!(res.content.as_deref(), Some("done"));
        let b = bodies.lock().unwrap();
        assert_eq!(b.len(), 1);
        let content = &b[0]["messages"][1]["content"];
        assert!(content.is_array(), "帶圖訊息應以 parts 陣列呈現：{content}");
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "檢索結果含圖");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[1]["image_url"]["url"], "data:image/jpeg;base64,QUJD");
        assert!(b[0]["messages"][0]["content"].is_string(), "純文字訊息保持字串形");
    }

    /// K5 定位者降級：provider 400 拒收 content-parts → 剝圖＋注入
    /// visual_unreadable 指示重試一次；第二次請求為純文字。
    #[tokio::test]
    async fn k5_provider_400_falls_back_to_locator_text() {
        let (base, bodies) = spawn_capture_stub(vec![400, 200]).await;
        let ep = endpoint(&base);
        let msgs = vec![
            ChatMessage::system("s"),
            ChatMessage::tool_with_images(
                "call_1",
                "檢索結果含圖",
                vec!["data:image/jpeg;base64,QUJD".into()],
            ),
        ];
        let res = chat(&ep, &sampling(), &msgs, &[]).await.unwrap();
        assert_eq!(res.content.as_deref(), Some("done"));
        let b = bodies.lock().unwrap();
        assert_eq!(b.len(), 2, "首次 400 後應恰好重試一次");
        assert!(b[0]["messages"][1]["content"].is_array(), "首問應帶圖");
        let c2 = &b[1]["messages"][1]["content"];
        assert!(c2.is_string(), "降級後應為純文字：{c2}");
        assert!(
            c2.as_str().unwrap().contains("visual_unreadable"),
            "降級請求應含定位者指示：{c2}"
        );
        assert!(
            !c2.as_str().unwrap().contains("image_url"),
            "降級請求不得殘留圖片 parts"
        );
    }

    fn endpoint(base: &str) -> LlmEndpoint {
        LlmEndpoint {
            provider: "ollama".into(),
            model: "test-model".into(),
            base_url: base.into(),
            has_api_key: false,
        }
    }

    fn sampling() -> SamplingParams {
        SamplingParams { temperature: 0.2, max_tokens: 64 }
    }

    /// W2：5xx 屬暫態——指數退避後重試成功；usage 正確帶回（測試建置退避為毫秒級）。
    #[tokio::test]
    async fn retries_5xx_then_succeeds_with_usage() {
        let (base, q) = spawn_stub(vec![500, 500, 200]).await;
        let res = complete(&endpoint(&base), &sampling(), "s", "u").await.unwrap();
        assert_eq!(res.content, "{\"done\": true}");
        let u = res.usage.expect("usage 應帶回");
        assert_eq!(u.total_tokens, Some(15));
        assert_eq!(u.prompt_tokens, Some(10));
        assert_eq!(u.completion_tokens, Some(5));
        assert!(q.lock().unwrap().is_empty(), "三次嘗試全消耗");
    }

    /// W2：4xx 屬永久失敗——立即回錯、不重試（僅打一次）。
    #[tokio::test]
    async fn permanent_4xx_fails_fast() {
        let (base, q) = spawn_stub(vec![401, 200]).await;
        let err = complete(&endpoint(&base), &sampling(), "s", "u").await.unwrap_err();
        assert!(err.to_string().contains("401"), "{err}");
        assert_eq!(q.lock().unwrap().len(), 1, "預留的 200 未消耗＝沒有重試");
    }

    /// M1：chat() 解析原生 tool_calls（arguments 由 JSON 字串解析成 Value）。
    #[tokio::test]
    async fn chat_parses_tool_calls() {
        let (base, _seen) = spawn_script_stub(vec![serde_json::json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "gbrain_search", "arguments": "{\"query\":\"良率\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })])
        .await;
        let res = chat(&endpoint(&base), &sampling(), &[ChatMessage::user("hi")], &[])
            .await
            .unwrap();
        assert_eq!(res.tool_calls.len(), 1);
        let tc = &res.tool_calls[0];
        assert_eq!(tc.name, "gbrain_search");
        assert_eq!(tc.id, "call_1");
        assert_eq!(tc.arguments["query"], "良率");
        assert!(res.content.is_none());
        assert_eq!(res.finish_reason.as_deref(), Some("tool_calls"));
    }

    /// M1：多訊息歷史按序上線——user→assistant(tool_calls)→tool 逐條帶 tool_call_id。
    #[tokio::test]
    async fn chat_sends_multi_message_history_with_tool_results() {
        let (base, seen) = spawn_script_stub(vec![]).await;
        let msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("問"),
            ChatMessage::assistant(
                "",
                vec![ToolCall {
                    id: "call_9".into(),
                    name: "gbrain_think".into(),
                    arguments: serde_json::json!({"query": "x"}),
                }],
            ),
            ChatMessage::tool("call_9", "檢索結果"),
        ];
        let res = chat(&endpoint(&base), &sampling(), &msgs, &[]).await.unwrap();
        assert!(res.tool_calls.is_empty());
        let bodies = seen.lock().unwrap();
        assert_eq!(bodies.len(), 1);
        let sent = &bodies[0];
        let roles: Vec<&str> = sent["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "tool"]);
        assert_eq!(sent["messages"][2]["tool_calls"][0]["id"], "call_9");
        assert_eq!(
            sent["messages"][2]["tool_calls"][0]["function"]["arguments"],
            r#"{"query":"x"}"#.to_string()
        );
        assert_eq!(sent["messages"][3]["tool_call_id"], "call_9");
        assert_eq!(sent["messages"][3]["content"], "檢索結果");
    }

    /// M1：tools 非空時請求帶 `tools` 欄位（OpenAI function 形狀）；空時整個欄位不出現。
    #[tokio::test]
    async fn chat_tools_field_presence() {
        let (base, seen) = spawn_script_stub(vec![]).await;
        let tools = vec![ToolDef {
            name: "gbrain_search".into(),
            description: "檢索".into(),
            parameters: serde_json::json!({"type":"object","properties":{}}),
        }];
        chat(&endpoint(&base), &sampling(), &[ChatMessage::user("a")], &tools)
            .await
            .unwrap();
        chat(&endpoint(&base), &sampling(), &[ChatMessage::user("b")], &[])
            .await
            .unwrap();
        let bodies = seen.lock().unwrap();
        assert!(bodies[0]["tools"].is_object() || bodies[0]["tools"].is_array());
        assert_eq!(bodies[0]["tools"][0]["type"], "function");
        assert_eq!(bodies[0]["tools"][0]["function"]["name"], "gbrain_search");
        assert!(bodies[1].get("tools").is_none(), "空 tools 不應上線");
    }

    /// M1：400＋訊息提及 tools → ToolsUnsupported（呼叫端降級的訊號）。
    #[tokio::test]
    async fn chat_400_tools_classified_unsupported() {
        // 佇列：chat（帶 tools）吃第一個 400 → ToolsUnsupported；
        // complete（不帶 tools）吃第二個 400 → 一般錯誤；200 留作不重試的證據。
        let q: Arc<Mutex<VecDeque<u16>>> = Arc::new(Mutex::new(
            vec![400, 400, 200].into_iter().collect(),
        ));
        let q2 = Arc::clone(&q);
        let app = axum::Router::new().route(
            "/chat/completions",
            post(move || {
                let q = Arc::clone(&q2);
                async move {
                    let code = q.lock().unwrap().pop_front().unwrap_or(400);
                    if code == 400 {
                        (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"error": {"message": "model does not support tools"}})),
                        )
                    } else {
                        (
                            axum::http::StatusCode::OK,
                            Json(serde_json::json!({"choices": [{"message": {"content": "ok"}}]})),
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let base = format!("http://{addr}");
        let tools = vec![ToolDef {
            name: "t".into(),
            description: "d".into(),
            parameters: serde_json::json!({"type":"object"}),
        }];
        let err = chat(&endpoint(&base), &sampling(), &[ChatMessage::user("a")], &tools)
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<ToolsUnsupported>().is_some(),
            "應分類為 ToolsUnsupported：{err:#}"
        );
        // 不帶 tools 的同型 400（其他原因）→ 不分類為 ToolsUnsupported。
        let err2 = complete(&endpoint(&base), &sampling(), "s", "u").await.unwrap_err();
        assert!(err2.downcast_ref::<ToolsUnsupported>().is_none());
    }

    /// M1：arguments 直接給物件的 provider（不按規範）也容得下。
    #[tokio::test]
    async fn chat_tolerates_object_arguments() {
        let (base, _seen) = spawn_script_stub(vec![serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "c1",
                        "function": {"name": "finish", "arguments": {"text": "好"}}
                    }]
                }
            }]
        })])
        .await;
        let res = chat(&endpoint(&base), &sampling(), &[ChatMessage::user("hi")], &[])
            .await
            .unwrap();
        assert_eq!(res.tool_calls[0].arguments["text"], "好");
    }
}
