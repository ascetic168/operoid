//! 公司共用庫的內建預設技能（冷啟動種子）。
//!
//! `ocore/default-skills/` 由 `scripts/import_kwp_skills.py` 自 Anthropic
//! 《knowledge-work-plugins》策展導入（Apache-2.0；來源 commit、授權與 linter
//! 報告見目錄內 `SOURCE.md`／`IMPORT-REPORT.md`），編譯期以 `include_dir` 內嵌。
//! oserver 啟動時種子進 `<data_dir>/skills/library/`，升級安全語意：
//! - 外掛目錄**已存在即整顆跳過**——使用者改過的技能永不覆寫；
//! - `<library>/.seeded/<plugin>` 標記已種過：目錄被刪除＝admin 策展決定，
//!   **不復活**；
//! - 要刷新內建內容：刪除外掛目錄**與**其 `.seeded` 標記後重啟。
//! 種子先寫暫存目錄再改名，避免半套目錄被誤判為已存在。

use std::io;
use std::path::Path;

use include_dir::{include_dir, Dir, DirEntry};

/// 內建預設技能樹（策展來源與授權見 `default-skills/SOURCE.md`）。
/// include_dir 的路徑以 proc-macro 行程 CWD 為基準，須明確錨定 crate 根。
static DEFAULT_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/default-skills");

/// 「已種過」標記目錄名（位於共用庫根下；無 SKILL.md，掃描面不列示）。
const SEEDED_MARKER_DIR: &str = ".seeded";

/// 種子結果（啟動日誌用）。
#[derive(Debug, Default)]
pub struct SeedReport {
    /// 本次補進的外掛目錄名。
    pub seeded: Vec<String>,
    /// 已存在而跳過的外掛目錄名（可能含使用者修改——永不覆寫）。
    pub skipped: Vec<String>,
    /// 種過但目錄已被刪除（admin 策展決定）——不復活。
    pub admin_removed: Vec<String>,
    /// 失敗的外掛目錄與原因（僅記錄，不中斷啟動——技能面可容忍缺件）。
    pub errors: Vec<String>,
}

/// 將內建預設技能種子進公司共用庫（冪等；升級安全語意見模組文件）。
pub fn seed_default_library(data_dir: &Path) -> SeedReport {
    let library = crate::skills::library_root(data_dir);
    let markers = library.join(SEEDED_MARKER_DIR);
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
        let marker = markers.join(name);
        if marker.exists() {
            report.admin_removed.push(name.to_string());
            continue;
        }
        // 先寫暫存目錄再改名：中途失敗不留半套目錄（半套會在下次啟動被判為
        // 「已存在」而永不補齊）。標記在改名**之後**寫——若崩潰在兩步之間，
        // 目錄已存在會走「跳過」，下次啟動補不上標記的代價只是「刪除後復活
        // 一次」，遠優於「有標記無目錄」的永久跳過。
        let staging = library.join(format!(".{name}.seeding"));
        let _ = std::fs::remove_dir_all(&staging);
        let outcome = write_dir_recursively(plugin, &staging)
            .and_then(|()| std::fs::rename(&staging, &target))
            .and_then(|()| {
                std::fs::create_dir_all(&markers)?;
                std::fs::write(&marker, b"")
            });
        match outcome {
            Ok(()) => report.seeded.push(name.to_string()),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging);
                let _ = std::fs::remove_file(&marker);
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

    #[test]
    fn seed_admin_deleted_plugin_is_not_resurrected() {
        let dir = tmp("seed-delete");
        let library = crate::skills::library_root(&dir);
        let n = DEFAULT_SKILLS.dirs().count();
        assert!(seed_default_library(&dir).errors.is_empty());

        // admin 刪除整顆外掛目錄（標記仍在）→ 重啟不復活。
        let victim = DEFAULT_SKILLS.dirs().next().unwrap();
        let name = victim.path().file_name().and_then(|n| n.to_str()).unwrap().to_string();
        std::fs::remove_dir_all(library.join(&name)).unwrap();
        let r2 = seed_default_library(&dir);
        assert!(r2.admin_removed.contains(&name), "應記為 admin 已移除");
        assert!(!r2.seeded.contains(&name));
        assert!(!r2.skipped.contains(&name));
        assert_eq!(r2.seeded.len() + r2.admin_removed.len() + r2.skipped.len(), n);

        // 刷新路徑：連 `.seeded` 標記一起刪 → 重新種子。
        std::fs::remove_file(library.join(".seeded").join(&name)).unwrap();
        let r3 = seed_default_library(&dir);
        assert!(r3.seeded.contains(&name), "刪除標記後應重種");
        assert!(r3.errors.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
