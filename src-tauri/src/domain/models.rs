//! Domain 型別（Phase 0 地基）——對應 Handbook 核心概念的最小可擴充模型。
//!
//! 設計原則：忠於 Handbook 章節、欄位最小但可擴充。每個型別將來會長，但 Phase 0
//! 只放「身份 ＋ 歸屬 Workspace ＋ 生命週期狀態 ＋ 必要 provenance」。其餘組成
//! （Inbox、Memory、Metrics、Capability/Resources、Task、Event…）一律延後。
//!
//! 慣例對齊：
//! - ID 為 `String`（slug 衍生，見 [`crate::domain::store::id_from_name`]；與既有
//!   `BrainEntry.id: String` 一致）。
//! - 時間戳為 `String`（RFC3339／UTC），沿用專案「時間戳為字串」慣例。
//! - 列舉序列化為小寫（`#[serde(rename_all = "lowercase")]`）以利向前相容。

use serde::{Deserialize, Serialize};

/// RFC3339（UTC）時間戳字串。
pub type Timestamp = String;

// ───────────────── Workspace（Handbook Ch.03）─────────────────

/// Workspace——組織。Emploid 最外層容器；一切事物都隸屬恰一個 Workspace。
///
/// 它是邊界與容器，不是行為者。Phase 0 只攜帶身份、生命週期狀態與建立時間；
/// 組織的設定／政策等留待日後。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: WorkspaceStatus,
    pub created_at: Timestamp,
}

/// Workspace 生命週期狀態（Ch.03 §5）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceStatus {
    Created,
    Active,
    Suspended,
    Archived,
}

// ───────────────── Project（Handbook Ch.09）─────────────────

/// Project——有界的協作倡議。一隊 Employee 在其中並行＋循序合作、產出共享 Artifact，
/// 彼此不互相擁有（Ch.09）。Phase 5 最小：身份＋生命週期狀態。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub status: ProjectStatus,
    pub created_at: Timestamp,
}

/// Project 生命週期狀態（Ch.09，最小）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Completed,
    Archived,
}

// ───────────────── Brain 參照（Handbook Ch.05 / D1）─────────────────

/// Employee 對 Brain 的**參照**（非副本）。員工擁有責任，腦擁有知識；二者不塌縮（Principle 1）。
///
/// Phase 0 唯一後端為 GBrain（D1），故 `brain_id` 對應 [`crate::config::BrainEntry`]
/// 的 `id`（例如 `"__default__"`）。日後成為多後端時可演化為列舉。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainRef {
    pub brain_id: String,
}

// ───────────────── Employee（Handbook Ch.04）─────────────────

/// Employee——Emploid 中**唯一實際做事**的物件（Employee ≡ Agent）。
///
/// Phase 0 先把 Spec/Status 合併成最小集合：身份、所屬 Workspace、腦參照、
/// 角色（佔位）、運行狀態、建立時間。Handbook 的完整 10 屬性（Inbox、Commitments、
/// Memory、Metrics、Capability、Resources…）與 Spec/Status、Template/Instance
/// 的正式切分，留待 Phase 1 起逐步長出。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Employee {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub brain: BrainRef,
    /// 角色定義佔位（完整 Role：mission／authority／KPI／SOP 延後）。
    #[serde(default)]
    pub role: Option<String>,
    /// 溯源：由哪個 [`EmployeeTemplate`] 部署而來（None＝獨立建立，如 `agent_seed`／`agent_recruit`）。
    #[serde(default)]
    pub template_id: Option<String>,
    /// 運行狀態。Phase 0 僅持久化，不驅動行為（Runtime 在 Phase 1）。
    pub state: EmployeeState,
    pub created_at: Timestamp,
}

/// Employee 運行狀態（Ch.04 §5.1）。Sleep 為預設休息態。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmployeeState {
    Created,
    Idle,
    Working,
    Waiting,
    Sleeping,
    Paused,
    Error,
}

// ───────────────── EmployeeTemplate（Handbook Ch.04 §7）─────────────────

/// EmployeeTemplate——一種員工的**可重用定義**（Ch.04 §7 Template）。
///
/// 部署（deploy）成多個獨立 [`Employee`] Instance：Instance 抄襲 template 的 `brain`／`role`，
/// 但各自擁有 inbox／commitment／memory／artifact（即各自的 `Employee`）。腦透過 `brain_id`
/// 仍是 live 共享（Phase 3）；role 為部署快照（Phase 4 不做即時傳播）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmployeeTemplate {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub brain: BrainRef,
    #[serde(default)]
    pub role: Option<String>,
    pub created_at: Timestamp,
}

// ───────────────── Artifact（Handbook Ch.06）─────────────────

/// Artifact——工作的耐久產出，Workspace 擁有的 first-class 公民。
///
/// 具身份、provenance、版本。Phase 0 內容為純文字；型別為自由字串。
/// 「工作未產出 Artifact 即未完成」原則的載體（vigilant／trivial 例外見 Ch.06 §2）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    /// 產出型別（report／analysis／code…；先自由字串，日後再列舉化）。
    pub artifact_type: String,
    /// 文字內容（Phase 0）。
    pub content: String,
    /// 產出者 Employee id（provenance）。
    pub produced_by: String,
    /// 產出此 Artifact 的 Task id（provenance）。
    #[serde(default)]
    pub source_task_id: Option<String>,
    /// 此 Artifact 服務的 Commitment id（provenance）。
    #[serde(default)]
    pub source_commitment_id: Option<String>,
    /// 前一版本 id（修訂鏈；原版為 None）。
    #[serde(default)]
    pub revised_from_id: Option<String>,
    /// 所屬 Project（共享 Artifact 的歸屬；Ch.09）。
    #[serde(default)]
    pub project_id: Option<String>,
    pub version: u32,
    pub status: ArtifactStatus,
    pub created_at: Timestamp,
}

/// Artifact 生命週期狀態（Ch.06 §6）。Draft→Committed 為「工作成真」的瞬間。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactStatus {
    Draft,
    Committed,
    Revised,
    Superseded,
    Archived,
}

// ───────────────── Commitment（Handbook Ch.11）─────────────────

/// Commitment——持久責任。相對於短命的 Task，它橫跨數日／數週，直到完成條件滿足。
///
/// Phase 0 攜帶：所屬 Workspace、負責 Employee、完成條件、狀態。其衍生的 Tasks、
/// 歷史、關聯（Artifact／Project）延後。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Commitment {
    pub id: String,
    pub workspace_id: String,
    /// 對此 Commitment 負責的 Employee id（「誰擁有它」）。
    pub owner_employee_id: String,
    pub title: String,
    /// 完成條件——done 的定義。
    pub completion_condition: String,
    pub status: CommitmentStatus,
    pub created_at: Timestamp,
    /// 最近一次活動時間（task 產生／狀態變更時更新）。
    #[serde(default)]
    pub updated_at: Timestamp,
}

/// Commitment 生命週期狀態（Ch.11 §5）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommitmentStatus {
    Proposed,
    Created,
    Active,
    Suspended,
    Satisfied,
    Rejected,
    Archived,
}

// ───────────────── Task（Handbook Ch.10）─────────────────

/// Task——一個可執行目標，Employee 的最小工作單位。短命：進 Inbox、做完即結束。
///
/// Phase 1：循環把传入的 query 當作一個 Task（Created→Completed）以示範生命週期；
/// 真正的 Inbox 排程、Waiting/Failed 分支隨 Runtime 成熟再加。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub workspace_id: String,
    /// 負責執行的 Employee id。
    pub owner_employee_id: String,
    /// 目標描述（要完成什麼）。
    pub objective: String,
    /// 輸入（給 Tool 的原始 query 等）。
    pub input: String,
    pub status: TaskStatus,
    /// 完成時產出的 Artifact id（provenance）。
    #[serde(default)]
    pub output_artifact_id: Option<String>,
    /// 所屬 Commitment（linkage，Ch.10）；獨立 task 為 None。
    #[serde(default)]
    pub commitment_id: Option<String>,
    /// 所屬 Project（Ch.09）；獨立 task 為 None。
    #[serde(default)]
    pub project_id: Option<String>,
    pub created_at: Timestamp,
}

/// Task 生命週期狀態（Ch.10 §5）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Created,
    Assigned,
    InProgress,
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

// ───────────────── Memory（Handbook Ch.15）─────────────────

/// Memory——單一 Employee 的**工作記憶**（scratchpad），與 Knowledge／Brain 長期記憶有別。
///
/// Wake 時還原、Sleep 時持久；bounded、會輪替。Phase 1 最小：一串 notes ＋上次產出的
/// artifact 指標。以 `employee_id` 為鍵（每 Employee 一份）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Memory {
    pub employee_id: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub last_artifact_id: Option<String>,
    pub updated_at: Timestamp,
}

// ───────────────── Event（Handbook Ch.14，Phase 6d 輕量）─────────────────

/// Event——某件事已發生的**不可變紀錄**（Ch.14）。Phase 6d 輕量落地：append-only（只 INSERT），
/// 記生命週期大事（`wake`／`sleep`／`stalled`／`satisfied`／`errored`），供監看顯示歷程。
/// 完整 Ch.14（event sourcing／串流／被 Trigger 消費／保留政策）列未來。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub workspace_id: String,
    pub employee_id: String,
    /// 事件類別（自由字串，常見見上）。
    pub kind: String,
    pub detail: String,
    pub created_at: Timestamp,
}

// ───────────────── Message（Handbook Ch.16，Phase 7b）─────────────────

/// Message 方向：In＝人類→員工、Out＝員工→人類。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageDirection {
    In,
    Out,
}

/// Message——人類與 Employee 一趟對話往返的紀錄（Ch.16）。**互動層，非工作產出**——
/// 耐久結果仍是 Artifact／Commitment；Message 承載往返本身，供對話頁回顧。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub workspace_id: String,
    pub employee_id: String,
    pub direction: MessageDirection,
    pub text: String,
    /// 此趟往返所屬的 Commitment（可選）。
    #[serde(default)]
    pub commitment_id: Option<String>,
    /// 員工回覆附帶的實質產出 Artifact（可選；Out 才有）。
    #[serde(default)]
    pub artifact_id: Option<String>,
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_serde_roundtrip() {
        let ws = Workspace {
            id: "acme".into(),
            name: "Acme".into(),
            description: Some("demo".into()),
            status: WorkspaceStatus::Created,
            created_at: "2026-07-30T00:00:00+00:00".into(),
        };
        let v = serde_json::to_value(&ws).unwrap();
        let back: Workspace = serde_json::from_value(v).unwrap();
        assert_eq!(back, ws);
        // 選擇性欄位缺省時仍可還原（向前相容）
        let json = serde_json::json!({
            "id": "x", "name": "X", "status": "active", "created_at": "t"
        });
        let ws2: Workspace = serde_json::from_value(json).unwrap();
        assert_eq!(ws2.description, None);
    }

    #[test]
    fn enums_serialize_lowercase() {
        assert_eq!(
            serde_json::to_string(&WorkspaceStatus::Suspended).unwrap(),
            "\"suspended\""
        );
        assert_eq!(
            serde_json::to_string(&EmployeeState::Sleeping).unwrap(),
            "\"sleeping\""
        );
        assert_eq!(
            serde_json::to_string(&ArtifactStatus::Committed).unwrap(),
            "\"committed\""
        );
        assert_eq!(
            serde_json::to_string(&CommitmentStatus::Satisfied).unwrap(),
            "\"satisfied\""
        );
    }

    #[test]
    fn employee_with_brain_ref_roundtrip() {
        let emp = Employee {
            id: "procurement-steve".into(),
            workspace_id: "acme".into(),
            name: "Steve".into(),
            brain: BrainRef {
                brain_id: "__default__".into(),
            },
            role: None,
            template_id: None,
            state: EmployeeState::Created,
            created_at: "t".into(),
        };
        let v = serde_json::to_value(&emp).unwrap();
        let back: Employee = serde_json::from_value(v).unwrap();
        assert_eq!(back, emp);
        assert_eq!(back.brain.brain_id, "__default__");
    }

    #[test]
    fn artifact_and_commitment_roundtrip() {
        let art = Artifact {
            id: "a1".into(),
            workspace_id: "acme".into(),
            title: "Report".into(),
            artifact_type: "report".into(),
            content: "body".into(),
            produced_by: "procurement-steve".into(),
            source_task_id: None,
            source_commitment_id: None,
            revised_from_id: None,
            project_id: None,
            version: 1,
            status: ArtifactStatus::Draft,
            created_at: "t".into(),
        };
        let v = serde_json::to_value(&art).unwrap();
        assert_eq!(serde_json::from_value::<Artifact>(v).unwrap(), art);

        let com = Commitment {
            id: "c1".into(),
            workspace_id: "acme".into(),
            owner_employee_id: "procurement-steve".into(),
            title: "Track PO".into(),
            completion_condition: "goods received".into(),
            status: CommitmentStatus::Active,
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        let v = serde_json::to_value(&com).unwrap();
        assert_eq!(serde_json::from_value::<Commitment>(v).unwrap(), com);
    }
}
