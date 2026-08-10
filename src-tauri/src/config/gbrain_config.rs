//! 讀取並「使用」GBrain 的 ~/.gbrain/config.json。
//!
//! 核心原則：GBrain config.json 是腦行為的權威來源。這裡把它的欄位解析出來供
//! 系統直接取用（chat_model / embedding_model / schema_pack / database_path /
//! provider_base_urls ...），並解析 LLM provider 路由（base URL + env key）。
//!
//! # 兩種 config plane（v0.42+ 重要語意）
//! gbrain 的設定分兩層，**不同 key 寫不同 plane**：
//! - **DB plane**（權威）：`chat_model`、`models.default`、`models.think`、
//!   `models.tier.*`。`gbrain config set <key> <value>` 寫入此層；runtime 優先讀此層。
//!   注意：**檔案裡的對應鍵會被 DB 層靜默蓋過**（這是本模組 file-plane 編輯器失效的根因）。
//! - **file plane**：`provider_base_urls`、`embedding_*`、`schema_pack`、`engine` 等。
//!   `gbrain config set` 對這些 key 多為 no-op，只能直讀直寫 `config.json`。
//!
//! 因此設定頁的 model/tier 編輯**必須走 CLI**（`gbrain config set`），而
//! `provider_base_urls` 編輯**必須直寫檔案**。見 `config/mod.rs` 的對應指令。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::i18n::AppError;

/// v0.42 tier 路由的四個層級名稱（utility / reasoning / deep / subagent）。
pub const TIER_NAMES: &[&str] = &["utility", "reasoning", "deep", "subagent"];

/// 新腦建立／GUI 首次設定時的預設 chat model（v0.42 起改用智譜 GLM）。
pub const DEFAULT_CHAT_MODEL: &str = "zhipu:glm-5.2";

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
    /// file-plane 殘值（`models.tier.*`）。**注意**：gbrain runtime 讀的是 DB plane，
    /// 檔案裡的 tier 值通常會被 DB 層蓋過；真正生效的值需透過 `gbrain config get` 取得。
    #[serde(default)]
    pub models: Option<ModelsSection>,
}

/// `models` 區段（file-plane 殘值；DB plane 才是權威）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelsSection {
    #[serde(default, rename = "default")]
    #[allow(dead_code)]
    pub default: Option<String>,
    #[serde(default, rename = "think")]
    #[allow(dead_code)]
    pub think: Option<String>,
    #[serde(default, rename = "tier")]
    pub tier: Option<TierModels>,
}

/// `models.tier.*`（file-plane 殘值；DB plane 才是權威）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TierModels {
    #[serde(default)]
    pub utility: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub deep: Option<String>,
    #[serde(default)]
    pub subagent: Option<String>,
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
///
/// **deprecated（v0.42+）**：此函式寫 file plane，但 gbrain v0.42 的 `models.*` 是
/// DB plane，runtime 會蓋過檔案層。新程式碼應走 `gbrain config set`（DB plane），
/// 見 `config/mod.rs` 的 `set_gbrain_model` / `set_gbrain_models_all`。此函式保留供
/// `brains.rs::sync_new_brain_models` 既有流程與測試使用。
#[deprecated(note = "v0.42+: models.* 是 DB plane，改走 gbrain config set（見 mod.rs 指令）")]
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
///
/// 注意：此處讀的是 file-plane 殘值；v0.42+ 真正生效的值在 DB plane（`gbrain config get`）。
pub fn models_default_of(raw: &serde_json::Value) -> Option<String> {
    raw.get("models")
        .and_then(|m| m.get("default"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 讀 raw config.json 的 `models.tier.<name>`（file-plane 殘值）。
#[allow(dead_code)]
pub fn tier_of(raw: &serde_json::Value, tier: &str) -> Option<String> {
    raw.get("models")
        .and_then(|m| m.get("tier"))
        .and_then(|t| t.get(tier))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 設定頁 provider 下拉清單：所有 gbrain 支援的 provider（含 anthropic）。
/// 注意：anthropic/zeroentropy 走 native schema，**Operoid 的 llm.rs 不支援**，
/// 僅 gbrain 原生任務（think/ask/dream）與前端顯示用。
pub const ALL_PROVIDERS: &[&str] = &[
    "groq",
    "openai",
    "anthropic",
    "ollama",
    "deepseek",
    "together",
    "openrouter",
    "zhipu",
    "dashscope",
    "zeroentropy",
];

/// provider → 預設 OpenAI 相容 base URL（**僅 OpenAI 相容 provider**）。
/// anthropic/zeroentropy 的 schema 非 OpenAI 相容，故不在此表——其端點須由
/// `provider_base_urls` 顯式覆寫，且僅 gbrain 原生任務使用，llm.rs 不會呼叫。
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
        // anthropic/zeroentropy：非 OpenAI 相容，無預設 base URL。
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
    #![allow(deprecated)] // sync_models_to_chat 已標 deprecated，但其行為仍需測試
    use super::*;

    #[test]
    fn default_chat_model_is_zhipu_glm() {
        // v0.42 起預設改用智譜 GLM（groq 對 file-plane 的 base_url 處理有問題）
        assert_eq!(DEFAULT_CHAT_MODEL, "zhipu:glm-5.2");
        // 確認它是合法的 provider:model 格式
        assert_eq!(
            split_chat_model(DEFAULT_CHAT_MODEL),
            Some(("zhipu", "glm-5.2"))
        );
    }

    #[test]
    fn all_providers_includes_anthropic() {
        // v0.42 subagent tier 偏好 Anthropic（prompt caching）；下拉必須包含
        assert!(ALL_PROVIDERS.contains(&"anthropic"));
        assert!(ALL_PROVIDERS.contains(&"zhipu"));
        assert!(ALL_PROVIDERS.contains(&"groq"));
    }

    #[test]
    fn anthropic_has_no_default_base_url() {
        // anthropic 非 OpenAI 相容；default_base_url 刻意排除（llm.rs 不支援）
        assert!(default_base_url("anthropic").is_none());
        assert!(default_base_url("zhipu").is_some()); // zhipu 是 OpenAI 相容
    }

    #[test]
    fn tier_names_are_four() {
        assert_eq!(TIER_NAMES, &["utility", "reasoning", "deep", "subagent"]);
    }

    #[test]
    fn tier_of_reads_nested() {
        let raw = serde_json::json!({"models": {"tier": {"subagent": "anthropic:claude-sonnet-4-6"}}});
        assert_eq!(
            tier_of(&raw, "subagent"),
            Some("anthropic:claude-sonnet-4-6".into())
        );
        assert_eq!(tier_of(&raw, "utility"), None); // 未設定
        assert_eq!(tier_of(&serde_json::json!({}), "subagent"), None); // 無 models
    }

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
            split_chat_model("zhipu:glm-5.2"),
            Some(("zhipu", "glm-5.2"))
        );
        assert_eq!(split_chat_model("noseparator"), None);
    }

    #[test]
    fn resolves_zhipu_default() {
        let mut c = GBrainConfig::default();
        c.chat_model = Some("zhipu:glm-5.2".into());
        let ep = resolve_endpoint(&c).unwrap();
        assert_eq!(ep.provider, "zhipu");
        assert_eq!(ep.base_url, "https://open.bigmodel.cn/api/paas/v4");
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
        c.chat_model = Some("zhipu:glm-5.2".into());
        c.provider_base_urls.insert(
            "zhipu".into(),
            "https://open.bigmodel.cn/api/coding/paas/v4".into(),
        );
        let ep = resolve_endpoint(&c).unwrap();
        assert_eq!(ep.base_url, "https://open.bigmodel.cn/api/coding/paas/v4");
    }
}
