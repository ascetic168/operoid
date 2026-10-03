//! 知識授權的型別契約（M1-WP-C3）。
//!
//! 提示詞 §4 草案的 M1 最小集：`Principal`／`AccessContext`／`KnowledgeScope`／
//! `KnowledgePolicy`。慣例比照登記表（serde snake_case、`#[serde(default)]` 友善）。
//! 刻意**不**包含 delegation／expires_at（提示詞 §4.1 的臨時授權面）——屬 WP-C10。

use serde::{Deserialize, Serialize};

/// Principal 類型（D6）。`Service` 保留型別、M1 不發實體（Q6 裁決）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    Human,
    AiEmployee,
    Service,
}

/// Principal——「這是誰」。M1 的兩個實體來源：`principal-operator`（bootstrap human）
/// 與 `ai:{employee_id}`（由 Employee 推導，讀時構造、不儲存——Q6）。
/// C7：`attrs` 為管理員可賦的授權屬性（疊加進 AccessContext；JSON blob 零遷移）。
/// R2（遠端化，C12b 最小版）：帳號密碼欄位（login_name/password_hash/disabled/
/// must_change_password）與 token 生命週期（見 [`PrincipalToken`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub principal_type: PrincipalType,
    #[serde(default)]
    pub employee_id: Option<String>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub attrs: PrincipalAttrs,
    /// C12a 舊制：單一 token 的 SHA-256 hex。**R2 起不再使用**（改 `principal_tokens`
    /// 表——一 principal 多 token）；欄位保留作舊 JSON 相容，舊 token 一律重簽。
    #[serde(default)]
    pub token_hash: Option<String>,
    /// R2：帳號密碼登入名（唯一）；None＝非帳號身份（operator／ai_employee）。
    #[serde(default)]
    pub login_name: Option<String>,
    /// R2：Argon2id PHC 字串（明文永不落 store）；None＝未設密碼（不可密碼登入）。
    #[serde(default)]
    pub password_hash: Option<String>,
    /// R2：停用旗標——停用即登入被拒＋全部 token 撤銷（稽核留痕）。
    #[serde(default)]
    pub disabled: bool,
    /// R2：臨時密碼旗標——true 時除改密碼／登出外，全部 API 回 403 auth.mustChangePassword。
    #[serde(default)]
    pub must_change_password: bool,
}

/// R2（遠端化缺口 8）：principal 的 API token 記錄——**一 principal 多 token**
/// （裝置／session 粒度）、可到期（TTL）、可逐 token 撤銷。明文只在簽發時回傳一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalToken {
    /// "tok-<16hex>"。
    pub id: String,
    pub principal_id: String,
    /// SHA-256 hex（C12a 沿用；明文比對永不出現在 store）。
    pub token_hash: String,
    /// RFC3339。
    pub created_at: String,
    /// RFC3339；None＝不過期（服務間長期 token）。
    #[serde(default)]
    pub expires_at: Option<String>,
    /// RFC3339；authn 時節流更新（60s 內不重寫）。
    #[serde(default)]
    pub last_used_at: Option<String>,
    /// 裝置／session 標記（如 "web-admin"、"obridge"）。
    #[serde(default)]
    pub label: Option<String>,
}

/// Principal 的授權屬性（C7b）——policy 的 principal 側條件（departments/projects）
/// 的資料來源。roles 條件隨 C12 的角色實體啟用（欄位先備）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalAttrs {
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub departments: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    /// C13a：保密等級clearance（None＝未賦，評估時視為 Internal——I9 天花板）。
    #[serde(default)]
    pub clearance: Option<SecurityLevel>,
}

/// 一次檢索（或任何知識存取）的授權脈絡——**只由伺服器端構造**（I4：呼叫端不得自稱）。
///
/// `roles`/`departments`/`projects` M1 恆空：屬性條件與正規化屬 WP-C7+（D2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessContext {
    pub principal_id: String,
    pub principal_type: PrincipalType,
    #[serde(default)]
    pub employee_id: Option<String>,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub departments: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    /// 發起檢索的任務（提示詞 §6 receipt 的 Who→Task 鏈；M1 僅記錄、不參與評估）。
    #[serde(default)]
    pub task_id: Option<String>,
    /// 檢索目的（receipt 記錄用）。
    #[serde(default)]
    pub purpose: Option<String>,
    /// C13a（I9）：保密天花板——None＝Internal（bootstrap 基準線）。
    #[serde(default)]
    pub clearance: Option<SecurityLevel>,
}

/// Scope 可見性（D1 source 分區模型）。`Personal` 刻意缺席——個人腦是獨立
/// `GBRAIN_HOME` 物理分區，不進企業授權語意（I6／提示詞 §19）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Company,
    Department,
    Project,
    Restricted,
}

/// 知識範圍——**Operoid 端的 scope→source 映射**（D3：權威在 Operoid）。
/// GBrain 只見 `source_ids`；scope 語意（部門/專案/受限）是 Operoid 的資產。
/// C7：`department`/`project` 支撐成員制條件（`department_membership` 旗標）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeScope {
    pub id: String,
    pub visibility: Visibility,
    /// 保密等級（C13a：全序 SecurityLevel；一 source 一等級——D4；default＝Internal）。
    #[serde(default = "default_security_level")]
    pub classification: SecurityLevel,
    /// 對應的 GBrain source id（`sources.id`，`[a-z0-9-]{1,32}`）。
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
    /// 部門歸屬（`department_membership` 成員制條件的資源側）。
    #[serde(default)]
    pub department: Option<String>,
    /// 專案歸屬（`project_membership` 成員制條件的資源側）。
    #[serde(default)]
    pub project: Option<String>,
}

/// 規則效果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Allow,
    Deny,
}

/// 一條授權規則。條件皆 `None`／`false`＝不限；`Some(list)`＝白名單。
/// C7 條件面（D-C7a）：principals/principal_types/scopes（既有）＋
/// departments/projects（**principal 側**交集）、classifications（**資源側**）、
/// department_membership/project_membership（成員制旗標）。roles 條件隨 C12。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    /// 越小越先評估（先到先得：DENY 命中即拒、ALLOW 命中即准）。
    pub priority: i64,
    pub effect: Effect,
    #[serde(default)]
    pub principals: Option<Vec<String>>,
    #[serde(default)]
    pub principal_types: Option<Vec<PrincipalType>>,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    /// principal 側：`ctx.departments ∩ 自身 ≠ ∅` 才命中。
    #[serde(default)]
    pub departments: Option<Vec<String>>,
    /// principal 側：`ctx.projects ∩ 自身 ≠ ∅` 才命中。
    #[serde(default)]
    pub projects: Option<Vec<String>>,
    /// 資源側：`scope.classification ∈ 自身` 才命中（C13a：等級枚舉）。
    #[serde(default)]
    pub classifications: Option<Vec<SecurityLevel>>,
    /// 成員制：要求 `scope.department ∈ ctx.departments`（scope 無 department 時不命中）。
    #[serde(default)]
    pub department_membership: bool,
    /// 成員制：要求 `scope.project ∈ ctx.projects`（scope 無 project 時不命中）。
    #[serde(default)]
    pub project_membership: bool,
}

/// 知識政策——**權威在 Operoid**（D3），singleton 版本化儲存（C5）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgePolicy {
    pub version: u32,
    pub rules: Vec<PolicyRule>,
}

/// C13a（D-C13a）：保密等級——全序，宣告序即強度序（派生 Ord）。
/// I9：`scope.classification > ctx.clearance` → 硬拒（grant 不可破）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Public,
    Internal,
    Confidential,
    Secret,
}

/// serde default：等級未標注＝Internal（bootstrap 基準線）。
pub fn default_security_level() -> SecurityLevel {
    SecurityLevel::Internal
}

/// 評估結論。`Deny` 帶原因（供 receipt／事件；對員工回中性文字——Rule 9）。
/// C10：`Deny.reason` 語意升級——`denied_by_rule:*`＝explicit deny（grant 不可破）；
/// `default_deny`＝無匹配（valid grant 可破——D-C10a 三段式）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny { reason: String },
}

/// C10（D5）：臨時授權狀態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantState {
    Active,
    Revoked,
    Expired,
}

/// C10（D5／提示詞 §5）：任務級臨時知識授權——principal×scope×TTL。
/// 有效判定**查詢時即時**（expires_at；D9 無快取）；`task_id`/`purpose` 為稽核關聯。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGrant {
    pub id: String,
    pub principal_id: String,
    /// 授予的 scope（grant 破的是 default deny——explicit deny 仍優先，D-C10a）。
    pub scope_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    /// RFC3339 屆期（此時刻前有效）。
    pub expires_at: String,
    pub state: GrantState,
    pub created_at: String,
}
