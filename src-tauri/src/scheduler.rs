//! Runtime 排程器——殼層接線（P1b，2026-08-18）。主迴圈已搬入 `ocore::scheduler`
//! （cfg 載入以閉包注入、db 路徑由殼層解析）；此處負責 Tauri 側接線：
//! 建 channel＋`app.manage(AppState)`＋以 `tauri::async_runtime::spawn` 起 loop
//! （setup 在 event loop 起來前執行，不能直接 `tokio::spawn`——照 `note_server` 先例）。


use tauri::{AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, InboundEvent, WakeSignal};
use crate::config::app_config;


/// P3：只 `app.manage(AppState)` 而不起本地排程 loop——runtime 由 oserver 擁有
/// （分離語意；GUI 關閉服務續跑）。殘留的 invoke 指令（tauri::State<AppState>）仍可用；
/// wake 信號無接收者時 try_send 靜默失敗（oserver 的 30s tick 會掃到同一個 task）。
pub fn manage_state_only<R: Runtime>(app: AppHandle<R>) {
    let cfg = app_config::load(&app).unwrap_or_default();
    let (wake_tx, _wake_rx) = tokio::sync::mpsc::channel::<WakeSignal>(64);
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel::<InboundEvent>(128);
    app.manage(AppState::new(wake_tx, event_tx, cfg.llm_concurrency));
}
