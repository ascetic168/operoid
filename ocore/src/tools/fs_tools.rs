//! 桌面工具：檔案讀寫編輯（M2）——員工工作區內的文件工作。
//!
//! 防呆對齊 ZCode 的 Read/Edit 紀律：
//! - `edit_file` **強制先讀**（未讀 → FILE_NOT_READ）、mtime 變更 → STALE_FILE、
//!   old_string 未命中 → NOT_FOUND、多處命中未 replace_all → AMBIGUOUS。
//! - `read_file`／`write_file` 記錄 FileStamp（mtime＋size）到回合的 FileState——
//!   staleness 判定的依據。
//! - 全部路徑經 [`workspace::resolve_in_workspace`]（知識邊界執行點）。
//!
//! 回傳值為員工可讀的結果文字（note）；語意性失敗（未讀就改、過期）也是文字——
//! 餵回模型讓它自癒，不自爆。

use serde_json::Value;

use super::workspace::{self, FileState, FileStamp};

/// 單檔讀取上限（bytes）。
pub const READ_MAX_FILE_SIZE: u64 = 256 * 1024;
/// 預設最大行數。
pub const READ_DEFAULT_MAX_LINES: usize = 2_000;

/// 讀取工作區檔案（行號格式；offset 1-based）。記錄 FileStamp 供 edit 驗 staleness。
pub fn read_file(root: &std::path::Path, args: &Value, fstate: &mut FileState) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("缺 path 參數")?;
    let full = workspace::resolve_in_workspace(root, path)?;
    let stamp = workspace::stamp_of(&full)?;
    if stamp.size > READ_MAX_FILE_SIZE {
        return Err(format!(
            "檔案過大（{} bytes > 上限 {}）——請用 run_command 拆讀或請人類處理",
            stamp.size, READ_MAX_FILE_SIZE
        ));
    }
    let bytes = std::fs::read(&full).map_err(|e| format!("讀取失敗：{e}"))?;
    let text = crate::proc::decode_buf(&bytes);
    let all_lines: Vec<&str> = if text.is_empty() { Vec::new() } else { text.lines().collect() };
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| v.max(1) as usize - 1)
        .unwrap_or(0);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(READ_DEFAULT_MAX_LINES)
        .max(1);
    if offset >= all_lines.len().max(1) && !all_lines.is_empty() {
        return Err(format!(
            "offset {offset} 超出檔案行數（共 {} 行）",
            all_lines.len()
        ));
    }
    let taken: Vec<String> = all_lines
        .iter()
        .skip(offset)
        .take(limit)
        .enumerate()
        .map(|(i, l)| format!("{:>6}\t{}", offset + i + 1, l))
        .collect();
    // 記錄讀取狀態（edit 的先讀驗證依據）。
    fstate.insert(full, stamp);
    let total = all_lines.len();
    if total == 0 {
        return Ok(format!("（{path}：空檔案）"));
    }
    let mut out = String::new();
    if offset > 0 || taken.len() < total {
        out.push_str(&format!(
            "（{path}：共 {total} 行，顯示 {}-{}）\n",
            offset + 1,
            offset + taken.len()
        ));
    }
    out.push_str(&taken.join("\n"));
    Ok(out)
}

/// 寫入工作區檔案（自動建目錄；覆寫既有檔）。回傳結果 note；寫後檔案狀態即為最新。
pub fn write_file(root: &std::path::Path, args: &Value, fstate: &mut FileState) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("缺 path 參數")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or("缺 content 參數")?;
    let full = workspace::resolve_in_workspace(root, path)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("建目錄失敗：{e}"))?;
    }
    std::fs::write(&full, content).map_err(|e| format!("寫入失敗：{e}"))?;
    let stamp: FileStamp = workspace::stamp_of(&full)?;
    fstate.insert(full, stamp);
    let lines = content.lines().count();
    Ok(format!(
        "已寫入 {path}（{lines} 行、{} bytes）。檔案狀態已在你的 context 中——不必再 read_file 驗證。",
        content.len()
    ))
}

/// 精確字串替換編輯（強制先讀＋staleness 驗證）。
pub fn edit_file(root: &std::path::Path, args: &Value, fstate: &mut FileState) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("缺 path 參數")?;
    let old = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .ok_or("缺 old_string 參數")?;
    let new = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .ok_or("缺 new_string 參數")?;
    let replace_all = args
        .get("replace_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if old.is_empty() {
        return Err("old_string 不可為空".into());
    }
    let full = workspace::resolve_in_workspace(root, path)?;
    // Read-before-edit：沒讀過（本回合）→ 拒絕。
    let recorded = *fstate
        .get(&full)
        .ok_or("FILE_NOT_READ：編輯前必須先用 read_file 讀過這個檔案")?;
    // Staleness：讀過但之後被（shell 等）改過 → 拒絕。
    let current = workspace::stamp_of(&full)?;
    if current != recorded {
        return Err(
            "STALE_FILE：檔案自上次讀取後已被修改——請重新 read_file 再編輯".into(),
        );
    }
    let bytes = std::fs::read(&full).map_err(|e| format!("讀取失敗：{e}"))?;
    let text = crate::proc::decode_buf(&bytes);
    let hits = text.matches(old).count();
    if hits == 0 {
        return Err(
            "NOT_FOUND：old_string 在檔案中找不到（須與檔案內容逐字相同，含縮排與換行）".into(),
        );
    }
    if hits > 1 && !replace_all {
        return Err(format!(
            "AMBIGUOUS：old_string 出現 {hits} 處——請擴大上下文使其唯一，或設 replace_all=true"
        ));
    }
    let updated = if replace_all {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    };
    std::fs::write(&full, &updated).map_err(|e| format!("寫入失敗：{e}"))?;
    let stamp = workspace::stamp_of(&full)?;
    fstate.insert(full, stamp);
    let n = if replace_all { hits } else { 1 };
    Ok(format!(
        "已更新 {path}（替換 {n} 處）。檔案狀態已在你的 context 中——不必再 read_file 驗證。"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "operoid-fs-tools-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn write_then_read_roundtrip_with_line_numbers() {
        let root = tmp();
        let mut fs = FileState::new();
        let n = write_file(&root, &json!({"path": "docs/a.md", "content": "第一行\n第二行"}), &mut fs).unwrap();
        assert!(n.contains("已寫入"), "{n}");
        let out = read_file(&root, &json!({"path": "docs/a.md"}), &mut fs).unwrap();
        assert!(out.contains("     1\t第一行"), "{out}");
        assert!(out.contains("     2\t第二行"), "{out}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_absolute_and_escape_paths() {
        let root = tmp();
        let mut fs = FileState::new();
        assert!(resolve_err(&root, &json!({"path": "C:/Windows/win.ini"}), &mut fs));
        assert!(resolve_err(&root, &json!({"path": "../outside.txt"}), &mut fs));
        assert!(resolve_err(&root, &json!({"path": "a/../../b.txt"}), &mut fs));
        std::fs::remove_dir_all(&root).ok();
    }

    fn resolve_err(root: &std::path::Path, args: &Value, fs: &mut FileState) -> bool {
        read_file(root, args, fs).is_err()
    }

    #[test]
    fn edit_requires_read_then_stale_then_success() {
        let root = tmp();
        let mut fs = FileState::new();
        // 檔案由外部建立（非本回合 write_file——寫過即視為已讀，ZCode 同款語意）。
        std::fs::write(root.join("r.txt"), "alpha beta gamma").unwrap();
        // 未讀就改 → FILE_NOT_READ
        let e = edit_file(&root, &json!({"path": "r.txt", "old_string": "beta", "new_string": "B"}), &mut fs)
            .unwrap_err();
        assert!(e.contains("FILE_NOT_READ"), "{e}");
        // 讀後改 → 成功
        read_file(&root, &json!({"path": "r.txt"}), &mut fs).unwrap();
        let n = edit_file(&root, &json!({"path": "r.txt", "old_string": "beta", "new_string": "B"}), &mut fs)
            .unwrap();
        assert!(n.contains("替換 1 處"), "{n}");
        let c = std::fs::read_to_string(root.join("r.txt")).unwrap();
        assert_eq!(c, "alpha B gamma");
        // 檔案被外部改動（size 亦變）→ STALE_FILE
        std::fs::write(root.join("r.txt"), "changed externally").unwrap();
        let e = edit_file(&root, &json!({"path": "r.txt", "old_string": "changed", "new_string": "x"}), &mut fs)
            .unwrap_err();
        assert!(e.contains("STALE_FILE"), "{e}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_seeds_file_state_and_allows_immediate_edit() {
        let root = tmp();
        let mut fs = FileState::new();
        write_file(&root, &json!({"path": "w.txt", "content": "one two"}), &mut fs).unwrap();
        // 剛寫過的檔案可直接編輯（寫過＝知道內容）。
        let n = edit_file(&root, &json!({"path": "w.txt", "old_string": "two", "new_string": "2"}), &mut fs)
            .unwrap();
        assert!(n.contains("替換 1 處"), "{n}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_creates_nested_dirs() {
        let root = tmp();
        let mut fs = FileState::new();
        // 深層新路徑（父目錄尚不存在）→ 自建目錄。
        let n = write_file(&root, &json!({"path": "a/b/c/report.md", "content": "hi"}), &mut fs).unwrap();
        assert!(n.contains("已寫入"), "{n}");
        assert_eq!(std::fs::read_to_string(root.join("a/b/c/report.md")).unwrap(), "hi");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn edit_ambiguous_and_not_found() {
        let root = tmp();
        let mut fs = FileState::new();
        write_file(&root, &json!({"path": "m.txt", "content": "x\nx\ny"}), &mut fs).unwrap();
        read_file(&root, &json!({"path": "m.txt"}), &mut fs).unwrap();
        let e = edit_file(&root, &json!({"path": "m.txt", "old_string": "x", "new_string": "z"}), &mut fs)
            .unwrap_err();
        assert!(e.contains("AMBIGUOUS"), "{e}");
        let e = edit_file(&root, &json!({"path": "m.txt", "old_string": "nope", "new_string": "z"}), &mut fs)
            .unwrap_err();
        assert!(e.contains("NOT_FOUND"), "{e}");
        let n = edit_file(
            &root,
            &json!({"path": "m.txt", "old_string": "x", "new_string": "z", "replace_all": true}),
            &mut fs,
        )
        .unwrap();
        assert!(n.contains("替換 2 處"), "{n}");
        std::fs::remove_dir_all(&root).ok();
    }
}
