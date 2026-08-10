//! Domain 模組（Phase 0 地基）——Operoid Agent-OS 核心概念的最小資料模型與持久化。
//!
//! 本模組為純 Rust（不依賴 Tauri），故可單測、且日後可換儲存後端（D2）。
//! Phase 0 不接任何 Tauri 指令／前端——這裡只立骨架＋證明能存。完整的 Runtime
//! （wake/restore/execute/commit/sleep）與 Tool 註冊屬 Phase 1。
//!
//! 與既有 GBrain GUI 的關係：Brain 透過 [`models::BrainRef`] 參照既有
//! [`crate::config::BrainEntry`] 的腦 id（D1：GBrain 為唯一 Knowledge/Brain 後端）。

// Phase 1 起，domain 大部分 API 已由 `runtime` 消費。仍惰性者：Commitment／
// CommitmentStatus（Phase 2）與少數 list_* 方法。Phase 2 接上後可收斂此 allow。
#![allow(dead_code, unused_imports)]

pub mod models;
pub mod sqlite_store;
pub mod store;
pub mod tools;

pub use models::{
    Artifact, ArtifactStatus, BrainRef, Commitment, CommitmentStatus, Employee, EmployeeState,
    EmployeeTemplate, Event, Memory, Message, MessageDirection, Project, ProjectStatus, Task,
    TaskStatus, Timestamp, Workspace, WorkspaceStatus,
};
pub use sqlite_store::SqliteStore;
pub use store::{id_from_name, next_id, now_rfc3339, JsonStore, Store};
pub use tools::{Reasoner, Tool, ToolCtx, ToolInput, ToolOutput, ToolSpec};
