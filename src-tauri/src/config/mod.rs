//! Config 模組：GBrain config（權威來源）+ 本系統自有設定。

pub mod app_config;
pub mod gbrain_config;

pub use app_config::{AppConfig, FactoryTargets};
pub use gbrain_config::{LlmEndpoint, LoadedConfig};

use serde::Serialize;
use tauri::{AppHandle, Runtime};

/// 前端顯示 GBrain config 的完整視圖。
#[derive(Serialize)]
pub struct GBrainConfigView {
    pub home: String,
    pub config_path: String,
    pub exists: bool,
    pub raw: serde_json::Value,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<i64>,
    pub schema_pack: Option<String>,
    pub engine: Option<String>,
    pub database_path: Option<String>,
    pub provider_base_urls: serde_json::Value,
    /// 解析後的 LLM 端點（解析失敗時為 None，前端據此提示）。
    pub llm_endpoint: Option<LlmEndpoint>,
    pub llm_error: Option<String>,
}

fn to_view(loaded: LoadedConfig) -> GBrainConfigView {
    let llm_endpoint = gbrain_config::resolve_endpoint(&loaded.config).ok();
    let llm_error = gbrain_config::resolve_endpoint(&loaded.config).err().map(|e| e.to_string());
    let c = &loaded.config;
    GBrainConfigView {
        home: loaded.home.to_string_lossy().into_owned(),
        config_path: loaded.path.to_string_lossy().into_owned(),
        exists: loaded.exists,
        raw: loaded.raw.clone(),
        chat_model: c.chat_model.clone(),
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

#[tauri::command]
pub fn get_gbrain_config() -> Result<GBrainConfigView, String> {
    let loaded = gbrain_config::load().map_err(|e| e.to_string())?;
    Ok(to_view(loaded))
}

#[tauri::command]
pub fn save_gbrain_config_raw(raw_json: serde_json::Value) -> Result<(), String> {
    let path = gbrain_config::config_path().map_err(|e| e.to_string())?;
    gbrain_config::save_raw(&path, &raw_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_config<R: Runtime>(app: AppHandle<R>) -> Result<AppConfig, String> {
    app_config::load(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> Result<(), String> {
    app_config::save(&app, &config).map_err(|e| e.to_string())
}
