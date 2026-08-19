//! GBrain 設定核心（P4 起居於 ocore）——設定頁視圖組裝與 model/provider 編輯。
//!
//! # v0.42 兩種 config plane
//! model/tier 設定走 **DB plane**（`gbrain config set`，runtime 權威）；
//! `provider_base_urls` 等走 **file plane**（直讀直寫 config.json，CLI 對它 no-op）。
//! 設定頁的 model 編輯走 CLI，provider_base_url 編輯走檔案。
//!
//! 介面收 `(exe, home)`（cfg 由殼／oserver 解析傳入）——不含任何 Tauri。

use serde::Serialize;

use crate::gbrain_cli::{config_get, config_set, config_unset, config_unset_pattern};
use crate::gbrain_config::{self, LoadedConfig, LlmEndpoint};
use crate::i18n::{AppError, L10n};

/// 前端顯示 GBrain config 的完整視圖。
#[derive(Serialize, Clone)]
pub struct GBrainConfigView {
    pub home: String,
    pub config_path: String,
    pub exists: bool,
    pub raw: serde_json::Value,
    pub chat_model: Option<String>,
    /// `models.default`（file-plane 殘值；真正生效值在 DB plane，見 tiers）。
    pub models_default: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<i64>,
    pub schema_pack: Option<String>,
    pub engine: Option<String>,
    pub database_path: Option<String>,
    pub provider_base_urls: serde_json::Value,
    /// v0.42 tier 路由：四層各自的有效模型（DB-plane 優先，否則 file/default）。
    pub tiers: TierModelsView,
    /// 每個 tier 的來源："db" | "file" | "default"（前端據此顯示狀態徽章）。
    pub tier_source: TierSourceView,
    /// DB plane 正在蓋過 file plane 的 model/tier 鍵清單（前端據此亮警告橫幅）。
    pub db_overrides: Vec<String>,
    /// 解析後的 LLM 端點（解析失敗時為 None，前端據此提示）。
    pub llm_endpoint: Option<LlmEndpoint>,
    /// LLM 端點解析失敗時的在地化訊息（代碼+參數，供前端翻譯）。
    pub llm_error: Option<L10n>,
}

/// 四個 tier 的有效模型值。
#[derive(Serialize, Clone, Default)]
pub struct TierModelsView {
    pub utility: Option<String>,
    pub reasoning: Option<String>,
    pub deep: Option<String>,
    pub subagent: Option<String>,
}

/// 每個 tier 的來源標記。
#[derive(Serialize, Clone, Default)]
pub struct TierSourceView {
    pub utility: String,
    pub reasoning: String,
    pub deep: String,
    pub subagent: String,
}

/// 由 file-plane `LoadedConfig` 建 view（不查 DB plane；給無法 spawn 子行程的路徑用）。
fn to_view_file_only(loaded: LoadedConfig) -> GBrainConfigView {
    let (llm_endpoint, llm_error) = match gbrain_config::resolve_endpoint(&loaded.config) {
        Ok(ep) => (Some(ep), None),
        Err(e) => (None, Some(L10n::from(e))),
    };
    let c = &loaded.config;
    // tier 從 file-plane 讀（殘值）；來源標 "file"（若有值）否則 "default"
    let file_tiers = c.models.as_ref().and_then(|m| m.tier.as_ref());
    let tier_val = |t: Option<&str>| -> (Option<String>, String) {
        match t {
            Some(v) if !v.is_empty() => (Some(v.to_string()), "file".into()),
            _ => (None, "default".into()),
        }
    };
    let (ut, us) = tier_val(file_tiers.and_then(|t| t.utility.as_deref()));
    let (rt, rs) = tier_val(file_tiers.and_then(|t| t.reasoning.as_deref()));
    let (dt, ds) = tier_val(file_tiers.and_then(|t| t.deep.as_deref()));
    let (st, ss) = tier_val(file_tiers.and_then(|t| t.subagent.as_deref()));
    GBrainConfigView {
        home: loaded.home.to_string_lossy().into_owned(),
        config_path: loaded.path.to_string_lossy().into_owned(),
        exists: loaded.exists,
        raw: loaded.raw.clone(),
        chat_model: c.chat_model.clone(),
        models_default: gbrain_config::models_default_of(&loaded.raw),
        embedding_model: c.embedding_model.clone(),
        embedding_dimensions: c.embedding_dimensions,
        schema_pack: c.schema_pack.clone(),
        engine: c.engine.clone(),
        database_path: c.database_path.clone(),
        provider_base_urls: serde_json::to_value(&c.provider_base_urls).unwrap_or_default(),
        tiers: TierModelsView {
            utility: ut,
            reasoning: rt,
            deep: dt,
            subagent: st,
        },
        tier_source: TierSourceView {
            utility: us,
            reasoning: rs,
            deep: ds,
            subagent: ss,
        },
        db_overrides: Vec::new(),
        llm_endpoint,
        llm_error,
    }
}

/// 由 DB-plane（`gbrain config get`）補正 tier 值與來源。
/// DB 有值 → 覆蓋 file 值，來源標 "db"，並記入 db_overrides（DB 一律視為覆寫，
/// 因為它是權威、會蓋過 file plane——即使 file 無值，DB 有值就代表「GUI 直寫檔案無效」）。
async fn enrich_with_db_plane(exe: &str, home: Option<&str>, view: &mut GBrainConfigView) {
    let mut db_overrides = Vec::new();
    for tier in gbrain_config::TIER_NAMES {
        let key = format!("models.tier.{}", tier);
        if let Ok(Some((value, _source))) = config_get(exe, home, &key).await {
            match *tier {
                "utility" => {
                    view.tiers.utility = Some(value);
                    view.tier_source.utility = "db".into();
                }
                "reasoning" => {
                    view.tiers.reasoning = Some(value);
                    view.tier_source.reasoning = "db".into();
                }
                "deep" => {
                    view.tiers.deep = Some(value);
                    view.tier_source.deep = "db".into();
                }
                "subagent" => {
                    view.tiers.subagent = Some(value);
                    view.tier_source.subagent = "db".into();
                }
                _ => {}
            }
            db_overrides.push(key);
        }
    }
    view.db_overrides = db_overrides;
}

/// 組裝設定頁視圖（file plane 為底，DB plane 補正）。
/// `exe=None`（找不到 gbrain 執行檔）→ 純 file-plane 視圖（不查 DB）。
pub async fn build_config_view(
    exe: Option<&str>,
    home: Option<&str>,
) -> Result<GBrainConfigView, AppError> {
    let loaded = gbrain_config::load_for(home)?;
    let mut view = to_view_file_only(loaded);
    if let Some(exe) = exe {
        enrich_with_db_plane(exe, home, &mut view).await;
    }
    Ok(view)
}

/// 限定 model/tier 鍵白名單（防 CLI injection）。
fn validate_model_key(key: &str) -> Result<(), AppError> {
    let allowed = [
        "chat_model",
        "models.default",
        "models.think",
        "models.tier.utility",
        "models.tier.reasoning",
        "models.tier.deep",
        "models.tier.subagent",
    ];
    if allowed.contains(&key) {
        Ok(())
    } else {
        Err(AppError::new("gbrain.configBadKey").p("key", key))
    }
}

/// 設單一 model/tier 鍵（走 DB plane via `gbrain config set`）。
/// key 限定白名單：chat_model / models.default / models.think / models.tier.*。
pub async fn set_model(exe: &str, home: Option<&str>, key: &str, value: &str) -> Result<(), AppError> {
    validate_model_key(key)?;
    if value.trim().is_empty() {
        return Err(AppError::new("gbrain.configEmptyValue"));
    }
    config_set(exe, home, key, value.trim()).await
}

/// 單一模型同步到全部 tier + chat_model + models.default/think（v0.42「勾選同步」用）。
pub async fn set_models_all(exe: &str, home: Option<&str>, model: &str) -> Result<(), AppError> {
    if model.trim().is_empty() {
        return Err(AppError::new("gbrain.configEmptyValue"));
    }
    let model = model.trim();
    let keys = [
        "chat_model",
        "models.default",
        "models.think",
        "models.tier.utility",
        "models.tier.reasoning",
        "models.tier.deep",
        "models.tier.subagent",
    ];
    for k in keys {
        config_set(exe, home, k, model).await?;
    }
    Ok(())
}

/// 從 DB plane 移除單一 model/tier 鍵（讓 file plane 或 default 生效）。
pub async fn unset_model(exe: &str, home: Option<&str>, key: &str) -> Result<(), AppError> {
    validate_model_key(key)?;
    config_unset(exe, home, key).await
}

/// 清除所有 DB-plane 的 model/tier 覆寫。修復用：一鍵回到 file plane 為準。
pub async fn clear_db_overrides(exe: &str, home: Option<&str>) -> Result<(), AppError> {
    config_unset_pattern(exe, home, "models.tier").await?;
    for k in ["chat_model", "models.default", "models.think"] {
        let _ = config_unset(exe, home, k).await;
    }
    Ok(())
}

/// 設 provider_base_url（**直寫檔案**，因 gbrain CLI 對此 key no-op）。
/// base_url=None → 移除該 provider 的覆寫（用預設）。
pub fn set_provider_base_url(
    home: Option<&str>,
    provider: &str,
    base_url: Option<&str>,
) -> Result<(), AppError> {
    if !gbrain_config::ALL_PROVIDERS.contains(&provider) {
        return Err(AppError::new("config.unknownProvider").p("provider", provider));
    }
    let path = gbrain_config::config_path_for(home)?;
    let loaded = gbrain_config::load_for(home)?;
    let mut raw = loaded.raw;
    if !raw.is_object() {
        raw = serde_json::json!({});
    }
    let url_trimmed = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    match raw.get_mut("provider_base_urls").and_then(|v| v.as_object_mut()) {
        Some(map) => match url_trimmed {
            Some(url) => {
                map.insert(provider.to_string(), serde_json::Value::String(url));
            }
            None => {
                map.remove(provider); // 移除覆寫
            }
        },
        None => {
            if let Some(url) = url_trimmed {
                let mut map = serde_json::Map::new();
                map.insert(provider.to_string(), serde_json::Value::String(url));
                raw["provider_base_urls"] = serde_json::Value::Object(map);
            }
            // 無 map 且無新值 → 不動
        }
    }
    gbrain_config::save_raw(&path, &raw)?;
    Ok(())
}

/// 直寫整份 config.json（file-plane；raw 進階編輯器用）。
/// 注意：model/tier 鍵寫此處會被 DB plane 蓋過——設定頁改用 set_model 等指令。
/// 故此處如實存使用者輸入的 raw JSON，**不**再偷偷同步 models.default/think（E3 退役）。
pub fn save_raw(home: Option<&str>, raw_json: &serde_json::Value) -> Result<(), AppError> {
    let path = gbrain_config::config_path_for(home)?;
    gbrain_config::save_raw(&path, raw_json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_key_whitelist() {
        assert!(validate_model_key("chat_model").is_ok());
        assert!(validate_model_key("models.tier.deep").is_ok());
        assert!(validate_model_key("rm -rf").is_err());
        assert!(validate_model_key("provider_base_urls").is_err());
    }
}
