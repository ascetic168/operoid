//! 工作區沙箱（M2）——員工桌面工具的路徑邊界。
//!
//! 員工工作區＝`<employee_output_path>/<employee_id>/`（與 write-note 沙箱同根，不新增
//! 設定）。**這是知識邊界的執行點**：桌面工具的路徑一律經 [`resolve_in_workspace`]，
//! 工作區外（絕對路徑、`..` 逃逸、symlink 跳出）一律拒絕——員工不能經檔案工具讀到
//! 未經授權的知識（Handbook 知識邊界：未授權知識不進 LLM context）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// 檔案讀取狀態（mtime＋size）——edit 的 read-before-edit／staleness 防呆依據。
/// 生命週期＝一次對話回合（TurnSession），跨回合不保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub mtime: SystemTime,
    pub size: u64,
}

/// 一個回合的檔案讀取狀態（path → stamp）。
pub type FileState = HashMap<PathBuf, FileStamp>;

/// 員工工作區根目錄（`<employee_output_root>/<employee_id>`）。
pub fn workspace_root(employee_output_root: &std::path::Path, employee_id: &str) -> PathBuf {
    employee_output_root.join(employee_id)
}

/// 確保工作區存在（冪等）。
pub fn ensure_workspace(root: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)
}

/// 解析工作區內路徑並驗邊界（M2 的核心防護）。
///
/// 接受相對路徑（含子目錄）；拒絕絕對路徑與任何 `..`／根元件。存在與否不限（寫入前
/// 呼叫端自行處理）。canonicalize 前綴檢查攔 symlink 跳出——不存在的目標以「父目錄
/// canonical」驗（仿 write-note 的包含檢查）。
pub fn resolve_in_workspace(root: &std::path::Path, path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path 不可為空".into());
    }
    let p = std::path::Path::new(trimmed);
    if p.is_absolute() {
        return Err(format!("path 不可為絕對路徑（工作區內相對路徑）：{trimmed}"));
    }
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(_) => {}
            other => {
                return Err(format!(
                    "path 僅接受工作區內相對路徑（拒絕 {other:?}）：{trimmed}"
                ))
            }
        }
    }
    let full = root.join(p);
    // 邊界驗證：目標存在 → canonical 全比對；不存在 → canonical 父目錄比對（父須存在）。
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("工作區根目錄不可用（{root:?}）：{e}"))?;
    let check = |c: PathBuf| -> Result<(), String> {
        if c.starts_with(&canonical_root) {
            Ok(())
        } else {
            Err(format!("路徑越出工作區邊界（拒絕）：{trimmed}"))
        }
    };
    match full.canonicalize() {
        Ok(c) => check(c)?,
        Err(_) => {
            // 目標（或其父目錄）尚不存在：以最深存在祖先驗邊界——寫入工具會自建目錄。
            let mut anc = full.as_path();
            let canonical_ancestor = loop {
                match anc.canonicalize() {
                    Ok(c) => break c,
                    Err(_) => anc = anc.parent().ok_or_else(|| format!("無效路徑：{trimmed}"))?,
                }
            };
            check(canonical_ancestor)?;
        }
    }
    Ok(full)
}

/// 取檔案的 mtime＋size（不存在 → Err）。
pub fn stamp_of(path: &std::path::Path) -> Result<FileStamp, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("讀取檔案資訊失敗：{e}"))?;
    Ok(FileStamp {
        mtime: meta
            .modified()
            .map_err(|e| format!("讀取 mtime 失敗：{e}"))?,
        size: meta.len(),
    })
}
