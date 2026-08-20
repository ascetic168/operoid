//! 啟動時前置程式檢查（P5 重新設計）：git / bun / gbrain。
//!
//! **設計原則（2026-08-20 使用者定調）**：路徑已知者以**檔案存在**判斷（近零成本），
//! 不用「執行看看」證明——gbrain 的 spawn 是 bun runtime 冷啟（首次可達 20s），
//! 絕不能掛在啟動關鍵路徑。版本字串只是顯示用：走 [`PrereqCache`] 快取，
//! 缺漏由呼叫端**背景**刷新（[`refresh_details`]）。
//!
//! - **git**：系統安裝、無慣例路徑——保留 spawn（~0.02s，便宜）。
//! - **bun**：per-user 慣例路徑 `~/.bun/bin`——存在即可用，不 spawn。
//! - **gbrain**：`gbrain_exe_path` 設定值——存在即可用，不 spawn。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i18n::L10n;

/// 版本字串快取（存 AppConfig；背景刷新後回寫）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrereqCache {
    pub git: Option<String>,
    pub bun: Option<String>,
    pub gbrain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DepStatus {
    pub name: String,
    pub available: bool,
    /// 版本字串（語言中性）；無快取時為 None（前端可顯示「檢查中」或留白）。
    pub detail: Option<String>,
    /// 安裝說明（在地化代碼）。
    pub install_hint: L10n,
    pub url: String,
}

/// 跑 `<cmd> <args>`；成功回 stdout(或 stderr)第一行，失敗回 None。
fn probe(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args).env("PYTHONUTF8", "1");
    crate::proc::no_console(&mut c);
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = if !out.stdout.is_empty() { &out.stdout } else { &out.stderr };
    let s = String::from_utf8_lossy(raw);
    Some(s.lines().next().unwrap_or("").trim().to_string())
}

fn bun_path(user_home: Option<&Path>) -> Option<PathBuf> {
    user_home.map(|h| h.join(".bun").join("bin").join(bun_bin_name()))
}

fn bun_bin_name() -> &'static str {
    if cfg!(windows) { "bun.exe" } else { "bun" }
}

fn gbrain_bin_name() -> &'static str {
    if cfg!(windows) { "gbrain.exe" } else { "gbrain" }
}

/// 前置檢查（**快速路徑**——不 spawn bun/gbrain，<10ms）。
///
/// - `gbrain_exe`：設定的 exe 路徑（存在即 available）；
/// - `user_home`：bun 慣例路徑解析（桌面殼直接給；oserver 由 settings_dir 推導）；
/// - `cache`：版本字串快取（無則 detail=None，呼叫端背景補）。
pub fn check_all(gbrain_exe: &str, user_home: Option<&Path>, cache: &PrereqCache) -> Vec<DepStatus> {
    let mut deps = Vec::new();

    // git：系統安裝、spawn 便宜（~0.02s）——直接探測，版本即時可得。
    let git_ok = probe("git", &["--version"]);
    deps.push(DepStatus {
        name: "git".into(),
        available: git_ok.is_some(),
        detail: git_ok.or_else(|| cache.git.clone()),
        install_hint: L10n::new("prereq.git.hint"),
        url: "https://git-scm.com/downloads".into(),
    });

    // bun：慣例路徑存在即可用——不 spawn（服務/桌面模式同一判斷，無 PATH 硬試）。
    let bun_available = bun_path(user_home).map(|p| p.exists()).unwrap_or(false);
    deps.push(DepStatus {
        name: "bun".into(),
        available: bun_available,
        detail: cache.bun.clone(),
        install_hint: L10n::new("prereq.bun.hint"),
        url: "https://bun.com/docs/installation#installation".into(),
    });

    // gbrain：設定的 exe 存在即可用——不 spawn（bun runtime 冷啟可達 20s）。
    let gbrain_available = Path::new(gbrain_exe).exists();
    deps.push(DepStatus {
        name: "gbrain".into(),
        available: gbrain_available,
        detail: cache.gbrain.clone(),
        install_hint: L10n::new("prereq.gbrain.hint"),
        url: "https://github.com/garrytan/gbrain#cli-standalone-no-agent".into(),
    });

    deps
}

/// 背景／手動刷新版本字串（會 spawn bun＋gbrain——gbrain 是 bun runtime，冷啟慢；
/// 僅供非關鍵路徑使用：快取缺漏時的背景補值、設定頁「重新檢查」）。
pub fn refresh_details(gbrain_exe: &str, user_home: Option<&Path>) -> PrereqCache {
    PrereqCache {
        git: probe("git", &["--version"]),
        bun: bun_path(user_home).and_then(|p| probe(&p.to_string_lossy(), &["--version"])),
        gbrain: (|| {
            let exe = if Path::new(gbrain_exe).exists() {
                gbrain_exe.to_string()
            } else {
                bun_path(user_home)
                    .map(|p| p.with_file_name(gbrain_bin_name()))
                    .filter(|p| p.exists())
                    .map(|p| p.to_string_lossy().into_owned())?
            };
            probe(&exe, &["version"])
        })(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 快速路徑不 spawn bun/gbrain：三項都有結果、bun/gbrain 的 detail 來自快取。
    #[test]
    fn fast_check_uses_cache_without_spawn() {
        let cache = PrereqCache {
            git: None,
            bun: Some("1.2.3".into()),
            gbrain: Some("0.46.0".into()),
        };
        let deps = check_all("/nonexistent/gbrain", None, &cache);
        let bun = deps.iter().find(|d| d.name == "bun").unwrap();
        assert_eq!(bun.detail.as_deref(), Some("1.2.3"));
        let gbrain = deps.iter().find(|d| d.name == "gbrain").unwrap();
        assert!(!gbrain.available);
        assert_eq!(gbrain.detail.as_deref(), Some("0.46.0"));
        // git 即時探測（測試環境必有 git）——available 且有版本。
        let git = deps.iter().find(|d| d.name == "git").unwrap();
        assert!(git.available);
        assert!(git.detail.is_some());
    }
}
