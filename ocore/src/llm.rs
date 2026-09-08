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
    /// W2：OpenAI 相容回應的 usage 區塊（成本記錄 lite；provider 沒給 → None）。
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
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
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..3u32 {
        let backoff = retry_backoff(attempt);
        let mut req = client.post(&url).json(&body);
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
            let chat: ChatResponse = resp.json().await.context("LLM 回應非預期 JSON")?;
            let content = chat
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .ok_or_else(|| anyhow!("LLM 回應沒有 content"))?;
            return Ok(ChatCompletion { content, usage: chat.usage });
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
        return Err(anyhow!("LLM 回應非 2xx（{status}）：{text}"));
    }
    Err(last_err.unwrap_or_else(|| anyhow!("LLM 重試耗盡")))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
