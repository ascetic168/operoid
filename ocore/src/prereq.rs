//! 啟動時前置程式檢查：git / bun / gbrain（P1c 起居於 ocore）。
//! 缺漏則回報安裝說明與連結。指令層（check_prerequisites）留在桌面殼。

use std::path::Path;

use serde::Serialize;

use crate::i18n::L10n;
use crate::proc::no_console;

#[derive(Debug, Serialize)]
pub struct DepStatus {
    pub name: String,
    pub available: bool,
    /// 版本字串（語言中性）；找不到時為 None（前端依 available=false 顯示在地化提示）。
    pub detail: Option<String>,
    /// 安裝說明（在地化代碼）。
    pub install_hint: L10n,
    pub url: String,
}

/// 跑 `<cmd> <args>`；成功回 stdout(或 stderr)第一行，失敗回 None。
/// `cmd` 可為絕對路徑（fallback 用——服務模式 LocalSystem 的 PATH 不含使用者安裝目錄）。
fn probe(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args).env("PYTHONUTF8", "1");
    no_console(&mut c);
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = if !out.stdout.is_empty() { &out.stdout } else { &out.stderr };
    let s = String::from_utf8_lossy(raw);
    Some(s.lines().next().unwrap_or("").trim().to_string())
}

/// 檢查 git / bun / gbrain 是否可用（`gbrain_exe` 為設定的 exe 路徑；不存在退 PATH）。
///
/// `user_home`（Some＝服務模式由 settings_dir 推導；None＝桌面殼直接給 home）：
/// bun 與 gbrain 常以 per-user 安裝（`~/.bun/bin`）——服務以 LocalSystem 跑時 PATH
/// 不含之，故 PATH 探測失敗後退絕對路徑（P5 修：服務模式下 bun 誤報找不到）。
pub fn check_all(gbrain_exe: &str, user_home: Option<&std::path::Path>) -> Vec<DepStatus> {
    let mut deps = Vec::new();

    let git_ok = probe("git", &["--version"]);
    deps.push(DepStatus {
        name: "git".into(),
        available: git_ok.is_some(),
        detail: git_ok,
        install_hint: L10n::new("prereq.git.hint"),
        url: "https://git-scm.com/downloads".into(),
    });

    let bun_cmd = probe_first(
        &["bun"],
        user_home.map(|h| h.join(".bun").join("bin").join(bun_bin_name())),
    );
    let bun_ok = bun_cmd.as_ref().map(|(v, _)| v.clone());
    deps.push(DepStatus {
        name: "bun".into(),
        available: bun_ok.is_some(),
        detail: bun_ok,
        install_hint: L10n::new("prereq.bun.hint"),
        url: "https://bun.com/docs/installation#installation".into(),
    });

    // gbrain：優先用設定的 exe 路徑 → 使用者 .bun/bin → PATH
    let gbrain_cmd = if Path::new(gbrain_exe).exists() {
        gbrain_exe.to_string()
    } else if let Some(fallback) = user_home
        .map(|h| h.join(".bun").join("bin").join(gbrain_bin_name()))
        .filter(|p| p.exists())
    {
        fallback.to_string_lossy().into_owned()
    } else {
        "gbrain".to_string()
    };
    let gbrain_ok = probe(&gbrain_cmd, &["version"]);
    deps.push(DepStatus {
        name: "gbrain".into(),
        available: gbrain_ok.is_some(),
        detail: gbrain_ok,
        install_hint: L10n::new("prereq.gbrain.hint"),
        url: "https://github.com/garrytan/gbrain#cli-standalone-no-agent".into(),
    });

    deps
}


#[cfg(windows)]
fn bun_bin_name() -> &'static str {
    "bun.exe"
}
#[cfg(not(windows))]
fn bun_bin_name() -> &'static str {
    "bun"
}
#[cfg(windows)]
fn gbrain_bin_name() -> &'static str {
    "gbrain.exe"
}
#[cfg(not(windows))]
fn gbrain_bin_name() -> &'static str {
    "gbrain"
}

/// 依序探測：PATH 上的 cmd → 絕對路徑 fallback。回（版本輸出, 實際命中的指令）。
fn probe_first(path_cmd: &[&str], fallback: Option<std::path::PathBuf>) -> Option<(String, String)> {
    if let Some(v) = probe(path_cmd[0], &["--version"]) {
        return Some((v, path_cmd[0].to_string()));
    }
    if let Some(p) = fallback.filter(|p| p.exists()) {
        let s = p.to_string_lossy().into_owned();
        if let Some(v) = probe(&s, &["--version"]) {
            return Some((v, s));
        }
    }
    None
}
