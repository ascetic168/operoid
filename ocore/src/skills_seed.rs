//! 公司共用庫的內建預設技能（冷啟動種子）。
//!
//! `ocore/default-skills/` 由 `scripts/import_kwp_skills.py` 自 Anthropic
//! 《knowledge-work-plugins》策展導入（Apache-2.0；來源 commit、授權與 linter
//! 報告見目錄內 `SOURCE.md`／`IMPORT-REPORT.md`），編譯期以 `include_dir` 內嵌。
//! oserver 啟動時種子進 `<data_dir>/skills/library/`：**只補缺的整顆外掛目錄**——
//! 已存在者永不覆寫（admin 刪除的外掛不會復活；更新內建內容＝刪除該外掛目錄後
//! 重啟，或走員工提案流）。種子先寫暫存目錄再改名，避免半套目錄被誤判為已存在。

use std::io;
use std::path::Path;

use include_dir::{include_dir, Dir, DirEntry};

/// 內建預設技能樹（策展來源與授權見 `default-skills/SOURCE.md`）。
/// include_dir 的路徑以 proc-macro 行程 CWD 為基準，須明確錨定 crate 根。
static DEFAULT_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/default-skills");

/// 種子結果（啟動日誌用）。
#[derive(Debug, Default)]
pub struct SeedReport {
    /// 本次補進的外掛目錄名。
    pub seeded: Vec<String>,
    /// 已存在而跳過的外掛目錄名。
    pub skipped: Vec<String>,
    /// 失敗的外掛目錄與原因（僅記錄，不中斷啟動——技能面可容忍缺件）。
    pub errors: Vec<String>,
}

/// 將內建預設技能種子進公司共用庫（冪等：外掛目錄存在即整顆跳過）。
pub fn seed_default_library(data_dir: &Path) -> SeedReport {
    let library = crate::skills::library_root(data_dir);
    let mut report = SeedReport::default();
    for plugin in DEFAULT_SKILLS.dirs() {
        let Some(name) = plugin.path().file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let target = library.join(name);
        if target.is_dir() {
            report.skipped.push(name.to_string());
            continue;
        }
        // 先寫暫存目錄再改名：中途失敗不留半套目錄（半套會在下次啟動被判為
        // 「已存在」而永不補齊）。
        let staging = library.join(format!(".{name}.seeding"));
        let _ = std::fs::remove_dir_all(&staging);
        match write_dir_recursively(plugin, &staging).and_then(|()| std::fs::rename(&staging, &target))
        {
            Ok(()) => report.seeded.push(name.to_string()),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging);
                report.errors.push(format!("{name}: {e}"));
            }
        }
    }
    report
}

/// 把內嵌目錄整棵寫到磁碟（`entries()` 只給直接子項，須自行遞迴；
/// `path()` 相對內嵌根，取 `file_name()` 接在累積目標下）。
fn write_dir_recursively(dir: &Dir<'_>, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                let name = sub.path().file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("無名內嵌目錄：{}", sub.path().display()))
                })?;
                write_dir_recursively(sub, &target.join(name))?;
            }
            DirEntry::File(file) => {
                let name = file.path().file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("無名內嵌檔：{}", file.path().display()))
                })?;
                std::fs::write(target.join(name), file.contents())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "operoid-skills-seed-{tag}-{}-{}",
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
    fn seed_copies_missing_plugins_and_is_idempotent() {
        let dir = tmp("seed");
        let n = DEFAULT_SKILLS.dirs().count();
        assert!(n > 0, "內建預設技能庫不應為空");
        let r1 = seed_default_library(&dir);
        assert_eq!(r1.seeded.len(), n);
        assert!(r1.errors.is_empty(), "seed 錯誤：{:?}", r1.errors);
        // 每顆種子外掛在磁碟上至少含一個 SKILL.md（掃描面可見）——驗證磁碟產物
        // 而非內嵌樹，確保遞迴寫入沒漏層。
        let library = crate::skills::library_root(&dir);
        for plugin in DEFAULT_SKILLS.dirs() {
            let target = library.join(plugin.path());
            assert!(target.is_dir(), "{} 未落地", plugin.path().display());
            assert!(has_skill_md(&target), "{} 內無 SKILL.md", plugin.path().display());
        }
        // 冪等：第二次啟動不再種任何東西。
        let r2 = seed_default_library(&dir);
        assert!(r2.seeded.is_empty());
        assert_eq!(r2.skipped.len(), n);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn has_skill_md(dir: &Path) -> bool {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_file() && p.file_name().is_some_and(|n| n == crate::skills::SKILL_MD) {
                return true;
            }
            if p.is_dir() && has_skill_md(&p) {
                return true;
            }
        }
        false
    }

    #[test]
    fn seed_never_overwrites_existing_plugin_dir() {
        let dir = tmp("seed-keep");
        let library = crate::skills::library_root(&dir);
        let first = DEFAULT_SKILLS
            .dirs()
            .next()
            .expect("內建預設技能庫不應為空")
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap()
            .to_string();
        let target = library.join(&first);
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("admin-marker.txt"), "keep").unwrap();
        let r = seed_default_library(&dir);
        assert!(r.skipped.contains(&first));
        assert!(!r.seeded.contains(&first));
        assert_eq!(std::fs::read_to_string(target.join("admin-marker.txt")).unwrap(), "keep");
        std::fs::remove_dir_all(&dir).ok();
    }
}
