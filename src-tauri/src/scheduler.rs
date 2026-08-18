//! Runtime 排程器——殼層接線（P1b，2026-08-18）。主迴圈已搬入 `ocore::scheduler`
//! （cfg 載入以閉包注入、db 路徑由殼層解析）；此處負責 Tauri 側接線：
//! 建 channel＋`app.manage(AppState)`＋以 `tauri::async_runtime::spawn` 起 loop
//! （setup 在 event loop 起來前執行，不能直接 `tokio::spawn`——照 `note_server` 先例）。

use std::sync::Arc;

use tauri::{async_runtime, AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, InboundEvent, WakeSignal};
use crate::config::app_config;
use crate::runtime::agent_db_path;

/// 啟動排程器：建 channel＋共享狀態、`app.manage(AppState)`、spawn 常駐 loop。
///
/// `agent_os_enabled` 不在此把關——loop 內每輪自查，讓使用者於設定開關後下次 tick 即生效。
pub fn start<R: Runtime>(app: AppHandle<R>) {
    let cfg = app_config::load(&app).unwrap_or_default();
    let permits = cfg.llm_concurrency;
    let (wake_tx, wake_rx) = tokio::sync::mpsc::channel::<WakeSignal>(64);
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<InboundEvent>(128);
    let state = AppState::new(wake_tx, event_tx, permits);
    app.manage(state.clone()); // AppState 全 Arc 欄位（Clone 共享同一份），殼與 loop 同一狀態
    let db_path = agent_db_path(&app)
        .unwrap_or_else(|_| std::env::temp_dir().join("operoid.db")); // 兜底：資料目錄不可解析時退 temp（罕見）
    let app_for_cfg = app.clone();
    let load_cfg: ocore::scheduler::CfgLoader =
        Arc::new(move || app_config::load(&app_for_cfg));
    async_runtime::spawn(ocore::scheduler::scheduler_loop(
        state, load_cfg, db_path, wake_rx, event_rx,
    ));
}
