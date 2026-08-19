//! 工廠指令層（P1c 殼）。核心（轉換/寫入/事件 emit）已搬入 `ocore::factories`；
//! 此處留 `#[tauri::command]` 與 `factory_open_dir`（open crate／VS Code，桌面專屬）。

pub use ocore::factories::*;

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::agent_state::AppState;
use crate::config;
use crate::i18n::AppError;
use ocore::proc::no_console;

fn app_cfg<R: Runtime>(app: &AppHandle<R>) -> Result<config::AppConfig, String> {
    config::app_config::load(app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn factory_save_authored<R: Runtime>(
    app: AppHandle<R>,
    factory: String,
    markdown: String,
    existing_slug: Option<String>,
    target_repo: Option<String>,
) -> Result<AuthoredResult, AppError> {
    let cfg = app_cfg(&app)?;
    let state = app.state::<AppState>();
    save_authored_core(
        &cfg,
        Some(&state),
        &factory,
        &markdown,
        existing_slug.as_deref(),
        target_repo.as_deref(),
    )
    .await
}

#[derive(Debug, Serialize)]
pub struct OpenDirResult {
    /// "vscode" | "filemanager"
    pub opened_with: String,
    /// 開啟的目錄絕對路徑
    pub path: String,
}

/// 點工廠卡圖示：以 VS Code 開啟該工廠目錄；沒裝 VS Code 則以系統預設檔案管理員開啟。
/// 目錄不存在會先建立。inbox 不支援——其筆記由 `gbrain capture` 寫入知識庫內部儲存、
/// 無可瀏覽資料夾（前端已停用 inbox 圖示）；若被呼叫會回 `factories.openDirInboxHint` 錯誤。
#[tauri::command]
pub fn factory_open_dir<R: Runtime>(
    app: AppHandle<R>,
    factory: String,
    target_repo: Option<String>,
) -> Result<OpenDirResult, AppError> {
    let cfg = app_cfg(&app)?;
    let notes = PathBuf::from(target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()));

    let subdir = match factory.as_str() {
        "inbox" => return Err(AppError::new("factories.openDirInboxHint")),
        "people" => "people".to_string(),
        "companies" => "companies".to_string(),
        "meeting" => "meetings".to_string(),
        "concepts" => "concepts".to_string(),
        "projects" => "projects".to_string(),
        other => return Err(AppError::new("factory.unknown").p("factory", other)),
    };
    let dir = notes.join(&subdir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path_str = dir.to_string_lossy().to_string();
    if has_vscode() {
        launch_vscode(&dir)?;
        Ok(OpenDirResult { opened_with: "vscode".into(), path: path_str })
    } else {
        open::that(&dir).map_err(|e| e.to_string())?;
        Ok(OpenDirResult { opened_with: "filemanager".into(), path: path_str })
    }
}

/// 偵測 VS Code CLI（`code`）是否在 PATH 上。Windows 的 `code` 是 `.cmd` shim，
/// CreateProcess 不搜尋 PATHEXT，故用 `cmd /C where code`；macOS/Linux 用 `which code`。
///
/// 注意：必須用 `#[cfg(...)]`（編譯期屬性）而非 `cfg!()`（runtime 常數）。`launch_vscode`
/// 的 Windows 分支呼叫了 Windows 專屬 API（`raw_arg`），若用 `cfg!()` 寫成 if/else，
/// 另一個分支在 macOS/Linux 仍會被編譯，而 `std::os::windows` 在那裡不存在 → 編譯失敗。
/// `#[cfg]` 直接在編譯期挑掉不相關分支，與 `ocore::proc::no_console` 同模式。
fn has_vscode() -> bool {
    #[cfg(target_os = "windows")]
    {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "where", "code"]);
        no_console(&mut c);
        return c
            .output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = std::process::Command::new("which");
        c.arg("code");
        no_console(&mut c);
        return c
            .output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);
    }
}

/// 以 `code <dir>` 開啟（fire-and-forget）。Windows 須經 `cmd /C` 並用 `raw_arg`
/// 保留路徑引號，避免含空格的路徑被 cmd 重新切片；macOS/Linux 直接 `code <dir>`。
fn launch_vscode(dir: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let mut c = std::process::Command::new("cmd");
        // raw_arg 把 `/C code "C:\path with spaces"` 整段原樣送出，保留引號
        c.raw_arg(format!("/C code \"{}\"", dir.display()));
        no_console(&mut c);
        c.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = std::process::Command::new("code");
        c.arg(dir);
        no_console(&mut c);
        c.spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn factory_run<R: Runtime>(
    app: AppHandle<R>,
    factory: String,
    paths: Vec<String>,
    target_repo: Option<String>,
) -> Result<PreviewResult, AppError> {
    let cfg = app_cfg(&app)?;
    run_core(&cfg, &factory, &paths, target_repo.as_deref()).await
}

/// 覆蓋寫入(使用者預覽後編輯過的頁面)＋事件 emit。
#[tauri::command]
pub fn factory_write_pages<R: Runtime>(
    app: AppHandle<R>,
    pages: Vec<WritePage>,
    target_repo: Option<String>,
) -> Result<WriteResult, AppError> {
    let cfg = app_cfg(&app)?;
    let notes = PathBuf::from(target_repo.unwrap_or_else(|| cfg.notes_repo_path.clone()));
    let result = write_pages_core(&notes, &pages);
    let state = app.state::<AppState>();
    emit_factory_events(&state, &cfg, &pages);
    Ok(result)
}

/// 重建 companies:掃描 people/ 的 `公司/組織:` bullet → companies/*.md。
/// enriched 頁(`enriched: true` 或 `<!-- enriched -->`)凍結不覆蓋。
#[tauri::command]
pub fn extract_companies_run<R: Runtime>(
    app: AppHandle<R>,
    clean: bool,
    target_repo: Option<String>,
) -> Result<WriteResult, AppError> {
    let cfg = app_cfg(&app)?;
    extract_companies_core(&cfg, clean, target_repo.as_deref())
}
