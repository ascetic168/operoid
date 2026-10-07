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
    build_monthly_feedback, build_reasoner, build_tool_ctx, load_registry_for_runs, record_event,
    run_commitments_for_employee, run_inbox_with_stop, AGENT_WS,
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
    let mut last_day: Option<String> = None; // R6c：日界偵測（流量預算）
    let mut last_month: Option<String> = None; // R6d：月界偵測（月度回饋）
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let startup = !started;
                started = true;
                let _ = scan_commitments(&state, &load_cfg, &db_path, startup).await;
                let _ = reset_errored(&load_cfg, &db_path).await; // 復原：Error 死巷→重試
                scan_registry_expiry(&db_path); // R3：登記表屆期通知（冪等；效力由查表即時判斷）
                // C10（D-C10b）：grants 屆期掃描（冪等狀態翻轉＋事件；判定的主閘是查詢時即時檢查）。
                expire_grants_due_daily(&db_path);
                // R6c/R6d：日界／月界觸發（事件鍵冪等——重啟不重複記）。
                let today = chrono::Utc::now().date_naive().to_string();
                if last_day.as_deref() != Some(today.as_str()) {
                    scan_registry_budget(&db_path);
                    // C9（D-C9d）：receipts 保留策略——每日清一次（90 天前）。
                    prune_receipts_daily(&db_path);
                    // F2（對話表現力）：events 保留策略——每日清一次（細粒度事件 30 天前）。
                    prune_events_daily(&db_path);
                    last_day = Some(today);
                }
                let this_month = chrono::Utc::now().format("%Y-%m").to_string();
                if last_month.as_deref() != Some(this_month.as_str()) {
                    scan_registry_monthly_feedback(&db_path);
                    last_month = Some(this_month);
                }
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
    let registry = load_registry_for_runs(db_path, store);
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
        let registry = registry.clone();
        Some(async move {
            let _guard = guard; // 釋放於此 future 完成（含錯誤路徑）
            if let Ok((tool, ctx)) = build_tool_ctx(cfg, store, &id, registry, db_path) {
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

/// R3（Ch.20 §5.3 屆期重簽）：登記表屆期掃描——已逾期的自動層類別記 `category_lapsed`
/// 事件提醒人類重簽。**冪等**：同 id＋同 expiry 只記一次。注意：類別的失效**不**依賴
/// 此處（`classify_proposal` 查表即時判斷，R2）——本掃描只做通知，無狀態可漂移。
fn scan_registry_expiry(db_path: &std::path::Path) {
    let Some(data_dir) = db_path.parent() else {
        return;
    };
    let Ok(Some(reg)) = crate::registry::load_registry(data_dir) else {
        return; // 無登記表（或缺檔）→ 無可屆期者
    };
    let now = crate::domain::store::now_rfc3339();
    let lapsed: Vec<(String, String)> = reg
        .categories
        .iter()
        .filter(|c| !matches!(c.tier, crate::registry::DelegationTier::Human))
        .filter(|c| crate::registry::expiry_lapsed(c, &now))
        .map(|c| (c.id.clone(), c.expiry.clone().unwrap_or_default()))
        .collect();
    if lapsed.is_empty() {
        return;
    }
    let Ok(store) = SqliteStore::open(db_path) else {
        return;
    };
    let existing = store.list_events_by_employee("registry", 1000).unwrap_or_default();
    for (id, expiry) in lapsed {
        // 冪等鍵＝類別 id＋expiry 值：重簽（改 expiry）後再次逾期會重新通知。
        let dup = existing.iter().any(|ev| {
            ev.kind == "category_lapsed"
                && ev.detail.contains(&format!("「{id}」"))
                && ev.detail.contains(&expiry)
        });
        if !dup {
            record_event(
                &store,
                AGENT_WS,
                "registry",
                "category_lapsed",
                format!("類別「{id}」已於 {expiry} 逾期，自動啟用失效，待人類重簽（Ch.20 §5.3）"),
            );
        }
    }
}

/// R6c（M4 流量預算）：每日首 tick 統計近 7 天——`proposed` 超過登記表
/// `weekly_proposal_budget` → `budget_exceeded`（白名單過窄警報）；零人類核可但
/// `auto_activated`≥10 → `gate_bypass_warning`（檢查是否被不當繞過）。以週一日期為冪等鍵。
/// C10（D-C10b）：grants 屆期掃描（30s tick；輕量冪等——翻轉即狀態欄變更）。
fn expire_grants_due_daily(db_path: &std::path::Path) {
    match SqliteStore::open(db_path) {
        Ok(store) => {
            if let Err(e) = crate::knowledge::grants::expire_grants_due(&store) {
                eprintln!("[scheduler] expire_grants_due 失敗：{e}");
            }
        }
        Err(e) => eprintln!("[scheduler] expire_grants_due 開庫失敗：{e}"),
    }
}

/// C9（D-C9d）：receipts 保留策略——每日清一次超過保留期的收據（>0 筆時記事件）。
fn prune_receipts_daily(db_path: &std::path::Path) {
    match SqliteStore::open(db_path) {
        Ok(store) => {
            match crate::knowledge::service::prune_receipts(
                &store,
                crate::knowledge::service::RECEIPT_RETENTION_DAYS,
            ) {
                Ok(n) if n > 0 => eprintln!("[scheduler] receipts pruned: {n}"),
                _ => {}
            }
        }
        Err(e) => eprintln!("[scheduler] prune_receipts 開庫失敗：{e}"),
    }
}

/// F2（對話表現力）：events 保留策略——每日清一次超過保留期的細粒度事件（>0 筆時記事件）。
fn prune_events_daily(db_path: &std::path::Path) {
    match SqliteStore::open(db_path) {
        Ok(store) => match crate::runtime::prune_events_daily(&store) {
            Ok(n) if n > 0 => eprintln!("[scheduler] events pruned: {n}"),
            _ => {}
        },
        Err(e) => eprintln!("[scheduler] prune_events 開庫失敗：{e}"),
    }
}

fn scan_registry_budget(db_path: &std::path::Path) {
    let Some(data_dir) = db_path.parent() else {
        return;
    };
    let Ok(Some(reg)) = crate::registry::load_registry(data_dir) else {
        return;
    };
    let Ok(store) = SqliteStore::open(db_path) else {
        return;
    };
    let now_t = chrono::Utc::now();
    let cutoff = now_t - chrono::Duration::days(7);
    let evs = store.list_recent_events(5000).unwrap_or_default();
    let in_window = |kind: &str| {
        evs.iter()
            .filter(|e| e.kind == kind)
            .filter(|e| {
                chrono::DateTime::parse_from_rfc3339(&e.created_at).map_or(false, |t| t >= cutoff)
            })
            .count()
    };
    let proposed = in_window("proposed");
    let auto = in_window("auto_activated");
    let week: String = {
        use chrono::Datelike;
        (now_t.date_naive() - chrono::Duration::days(now_t.weekday().num_days_from_monday() as i64))
            .to_string()
    };
    let registry_evs = store.list_events_by_employee("registry", 1000).unwrap_or_default();
    let already =
        |kind: &str| registry_evs.iter().any(|e| e.kind == kind && e.detail.contains(&week));
    if let Some(budget) = reg.weekly_proposal_budget {
        if proposed > budget as usize && !already("budget_exceeded") {
            record_event(
                &store,
                AGENT_WS,
                "registry",
                "budget_exceeded",
                format!("近 7 天人類核可提案 {proposed} 件超過預算 {budget}——白名單可能過窄（週 {week}）"),
            );
        }
    }
    if proposed == 0 && auto >= 10 && !already("gate_bypass_warning") {
        record_event(
            &store,
            AGENT_WS,
            "registry",
            "gate_bypass_warning",
            format!("近 7 天零人類核可但自動啟用 {auto} 筆——請抽審確認歸類正當（週 {week}）"),
        );
    }
}

/// R6d（M5 回饋閉環）：彙整**上一個月**的核可品質啟發式（放對／放錯候選＋攔截數，
/// 各取一例）→ `monthly_feedback` 事件（冪等：同月只記一次）。啟發式供校準參考——
/// 不做攔對因果判定。
fn scan_registry_monthly_feedback(db_path: &std::path::Path) {
    use chrono::Datelike;
    let Ok(store) = SqliteStore::open(db_path) else {
        return;
    };
    let now = chrono::Utc::now();
    let (year, month) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    let month_key = format!("{year:04}-{month:02}");
    let dup = store
        .list_events_by_employee("registry", 1000)
        .unwrap_or_default()
        .iter()
        .any(|e| e.kind == "monthly_feedback" && e.detail.contains(&month_key));
    if dup {
        return;
    }
    let all = store.list_all_commitments().unwrap_or_default();
    let fb = build_monthly_feedback(&all, year, month);
    record_event(
        &store,
        AGENT_WS,
        "registry",
        "monthly_feedback",
        format!(
            "{month_key} 核可品質（啟發式）：放對候選 {}、放錯候選 {}、攔截 {}；例：放對 {:?}／放錯 {:?}／攔截 {:?}",
            fb.approved_satisfied,
            fb.approved_failed,
            fb.rejected,
            fb.example_satisfied,
            fb.example_failed,
            fb.example_rejected
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrainRef, CommitmentStatus};

    /// R3：屆期掃描——只通知逾期類別（有效類別不通知），且同 id＋同 expiry 冪等。
    #[test]
    fn registry_expiry_scan_notifies_once_per_expiry() {
        let dir = std::env::temp_dir().join(format!(
            "operoid-sched-reg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("operoid.db");
        std::fs::write(
            dir.join("action-registry.json"),
            serde_json::json!({
                "categories": [
                    { "id": "lapsed-cat", "description": "x",
                      "questions": { "reversible": true, "blast_radius": "internal", "accountable": "c" },
                      "tier": "fenced", "expiry": "2020-01-01T00:00:00Z", "evidence": "e" },
                    { "id": "live-cat", "description": "x",
                      "questions": { "reversible": true, "blast_radius": "internal", "accountable": "c" },
                      "tier": "fenced", "expiry": "2099-12-31T00:00:00Z", "evidence": "e" }
                ]
            })
            .to_string(),
        )
        .unwrap();

        scan_registry_expiry(&db);
        {
            let store = SqliteStore::open(&db).unwrap();
            let evs = store.list_events_by_employee("registry", 100).unwrap();
            let lapsed: Vec<_> = evs.iter().filter(|e| e.kind == "category_lapsed").collect();
            assert_eq!(lapsed.len(), 1, "只通知逾期類別（有效類別不通知）");
            assert!(lapsed[0].detail.contains("lapsed-cat"));
        }
        scan_registry_expiry(&db);
        let store = SqliteStore::open(&db).unwrap();
        let count = store
            .list_events_by_employee("registry", 100)
            .unwrap()
            .iter()
            .filter(|e| e.kind == "category_lapsed")
            .count();
        assert_eq!(count, 1, "同 id＋同 expiry 冪等：不重複記");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// R6c：流量預算——proposed 超過 weekly_proposal_budget → budget_exceeded
    /// （週冪等：再掃不重複記）。
    #[test]
    fn registry_budget_scan_flags_over_budget_once() {
        let dir = std::env::temp_dir().join(format!(
            "operoid-sched-budget-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("operoid.db");
        std::fs::write(
            dir.join("action-registry.json"),
            serde_json::json!({ "weekly_proposal_budget": 1, "categories": [] }).to_string(),
        )
        .unwrap();
        {
            let store = SqliteStore::open(&db).unwrap();
            for i in 0..2 {
                record_event(&store, "ws", &format!("e{i}"), "proposed", format!("p{i}"));
            }
        }
        scan_registry_budget(&db);
        let count = || {
            SqliteStore::open(&db)
                .unwrap()
                .list_events_by_employee("registry", 100)
                .unwrap()
                .iter()
                .filter(|e| e.kind == "budget_exceeded")
                .count()
        };
        assert_eq!(count(), 1, "超預算須記警報");
        scan_registry_budget(&db);
        assert_eq!(count(), 1, "同週冪等：不重複記");
        std::fs::remove_dir_all(&dir).ok();
    }

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
            owner_principal: None,
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
            category_id: None,
            gate_reason: None,
            review_pending: false,
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
