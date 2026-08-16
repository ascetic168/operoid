//! Operoid — Tauri backend entry.
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
mod event_bus;
mod factories;
mod gbrain_cli;
mod ingress_server;
mod i18n;
mod llm;
mod note_server;
mod obridge_cfg;
pub mod outbound;
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
        name: "Operoid",
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

/// 一次性 identifier 遷移（Emploid→Operoid）。
///
/// Tauri 以 `tauri.conf.json` 的 `identifier` 決定 app-data 目錄名。改名後
/// identifier 從 `com.emploid.studio` 變為 `com.operoid.studio`，既有使用者的
/// `app-settings.json`（gbrain 路徑、腦清單、locale…）會在新目錄找不到。本函式在
/// 啟動時檢查舊目錄，若新目錄尚無 `app-settings.json` 就把舊目錄內容遞迴複製過來。
///
/// 冪等：新目錄已有 `app-settings.json` 時直接返回（遷移已完成或全新安裝）。
fn migrate_app_data_dir() {
    let config_dir = match dirs::config_dir() {
        Some(d) => d,
        None => return,
    };
    let old_dir = config_dir.join("com.emploid.studio");
    let new_dir = config_dir.join("com.operoid.studio");
    // 新目錄已有設定檔 → 遷移已完成或全新安裝，無事可做。
    if new_dir.join("app-settings.json").exists() {
        return;
    }
    if !old_dir.is_dir() {
        return;
    }
    let _ = std::fs::create_dir_all(&new_dir);
    // 遞迴複製舊目錄下所有檔案至新目錄（保留子目錄結構）。
    let _ = copy_dir_contents(&old_dir, &new_dir);
}

/// 遞迴複製 `src` 下所有檔案／子目錄至 `dst`（已存在則覆寫）。
fn copy_dir_contents(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = entry.file_type()?;
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_contents(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
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
            obridge_cfg::obridge_config_load,
            obridge_cfg::obridge_config_save,
            runtime::agent_inbox_summary,
            runtime::agent_recent_events,
        ])
        .setup(|app| {
            // 一次性 identifier 遷移：Emploid→Operoid 改名後，把舊 app-data 目錄
            // （com.emploid.studio）的內容搬至新目錄（com.operoid.studio），讓既有
            // 使用者的 app-settings.json（gbrain 路徑、腦清單、locale…）無痛延續。
            // 冪等：新目錄已有 app-settings.json 就跳過。
            migrate_app_data_dir();
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
            // E7：外部事件 ingress server（opt-in；port＋secret 皆有設才啟動）。
            // 供外部 bridge（Email／IM／…）以 `POST /event` 投遞事件 → dispatch_event 喚醒員工。
            if let Some(p) = ingress_server::start(app.handle().clone()) {
                eprintln!("[ingress] 進氣口就緒：127.0.0.1:{p}/event");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Operoid");
}
