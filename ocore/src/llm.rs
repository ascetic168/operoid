//! LLM 結構化呼叫 — 重用 GBrain config 解析出的端點（chat_model + provider_base_urls + env key）。
//!
//! 採 OpenAI 相容 `/chat/completions`（groq/openai/ollama/deepseek/together/... 皆相容）。
//! anthropic 的 schema 不同，未支援（resolve_endpoint 的 default_base_url 不含 anthropic）。

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
    gbrain_config::env_key(&endpoint.provider)
        .and_then(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// 呼叫一次 chat completion，回傳純文字回應。
pub async fn complete(
    endpoint: &LlmEndpoint,
    sampling: &SamplingParams,
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
        temperature: sampling.temperature,
        max_tokens: sampling.max_tokens,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let key = env_key_for(endpoint);
    // 429（速率限制）與暫時性網路錯誤：等待後重試——TPM 隨時間回補。
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..3u32 {
        let mut req = client.post(&url).json(&body);
        if let Some(k) = &key {
            req = req.header("Authorization", format!("Bearer {k}"));
        }
        let resp = match req.send().await.context("LLM 請求失敗") {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };
        let status = resp.status();
        if status.is_success() {
            let chat: ChatResponse = resp.json().await.context("LLM 回應非預期 JSON")?;
            return chat
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .ok_or_else(|| anyhow!("LLM 回應沒有 content"));
        }
        if status.as_u16() == 429 && attempt < 2 {
            eprintln!("[llm] 429 rate limit，20s 後重試（attempt {}）", attempt + 1);
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            continue;
        }
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("LLM 回應非 2xx（{status}）：{text}"));
    }
    Err(last_err.unwrap_or_else(|| anyhow!("LLM 重試耗盡")))
}
