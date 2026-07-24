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

use crate::i18n::AppError;

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

/// 由顯式 home（.gbrain 的「父目錄」= GBRAIN_HOME 值）解析 .gbrain 路徑。
/// `None` = 預設腦（~/.gbrain）。不讀 `std::env`，由呼叫端傳作用中腦。
pub fn resolve_home_for(home: Option<&str>) -> Result<PathBuf> {
    match home.map(str::trim).filter(|h| !h.is_empty()) {
        Some(h) => {
            let p = PathBuf::from(h);
            // GBRAIN_HOME 必須絕對、無 ..（與 gbrain configDir 一致）
            if p.is_absolute() && !p.components().any(|c| matches!(c, std::path::Component::ParentDir))
            {
                Ok(p.join(".gbrain"))
            } else {
                Ok(dirs::home_dir().context("無法解析使用者 home 目錄")?.join(".gbrain"))
            }
        }
        None => {
            let h = dirs::home_dir().context("無法解析使用者 home 目錄")?;
            Ok(h.join(".gbrain"))
        }
    }
}

pub fn config_path_for(home: Option<&str>) -> Result<PathBuf> {
    Ok(resolve_home_for(home)?.join("config.json"))
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

/// 載入「指定腦」的 config（`home` = GBRAIN_HOME 父目錄；None = 預設腦）。
pub fn load_for(home: Option<&str>) -> Result<LoadedConfig> {
    let home = resolve_home_for(home)?;
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

/// 確保 raw config.json 的 `models.default` / `models.think` 與 `chat_model` 一致。
///
/// `gbrain think` 的 model 解析鏈為 `models.think → models.default → $GBRAIN_MODEL
/// → anthropic:claude-opus`（hard-coded fallback），**完全不讀頂層 `chat_model`**。
/// 本函式把 chat_model 的值同步寫進 `models.default`/`models.think`，讓 GUI 設的
/// 主模型真正對 think/ask 生效（否則 think 靜默 fallback 到 anthropic opus，並跟你要
/// ANTHROPIC_API_KEY）。
///
/// 冪等：已一致則不動。`chat_model` 缺失或空字串 → 不動。保留 `models` 內其他鍵。
/// 回傳 `true` 表示有改動（用於測試/日誌）。
pub fn sync_models_to_chat(raw: &mut serde_json::Value) -> bool {
    // 先取出 chat_model 字串（own），避免後續 get_mut("models") 時借用衝突。
    let chat = match raw.get("chat_model").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return false,
    };
    // root 非物件（如 Null/陣列）無法寫入巢狀鍵，放棄。
    if !raw.is_object() {
        return false;
    }
    let target = serde_json::Value::String(chat);
    if raw.get("models").map(|v| v.is_object()).unwrap_or(false) {
        // models 已是物件，保留其他鍵
    } else {
        raw["models"] = serde_json::Value::Object(serde_json::Map::new());
    }
    let models = raw
        .get_mut("models")
        .and_then(|v| v.as_object_mut())
        .expect("models 剛確保為物件");
    let mut changed = false;
    if models.get("default") != Some(&target) {
        models.insert("default".into(), target.clone());
        changed = true;
    }
    if models.get("think") != Some(&target) {
        models.insert("think".into(), target);
        changed = true;
    }
    changed
}

/// 讀 raw config.json 的 `models.default`（供設定頁顯示「think/ask 實際使用的模型」）。
pub fn models_default_of(raw: &serde_json::Value) -> Option<String> {
    raw.get("models")
        .and_then(|m| m.get("default"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
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
/// 否則退回 provider 預設；key 從環境變數取。錯誤回在地化代碼（供前端翻譯）。
pub fn resolve_endpoint(config: &GBrainConfig) -> Result<LlmEndpoint, AppError> {
    let chat_model = config
        .chat_model
        .as_deref()
        .ok_or_else(|| AppError::new("config.noChatModel"))?;
    let (provider, model) = split_chat_model(chat_model)
        .ok_or_else(|| AppError::new("config.badChatModel").p("chatModel", chat_model))?;
    let base_url = config
        .provider_base_urls
        .get(provider)
        .cloned()
        .or_else(|| default_base_url(provider).map(|s| s.to_string()))
        .ok_or_else(|| AppError::new("config.unknownProvider").p("provider", provider))?;
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
    fn sync_creates_models_when_missing() {
        let mut raw = serde_json::json!({"chat_model": "groq:llama-3.3-70b-versatile"});
        assert!(sync_models_to_chat(&mut raw));
        assert_eq!(raw["models"]["default"], "groq:llama-3.3-70b-versatile");
        assert_eq!(raw["models"]["think"], "groq:llama-3.3-70b-versatile");
    }

    #[test]
    fn sync_noop_without_chat_model() {
        let mut raw = serde_json::json!({"engine": "pglite"});
        assert!(!sync_models_to_chat(&mut raw));
        assert!(raw.get("models").is_none());

        // 空 chat_model 也不動
        let mut raw2 = serde_json::json!({"chat_model": ""});
        assert!(!sync_models_to_chat(&mut raw2));
    }

    #[test]
    fn sync_preserves_other_model_keys() {
        let mut raw = serde_json::json!({
            "chat_model": "groq:x",
            "models": {"default": "old", "custom": "keep-me"}
        });
        assert!(sync_models_to_chat(&mut raw));
        assert_eq!(raw["models"]["default"], "groq:x");
        assert_eq!(raw["models"]["think"], "groq:x");
        assert_eq!(raw["models"]["custom"], "keep-me"); // 其他鍵保留
    }

    #[test]
    fn sync_is_idempotent() {
        let mut raw = serde_json::json!({"chat_model": "groq:x"});
        assert!(sync_models_to_chat(&mut raw));
        assert!(!sync_models_to_chat(&mut raw)); // 第二次無改動
    }

    #[test]
    fn models_default_of_reads_nested() {
        let raw = serde_json::json!({"models": {"default": "groq:y"}});
        assert_eq!(models_default_of(&raw), Some("groq:y".into()));
        assert_eq!(models_default_of(&serde_json::json!({})), None);
        assert_eq!(
            models_default_of(&serde_json::json!({"models": {"think": "groq:z"}})),
            None
        );
    }

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
