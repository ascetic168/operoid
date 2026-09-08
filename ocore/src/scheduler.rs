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
use crate::domain::{Commitment, Employee, EmployeeState, SqliteStore, Store};
use crate::event_bus;
use crate::outbound::OutboundConfig;
use crate::runtime::{
    build_reasoner, build_tool_ctx, run_commitments_for_employee, run_inbox_with_stop,
};

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
                let startup = !started;
                started = true;
                let _ = scan_commitments(&state, &load_cfg, &db_path, startup).await;
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
        .filter(|e| e.state == EmployeeState::Error && !e.archived) // W1：封存者不復原重試
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
            && !e.archived // W1（E13）：封存員工不再被喚醒
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
    let cancel = crate::agent_state::CancelWatch::from_state(state);
    let futs = candidates.into_iter().filter_map(|id| {
        let guard = state.try_acquire(&id)?; // 已在跑則跳過
        let permits = Arc::clone(&permits);
        let outbound = outbound.clone();
        let cancel = cancel.clone();
        Some(async move {
            let _guard = guard; // 釋放於此 future 完成（含錯誤路徑）
            if let Ok((tool, ctx)) = build_tool_ctx(cfg, store, &id) {
                // Reasoner 為可選：有則訊息走對話回合，無則退回 gbrain 單發（守 6c 行為）。
                let reasoner = match build_reasoner(cfg, store, &id, permits, db_path) {
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
                if let Err(e) =
                    run_inbox_with_stop(&id, &tool, rref, &ctx, store, &outbound, &cancel).await
                {
                    eprintln!("[scheduler] run_inbox({id}) failed: {e}");
                }
            }
        })
    });
    future::join_all(futs).await;
    Ok(())
}

/// W2：承諾喚醒候選謂詞。`startup=true`（啟動掃描）：Sleeping＋未封存＋有 Active 承諾
/// 即喚醒（既有語意）。`startup=false`（每 tick）：另需有「退避到期」的承諾——
/// `next_retry_at ≤ now` 且 `retry_count < MAX_COMMITMENT_RETRIES`（健康承諾
/// `next_retry_at=None` 不重跑——只有出錯過的才會被自動再喚醒）。
fn commitment_wake_due(e: &Employee, commitments: &[Commitment], startup: bool) -> bool {
    if e.state != EmployeeState::Sleeping || e.archived || commitments.is_empty() {
        return false;
    }
    if startup {
        return true;
    }
    let now = chrono::Utc::now();
    commitments.iter().any(|c| {
        if c.retry_count >= crate::runtime::MAX_COMMITMENT_RETRIES {
            return false; // 耗盡：待人類
        }
        match c.next_retry_at.as_deref() {
            None => false, // 健康承諾（沒出錯過）：不自動重跑
            Some(t) => match chrono::DateTime::parse_from_rfc3339(t) {
                Ok(dt) => dt.with_timezone(&chrono::Utc) <= now,
                Err(_) => true, // 時間戳解析失敗 fail-safe 視為到期——寧可多跑，不可卡死。
            },
        }
    })
}

/// 承諾掃描：喚醒候選員工，交給 [`run_commitments_for_employee`]（清 Inbox →
/// 對每個 Active commitment 跑 run_autonomous）。啟動／每 tick 的候選差異見
/// [`commitment_wake_due`]（W2／T2 backpressure）。
async fn scan_commitments(
    state: &AppState,
    load_cfg: &CfgLoader,
    db_path: &std::path::Path,
    startup: bool,
) -> anyhow::Result<()> {
    let cfg = load_cfg()?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(db_path)?;
    let mut candidates: Vec<String> = Vec::new();
    for e in store.list_all_employees()? {
        let coms = store.list_active_commitments_by_owner(&e.id)?;
        if commitment_wake_due(&e, &coms, startup) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrainRef, CommitmentStatus};

    fn emp(state: EmployeeState, archived: bool) -> Employee {
        Employee {
            id: "e1".into(),
            workspace_id: "ws".into(),
            name: "E".into(),
            brain: BrainRef { brain_id: "b".into() },
            role: None,
            template_id: None,
            state,
            archived,
            tools: None,
            created_at: "t".into(),
        }
    }

    fn com(retry_count: u32, next_retry_at: Option<String>) -> Commitment {
        Commitment {
            id: "c1".into(),
            workspace_id: "ws".into(),
            owner_employee_id: "e1".into(),
            title: "t".into(),
            completion_condition: "c".into(),
            status: CommitmentStatus::Active,
            retry_count,
            next_retry_at,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn in_future() -> Option<String> {
        Some((chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339())
    }
    fn in_past() -> Option<String> {
        Some((chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339())
    }

    /// W2：候選謂詞——啟動掃描＝有 Active 即喚醒（封存除外）；每 tick＝僅退避到期者。
    #[test]
    fn commitment_wake_due_matrix() {
        let sleeping = emp(EmployeeState::Sleeping, false);
        assert!(commitment_wake_due(&sleeping, &[com(0, None)], true), "啟動：健康承諾也喚醒");
        assert!(!commitment_wake_due(&sleeping, &[com(0, None)], false), "每 tick：健康承諾不重跑");
        assert!(commitment_wake_due(&sleeping, &[com(1, in_past())], false), "退避到期 → 喚醒");
        assert!(!commitment_wake_due(&sleeping, &[com(1, in_future())], false), "退避未到期 → 不喚醒");
        assert!(!commitment_wake_due(&sleeping, &[com(3, in_past())], false), "重試耗盡 → 不喚醒");
        assert!(!commitment_wake_due(&sleeping, &[], true), "無 Active 承諾 → 不喚醒");
        assert!(
            !commitment_wake_due(&emp(EmployeeState::Sleeping, true), &[com(0, None)], true),
            "封存 → 永不喚醒"
        );
        assert!(
            !commitment_wake_due(&emp(EmployeeState::Working, false), &[com(0, None)], true),
            "非 Sleeping → 不喚醒"
        );
        // 員工有多個承諾時，任一到期即喚醒（run_commitments_for_employee 會全部跑）。
        assert!(
            commitment_wake_due(&sleeping, &[com(0, None), com(1, in_past())], false),
            "任一承諾到期即喚醒"
        );
    }
}
