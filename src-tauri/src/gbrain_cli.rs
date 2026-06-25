//! gbrain CLI 包裝 — spawn gbrain.exe / git、串流輸出、寬容解碼。
//!
//! 操作（stat/sync/extract/ask/think + 診斷）對應 v0.42.51 指令（見 plan）。
//! 輸出以 Tauri Channel 逐行串流到前端；最終結果由指令回傳值交付。
//! Windows 編碼：子行程設 PYTHONUTF8=1；stdout 先嘗試 UTF-8，失敗退 cp950(BIG5)。

use std::path::Path;
use std::process::Stdio;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::config;
use crate::i18n::{AppError, L10n};

/// 串流到前端的一行輸出。
#[derive(Clone, Serialize)]
pub struct CliLine {
    pub stream: String, // "stdout" | "stderr" | "step"
    pub text: String,
}

/// 指令最終結果。
#[derive(Serialize)]
pub struct OpResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub note: Option<L10n>,
}

/// 寬容解碼：UTF-8 優先，失敗退 BIG5(cp950)，去尾換行。
fn decode_line(bytes: &[u8]) -> String {
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::BIG5.decode(bytes);
            cow.into_owned()
        }
    };
    s.trim_end_matches(['\r', '\n']).to_string()
}

/// 子行程環境：PYTHONUTF8=1 + 作用中腦的 GBRAIN_HOME（None=預設腦，不設）。
pub(crate) fn env_for_child(cfg: &config::AppConfig) -> Vec<(&'static str, std::ffi::OsString)> {
    env_for_brain(cfg.active_env_home())
}

/// 由顯式 home（GBRAIN_HOME 值；None=預設腦）組子行程環境。
pub(crate) fn env_for_brain(home: Option<&str>) -> Vec<(&'static str, std::ffi::OsString)> {
    let mut env: Vec<(&'static str, std::ffi::OsString)> = vec![("PYTHONUTF8", "1".into())];
    if let Some(h) = home.map(str::trim).filter(|h| !h.is_empty()) {
        env.push(("GBRAIN_HOME", h.into()));
    }
    env
}

/// 寬容解碼整段 buffer：UTF-8 優先，失敗退 BIG5(cp950)。
pub(crate) fn decode_buf(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::BIG5.decode(bytes);
            cow.into_owned()
        }
    }
}

/// 壓制 Windows console 子視窗。release build 為 GUI 子系統（見 `main.rs` 的
/// `windows_subsystem = "windows"`），spawn 子行程時 Windows 會為其配置一個新 console，
/// 造成黑色視窗閃現（dev 因附掛終端機不會）。設 `CREATE_NO_WINDOW` 即可避免。非 Windows 為 no-op。
///
/// `std::process::Command` 的 `creation_flags` 來自 std 的 `CommandExt` trait；
/// `tokio::process::Command` 則是自帶 inherent method（且未實作 std 的 `CommandExt`），
/// 故分兩個函式，無法用單一泛型涵蓋。
#[cfg(windows)]
pub(crate) fn no_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt as _;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
#[allow(unused_variables)]
pub(crate) fn no_console(cmd: &mut std::process::Command) {}

/// 同上，給 `tokio::process::Command`（串流/捕獲子行程用）。
#[cfg(windows)]
pub(crate) fn no_console_async(cmd: &mut Command) {
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub(crate) fn no_console_async(_cmd: &mut Command) {}

/// 跑一個子行程並**捕獲**整段 stdout（不串流），回傳 (exit_code, stdout)。
/// 給需要解析 JSON 輸出的指令（如 `sources list --json`）用。
pub(crate) async fn run_capture<R: Runtime>(
    _app: &AppHandle<R>,
    program: &str,
    args: &[&str],
    env: &[(&str, std::ffi::OsString)],
) -> std::io::Result<(i32, String, String)> {
    let mut cmd = Command::new(program);
    no_console_async(&mut cmd);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let out = cmd.output().await?;
    let code = out.status.code().unwrap_or(-1);
    Ok((code, decode_buf(&out.stdout), decode_buf(&out.stderr)))
}

/// 跑一個子行程，逐行把 stdout/stderr 透過 channel 推給前端；回傳 exit code。
pub(crate) async fn run_child<R: Runtime>(
    _app: &AppHandle<R>,
    ch: &Channel<CliLine>,
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    env: &[(&str, std::ffi::OsString)],
) -> std::io::Result<i32> {
    let mut cmd = Command::new(program);
    no_console_async(&mut cmd);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child: Child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    // stderr 另開一個 task，邊收邊推；channel 可跨 task（Clone）。
    let ch2 = ch.clone();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = ch2.send(CliLine {
                stream: "stderr".into(),
                text: line,
            });
        }
    });

    // stdout 用原始位元組讀（read_until + buffer），以便寬容解碼 cp950。
    let mut reader = BufReader::new(stdout);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf).await {
            Ok(n) => n,
            Err(_) => break,
        };
        if n == 0 {
            break;
        }
        let text = decode_line(&buf);
        if !text.is_empty() {
            let _ = ch.send(CliLine {
                stream: "stdout".into(),
                text,
            });
        }
    }
    let _ = stderr_task.await;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}

fn resolve_gbrain<R: Runtime>(app: &AppHandle<R>) -> Result<(config::AppConfig, String), AppError> {
    let cfg = config::app_config::load(app).map_err(|e| e.to_string())?;
    let exe = cfg.gbrain_exe_path.clone();
    if !Path::new(&exe).exists() {
        return Err(AppError::new("gbrain.exeNotFound").p("path", &exe));
    }
    Ok((cfg, exe))
}

/// 統一操作分派。`op` ∈ stats|sync|extract|embed|ask|think|doctor|orphans|storage|graph-query。
/// `arg` 為 ask/think/graph-query 的查詢或 slug；think 可用 `anchor:<slug>` 前綴。
#[tauri::command]
pub async fn op_run<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<CliLine>,
    op: String,
    arg: Option<String>,
) -> Result<OpResult, AppError> {
    let (cfg, exe) = resolve_gbrain(&app)?;
    let env = env_for_child(&cfg);
    let notes = cfg.notes_repo_path.clone();
    let notes_path = Path::new(&notes);

    macro_rules! run {
        ($args:expr) => {
            run_child(&app, &on_event, &exe, $args, None, &env).await
        };
    }

    match op.as_str() {
        "stats" => {
            let code = run!(&["stats"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "extract" => {
            let code = run!(&["extract", "--stale"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "embed" => {
            let code = run!(&["embed", "--stale"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "doctor" => {
            let code = run!(&["doctor", "--fast"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "orphans" => {
            let code = run!(&["orphans"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "storage" => {
            let code = run!(&["storage", "status"]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "graph-query" => {
            let slug = arg.ok_or_else(|| AppError::new("op.needArg").p("op", "graph-query"))?;
            let code = run!(&["graph-query", &slug]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "ask" => {
            let q = arg.ok_or_else(|| AppError::new("op.needArg").p("op", "ask"))?;
            let code = run!(&["ask", &q]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "think" => {
            let raw = arg.ok_or_else(|| AppError::new("op.needArg").p("op", "think"))?;
            // 支援 "anchor:<slug>\n<question>" 把 --anchor 拆出來
            let (anchor, question) = match raw.strip_prefix("anchor:") {
                Some(rest) => match rest.split_once('\n') {
                    Some((slug, q)) => (Some(slug.to_string()), q.to_string()),
                    None => (None, rest.to_string()),
                },
                None => (None, raw),
            };
            let mut args: Vec<String> = vec!["think".into(), question];
            if let Some(a) = anchor {
                args.push("--anchor".into());
                args.push(a);
            }
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let code = run!(&refs).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "sync" => run_sync(&app, &on_event, &exe, notes_path, &env, &cfg).await,
        other => Err(AppError::new("op.unknown").p("op", other)),
    }
}

impl OpResult {
    fn from_code(code: i32) -> Self {
        OpResult {
            success: code == 0,
            exit_code: Some(code),
            note: None,
        }
    }
}

/// git add -A + commit（best-effort：非零退出碼＝無新變更，不視為錯誤）。
/// 用於 sync 前確保 working-tree 變更已進 git（gbrain sync 是 git-based incremental，
/// 未 commit 的變更不會被同步）。回傳 commit 的 exit code；io 層級錯誤（指令啟動失敗）
/// 以 Err 傳播，由呼叫者決定是否中斷。
pub(crate) async fn git_add_commit<R: Runtime>(
    app: &AppHandle<R>,
    ch: &Channel<CliLine>,
    repo: &Path,
) -> std::io::Result<i32> {
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ git add -A".into() });
    // add 失敗不中斷（best-effort）；commit 才回傳結果。
    let _ = run_child(app, ch, "git", &["add", "-A"], Some(repo), &[]).await;

    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let msg = format!("GBrainStudio sync {stamp}");
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ git commit".into() });
    let commit_code = run_child(app, ch, "git", &["commit", "-m", &msg], Some(repo), &[]).await?;
    if commit_code != 0 {
        let _ = ch.send(CliLine {
            stream: "step".into(),
            text: "（無新變更可 commit；仍繼續 sync 已 commit 的差異）".into(),
        });
    }
    Ok(commit_code)
}

/// sync 完整流程：git add+commit → gbrain sync →（偵測 defer）embed --stale → extract --stale。
async fn run_sync<R: Runtime>(
    app: &AppHandle<R>,
    ch: &Channel<CliLine>,
    exe: &str,
    notes: &Path,
    env: &[(&str, std::ffi::OsString)],
    cfg: &config::AppConfig,
) -> Result<OpResult, AppError> {
    if !notes.exists() {
        return Err(AppError::new("op.notesNotFound").p("path", notes.display()));
    }
    // git add -A + commit（io 錯誤才中斷；無新變更不中斷）。
    let _ = git_add_commit(app, ch, notes).await.map_err(|e| e.to_string())?;

    // gbrain sync --repo <notes> [--no-pull] --yes
    let notes_str = notes.to_string_lossy().into_owned();
    let mut sync_args: Vec<String> = vec!["sync".into(), "--repo".into(), notes_str, "--yes".into()];
    if cfg.sync_no_pull {
        sync_args.insert(3, "--no-pull".into());
    }
    let refs: Vec<&str> = sync_args.iter().map(|s| s.as_str()).collect();
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ gbrain sync".into() });
    let code = run_child(app, ch, exe, &refs, None, env)
        .await
        .map_err(|e| e.to_string())?;

    // 偵測 defer：sync 大批次會印 "deferring"。這裡以 doctor 檢查 stale 為輔；
    // 簡單起見，sync 後一律補 embed --stale + extract --stale（idempotent、安全）。
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ gbrain embed --stale".into() });
    let _ = run_child(app, ch, exe, &["embed", "--stale"], None, env).await;
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ gbrain extract --stale".into() });
    let _ = run_child(app, ch, exe, &["extract", "--stale"], None, env).await;

    Ok(OpResult {
        success: code == 0,
        exit_code: Some(code),
        note: Some(L10n::new("op.syncDone")),
    })
}
