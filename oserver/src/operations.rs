//! 長跑操作（op_run／brain sync／bind）的 **ring buffer 主控台**（P4）。
//!
//! 取代 Tauri `Channel<CliLine>` 串流：POST 建操作 → 回 `{operation_id}`；
//! GET 輪詢取增量行（`?since=<n>`）與最終結果。行數上限 `MAX_LINES`（超過丟最舊，
//! `dropped` 記被丟行數——前端如有需要可提示截斷）。操作完成後保留供輪詢，
//! 登記表超過 `MAX_SESSIONS` 時清最舊。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::json;

use ocore::gbrain_cli::{CliLine, OpResult};

/// 每個操作保留的行數上限（ring）。
const MAX_LINES: usize = 4000;
/// 操作登記表上限（超過清最舊已完成者）。
const MAX_SESSIONS: usize = 64;

#[derive(Clone, Serialize)]
pub struct OpSnapshot {
    pub operation_id: String,
    /// `since` 之後的新行（含被 ring 丟棄時的提示行）。
    pub lines: Vec<CliLine>,
    pub done: bool,
    pub result: Option<OpResult>,
    /// 因 ring 上限被丟棄的總行數（診斷用）。
    pub dropped: usize,
}

struct OpSession {
    lines: Vec<CliLine>,
    dropped: usize,
    done: bool,
    result: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct OpRegistry {
    sessions: Mutex<HashMap<String, OpSession>>,
    counter: Mutex<u64>,
}

impl OpRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&self) -> String {
        let mut c = self.counter.lock().expect("op counter");
        *c += 1;
        format!("op-{}", *c)
    }

    /// 建立操作 session，回傳 (id, sink)。sink 把行推入 ring（捕獲 Arc<Self>，無 unsafe）。
    pub fn create(self: &std::sync::Arc<Self>) -> (String, ocore::gbrain_cli::LineSink) {
        let id = self.next_id();
        {
            let mut g = self.sessions.lock().expect("op registry");
            // 邊界化：超過上限時移除最舊的已完成 session（HashMap 無序——以 id 數值近似）。
            if g.len() >= MAX_SESSIONS {
                let done_ids: Vec<String> = g
                    .iter()
                    .filter(|(_, s)| s.done)
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in done_ids.iter().take(g.len() - MAX_SESSIONS + 1) {
                    g.remove(k);
                }
            }
            g.insert(
                id.clone(),
                OpSession { lines: Vec::new(), dropped: 0, done: false, result: None },
            );
        }
        let reg = std::sync::Arc::clone(self);
        let id2 = id.clone();
        let sink: ocore::gbrain_cli::LineSink =
            std::sync::Arc::new(move |line: CliLine| reg.push(&id2, line));
        (id, sink)
    }

    fn push(&self, id: &str, line: CliLine) {
        let mut g = self.sessions.lock().expect("op registry");
        if let Some(s) = g.get_mut(id) {
            if s.lines.len() >= MAX_LINES {
                s.lines.remove(0);
                s.dropped += 1;
            }
            s.lines.push(line);
        }
    }

    /// 完成操作：記結果、標 done。
    pub fn finish(&self, id: &str, result: serde_json::Value) {
        let mut g = self.sessions.lock().expect("op registry");
        if let Some(s) = g.get_mut(id) {
            s.done = true;
            s.result = Some(result);
        }
    }

    /// 失敗完成：記錯誤形狀（前端以 result.success=false 呈現）。
    pub fn finish_err(&self, id: &str, err: &ocore::i18n::AppError) {
        self.finish(id, json!({"success": false, "exit_code": null, "note": null, "error": err}));
    }

    /// 輪詢快照：`since` 之後的新行＋完成狀態。未知 id → None。
    pub fn snapshot(&self, id: &str, since: usize) -> Option<OpSnapshot> {
        let g = self.sessions.lock().expect("op registry");
        let s = g.get(id)?;
        let lines = s.lines.iter().skip(since).cloned().collect();
        Some(OpSnapshot {
            operation_id: id.to_string(),
            lines,
            done: s.done,
            result: s
                .result
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            dropped: s.dropped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_collects_lines_and_finishes() {
        let reg = std::sync::Arc::new(OpRegistry::new());
        let (id, sink) = reg.create();
        sink(CliLine { stream: "stdout".into(), text: "a".into() });
        sink(CliLine { stream: "step".into(), text: "b".into() });
        let snap = reg.snapshot(&id, 0).unwrap();
        assert_eq!(snap.lines.len(), 2);
        assert!(!snap.done);
        // 增量輪詢
        let snap = reg.snapshot(&id, 1).unwrap();
        assert_eq!(snap.lines.len(), 1);
        assert_eq!(snap.lines[0].text, "b");
        reg.finish(&id, json!({"success": true, "exit_code": 0, "note": null}));
        let snap = reg.snapshot(&id, 2).unwrap();
        assert!(snap.done);
        assert!(snap.result.unwrap().success);
        // 未知 id
        assert!(reg.snapshot("nope", 0).is_none());
    }
}
