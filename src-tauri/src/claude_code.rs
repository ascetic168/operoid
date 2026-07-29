//! 從 Emploid 啟動 Claude Code：以所選腦的 GBRAIN_HOME 啟動 gbrain MCP server，
//! 在使用者選的工作目錄、用指定的終端機，執行 `claude --mcp-config <file>`。

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::config;
use crate::gbrain_cli::no_console;
use crate::i18n::AppError;

#[derive(Debug, Serialize)]
pub struct TerminalInfo {
    pub id: String,
    pub label: String,
    pub available: bool,
}

#[derive(Debug, Serialize)]
pub struct ClaudeStatus {
    pub claude_installed: bool,
    pub claude_version: Option<String>,
    pub gbrain_exe: String,
    pub gbrain_ready: bool,
    pub terminals: Vec<TerminalInfo>,
}

/// 偵測 claude CLI。Windows 的 `claude` 是 `.cmd` shim，須 `cmd /C` 解析。
/// 偵測用 no_console 壓制閃窗；正式 spawn_terminal **不可**套 no_console。
fn probe_claude() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let mut c = std::process::Command::new("cmd");
        c.raw_arg("/C claude --version");
        no_console(&mut c);
        let out = c.output().ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = std::process::Command::new("claude");
        c.arg("--version");
        no_console(&mut c);
        let out = c.output().ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// 偵測某指令是否在 PATH 上。Windows `where`（認 .cmd/.exe）；unix `which`。
fn which(bin: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let mut c = std::process::Command::new("cmd");
        c.raw_arg(format!("/C where {bin}"));
        no_console(&mut c);
        c.output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = std::process::Command::new("which");
        c.arg(bin);
        no_console(&mut c);
        c.output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }
}

/// 本平台內建 profile 清單（已按優先序；首個 available 即預設）。
fn list_terminals() -> Vec<TerminalInfo> {
    let t = |id: &str, label: &str, available: bool| TerminalInfo {
        id: id.into(),
        label: label.into(),
        available,
    };
    #[cfg(target_os = "windows")]
    {
        vec![
            t("wt", "Windows Terminal", which("wt")),
            t("powershell", "Windows PowerShell", which("powershell")),
            t("pwsh", "PowerShell 7+", which("pwsh")),
            t("cmd", "命令提示字元 (cmd)", true),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            t("terminal", "Terminal.app", true),
            t("iterm", "iTerm2", Path::new("/Applications/iTerm.app").exists()),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            t("gnome-terminal", "GNOME Terminal", which("gnome-terminal")),
            t("konsole", "Konsole", which("konsole")),
            t("xterm", "xterm", which("xterm")),
        ]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// 以內建 profile 開 terminal 執行 claude_cmd（含引號的完整指令），工作目錄=cwd。
fn spawn_profile(id: &str, cwd: &Path, claude_cmd: &str) -> std::io::Result<()> {
    let cwd_s = cwd.to_string_lossy().to_string();
    match id {
        #[cfg(target_os = "windows")]
        "wt" => {
            use std::os::windows::process::CommandExt as _;
            // wt 在 cwd 開新視窗；包 cmd /K 以解析 claude.cmd。
            let mut c = std::process::Command::new("wt");
            c.raw_arg(format!("-d \"{cwd_s}\" cmd /K {claude_cmd}"));
            c.spawn()?;
        }
        #[cfg(target_os = "windows")]
        "powershell" | "pwsh" => {
            std::process::Command::new(id)
                .args(["-NoExit", "-Command", &format!("Set-Location '{cwd_s}'; {claude_cmd}")])
                .spawn()?;
        }
        #[cfg(target_os = "windows")]
        "cmd" => {
            use std::os::windows::process::CommandExt as _;
            let mut c = std::process::Command::new("cmd");
            c.raw_arg(format!("/K {claude_cmd}"));
            c.current_dir(cwd);
            c.spawn()?;
        }
        #[cfg(target_os = "macos")]
        "terminal" => {
            let script = format!(
                "tell application \"Terminal\" to do script \"cd \\\"{cwd_s}\\\" && {claude_cmd}\""
            );
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn()?;
        }
        #[cfg(target_os = "macos")]
        "iterm" => {
            let script = format!(
                "tell application \"iTerm\" to create window with default profile\n\
                 tell current session of current window to write text \"cd \\\"{cwd_s}\\\" && {claude_cmd}\""
            );
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn()?;
        }
        #[cfg(target_os = "linux")]
        "gnome-terminal" => {
            std::process::Command::new("gnome-terminal")
                .args(["--working-directory", &cwd_s, "--", "sh", "-c", &format!("{claude_cmd}; exec sh")])
                .spawn()?;
        }
        #[cfg(target_os = "linux")]
        "konsole" => {
            std::process::Command::new("konsole")
                .args(["--workdir", &cwd_s, "-e", "sh", "-c", &format!("{claude_cmd}; exec sh")])
                .spawn()?;
        }
        #[cfg(target_os = "linux")]
        "xterm" => {
            std::process::Command::new("xterm")
                .args(["-e", "sh", "-c", &format!("cd '{cwd_s}' && {claude_cmd}")])
                .spawn()?;
        }
        other => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown terminal: {other}"),
            ));
        }
    }
    Ok(())
}

/// 自訂範本：{cwd}/{cmd} 套換後經 OS shell 執行，並以 current_dir 綁 cwd。
/// 預設範本 `{cmd}` 即可（cwd 自動綁）；要 WezTerm/Git Bash 等自行填，
/// 如 `wt -d "{cwd}" cmd /k {cmd}`。
fn spawn_custom(template: &str, cwd: &Path, claude_cmd: &str) -> std::io::Result<()> {
    let rendered = template
        .replace("{cwd}", &cwd.to_string_lossy())
        .replace("{cmd}", claude_cmd);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let mut c = std::process::Command::new("cmd");
        c.raw_arg(format!("/C {rendered}"));
        c.current_dir(cwd);
        c.spawn()?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("sh")
            .args(["-c", &rendered])
            .current_dir(cwd)
            .spawn()?;
    }
    Ok(())
}

#[tauri::command]
pub fn claude_code_status<R: Runtime>(app: AppHandle<R>) -> Result<ClaudeStatus, AppError> {
    let cfg = config::app_config::load(&app).unwrap_or_default();
    let version = probe_claude();
    Ok(ClaudeStatus {
        claude_installed: version.is_some(),
        claude_version: version,
        gbrain_exe: cfg.gbrain_exe_path.clone(),
        gbrain_ready: Path::new(&cfg.gbrain_exe_path).exists(),
        terminals: list_terminals(),
    })
}

/// 寫本次啟動的 MCP 設定檔（gbrain serve + GBRAIN_HOME + PYTHONUTF8），回傳路徑。
fn write_mcp_config(
    app_data: &Path,
    gbrain_exe: &str,
    brain_home: Option<&str>,
) -> Result<PathBuf, AppError> {
    let mut env = serde_json::Map::new();
    env.insert("PYTHONUTF8".into(), serde_json::Value::String("1".into()));
    if let Some(h) = brain_home {
        env.insert("GBRAIN_HOME".into(), serde_json::Value::String(h.into()));
    }
    let cfg = serde_json::json!({
        "mcpServers": { "gbrain": { "type": "stdio", "command": gbrain_exe, "args": ["serve"], "env": env } }
    });
    let path = app_data.join("claude-gbrain-mcp.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path)
}

#[tauri::command]
pub fn claude_code_launch<R: Runtime>(
    app: AppHandle<R>,
    brain_id: Option<String>,
    cwd: String,
    // profile id；None=取首個可用內建 profile；"custom"=用 template
    terminal: Option<String>,
    template: Option<String>,
) -> Result<(), AppError> {
    let cfg = config::app_config::load(&app).map_err(|e| e.to_string())?;
    let cwd_path = Path::new(&cwd);
    if !cwd_path.is_dir() {
        return Err(AppError::new("claude.cwdNotFound").p("cwd", &cwd));
    }
    if !Path::new(&cfg.gbrain_exe_path).exists() {
        return Err(AppError::new("gbrain.exeNotFound").p("path", &cfg.gbrain_exe_path));
    }

    // 解析腦（指定 or 作用中）→ GBRAIN_HOME（None = 預設腦 ~/.gbrain，不設 env）。
    let brain_home: Option<String> = brain_id
        .as_deref()
        .and_then(|id| cfg.brains.iter().find(|b| b.id == id))
        .or_else(|| cfg.active_brain())
        .and_then(|b| b.env_home().map(|s| s.to_string()));

    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
    let config_path = write_mcp_config(&app_data, &cfg.gbrain_exe_path, brain_home.as_deref())?;
    let claude_cmd = format!("claude --mcp-config \"{}\"", config_path.to_string_lossy());

    match terminal.as_deref() {
        Some("custom") => {
            let tpl = template
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| AppError::new("claude.noCustomTemplate"))?;
            spawn_custom(&tpl, cwd_path, &claude_cmd).map_err(|e| e.to_string())?;
        }
        None => {
            let id = list_terminals()
                .into_iter()
                .find(|t| t.available)
                .map(|t| t.id)
                .unwrap_or_else(|| "cmd".into());
            spawn_profile(&id, cwd_path, &claude_cmd).map_err(|e| e.to_string())?;
        }
        Some(id) => {
            spawn_profile(id, cwd_path, &claude_cmd).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
