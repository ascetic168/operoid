//! 啟動時前置程式檢查——指令層（P1c 殼）。核心已搬入 `ocore::prereq`。

pub use ocore::prereq::*;

use tauri::{AppHandle, Runtime};

use crate::config;

#[tauri::command]
pub fn check_prerequisites<R: Runtime>(app: AppHandle<R>) -> Result<Vec<DepStatus>, String> {
    let cfg = config::app_config::load(&app).unwrap_or_default();
    let cache = cfg.prereq_cache.clone().unwrap_or_default();
    let deps = check_all(&cfg.gbrain_exe_path, dirs::home_dir().as_deref(), &cache);
    // 版本快取缺漏 → 背景刷新（spawn gbrain 是 bun 冷啟——絕不掛啟動路徑）。
    let needs_refresh = cache.bun.is_none() || cache.gbrain.is_none();
    if needs_refresh {
        let app2 = app.clone();
        std::thread::spawn(move || {
            let cfg = config::app_config::load(&app2).unwrap_or_default();
            let fresh = ocore::prereq::refresh_details(
                &cfg.gbrain_exe_path,
                dirs::home_dir().as_deref(),
            );
            let mut cfg2 = config::app_config::load(&app2).unwrap_or_default();
            cfg2.prereq_cache = Some(fresh);
            let _ = config::app_config::save(&app2, &cfg2);
        });
    }
    Ok(deps)
}
