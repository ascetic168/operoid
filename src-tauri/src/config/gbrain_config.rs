//! 讀取並「使用」GBrain 的 ~/.gbrain/config.json。
//!
//! 核心原則：GBrain config.json 是腦行為的權威來源。這裡把它的欄位解析出來供
//! 系統直接取用（chat_model / embedding_model / schema_pack / database_path /
//! provider_base_urls ...），並解析 LLM provider 路由（base URL + env key）。
//! provider_base_urls 是 file-plane 鍵（gbrain config set 對它是 no-op），讀寫都走檔案。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// ~/.gbrain/config.json 的已知欄位（其餘保留於 `raw`）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GBrainConfig {
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub database_path: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_dimensions: Option<i64>,
    #[serde(default)]
    pub chat_model: Option<String>,
    #[serde(default)]
    pub schema_pack: Option<String>,
    /// file-plane 鍵；gbrain CLI 對它 no-op，須手編此檔。
    #[serde(default)]
    pub provider_base_urls: HashMap<String, String>,
}

/// 解析後的 LLM 端點（給 llm.rs 與前端顯示用）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LlmEndpoint {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub has_api_key: bool,
}

/// 解析 GBrain 的 home 目錄（.gbrain 所在）：
/// - 環境 GBRAIN_HOME 設了 → `<GBRAIN_HOME>/.gbrain`（gbrain 自己補 .gbrain）
/// - 否則 → `~/.gbrain`
pub fn resolve_home() -> Result<PathBuf> {
    if let Ok(home_env) = std::env::var("GBRAIN_HOME") {
        let p = PathBuf::from(home_env);
        // GBRAIN_HOME 必須絕對、無 ..（與 gbrain configDir 一致）
        if p.is_absolute() && !p.components().any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Ok(p.join(".gbrain"));
        }
    }
    let h = dirs::home_dir().context("無法解析使用者 home 目錄")?;
    Ok(h.join(".gbrain"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(resolve_home()?.join("config.json"))
}

/// 載入 GBrain config（檔案不存在則回傳帶 exists=false 的預設值 + 路徑）。
pub struct LoadedConfig {
    pub home: PathBuf,
    pub path: PathBuf,
    pub exists: bool,
    pub config: GBrainConfig,
    /// 完整原始文件（給 file-plane 編輯器；不存在時為 Null）。
    pub raw: serde_json::Value,
}

pub fn load() -> Result<LoadedConfig> {
    let home = resolve_home()?;
    let path = home.join("config.json");
    if !path.exists() {
        return Ok(LoadedConfig {
            home,
            path,
            exists: false,
            config: GBrainConfig::default(),
            raw: serde_json::Value::Null,
        });
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("讀取 {} 失敗", path.display()))?;
    let raw: serde_json::Value = serde_json::from_str(&text).context("config.json 不是合法 JSON")?;
    let config: GBrainConfig = serde_json::from_value(raw.clone()).unwrap_or_default();
    Ok(LoadedConfig {
        home,
        path,
        exists: true,
        config,
        raw,
    })
}

/// 將 file-plane 的 JSON 寫回 config.json（覆寫整份檔）。
pub fn save_raw(path: &Path, json: &serde_json::Value) -> Result<()> {
    let pretty = serde_json::to_string_pretty(json)?;
    std::fs::write(path, pretty)?;
    Ok(())
}

/// provider → 預設 OpenAI 相容 base URL。
pub fn default_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "groq" => Some("https://api.groq.com/openai/v1"),
        "openai" => Some("https://api.openai.com/v1"),
        "ollama" => Some("http://localhost:11434/v1"),
        "deepseek" => Some("https://api.deepseek.com/v1"),
        "together" => Some("https://api.together.xyz/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "zhipu" => Some("https://open.bigmodel.cn/api/paas/v4"),
        "dashscope" => Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        // anthropic 的 schema 非 OpenAI 相容；llm.rs 僅支援 OpenAI 相容端點。
        _ => None,
    }
}

/// provider → 取 API key 的環境變數名（ollama 等免 auth 回 None）。
pub fn env_key(provider: &str) -> Option<&'static str> {
    match provider {
        "groq" => Some("GROQ_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "zeroentropy" => Some("ZEROENTROPY_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        "together" => Some("TOGETHER_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "zhipu" => Some("ZHIPUAI_API_KEY"),
        "dashscope" => Some("DASHSCOPE_API_KEY"),
        "ollama" => None,
        _ => None,
    }
}

/// 從 chat_model（如 `groq:llama-3.3-70b-versatile`）解析 provider 與 model id。
pub fn split_chat_model(chat_model: &str) -> Option<(&str, &str)> {
    let (p, m) = chat_model.split_once(':')?;
    if p.is_empty() || m.is_empty() {
        return None;
    }
    Some((p, m))
}

/// 依 config 解析 LLM 端點：base URL 優先用 provider_base_urls（file-plane），
/// 否則退回 provider 預設；key 從環境變數取。
pub fn resolve_endpoint(config: &GBrainConfig) -> Result<LlmEndpoint> {
    let chat_model = config
        .chat_model
        .as_deref()
        .context("config.json 沒有 chat_model")?;
    let (provider, model) = split_chat_model(chat_model)
        .with_context(|| format!("chat_model 格式不符 provider:model：{chat_model}"))?;
    let base_url = config
        .provider_base_urls
        .get(provider)
        .cloned()
        .or_else(|| default_base_url(provider).map(|s| s.to_string()))
        .with_context(|| format!("未知 provider，且無 provider_base_urls 覆寫：{provider}"))?;
    let has_api_key = match env_key(provider) {
        Some(k) => std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false),
        None => true, // ollama 等 no-auth
    };
    Ok(LlmEndpoint {
        provider: provider.to_string(),
        model: model.to_string(),
        base_url,
        has_api_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_chat_model() {
        assert_eq!(
            split_chat_model("groq:llama-3.3-70b-versatile"),
            Some(("groq", "llama-3.3-70b-versatile"))
        );
        assert_eq!(split_chat_model("noseparator"), None);
    }

    #[test]
    fn resolves_groq_default() {
        let mut c = GBrainConfig::default();
        c.chat_model = Some("groq:llama-3.3-70b-versatile".into());
        let ep = resolve_endpoint(&c).unwrap();
        assert_eq!(ep.provider, "groq");
        assert_eq!(ep.base_url, "https://api.groq.com/openai/v1");
    }

    #[test]
    fn provider_base_urls_overrides() {
        let mut c = GBrainConfig::default();
        c.chat_model = Some("groq:local".into());
        c.provider_base_urls
            .insert("groq".into(), "http://localhost:11434/v1".into());
        let ep = resolve_endpoint(&c).unwrap();
        assert_eq!(ep.base_url, "http://localhost:11434/v1");
    }
}
