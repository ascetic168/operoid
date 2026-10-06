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
#[derive(Clone)]
pub struct ToolCtx {
    pub gbrain_exe: String,
    /// GBRAIN_HOME 值；`None` = 預設腦（~/.gbrain）。
    pub gbrain_home: Option<String>,
    /// 腦的 chat_model（如 `zhipu:glm-5.2`）。`Some` 時 think 子行程加 `--model` 顯式指定，
    /// 避免 gbrain 的 model 解析鏈（`models.think → models.default → $GBRAIN_MODEL → opus`）
    /// fallback 到 anthropic——DB-plane models.* 未設時 synthesis 會找 ANTHROPIC_API_KEY 失敗（E9）。
    pub chat_model: Option<String>,
    /// MCP client（`gbrain serve` stdio；`gbrain_transport = "mcp"` 時注入）。
    /// `None` = 純 CLI 模式。工具實作：MCP 優先、失敗 fallback CLI 子行程。
    pub mcp: Option<std::sync::Arc<crate::gbrain_mcp::GbrainMcpClient>>,
    /// W3（D-H3）：員工工具 allowlist（建構期閘門——僅列於其中的可選工具會被建構；
    /// 空＝預設集 think／search／send）。比對鍵如 [`crate::write_note::TOOL_WRITE_NOTE`]。
    pub allowed_tools: std::collections::HashSet<String>,
    /// W3（D-H2）：員工產出根目錄（write-note 沙箱＝`cfg.employee_output_path`）。
    pub employee_output_root: std::path::PathBuf,
    /// 動作類別登記表（Ch.20 §5；`build_tool_ctx` 載入，propose 分類與圍欄執行查此表）。
    /// `None`＝無登記表——從嚴預設：全部提案走人類核可。
    pub registry: Option<std::sync::Arc<crate::registry::ActionRegistry>>,
    /// C4（D-C4）：檢索授權脈絡——**只由伺服器端構造**（`knowledge::identity` 的兩個
    /// 推導入口；I4：呼叫端不得自稱）。知識檢索的 policy 評估與 receipt 查此欄。
    pub access: crate::knowledge::types::AccessContext,
    /// C6（D1/D7）：知識檢索服務——**生產路徑必為 Some**（`build_tool_ctx` 建構）；
    /// `None` 時內層工具退回 legacy 直接路徑（僅 real_* 測試使用）。檢索一律經
    /// policy→授權集→receipts（I1/I2）。
    pub knowledge: Option<std::sync::Arc<crate::knowledge::service::KnowledgeService>>,
    /// M1：對話回合 tool-loop 的步數保險絲（`AppConfig::turn_max_steps`，預設 40）。
    pub turn_max_steps: u32,
    /// M1：工具結果餵回 LLM 的字元上限（`AppConfig::tool_result_max_chars`，預設 8,000）。
    pub tool_result_max_chars: usize,
}

impl std::fmt::Debug for ToolCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCtx")
            .field("gbrain_exe", &self.gbrain_exe)
            .field("gbrain_home", &self.gbrain_home)
            .field("chat_model", &self.chat_model)
            .field("mcp", &self.mcp.is_some())
            .field("allowed_tools", &self.allowed_tools)
            .field("employee_output_root", &self.employee_output_root)
            .field("registry", &self.registry.is_some())
            .field("access", &self.access.principal_id)
            .field("knowledge", &self.knowledge.is_some())
            .field("turn_max_steps", &self.turn_max_steps)
            .finish()
    }
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

/// `Reasoner::chat_step` 的回傳 future（boxed、Send）——native tool-loop 的一步。
pub type ChatStepFuture<'a> = Pin<
    Box<dyn Future<Output = anyhow::Result<crate::llm::LlmTurn>> + Send + 'a>,
>;

/// 推理器（Handbook Ch.13 §4 修訂）：以 Employee 的 Brain 做通用**推理**——規劃下一步、評估完成條件。
///
/// 與 [`Tool`]（知識檢索，gbrain think）有別：Reasoner 做推理而非檢索（Principle 1：知識≠工作者），
/// 回傳**結構化 JSON**（schema 由呼叫端於 prompt 中約定）以利 Runtime 穩健解析。Runtime 只編排循環
/// 形狀、依 Brain 的判斷決定何時睡眠——內容判斷仍是 Employee 的（Principle 10）。
///
/// M1 雙協議：[`Reasoner::reason`] 為文字 JSON 協議（單發結構化輸出）；[`Reasoner::chat_step`]
/// 為**原生 function calling** 協議（多訊息歷史＋真工具呼叫）。支援何者由
/// [`Reasoner::supports_native_tools`] 表態——預設不支援，故測試 stub 與既有實作零改動。
pub trait Reasoner: Send + Sync {
    fn reason<'a>(&'a self, system: &'a str, user: &'a str) -> ReasonerFuture<'a>;

    /// native tool-loop 的一步：送出多訊息歷史＋工具定義，回模型的文字或工具呼叫。
    /// 僅在 [`Reasoner::supports_native_tools`] 為 true 時會被呼叫。
    fn chat_step<'a>(
        &'a self,
        _messages: &'a [crate::llm::ChatMessage],
        _tools: &'a [crate::llm::ToolDef],
    ) -> ChatStepFuture<'a> {
        Box::pin(async {
            anyhow::bail!("此 Reasoner 不支援原生工具協議（supports_native_tools=false）")
        })
    }

    /// 是否支援原生 function calling（受設定 `llm_protocol` 影響；session 內可降級）。
    fn supports_native_tools(&self) -> bool {
        false
    }
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
