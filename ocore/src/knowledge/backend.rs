//! 檢索後端抽象（M1-WP-C3）——KnowledgeService 與 GBrain 之間的最後一層。
//!
//! 契約要點（M0 驗證 V1/V2/V5 的結論，2026-10-02）：
//! - GBrain 的 `source_id` 過濾**逐呼叫只吃純量**（MCP `tools/call`、CLI `--source`、
//!   `gbrain call` 三面一致；陣列 `sourceIds` 只存在 engine 內部、無 caller 面）。
//!   因此 [`BackendQuery`] 攜帶授權集合，實作端**逐 source 呼叫後合併**。
//! - `think` op 不接受任何 source 參數（範圍屬行程級 OperationContext）——M1 的
//!   [`RetrieveKind::Think`] 由實作端導向 `query`（員工工具描述不變；D2/I3：檢索什麼
//!   由 policy 決定，與工具種類無關）。
//! - 引擎內部另有 `sourceIds` 陣列路徑（HTTP OAuth transport 的 `allowedSources`），
//!   未來遠端部署（Q9）可單呼叫跨 source——M1 不使用。

use super::types::AccessContext;
use crate::domain::tools::ToolFuture;
use serde::Serialize;

/// 檢索種類（員工動作語意；M1 兩者同樣導向 GBrain `query`——V2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrieveKind {
    Think,
    Search,
}

/// 一次授權檢索的後端請求。`source_ids` 必為 **policy 推導的授權集合**（I1/I4）——
/// 空集合表示拒絕，實作端不得發出任何呼叫。
#[derive(Debug, Clone)]
pub struct BackendQuery {
    pub kind: RetrieveKind,
    pub query: String,
    pub anchor: Option<String>,
    /// 授權 source 集合（`knowledge::policy::authorized_sources` 的產物）。
    pub source_ids: Vec<String>,
    pub limit: u32,
}

/// 檢索後端：把「授權集合」轉成一或多個帶 `source_id` 的 GBrain 呼叫。
/// 正式實作（MCP／CLI）屬 C6；[`super::fake::FakeBackend`] 供測試。
pub trait KnowledgeBackend: Send + Sync {
    fn retrieve<'a>(
        &'a self,
        access: &'a AccessContext,
        req: BackendQuery,
        ctx: &'a crate::domain::tools::ToolCtx,
    ) -> ToolFuture<'a>;
}
