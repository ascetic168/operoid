//! Agent-OS 共享狀態（Phase 6）：每員工 busy-lock（防排程器與指令競態）＋ 喚醒信號 channel。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    /// 工廠寫入（factory_save_authored／factory_write_pages 寫檔完成）。
    FactoryWritten,
    /// 外部訊息（webhook／Email／IM 經 E7 進氣口走此）。
    ExternalMessage,
}

/// 外部事件：工廠寫入、webhook、Email/IM 將來都走此型別（Event 匯流排進氣口）。
///
/// **薄外殼設計**（見 `docs/Operoid-設計-統一事件ingress契約.md`）：契約只載 Operoid 結構上
/// 需要的欄位（路由、去重鍵、時間、來源標籤、短標題）；**來源特有資訊（From／To／Cc／Bcc／
/// @mention／上下文）全部併入 `content`**，由 bridge 序列化、員工（LLM）讀了判斷。不立
/// `sender`／`recipients` 等頂層欄位（BCC／IM 流證明結構式通用欄位是 lowest-common-denominator 陷阱）。
///
/// 路由由 dispatcher（[`crate::event_bus::dispatch_event`]）依 `brain_id`／`employee_id` 決定喚醒誰。
/// 與 domain 層的 [`crate::domain::Event`]（生命週期紀錄 log）不同——此處是「進氣口」的訊號載體。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundEvent {
    pub kind: EventKind,
    /// 來源類型標籤（"email"／"slack"／"factory"／…）。僅供記錄／顯示，Operoid 不驗證。
    pub source: String,
    /// 路由錨點：喚醒**共用此腦的全部員工**（1:N 全喚醒）。bridge 端不需知道員工是誰。
    #[serde(default)]
    pub brain_id: Option<String>,
    /// 直接指定（如 IM 點對點）— 若有則優先於 `brain_id`。
    #[serde(default)]
    pub employee_id: Option<String>,
    /// 短標題（信件主旨／訊息首行／factory slug）— UI 氣泡／事件 log 用；可由 content 派生。
    pub title: String,
    /// ★ 整封原生訊息（bridge 序列化，含 From/To/Cc/Bcc/本文/上下文）。契約層不截斷；
    /// `review_prompt` 餵 LLM 時才截斷。這是 bridge「來源專家」價值所在。
    #[serde(default)]
    pub content: String,
    /// 去重鍵（來源系統的穩定 id：Email Message-Id／IM ts）。Operoid 以 `(source, external_ref)`
    /// 去重；訊息編輯應帶 version（`{id}#v2`）以區分。缺失則無去重（best-effort）。
    #[serde(default)]
    pub external_ref: Option<String>,
    /// RFC3339：事件在來源系統發生的時間（≠ Operoid 收到時間）。
    #[serde(default)]
    pub occurred_at: Option<String>,
    /// ★ 回覆錨點（outbound 路由）：bridge 自訂的不透明字串（如 `email:msg-123`／
    /// `slack:C123:T456`），Operoid 原樣保存、回覆時原樣帶回——bridge 以此定位回覆目標
    /// （Email thread／IM channel+thread），並配合 `source`/`employee_id` 選擇寄件身分。
    /// 純機讀路由欄位，不進 `review_prompt`（content 已持全文，LLM 不需處理路由）。
    /// 缺失則回覆不外發（僅留在 Operoid 對話歷史）。
    #[serde(default)]
    pub reply_to: Option<String>,
    /// factory 分類（meetings/people/companies）。外部來源通常不用（歷史欄位，保留）。
    #[serde(default)]
    pub category: Option<String>,
}

impl InboundEvent {
    /// 格式化成給 reasoner review 的文字——成為對話回合的 user 訊息。
    /// 依 `kind` 分枝：factory 走「知識庫新增內容」框架；外部訊息走「外部訊息」框架。
    /// From/To/Cc/Bcc／時間等都在 `content` 裡（bridge 已格式化），此處不再組裝。
    pub fn review_prompt(&self) -> String {
        const TRAILER: &str =
            "請審閱此內容；若有需要長期追蹤的事項（待辦、決議、行動項目），請提案。";
        match self.kind {
            EventKind::FactoryWritten => {
                let category = self.category.as_deref().unwrap_or("內容");
                format!(
                    "📋 知識庫新增內容（{category}）：〈{}〉。\n{TRAILER}\n\n內容預覽：\n{}",
                    self.title, self.content
                )
            }
            EventKind::ExternalMessage => {
                format!(
                    "📨 外部訊息（{source}）：〈{title}〉\n{TRAILER}\n\n{content}",
                    source = self.source,
                    title = self.title,
                    content = self.content,
                )
            }
        }
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
    /// E7 ingress 去重：已見 `(source, external_ref)`（session 內；重啟清空——bridge 應自追
    /// last-seen 以避免重啟後重推）。超過 `DEDUP_CAP` 時清空（粗略邊界化，避免無限成長）。
    seen_external_refs: Arc<Mutex<HashSet<(String, String)>>>,
}

/// ingress 去重集合上限（超過即清空、重新計算視窗）。v1 粗略邊界化。
const DEDUP_CAP: usize = 8192;

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
            seen_external_refs: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// E7 ingress 去重：若 `(source, external_ref)` 首見則記下並回 `true`；已見回 `false`
    /// （呼叫端應跳過 dispatch）。集合超過 `DEDUP_CAP` 時清空再記（邊界化）。
    pub fn is_new_external_ref(&self, source: &str, external_ref: &str) -> bool {
        let mut g = self.seen_external_refs.lock().expect("dedup lock poisoned");
        if g.len() > DEDUP_CAP {
            g.clear();
        }
        g.insert((source.to_string(), external_ref.to_string()))
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

    /// review_prompt 的 FactoryWritten 分支：含「知識庫新增內容」＋category＋content。
    #[test]
    fn review_prompt_factory_written() {
        let ev = InboundEvent {
            kind: EventKind::FactoryWritten,
            source: "factory".into(),
            brain_id: None,
            employee_id: None,
            title: "e-07-檢討會".into(),
            content: "決議 A、待辦 B。".into(),
            external_ref: None,
            occurred_at: None,
            reply_to: None,
            category: Some("meetings".into()),
        };
        let p = ev.review_prompt();
        assert!(p.contains("知識庫新增內容（meetings）"), "{p}");
        assert!(p.contains("〈e-07-檢討會〉"), "{p}");
        assert!(p.contains("決議 A、待辦 B。"), "{p}");
        assert!(p.contains("請提案"), "{p}");
    }

    /// review_prompt 的 ExternalMessage 分支（E7 進氣口用）：含「外部訊息」＋source＋content。
    /// From/To/Cc/Bcc 等都在 content 裡（bridge 序列化），review_prompt 不另組裝。
    #[test]
    fn review_prompt_external_message() {
        let ev = InboundEvent {
            kind: EventKind::ExternalMessage,
            source: "email".into(),
            brain_id: None,
            employee_id: Some("Steve-TW".into()),
            title: "RE: E-07 良率".into(),
            content: "From: 張雅婷\nTo: 趙建宏\nSubject: RE: E-07\n\n本體...".into(),
            external_ref: Some("<CAB123@mailer>#v1".into()),
            occurred_at: Some("2026-08-14T09:30:00+08:00".into()),
            reply_to: Some("email:msg-CAB123".into()),
            category: None,
        };
        let p = ev.review_prompt();
        assert!(p.contains("外部訊息（email）"), "{p}");
        assert!(p.contains("〈RE: E-07 良率〉"), "{p}");
        assert!(p.contains("From: 張雅婷"), "content 應原樣嵌入: {p}");
        assert!(p.contains("請提案"), "{p}");
        // external_ref／occurred_at 是結構欄位（去重／時間），不應出現在 prompt 文字裡。
        assert!(!p.contains("CAB123"), "external_ref 不應入 prompt: {p}");
    }

    /// InboundEvent 可從 JSON 反序列化（E7 HTTP 進氣口依賴）；缺欄走 default（Option→None）。
    #[test]
    fn inbound_event_deserializes_from_json() {
        let json = r#"{
            "kind": "ExternalMessage",
            "source": "slack",
            "employee_id": "Steve-TW",
            "title": "看一下這個",
            "content": "<@Steve-TW> E-07 的數據怪怪的"
        }"#;
        let ev: InboundEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.kind, EventKind::ExternalMessage);
        assert_eq!(ev.source, "slack");
        assert_eq!(ev.employee_id.as_deref(), Some("Steve-TW"));
        assert_eq!(ev.title, "看一下這個");
        assert!(ev.content.contains("E-07"));
        assert_eq!(ev.brain_id, None, "缺欄應 default 為 None");
        assert_eq!(ev.external_ref, None);
        assert_eq!(ev.reply_to, None, "缺欄應 default 為 None（不外發）");
        assert_eq!(ev.category, None);
    }
}
