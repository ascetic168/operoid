//! Runtime 排程器（Phase 6，Handbook Ch.13）——常駐 task，依 Trigger 驅動喚醒員工。
//!
//! P1b（2026-08-18）：自 src-tauri 搬入 ocore。`spawn_loop` 用 `tokio::spawn`（桌面殼的
//! `start(app)` 負責接線：`app.manage(AppState)`＋以閉包提供 cfg 載入與 db 路徑，
//! 見 src-tauri/src/scheduler.rs）。`select!` 在喚醒信號（Message-driven／Manual Trigger）
//! 與 30s tick（Time-driven）之間。
//!
//! 兩種掃描：
//! - **Inbox 掃描**（每次 tick／信號）：喚醒 `Sleeping` 且有待辦 task 的員工 → `run_inbox`。
//! - **承諾掃描**（僅啟動一次）：喚醒 `Sleeping` 且有 `Active` commitment 的員工 → `run_autonomous`。
//!   只在啟動跑，**不在每次 tick 重跑**——承諾驅動每輪是多次 LLM 呼叫，每次 tick 重跑會失控燒錢。
//!
//! 喚醒合取條件守原則 7：「Trigger 觸發 **且** 有工作」。busy-lock 防同一員工被並發執行。

use std::sync::Arc;
use std::time::Duration;

use futures::future;

use crate::agent_state::{AppState, InboundEvent, WakeSignal};
use crate::app_config::AppConfig;
use crate::domain::{EmployeeState, SqliteStore, Store};
use crate::event_bus;
use crate::outbound::OutboundConfig;
use crate::runtime::{build_reasoner, build_tool_ctx, run_commitments_for_employee, run_inbox};

/// cfg 載入器：殼層以閉包提供（桌面殼讀 tauri-plugin-store；未來 oserver 讀 operoid.toml）。
pub type CfgLoader = Arc<dyn Fn() -> anyhow::Result<AppConfig> + Send + Sync>;

/// 啟動排程器主迴圈（tokio::spawn）。`db_path` 為 operoid.db 路徑（殼層解析）。
///
/// `agent_os_enabled` 不在此把關——loop 內每輪自查，讓使用者於設定開關後下次 tick 即生效。
pub fn spawn_loop(
    state: AppState,
    load_cfg: CfgLoader,
    db_path: std::path::PathBuf,
    wake_rx: tokio::sync::mpsc::Receiver<WakeSignal>,
    event_rx: tokio::sync::mpsc::Receiver<InboundEvent>,
) {
    tokio::spawn(scheduler_loop(state, load_cfg, db_path, wake_rx, event_rx));
}

/// 排程器主迴圈。首次 tick 做承諾掃描（啟動喚醒）；其後每次 tick／信號只做 Inbox 掃描。
pub async fn scheduler_loop(
    state: AppState,
    load_cfg: CfgLoader,
    db_path: std::path::PathBuf,
    mut wake_rx: tokio::sync::mpsc::Receiver<WakeSignal>,
    mut event_rx: tokio::sync::mpsc::Receiver<InboundEvent>,
) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    let mut started = false;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if !started {
                    started = true;
                    let _ = scan_commitments(&state, &load_cfg, &db_path).await; // 啟動：承諾驅動喚醒
                }
                let _ = reset_errored(&load_cfg, &db_path).await; // 復原：Error 死巷→重試
                let _ = scan_inbox(&state, &load_cfg, &db_path).await;
            }
            Some(_sig) = wake_rx.recv() => { let _ = scan_inbox(&state, &load_cfg, &db_path).await; }
            Some(ev) = event_rx.recv() => {       // 外部事件（工廠寫入／webhook）
                if let Ok(cfg) = load_cfg() {
                    let _ = event_bus::dispatch_event(&state, &cfg, &db_path, ev).await;
                }
            }
        }
    }
}

/// 復原：把「有待辦工作卻卡在 Error」的員工重設為 Sleeping。
async fn reset_errored(load_cfg: &CfgLoader, db_path: &std::path::Path) -> anyhow::Result<()> {
    let cfg = load_cfg()?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(db_path)?;
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
async fn scan_inbox(
    state: &AppState,
    load_cfg: &CfgLoader,
    db_path: &std::path::Path,
) -> anyhow::Result<()> {
    let cfg = load_cfg()?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(db_path)?;
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
    let permits = state.llm_permits();
    let cfg = &cfg;
    let store = &store;
    let outbound = OutboundConfig {
        url: cfg.event_outbound_url.clone(),
        secret: cfg.event_outbound_secret.clone(),
    };
    let futs = candidates.into_iter().filter_map(|id| {
        let guard = state.try_acquire(&id)?; // 已在跑則跳過
        let permits = Arc::clone(&permits);
        let outbound = outbound.clone();
        Some(async move {
            let _guard = guard; // 釋放於此 future 完成（含錯誤路徑）
            if let Ok((tool, ctx)) = build_tool_ctx(cfg, store, &id) {
                // Reasoner 為可選：有則訊息走對話回合，無則退回 gbrain 單發（守 6c 行為）。
                let reasoner = match build_reasoner(cfg, store, &id, permits) {
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
                if let Err(e) = run_inbox(&id, &tool, rref, &ctx, store, &outbound).await {
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
async fn scan_commitments(
    state: &AppState,
    load_cfg: &CfgLoader,
    db_path: &std::path::Path,
) -> anyhow::Result<()> {
    let cfg = load_cfg()?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(db_path)?;
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
        let state = state.clone();
        let cfg = cfg.clone();
        let db_path = db_path.to_path_buf();
        async move {
            let _ = run_commitments_for_employee(&state, &cfg, &db_path, &id).await;
        }
    });
    future::join_all(futs).await;
    Ok(())
}
