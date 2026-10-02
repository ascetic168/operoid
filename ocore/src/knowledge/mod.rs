//! Enterprise C′ 知識模組（M1）——Permission-aware Knowledge Fabric 的 ocore 側。
//!
//! 計畫：`docs/Operoid-計畫-EnterpriseC-M1.md`（WP-C3–C6）；決策 D1–D9 見
//! `docs/enterprise-c-architecture.md`；安全不變式 I1–I8 見 `docs/enterprise-c-security-model.md`。
//!
//! M1 分工：
//! - **C3（本節）**：`types`（契約）＋`policy`（純函式授權評估，I3：LLM 不參與）＋
//!   `backend`（檢索後端 trait）＋`fake`（測試用假後端，實作 `pageReadFilter` 語意）。
//!   零生產行為變化——既有路徑不引用本模組。
//! - **C4**：identity（Principal／AccessContext 落地與推導）。
//! - **C5**：scope／policy 的 Store 持久化與 bootstrap。
//! - **C6**：`service`（唯一檢索邊界）＋receipts＋真實 GBrain 接線。
//!
//! 架構定位（D1）：Operoid 構造「授權 source 集合」，GBrain 以 `pageReadFilter`
//! （`source_id` 逐呼叫強制）執行——本模組是構造與執行之間的唯一邊界。

pub mod backend;
pub mod bootstrap;
pub mod fake;
pub mod grants;
pub mod service;
pub mod identity;
pub mod planner;
pub mod policy;
pub mod provision;
pub mod types;

#[cfg(test)]
mod tests_m1;

#[cfg(test)]
mod tests_c7;

#[cfg(test)]
mod tests_c8;

#[cfg(test)]
mod tests_c9;

#[cfg(test)]
mod tests_real;
