//! `ocore` — Operoid 核心域 crate（前後端分離計畫 P1a，2026-08-18）。
//!
//! 從 `src-tauri` 抽出的**純 Rust** 後端邏輯：domain 資料模型與 Store、agent 狀態、
//! LLM 呼叫、outbound（外發 Tool）、GBrain config 解析、i18n（AppError/L10n）、slug。
//! **零 Tauri 依賴**——桌面殼（src-tauri）與未來的服務 binary（oserver）共用此 crate。
//!
//! 搬遷原則（計畫 `docs/Operoid-計畫-前後端分離.md` P1）：純重構、GUI 行為零變、
//! 測試隨模組搬遷。`src-tauri` 以 `pub use ocore::<mod>;` 保持既有 `crate::<mod>::…`
//! 路徑零改動。

pub mod agent_state;
pub mod runtime;
pub mod scheduler;
pub mod app_config;
pub mod domain;
pub mod event_bus;
pub mod gbrain_config;
pub mod i18n;
pub mod llm;
pub mod outbound;
pub mod proc;
pub mod slug;
