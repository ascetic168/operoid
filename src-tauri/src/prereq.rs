//! 啟動時前置程式檢查——指令層（P1c 殼）。核心已搬入 `ocore::prereq`。

pub use ocore::prereq::*;

use tauri::{AppHandle, Runtime};

use crate::config;

#[tauri::command]
pub fn check_prerequisites<R: Runtime>(app: AppHandle<R>) -> Result<Vec<DepStatus>, String> {
    let cfg = config::app_config::load(&app).unwrap_or_default();
    Ok(check_all(&cfg.gbrain_exe_path))
}
