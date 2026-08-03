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
#[derive(Clone)]
pub struct AppState {
    busy: Arc<Mutex<HashSet<String>>>,
    wake_tx: mpsc::Sender<WakeSignal>,
}

impl AppState {
    pub fn new(wake_tx: mpsc::Sender<WakeSignal>) -> Self {
        Self {
            busy: Arc::new(Mutex::new(HashSet::new())),
            wake_tx,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_acquire_serializes_same_employee() {
        let (tx, _rx) = mpsc::channel::<WakeSignal>(8);
        let state = AppState::new(tx);

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
