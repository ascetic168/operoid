//! GBrainStudio — Tauri backend entry.
//!
//! Domain modules: config (Phase 1), converters (Phase 2), gbrain_cli (Phase 3),
//! llm + factories (Phase 4).

mod brains;
mod config;
mod converters;
mod factories;
mod gbrain_cli;
mod i18n;
mod llm;
mod prereq;

use serde::Serialize;
use tauri::Manager;

/// 回傳給前端的環境資訊（用來驗證 Rust↔JS 橋接與環境解析）。
#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub gbrain_home: String,
    pub notes_repo_default: String,
    pub gbrain_exe_default: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "GBrainStudio",
        version: env!("CARGO_PKG_VERSION"),
        // GBrain 以 GBRAIN_HOME 為準（gbrain 會自己補上 .gbrain）；未設則為 ~/.gbrain
        gbrain_home: std::env::var("GBRAIN_HOME").unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".gbrain").to_string_lossy().into_owned())
                .unwrap_or_default()
        }),
        notes_repo_default: dirs::home_dir()
            .map(|h| h.join("notes").to_string_lossy().into_owned())
            .unwrap_or_default(),
        gbrain_exe_default: dirs::home_dir()
            .map(|h| {
                h.join(".bun")
                    .join("bin")
                    .join("gbrain.exe")
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default(),
    }
}

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            app_info,
            ping,
            prereq::check_prerequisites,
            config::get_gbrain_config,
            config::save_gbrain_config_raw,
            config::get_app_config,
            config::save_app_config,
            config::set_locale,
            gbrain_cli::op_run,
            factories::factory_run,
            factories::factory_write_pages,
            factories::factory_save_authored,
            factories::extract_companies_run,
            brains::brains_list,
            brains::brains_add,
            brains::brains_remove,
            brains::brains_set_active,
            brains::brains_set_active_source,
            brains::brain_sources,
            brains::brain_source_add,
            brains::brain_source_remove,
            brains::brain_sync,
        ])
        .setup(|app| {
            // 確保 app data 目錄存在，供 tauri-plugin-store 寫入本系統設定。
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running GBrainStudio");
}
