//! Runtime 排程器（Phase 6，Handbook Ch.13）——常駐 task，依 Trigger 驅動喚醒員工。
//!
//! 在 `.setup()` 以 `tauri::async_runtime::spawn` 起來（**不是** `tokio::spawn`——setup 在
//! event loop 起來前執行，照 [`crate::note_server`] 先例）。`select!` 在喚醒信號
//! （Message-driven／Manual Trigger）與 30s tick（Time-driven）之間。
//!
//! 兩種掃描：
//! - **Inbox 掃描**（每次 tick／信號）：喚醒 `Sleeping` 且有待辦 task 的員工 → `run_inbox`。
//!   訊息／交接投遞的 task 在此被消費（成本低、即時）。
//! - **承諾掃描**（僅啟動一次）：喚醒 `Sleeping` 且有 `Active` commitment 的員工 → `run_autonomous`。
//!   只在啟動跑，**不在每次 tick 重跑**——承諾驅動每輪是多次 LLM 呼叫，每次 tick 重跑會失控燒錢；
//!   一個 commitment 在一次 session 內跑到 Satisfied（完成）／Suspended（0 進展卡住）／有進展則睡（下次啟動續）。
//!
//! 喚醒合取條件守原則 7：「Trigger 觸發 **且** 有工作」。busy-lock 防同一員工被並發執行。

use std::time::Duration;

use futures::future;
use tauri::{async_runtime, AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, WakeSignal};
use crate::config::app_config;
use crate::domain::{EmployeeState, SqliteStore, Store};
use crate::runtime::{
    agent_db_path, build_reasoner, build_tool_ctx, run_commitments_for_employee, run_inbox,
};

/// 啟動排程器：建 channel＋共享狀態、`app.manage(AppState)`、spawn 常駐 loop。
///
/// `agent_os_enabled` 不在此把關——loop 內每輪自查，讓使用者於設定開關後下次 tick 即生效。
pub fn start<R: Runtime>(app: AppHandle<R>) {
    let (wake_tx, wake_rx) = tokio::sync::mpsc::channel::<WakeSignal>(64);
    app.manage(AppState::new(wake_tx));
    async_runtime::spawn(scheduler_loop(app, wake_rx));
}

/// 排程器主迴圈。首次 tick 做承諾掃描（啟動喚醒）；其後每次 tick／信號只做 Inbox 掃描。
async fn scheduler_loop<R: Runtime>(
    app: AppHandle<R>,
    mut wake_rx: tokio::sync::mpsc::Receiver<WakeSignal>,
) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    let mut started = false;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if !started {
                    started = true;
                    let _ = scan_commitments(&app).await; // 啟動：承諾驅動喚醒
                }
                let _ = reset_errored(&app).await; // 復原：Error 死巷→重試
                let _ = scan_inbox(&app).await;
            }
            Some(_sig) = wake_rx.recv() => { let _ = scan_inbox(&app).await; }
        }
    }
}

/// 復原：把「有待辦工作卻卡在 Error」的員工重設為 Sleeping。
/// Error 無重試路徑（排程器只喚醒 Sleeping），會永久卡住；Phase 7b 起 run_autonomous 不再設 Error，
/// 此函主要清理歷史／異常卡住的員工，讓它們在下次掃描被喚醒重試。
async fn reset_errored<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(agent_db_path(app)?)?;
    for mut e in store
        .list_all_employees()?
        .into_iter()
        .filter(|e| e.state == EmployeeState::Error)
    {
        let has_work = !store.list_assigned_tasks_by_owner(&e.id)?.is_empty()
            || !store.list_active_commitments_by_owner(&e.id)?.is_empty();
        if has_work {
            e.state = EmployeeState::Sleeping;
            store.put_employee(&e)?;
            eprintln!("[scheduler] {} 卡在 Error 且有待辦→重設 Sleeping（重試）", e.id);
        }
    }
    Ok(())
}

/// Inbox 掃描：喚醒 Sleeping＋有待辦 task 的員工，併發跑 `run_inbox`（共吃一個 `&store`）。
async fn scan_inbox<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(agent_db_path(app)?)?;
    let mut candidates: Vec<String> = Vec::new();
    for e in store.list_all_employees()? {
        if e.state == EmployeeState::Sleeping
            && !store.list_assigned_tasks_by_owner(&e.id)?.is_empty()
        {
            candidates.push(e.id);
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let cfg = &cfg;
    let store = &store;
    let futs = candidates.into_iter().filter_map(|id| {
        let guard = state.try_acquire(&id)?; // 已在跑則跳過
        Some(async move {
            let _guard = guard; // 釋放於此 future 完成（含錯誤路徑）
            if let Ok((tool, ctx)) = build_tool_ctx(cfg, store, &id) {
                // Reasoner 為可選：有則訊息走對話回合，無則退回 gbrain 單發（守 6c 行為）。
                let reasoner = match build_reasoner(cfg, store, &id) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        eprintln!("[scheduler] build_reasoner({id}) 失敗（退化為 gbrain-only）: {e}");
                        None
                    }
                };
                let rref: Option<&dyn crate::domain::Reasoner> = match &reasoner {
                    Some(r) => Some(r),
                    None => None,
                };
                if let Err(e) = run_inbox(&id, &tool, rref, &ctx, store).await {
                    eprintln!("[scheduler] run_inbox({id}) failed: {e}");
                }
            }
        })
    });
    future::join_all(futs).await;
    Ok(())
}

/// 承諾掃描（啟動一次）：喚醒 Sleeping＋有 Active commitment 的員工，交給
/// [`run_commitments_for_employee`]（清 Inbox → 對每個 Active commitment 跑 run_autonomous）。
async fn scan_commitments<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(agent_db_path(app)?)?;
    let mut candidates: Vec<String> = Vec::new();
    for e in store.list_all_employees()? {
        if e.state == EmployeeState::Sleeping
            && !store.list_active_commitments_by_owner(&e.id)?.is_empty()
        {
            candidates.push(e.id);
        }
    }
    drop(store); // 各 helper 開自己的 connection
    if candidates.is_empty() {
        return Ok(());
    }
    let futs = candidates.into_iter().map(|id| {
        let app = app.clone();
        async move {
            let _ = run_commitments_for_employee(&app, &id).await;
        }
    });
    future::join_all(futs).await;
    Ok(())
}
