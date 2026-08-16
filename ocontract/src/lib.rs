//! Operoid 共享契約型別（Obridge 計畫，2026-08-16）。
//!
//! Operoid（Tauri app）與 Obridge（外部通道橋接器）之間的 **HTTP JSON 契約**在這裡單一
//! 定義——兩側共享同一份 struct，契約漂移在編譯期被抓（見 `docs/Operoid-設計-統一事件
//! ingress契約.md`）。本 crate **不依賴 Tauri／tokio**（純 serde 型別），Obridge 直接依賴。
//!
//! 兩個方向：
//! - **Inbound**：Obridge → `POST <operoid>/event`，body = [`InboundEvent`]。
//! - **Outbound**：Operoid → `POST <obridge>/send`，body = [`SendPayload`]。

use serde::{Deserialize, Serialize};

/// 外部事件種類（Event 匯流排進氣口）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    /// 工廠寫入（factory_save_authored／factory_write_pages 寫檔完成）。
    FactoryWritten,
    /// 外部訊息（webhook／Email／IM 經 E7 進氣口走此）。
    ExternalMessage,
}

/// 外部事件：工廠寫入、webhook、Email/IM 都走此型別（Event 匯流排進氣口）。
///
/// **薄外殼設計**（見契約文件）：契約只載 Operoid 結構上需要的欄位（路由、去重鍵、時間、
/// 來源標籤、短標題）；**來源特有資訊（From／To／Cc／Bcc／@mention／上下文）全部併入
/// `content`**，由 bridge 序列化、員工（LLM）讀了判斷。不立 `sender`／`recipients` 等頂層
/// 欄位（BCC／IM 流證明結構式通用欄位是 lowest-common-denominator 陷阱）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundEvent {
    pub kind: EventKind,
    /// 來源類型標籤——**Obridge 端為每個通道實例的設定值**（如 "email"、"exchange"）。
    /// 對 Operoid 是不透明標籤，僅供記錄／顯示／send 分派回 Obridge 時反向尋址。
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
    /// ★ 回覆錨點（outbound 路由）：bridge 自訂的不透明字串（如 `email:msg:%3C...%3E`／
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

/// Outbound payload（E12）：Operoid → Obridge `POST /send` 的 body。
///
/// `to` 是**自由字串**——Obridge 的 `email:msg:...` 錨點（回原 thread）或員工明示的新目標
/// （如 email 地址），由 Obridge 依 `source` 找到通道實例後解讀路由（薄外殼：Operoid 不建模
/// 收件人）。`employee_id` 供 Obridge 選寄件身分（各員工不同 From 地址等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPayload {
    /// 通道標籤——Obridge 以此分派給對應通道實例（與 InboundEvent.source 同一語意）。
    pub source: String,
    /// 送達目標（錨點或明示地址）。
    pub to: String,
    /// 發送的員工——Obridge 選寄件身分。
    pub employee_id: String,
    /// 外發全文。
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契約 JSON 往返：InboundEvent 缺欄（serde default）與 SendPayload 序列化形狀。
    #[test]
    fn contract_json_roundtrip() {
        let ev: InboundEvent = serde_json::from_str(
            r#"{"kind":"ExternalMessage","source":"email","title":"測試","content":"From: x\n\n本體"}"#,
        )
        .unwrap();
        assert_eq!(ev.external_ref, None);
        assert_eq!(ev.reply_to, None);
        assert!(ev.review_prompt().contains("外部訊息"));

        let p = SendPayload {
            source: "email".into(),
            to: "email:msg:%3Ca%40b%3E".into(),
            employee_id: "Steve-TW".into(),
            text: "回覆".into(),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["to"], "email:msg:%3Ca%40b%3E");
        let back: SendPayload = serde_json::from_value(v).unwrap();
        assert_eq!(back.employee_id, "Steve-TW");
    }
}
