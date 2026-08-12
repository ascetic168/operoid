//! Agent-OS 共享狀態（Phase 6）：每員工 busy-lock（防排程器與指令競態）＋ 喚醒信號 channel。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::mpsc;

/// 喚醒信號：交付給 Runtime 排程器，指示「這個員工有工作來了」。
///
/// 來源（Handbook Ch.12 Trigger 類型）：Message-driven（人類訊息）、Manual（交接／指令）、
/// Event-driven（啟動掃描）。`reason` 僅供觀測／記錄用。
#[derive(Debug, Clone, Serialize)]
pub struct WakeSignal {
    pub employee_id: String,
    pub reason: String,
}

/// 外部事件種類（Event 匯流排進氣口，Phase 7c）。
#[derive(Debug, Clone, Serialize)]
pub enum EventKind {
    /// 工廠寫入（factory_save_authored／factory_write_pages 寫檔完成）。
    FactoryWritten,
    /// 外部訊息（webhook／Email／IM 將來走此——Phase 2 webhook server）。
    ExternalMessage,
}

/// 外部事件：工廠寫入、webhook、Email/IM 將來都走此型別（Phase 7c Event 匯流排）。
///
/// 路由由 dispatcher（[`crate::event_bus::dispatch_event`]）依 `brain_id`／`employee_id` 決定喚醒誰。
/// 與 domain 層的 [`crate::domain::Event`]（生命週期紀錄 log）不同——此處是「進氣口」的訊號載體。
#[derive(Debug, Clone, Serialize)]
pub struct InboundEvent {
    pub kind: EventKind,
    /// 路由錨點：喚醒**共用此腦的全部員工**（1:N 全喚醒）。factory 端不需知道員工是誰。
    pub brain_id: Option<String>,
    /// 直接指定（如 IM 點對點）— 若有則優先於 `brain_id`。
    pub employee_id: Option<String>,
    /// 內容分類（meetings / people / companies / …）。
    pub category: Option<String>,
    /// 標題（如會議記錄的 slug／檔名）。
    pub title: String,
    /// 給員工 review 的內容摘要／預覽（review task 的 input 直接含此文字）。
    pub summary: String,
    /// provenance（factory / webhook / …）。
    pub source: String,
}

impl InboundEvent {
    /// 格式化成給 reasoner review 的文字——成為對話回合的 user 訊息。
    pub fn review_prompt(&self) -> String {
        let category = self.category.as_deref().unwrap_or("內容");
        format!(
            "📋 知識庫新增內容（{category}）：〈{}〉。\n\
             請審閱此內容；若有需要長期追蹤的事項（待辦、決議、行動項目），請提案。\n\n\
             內容預覽：\n{}",
            self.title, self.summary
        )
    }
}

/// 員工忙碌守衛：RAII——drop 時自動從 `busy` 集合移除該員工，釋放占用（含錯誤路徑）。
pub struct BusyGuard {
    busy: Arc<Mutex<HashSet<String>>>,
    id: String,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        if let Ok(mut g) = self.busy.lock() {
            g.remove(&self.id);
        }
    }
}

/// Agent-OS 共享狀態（`app.manage` 注入，commands 與排程器共用同一份）。
///
/// `busy` 記錄「正在執行」的 employee_ids——`agent_run`／`agent_run_task`／排程器喚醒都須先
/// `try_acquire`，已占用則拒絕（`agent_os.employeeBusy`），避免同一員工被並發執行造成
/// 狀態／記憶／任務競態（Phase 6 最關鍵的安全網）。`wake_tx` 把喚醒信號推給排程器。
/// `event_tx` 把外部事件推給排程器（Phase 7c Event 匯流排）。`llm_permits` 為全域 LLM 並發節流。
#[derive(Clone)]
pub struct AppState {
    busy: Arc<Mutex<HashSet<String>>>,
    wake_tx: mpsc::Sender<WakeSignal>,
    event_tx: mpsc::Sender<InboundEvent>,
    llm_permits: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    pub fn new(
        wake_tx: mpsc::Sender<WakeSignal>,
        event_tx: mpsc::Sender<InboundEvent>,
        permits_count: usize,
    ) -> Self {
        Self {
            busy: Arc::new(Mutex::new(HashSet::new())),
            wake_tx,
            event_tx,
            llm_permits: Arc::new(tokio::sync::Semaphore::new(permits_count.max(1))),
        }
    }

    /// 嘗試占用此員工；已在忙則回 `None`（呼叫端轉成 `agent_os.employeeBusy` 錯誤）。
    pub fn try_acquire(&self, employee_id: &str) -> Option<BusyGuard> {
        let mut g = self.busy.lock().expect("busy lock poisoned");
        if g.contains(employee_id) {
            return None;
        }
        g.insert(employee_id.to_string());
        Some(BusyGuard {
            busy: Arc::clone(&self.busy),
            id: employee_id.to_string(),
        })
    }

    /// 推一則喚醒信號給排程器（best-effort：channel 滿則丟棄，下次 30s tick 仍會掃到）。
    pub fn wake(&self, signal: WakeSignal) {
        let _ = self.wake_tx.try_send(signal);
    }

    /// 推一則外部事件給排程器（best-effort：channel 滿則丟棄）。sync `try_send`，可在 sync fn 內呼叫
    /// （如 `factory_write_pages` 寫檔後 emit）。下游 `dispatch_event` 路由後喚醒腦匹配的員工。
    pub fn emit(&self, ev: InboundEvent) {
        let _ = self.event_tx.try_send(ev);
    }

    /// 全域 LLM 並發 permit（節流「全部喚醒」的尖峰並發 LLM 呼叫；permit 滿則自動排隊等待）。
    pub fn llm_permits(&self) -> Arc<tokio::sync::Semaphore> {
        Arc::clone(&self.llm_permits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_acquire_serializes_same_employee() {
        let (tx, _rx) = mpsc::channel::<WakeSignal>(8);
        let (etx, _erx) = mpsc::channel::<InboundEvent>(8);
        let state = AppState::new(tx, etx, 4);

        let g1 = state.try_acquire("emp-1");
        assert!(g1.is_some(), "首次占用應成功");
        // 同一員工再次占用 → None（競態被擋下）
        assert!(state.try_acquire("emp-1").is_none(), "已在忙應拒絕");
        // 另一員工仍可占用
        assert!(state.try_acquire("emp-2").is_some());

        drop(g1); // 釋放 emp-1
        assert!(state.try_acquire("emp-1").is_some(), "釋放後可再次占用");
    }
}
