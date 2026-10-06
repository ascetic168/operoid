//! M2：員工的桌面工具組——ToolContract 層＋工作區沙箱＋檔案／shell 工具。
//!
//! 員工模型的四個面（見計畫「設計不變式」）：知識面（GBrain，唯一入口）／通道面
//! （bridge ingress）／**桌面面（本模組——員工自己的工作區：草稿、產出、資料處理）**／
//! 授權面（W3 allowlist：未授權的工具不註冊、員工看不到）。
//!
//! **知識邊界不變式**：工作區是員工的私有沙箱（`employee_output_path` 下），檔案工具
//! 的路徑檢查（[`workspace::resolve_in_workspace`]）就是知識邊界的執行點——工作區外
//! 一律拒讀，檔案工具永不繞過知識授權。`run_command` 屬信任圈工具：allowlist 即人類
//! 授權，工作區 cwd 是慣例邊界、非 OS 級強制（OS 級沙箱是後續工程）。
//!
//! **連接器註冊點**：[`contract::ToolContract`] 是所有對話工具的單一真相源（native
//! ToolDef 由此生成）。未來 FDC／IM 的拉取型連接器以 ToolContract 註冊進來即可接入
//! 同一條執行管線（推進型資料走 bridge ingress，零改動）。

pub mod contract;
pub mod fs_tools;
pub mod shell;
pub mod workspace;
