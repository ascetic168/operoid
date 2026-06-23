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
    pub note: Option<String>,
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

fn env_for_child(cfg: &config::AppConfig) -> Vec<(&'static str, std::ffi::OsString)> {
    let mut env: Vec<(&'static str, std::ffi::OsString)> =
        vec![("PYTHONUTF8", "1".into())];
    if let Some(home) = cfg.gbrain_home_override.as_ref().filter(|h| !h.is_empty()) {
        env.push(("GBRAIN_HOME", home.clone().into()));
    }
    env
}

/// 跑一個子行程，逐行把 stdout/stderr 透過 channel 推給前端；回傳 exit code。
async fn run_child<R: Runtime>(
    _app: &AppHandle<R>,
    ch: &Channel<CliLine>,
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    env: &[(&str, std::ffi::OsString)],
) -> std::io::Result<i32> {
    let mut cmd = Command::new(program);
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

fn resolve_gbrain<R: Runtime>(app: &AppHandle<R>) -> Result<(config::AppConfig, String), String> {
    let cfg = config::app_config::load(app).map_err(|e| e.to_string())?;
    let exe = cfg.gbrain_exe_path.clone();
    if !Path::new(&exe).exists() {
        return Err(format!("找不到 gbrain 執行檔：{exe}（請到設定頁修正 gbrain.exe 路徑）"));
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
) -> Result<OpResult, String> {
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
            let slug = arg.ok_or("graph-query 需要 slug")?;
            let code = run!(&["graph-query", &slug]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "ask" => {
            let q = arg.ok_or("ask 需要查詢字串")?;
            let code = run!(&["ask", &q]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "think" => {
            let raw = arg.ok_or("think 需要問題")?;
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
        other => Err(format!("未知操作：{other}")),
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

/// sync 完整流程：git add+commit → gbrain sync →（偵測 defer）embed --stale → extract --stale。
async fn run_sync<R: Runtime>(
    app: &AppHandle<R>,
    ch: &Channel<CliLine>,
    exe: &str,
    notes: &Path,
    env: &[(&str, std::ffi::OsString)],
    cfg: &config::AppConfig,
) -> Result<OpResult, String> {
    if !notes.exists() {
        return Err(format!("notes repo 不存在：{}", notes.display()));
    }
    // git add -A（無 remote，永不 push）
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ git add -A".into() });
    let _ = run_child(app, ch, "git", &["add", "-A"], Some(notes), &[]).await;

    // git commit（可能「nothing to commit」→ 非零，視為無新變更但繼續）
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let msg = format!("GBrainStudio sync {stamp}");
    let _ = ch.send(CliLine { stream: "step".into(), text: "▶ git commit".into() });
    let commit_code = run_child(app, ch, "git", &["commit", "-m", &msg], Some(notes), &[])
        .await
        .map_err(|e| e.to_string())?;
    if commit_code != 0 {
        let _ = ch.send(CliLine {
            stream: "step".into(),
            text: "（無新變更可 commit；仍繼續 sync 已 commit 的差異）".into(),
        });
    }

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
        note: Some("sync 完成（commit + sync + embed --stale + extract --stale）".into()),
    })
}
