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
    /// 工具特有參數（E12 tool-choice）：`query/anchor` 是檢索類工具的同義語意，裝不下
    /// 「寄給誰、寄什麼」這類工具專屬輸入——由各 Tool 自行解讀（如 send-external-message
    /// 讀 `to`/`text`）。檢索類工具忽略之。
    pub params: Option<serde_json::Map<String, Value>>,
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
    /// 腦的 chat_model（如 `zhipu:glm-5.2`）。`Some` 時 think 子行程加 `--model` 顯式指定，
    /// 避免 gbrain 的 model 解析鏈（`models.think → models.default → $GBRAIN_MODEL → opus`）
    /// fallback 到 anthropic——DB-plane models.* 未設時 synthesis 會找 ANTHROPIC_API_KEY 失敗（E9）。
    pub chat_model: Option<String>,
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

// ───────────────── Reasoner（推理器，Phase 6b）─────────────────

/// `Reasoner::reason` 的回傳 future（boxed、Send）。
pub type ReasonerFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<Value>> + Send + 'a>>;

/// 推理器（Handbook Ch.13 §4 修訂）：以 Employee 的 Brain 做通用**推理**——規劃下一步、評估完成條件。
///
/// 與 [`Tool`]（知識檢索，gbrain think）有別：Reasoner 做推理而非檢索（Principle 1：知識≠工作者），
/// 回傳**結構化 JSON**（schema 由呼叫端於 prompt 中約定）以利 Runtime 穩健解析。Runtime 只編排循環
/// 形狀、依 Brain 的判斷決定何時睡眠——內容判斷仍是 Employee 的（Principle 10）。
pub trait Reasoner: Send + Sync {
    fn reason<'a>(&'a self, system: &'a str, user: &'a str) -> ReasonerFuture<'a>;
}

/// 從 LLM 的文字回應中萃取首個 JSON 物件（容許 ```json…``` 包裹與前後散文）。
pub fn parse_json_value(raw: &str) -> anyhow::Result<Value> {
    let trimmed = raw.trim();
    let start = trimmed
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("reasoner 回應中找不到 JSON 物件"))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("reasoner 回應中找不到 JSON 物件結尾"))?;
    serde_json::from_str(&trimmed[start..=end])
        .map_err(|e| anyhow::anyhow!("reasoner JSON 解析失敗：{e}"))
}
