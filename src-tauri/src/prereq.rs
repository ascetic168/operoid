//! 啟動時前置程式檢查:git / bun / gbrain。缺漏則回報安裝說明與連結。

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::config;
use crate::i18n::L10n;

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

/// 跑 `<cmd> <args>`;成功回 stdout(或 stderr)第一行,失敗回 None。
fn probe(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .env("PYTHONUTF8", "1")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = if !out.stdout.is_empty() { &out.stdout } else { &out.stderr };
    let s = String::from_utf8_lossy(raw);
    Some(s.lines().next().unwrap_or("").trim().to_string())
}

/// 檢查 git / bun / gbrain 是否可用。
#[tauri::command]
pub fn check_prerequisites<R: Runtime>(app: AppHandle<R>) -> Result<Vec<DepStatus>, String> {
    let cfg = config::app_config::load(&app).unwrap_or_default();
    let mut deps = Vec::new();

    let git_ok = probe("git", &["--version"]);
    deps.push(DepStatus {
        name: "git".into(),
        available: git_ok.is_some(),
        detail: git_ok,
        install_hint: L10n::new("prereq.git.hint"),
        url: "https://git-scm.com/downloads".into(),
    });

    let bun_ok = probe("bun", &["--version"]);
    deps.push(DepStatus {
        name: "bun".into(),
        available: bun_ok.is_some(),
        detail: bun_ok,
        install_hint: L10n::new("prereq.bun.hint"),
        url: "https://bun.com/docs/installation#installation".into(),
    });

    // gbrain:優先用設定的 exe 路徑,否則退到 PATH 上的 gbrain
    let gbrain_cmd = if Path::new(&cfg.gbrain_exe_path).exists() {
        cfg.gbrain_exe_path.clone()
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

    Ok(deps)
}
