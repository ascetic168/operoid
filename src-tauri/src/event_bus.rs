//! Event 匯流排（Phase 7c）——外部事件（工廠寫入／webhook／Email-IM 將來）的進氣口與路由。
//!
//! 核心洞察：[`crate::runtime::agent_send_message`] 已經是完整的「外部輸入→Task→喚醒→對話→提案」
//! 管道。Event 匯流排不是新發明執行路徑，而是把這個「進氣口」泛化——dispatcher 做的就是
//! 「對每個腦匹配員工，重跑一次 `agent_send_message` 的內部邏輯」。下游（`run_inbox` →
//! `run_conversational_turn` → propose → UI）零改動。
//!
//! 路由策略：腦 → **全部**共用該腦的員工（1:N 全喚醒）。factory 端不需知道員工是誰；
//! `employee_id` 若有則優先（如 IM 點對點）。成本由 propose-approve 閘門控制後續執行。

use tauri::{AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, InboundEvent, WakeSignal};
use crate::config::app_config;
use crate::domain::{now_rfc3339, Message, MessageDirection, SqliteStore, Store, Task, TaskStatus};
use crate::runtime::{agent_db_path, fresh_id};

/// 路由並投遞一則外部事件：依 `employee_id`（優先）或 `brain_id`（全部共用此腦的員工）
/// 決定喚醒誰，對每位目標員工建一筆對話訊息（`Message{In}`）＋ Inbox task（`Assigned`）並喚醒。
///
/// 下游既有機制（`scan_inbox` → `run_inbox` → `run_conversational_turn`）零改動接手，
/// 事件 review 自動獲得完整的「審閱→回應→提案」能力——proposed commitment 與人類訊息提案
/// 共用同一套 UI（對話氣泡 + InboxView），無新增呈現路徑。
///
/// `objective` 固定為 `"Human message"`：`run_inbox` 的分派只對此 objective + reasoner 走
/// `run_conversational_turn`（具備 answer/ask/**propose** 能力）。重用之，事件 review 即獲得完整能力。
pub async fn dispatch_event<R: Runtime>(app: &AppHandle<R>, ev: InboundEvent) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    if !cfg.agent_os_enabled {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let store = SqliteStore::open(agent_db_path(app)?)?;

    // 路由：employee_id 優先；否則 brain_id → 全部共用此腦的員工。
    let targets = match (&ev.employee_id, &ev.brain_id) {
        (Some(id), _) => store.get_employee(id)?.into_iter().collect::<Vec<_>>(),
        (_, Some(bid)) => store.list_employees_by_brain(bid)?,
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
        // Task{objective:"Human message"}：走既有 run_conversational_turn 路徑（reasoner + propose）。
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
