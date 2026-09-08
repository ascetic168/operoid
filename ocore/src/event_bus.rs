//! Event 匯流排（Phase 7c）——外部事件（工廠寫入／webhook／Email-IM 將來）的進氣口與路由。
//!
//! 核心洞察：[`crate::runtime::run_inbox`] 等下游已經是完整的「外部輸入→Task→喚醒→對話→提案」
//! 管道。Event 匯流排不是新發明執行路徑，而是把這個「進氣口」泛化——dispatcher 做的就是
//! 「對每個腦匹配員工，投遞 Message{In}＋Inbox task 並喚醒」。下游零改動。
//!
//! 路由策略：腦 → **全部**共用該腦的員工（1:N 全喚醒）。factory 端不需知道員工是誰；
//! `employee_id` 若有則優先（如 IM 點對點）。成本由 propose-approve 閘門控制後續執行。
//!
//! P1b（2026-08-18）：自 src-tauri 搬入 ocore——不收 AppHandle，改收
//! （state, cfg, db_path）；桌面殼的 app 版包裝見 src-tauri/src/event_bus.rs。

use crate::agent_state::{AppState, InboundEvent, WakeSignal};
use crate::app_config::AppConfig;
use crate::domain::{now_rfc3339, Message, MessageDirection, SqliteStore, Store, Task, TaskStatus};
use crate::runtime::fresh_id;

/// 路由並投遞一則外部事件：依 `employee_id`（優先）或 `brain_id`（全部共用此腦的員工）
/// 決定喚醒誰，對每位目標員工建一筆對話訊息（`Message{In}`）＋ Inbox task（`Assigned`）並喚醒。
///
/// 下游既有機制（`scan_inbox` → `run_inbox` → 對話回合）零改動接手，
/// 事件 review 自動獲得完整的「審閱→回應→提案」能力——proposed commitment 與人類訊息提案
/// 共用同一套 UI（對話氣泡 + InboxView），無新增呈現路徑。
///
/// `objective` 固定為 `"Human message"`：`run_inbox` 的分派只對此 objective + reasoner 走
/// 對話回合（具備 answer/ask/**propose** 能力）。重用之，事件 review 即獲得完整能力。
pub async fn dispatch_event(
    state: &AppState,
    cfg: &AppConfig,
    db_path: &std::path::Path,
    ev: InboundEvent,
) -> anyhow::Result<()> {
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let store = SqliteStore::open(db_path)?;

    // 路由：employee_id 優先；否則 brain_id → 全部共用此腦的員工。
    // W1（E13）：封存員工不收新事件（歷史保留，解封即恢復）。
    let targets = match (&ev.employee_id, &ev.brain_id) {
        (Some(id), _) => store
            .get_employee(id)?
            .filter(|e| !e.archived)
            .into_iter()
            .collect::<Vec<_>>(),
        (_, Some(bid)) => store
            .list_employees_by_brain(bid)?
            .into_iter()
            .filter(|e| !e.archived)
            .collect(),
        _ => {
            eprintln!(
                "[event_bus] 事件〈{}〉無路由資訊（缺 brain_id／employee_id），丟棄",
                ev.title
            );
            return Ok(()); // 無路由資訊，best-effort 丟棄。
        }
    };
    if targets.is_empty() {
        eprintln!(
            "[event_bus] 事件〈{}〉路由命中 0 名員工（source={}）",
            ev.title, ev.source
        );
        return Ok(());
    }

    // E8：員工查詢前先同步圖譜（fire-and-forget；sync 完成前的 race 由 content 全文兜底）。
    maybe_spawn_brain_sync(cfg, ev.brain_id.as_deref(), !targets.is_empty());


    let prompt = ev.review_prompt();
    for emp in &targets {
        let now = now_rfc3339();
        // Message{In}：讓事件 review 出現在該員工的對話歷史（＝天然的 proposal 呈現處）。
        store.put_message(&Message {
            id: fresh_id("ev-in"),
            workspace_id: emp.workspace_id.clone(),
            employee_id: emp.id.clone(),
            direction: MessageDirection::In,
            text: prompt.clone(),
            source_commitment_id: None,
            proposed_commitment_id: None,
            artifact_id: None,
            created_at: now.clone(),
        })?;
        // Task{objective:"Human message"}：走既有對話回合路徑（reasoner + propose）。
        store.put_task(&Task {
            id: fresh_id("ev-task"),
            workspace_id: emp.workspace_id.clone(),
            owner_employee_id: emp.id.clone(),
            objective: "Human message".into(),
            input: prompt.clone(),
            status: TaskStatus::Assigned,
            output_artifact_id: None,
            commitment_id: None,
            project_id: None,
            external_reply_to: ev.reply_to.clone(),
            external_source: Some(ev.source.clone()),
            created_at: now.clone(),
        })?;
        state.wake(WakeSignal {
            employee_id: emp.id.clone(),
            reason: "event".into(),
        });
    }
    eprintln!(
        "[event_bus] 投遞事件〈{}〉給 {} 名員工（source={}）",
        ev.title,
        targets.len(),
        ev.source
    );
    Ok(())
}

/// E8：路由命中且有腦資訊時，fire-and-forget 同步該腦——員工隨後 `knowledge.invoke`
/// 即可查到剛寫入的完整長文（`content` 全文只是 sync 完成前 race 的兜底）。
/// 回傳是否已 spawn（gate 條件供單測斷言；spawn 本體 best-effort，失敗僅記 log）。
fn maybe_spawn_brain_sync(cfg: &AppConfig, brain_id: Option<&str>, has_targets: bool) -> bool {
    let Some(bid) = brain_id else { return false };
    if !has_targets {
        return false;
    }
    let cfg = cfg.clone();
    let bid = bid.to_string();
    tokio::spawn(async move {
        if let Err(e) =
            crate::brains::sync_brain_core(&cfg, &crate::gbrain_cli::noop_sink(), &bid, "all", None)
                .await
        {
            eprintln!("[event_bus] 事件觸發的 brain_sync 失敗（{bid}）：{e}");
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// E8 gate：缺 brain_id 或無路由目標都不觸發；兩者皆備才 spawn。
    /// spawn 本體對不存在的腦會在 config 查找處失敗（brains 為空），僅 eprintln、無副作用。
    #[tokio::test]
    async fn brain_sync_gate_requires_brain_and_targets() {
        let cfg = AppConfig::default();
        assert!(!maybe_spawn_brain_sync(&cfg, None, true), "缺 brain_id 不觸發");
        assert!(
            !maybe_spawn_brain_sync(&cfg, Some("no-such-brain"), false),
            "0 路由目標不觸發"
        );
        assert!(
            maybe_spawn_brain_sync(&cfg, Some("no-such-brain"), true),
            "兩者皆備觸發"
        );
        // 給背景 spawn 時間走完失敗路徑，避免測試行程提早退出的雜訊。
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    /// W1（E13）：封存員工不收事件——dispatch 不投遞 Message／Task（歷史保留、解封恢復）。
    #[tokio::test]
    async fn dispatch_skips_archived_employee() {
        use crate::agent_state::EventKind;
        use crate::domain::{
            Employee, EmployeeState, SqliteStore, Store, Workspace, WorkspaceStatus,
        };
        let dir = std::env::temp_dir().join(format!("operoid-evbus-arch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.db");
        {
            let store = SqliteStore::open(&db).unwrap();
            store
                .put_workspace(&Workspace {
                    id: "ws".into(),
                    name: "W".into(),
                    description: None,
                    status: WorkspaceStatus::Active,
                    created_at: "t".into(),
                })
                .unwrap();
            store
                .put_employee(&Employee {
                    id: "emp-arch".into(),
                    workspace_id: "ws".into(),
                    name: "E".into(),
                    brain: crate::domain::BrainRef { brain_id: "b1".into() },
                    role: None,
                    template_id: None,
                    state: EmployeeState::Sleeping,
                    archived: true,
                    tools: None,
                    created_at: "t".into(),
                })
                .unwrap();
        }
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let (etx, _erx) = tokio::sync::mpsc::channel(8);
        let state = AppState::new(tx, etx, 4);
        let mut cfg = AppConfig::default();
        cfg.agent_os_enabled = true;
        let ev = InboundEvent {
            kind: EventKind::ExternalMessage,
            source: "email".into(),
            brain_id: Some("b1".into()),
            employee_id: None,
            title: "測試".into(),
            content: "本體".into(),
            external_ref: None,
            occurred_at: None,
            reply_to: None,
            category: None,
        };
        dispatch_event(&state, &cfg, &db, ev).await.unwrap();
        let store = SqliteStore::open(&db).unwrap();
        assert!(
            store.list_messages_by_employee("emp-arch", 10).unwrap().is_empty(),
            "封存員工不應收到 Message"
        );
        assert!(
            store.list_assigned_tasks_by_owner("emp-arch").unwrap().is_empty(),
            "封存員工不應收到 Task"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
