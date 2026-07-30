//! Tool 抽象（Handbook Ch.08）——capability，永不決策（Principle 5）。
//!
//! 本模組為純 Rust（無 Tauri）。`Tool` trait 只暴露 `invoke`（執行），沒有「是否要動」
//! 的方法——要不要呼叫、呼叫順序，屬 Runtime（`crate::runtime`）的職責。這正是 Principle 5
//! 的結構保證：一個開始替 Employee 做決策的 Tool，就不再是 Tool。
//!
//! 回傳型用 boxed `Send` future（`Pin<Box<dyn Future + Send>>`），故 trait 為 object-safe，
//! Runtime 可用 `&dyn Tool` 傳入，測試塞 `StubTool`、正式塞 `GbrainThinkTool`（二者在
//! `crate::runtime` 實作）。

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool 規格（Ch.08 七件 Spec 的最小起點：先放 id＋描述；Permission/Timeout/Retry 等隨成熟補）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub id: String,
    pub description: String,
}

/// 一次 Tool 呼叫的輸入。
#[derive(Debug, Clone)]
pub struct ToolInput {
    pub query: String,
    /// 可選的錨點（gbrain think `--anchor <slug>`）。
    pub anchor: Option<String>,
}

/// 一次 Tool 呼叫的輸出。`text` 為合成本體，`meta` 為 best-effort 解析的量化指標。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub text: String,
    pub meta: Value,
}

/// Tool 執行脈絡（純資料，無 Tauri）。Phase 1 僅 gbrain 後端，故攜帶 gbrain exe 與
/// 已解析的腦 home（D1）。日後多後端時可演化為列舉／擴充。
#[derive(Debug, Clone)]
pub struct ToolCtx {
    pub gbrain_exe: String,
    /// GBRAIN_HOME 值；`None` = 預設腦（~/.gbrain）。
    pub gbrain_home: Option<String>,
}

/// `Tool::invoke` 的回傳 future（boxed、Send）。
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<ToolOutput>> + Send + 'a>>;

/// 一個 Tool：執行單一明確操作，永不決策。
///
/// `Send + Sync`：讓 `&dyn Tool` 可跨 await（Tauri 指令需 Send future）持有。
pub trait Tool: Send + Sync {
    fn spec(&self) -> &ToolSpec;
    fn invoke<'a>(&'a self, input: ToolInput, ctx: &'a ToolCtx) -> ToolFuture<'a>;
}
