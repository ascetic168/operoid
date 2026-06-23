//! LLM 結構化呼叫 — 重用 GBrain config 解析出的端點（chat_model + provider_base_urls + env key）。
//!
//! 採 OpenAI 相容 `/chat/completions`（groq/openai/ollama/deepseek/together/... 皆相容）。
//! anthropic 的 schema 不同，未支援（resolve_endpoint 的 default_base_url 不含 anthropic）。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{self, gbrain_config::LlmEndpoint, AppConfig};

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f64,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

/// 從環境變數取該 provider 的 API key（ollama 等回 None → 不帶 Authorization）。
fn env_key_for(endpoint: &LlmEndpoint) -> Option<String> {
    config::gbrain_config::env_key(&endpoint.provider)
        .and_then(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// 呼叫一次 chat completion，回傳純文字回應。
pub async fn complete(
    endpoint: &LlmEndpoint,
    cfg: &AppConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!(
        "{}/chat/completions",
        endpoint.base_url.trim_end_matches('/')
    );
    let body = ChatRequest {
        model: &endpoint.model,
        messages: vec![
            Message { role: "system", content: system },
            Message { role: "user", content: user },
        ],
        temperature: cfg.llm_temperature,
        max_tokens: cfg.llm_max_tokens,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let mut req = client.post(&url).json(&body);
    if let Some(key) = env_key_for(endpoint) {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.context("LLM 請求失敗")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("LLM 回應非 2xx（{status}）：{text}"));
    }
    let chat: ChatResponse = resp.json().await.context("LLM 回應非預期 JSON")?;
    chat.choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| anyhow!("LLM 回應沒有 content"))
}
