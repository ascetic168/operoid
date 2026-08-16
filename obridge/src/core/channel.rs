//! Channel 介面（native 側）——每個通道實例實作之（內建模組與 WASM 外掛的 wrapper 皆同）。
//!
//! 職責對稱於契約兩向：`run_inbound` 產出 [`InboundEvent`]（Operoid ingress 方向）、
//! `send` 消化 [`ocontract::SendPayload`] 的三欄（Operoid send endpoint 分派而來）。
//! Tool 不決策原則的 bridge 版：**通道不決定「喚醒誰」以外的內容判斷**——路由對映是設定，
//! 內容判斷一律留給 Operoid 端的員工。

use ocontract::InboundEvent;
use tokio::sync::mpsc;

/// 一個通道實例。
#[async_trait::async_trait]
pub trait Channel: Send + Sync {
    /// source 標籤（註冊鍵——send 分派依 `SendPayload.source` 反查）。
    fn source(&self) -> &str;
    /// 進氣：持續產出事件（poll 或 webhook 迴圈；實作自行處理間隔與重連）。
    /// 應在迴圈中持續運行直到 process 結束；錯誤記 log、不panic。
    async fn run_inbound(&self, tx: mpsc::Sender<InboundEvent>);
    /// 出氣：送出一則外部訊息。`to` 為錨點（`<source>:msg:...`）或明示目標（自由字串）。
    async fn send(&self, to: &str, employee_id: &str, text: &str) -> anyhow::Result<()>;
}

/// 通道註冊表：source 標籤 → 實例。send endpoint 的分派依據。
#[derive(Default)]
pub struct Registry {
    channels: std::collections::HashMap<String, std::sync::Arc<dyn Channel>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 註冊（source 重複 → Err——啟動期防呆）。
    pub fn register(&mut self, ch: std::sync::Arc<dyn Channel>) -> anyhow::Result<()> {
        let key = ch.source().to_string();
        if self.channels.contains_key(&key) {
            anyhow::bail!("通道 source 標籤重複：{key}");
        }
        self.channels.insert(key, ch);
        Ok(())
    }

    pub fn get(&self, source: &str) -> Option<std::sync::Arc<dyn Channel>> {
        self.channels.get(source).cloned()
    }

    pub fn sources(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }
}
