//! Config 模組：GBrain config（權威來源）+ 本系統自有設定。

pub mod app_config;
pub mod gbrain_config;

pub use app_config::{AppConfig, BrainEntry, FactoryTargets, DEFAULT_BRAIN_ID, SUPPORTED_LOCALES};
pub use gbrain_config::{LlmEndpoint, LoadedConfig};

use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::i18n::{AppError, L10n};

/// 前端顯示 GBrain config 的完整視圖。
#[derive(Serialize)]
pub struct GBrainConfigView {
    pub home: String,
    pub config_path: String,
    pub exists: bool,
    pub raw: serde_json::Value,
    pub chat_model: Option<String>,
    /// `models.default` — gbrain think/ask 實際讀取的模型（供前端確認同步生效）。
    pub models_default: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<i64>,
    pub schema_pack: Option<String>,
    pub engine: Option<String>,
    pub database_path: Option<String>,
    pub provider_base_urls: serde_json::Value,
    /// 解析後的 LLM 端點（解析失敗時為 None，前端據此提示）。
    pub llm_endpoint: Option<LlmEndpoint>,
    /// LLM 端點解析失敗時的在地化訊息（代碼+參數，供前端翻譯）。
    pub llm_error: Option<L10n>,
}

fn to_view(loaded: LoadedConfig) -> GBrainConfigView {
    let (llm_endpoint, llm_error) = match gbrain_config::resolve_endpoint(&loaded.config) {
        Ok(ep) => (Some(ep), None),
        Err(e) => (None, Some(L10n::from(e))),
    };
    let c = &loaded.config;
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
        llm_endpoint,
        llm_error,
    }
}

/// 作用中腦的 home（GBRAIN_HOME 值；None=預設腦）。
fn active_home<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app_config::load(app).ok()?.active_env_home().map(|s| s.to_string())
}

#[tauri::command]
pub fn get_gbrain_config<R: Runtime>(app: AppHandle<R>) -> Result<GBrainConfigView, AppError> {
    let home = active_home(&app);
    let loaded = gbrain_config::load_for(home.as_deref())?;
    Ok(to_view(loaded))
}

#[tauri::command]
pub fn save_gbrain_config_raw<R: Runtime>(
    app: AppHandle<R>,
    mut raw_json: serde_json::Value,
) -> Result<(), AppError> {
    let home = active_home(&app);
    let path = gbrain_config::config_path_for(home.as_deref())?;
    // 存檔前同步 models.default/think = chat_model：gbrain think/ask 不讀 chat_model，
    // 若 models.* 缺失會 fallback 到 anthropic opus（跟你要 ANTHROPIC_API_KEY）。
    // 這裡攔截，讓設定頁任何一次存檔都保證 think 用同一個模型。
    gbrain_config::sync_models_to_chat(&mut raw_json);
    gbrain_config::save_raw(&path, &raw_json)?;
    Ok(())
}

#[tauri::command]
pub fn get_app_config<R: Runtime>(app: AppHandle<R>) -> Result<AppConfig, AppError> {
    Ok(app_config::load(&app)?)
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> Result<(), AppError> {
    app_config::save(&app, &config)?;
    Ok(())
}

/// 設定介面語言覆寫。`locale=None` 清除覆寫（回到自動偵測）。
/// 非支援值會被 `normalize` 清成 None。回傳實際生效的 locale。
#[tauri::command]
pub fn set_locale<R: Runtime>(
    app: AppHandle<R>,
    locale: Option<String>,
) -> Result<Option<String>, AppError> {
    let mut c = app_config::load(&app)?;
    c.locale = locale
        .filter(|l| SUPPORTED_LOCALES.contains(&l.as_str()))
        .map(|s| s.to_string());
    app_config::save(&app, &c)?;
    Ok(c.locale.clone())
}
