//! 檔案自動分類——指令層（P1c 殼）。核心已搬入 `ocore::classifier`；此處 re-export
//! 保持 `crate::classifier::*` 路徑，並留 `factory_classify` 指令（AppHandle 依賴）。

pub use ocore::classifier::*;

use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::config;
use crate::i18n::AppError;

// ── Tauri 指令 ──────────────────────────────────────────────────────────
#[tauri::command]
pub async fn factory_classify<R: Runtime>(
    app: AppHandle<R>,
    paths: Vec<String>,
) -> Result<Vec<FileClassification>, AppError> {
    let cfg = config::app_config::load(&app)
        .map_err(|e| AppError::new("factory.classifyError").p("detail", e.to_string()))?;
    // 有可用的 LLM 端點才嘗試 Tier 3（無 key 且非 ollama → None，优雅退回純規則）。
    let endpoint = config::gbrain_config::load_for(cfg.active_env_home())
        .ok()
        .and_then(|loaded| config::gbrain_config::resolve_endpoint(&loaded.config).ok())
        .filter(|ep| ep.has_api_key || ep.provider == "ollama");

    let mut out = Vec::with_capacity(paths.len());
    for p in &paths {
        out.push(classify_one(Path::new(p), &cfg, endpoint.as_ref()).await);
    }
    Ok(out)
}
