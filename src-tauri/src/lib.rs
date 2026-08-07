//! Emploid — Tauri backend entry.
//!
//! Domain modules: config (Phase 1), converters (Phase 2), gbrain_cli (Phase 3),
//! llm + factories (Phase 4).

mod agent_state;
mod brains;
mod classifier;
mod claude_code;
mod config;
mod converters;
mod domain;
mod factories;
mod gbrain_cli;
mod i18n;
mod llm;
mod note_server;
mod note_view;
mod prereq;
mod runtime;
mod scheduler;

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
        name: "Emploid",
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
            config::set_gbrain_model,
            config::set_gbrain_models_all,
            config::unset_gbrain_model,
            config::clear_db_overrides,
            config::set_provider_base_url,
            config::get_app_config,
            config::save_app_config,
            config::set_locale,
            gbrain_cli::op_run,
            claude_code::claude_code_status,
            claude_code::claude_code_launch,
            factories::factory_run,
            factories::factory_open_dir,
            factories::factory_write_pages,
            factories::factory_save_authored,
            factories::extract_companies_run,
            classifier::factory_classify,
            brains::brains_list,
            brains::brains_add,
            brains::brains_remove,
            brains::brains_set_active,
            brains::brains_set_active_source,
            brains::brain_sources,
            brains::brain_source_add,
            brains::brain_source_remove,
            brains::brain_bind_source_path,
            brains::brain_sync,
            note_view::open_note,
            runtime::agent_seed,
            runtime::agent_recruit,
            runtime::agent_create_template,
            runtime::agent_deploy_instance,
            runtime::agent_ensure_workspace,
            runtime::agent_list_templates,
            runtime::agent_list_employees,
            runtime::agent_delete_template,
            runtime::agent_delete_employee,
            runtime::agent_rename_template,
            runtime::agent_rename_employee,
            runtime::agent_run,
            runtime::agent_create_commitment,
            runtime::agent_satisfy_commitment,
            runtime::agent_approve_commitment,
            runtime::agent_reject_commitment,
            runtime::agent_archive_commitment,
            runtime::agent_cancel_task,
            runtime::agent_revise_artifact,
            runtime::agent_list_state,
            runtime::agent_create_project,
            runtime::agent_run_team,
            runtime::agent_handoff_task,
            runtime::agent_run_task,
            runtime::agent_send_message,
            runtime::agent_clear_messages,
            runtime::agent_watch,
            runtime::agent_inbox_summary,
            runtime::agent_recent_events,
        ])
        .setup(|app| {
            // 確保 app data 目錄存在，供 tauri-plugin-store 寫入本系統設定。
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
            }
            // 啟動本地（迴環）HTTP server：瀏覽器預覽筆記時的回呼通道。
            // 渲染為按需（僅瀏覽器請求時），不寫磁碟檔案。
            let port = note_server::start(app.handle().clone());
            app.manage(note_server::NoteServer { port });
            // Phase 6：啟動 Runtime 排程器（常駐 task，依 Trigger 喚醒員工）。
            scheduler::start(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Emploid");
}
