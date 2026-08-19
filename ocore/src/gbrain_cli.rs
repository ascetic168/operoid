//! gbrain CLI 包裝（P1c 起居於 ocore）— spawn gbrain.exe / git、串流輸出、寬容解碼。
//!
//! 操作（stat/sync/extract/ask/think + 診斷）對應 v0.42.51 指令。
//! **串流抽象（`Channel<CliLine>` 手術）**：原 Tauri `ipc::Channel` 改為 [`LineSink`]
//! （`Arc<dyn Fn(CliLine)>` 回呼）——桌面殼橋接回 Tauri Channel、未來 oserver 橋接到
//! SSE／ring buffer。[`noop_sink`] 供 fire-and-forget 呼叫（E8 的 brain_sync 前置）。
//! Windows 編碼：子行程設 PYTHONUTF8=1；stdout 先嘗試 UTF-8，失敗退 cp950(BIG5)。

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::app_config::AppConfig;
use crate::i18n::{AppError, L10n};
use crate::proc::{decode_buf, env_for_brain, no_console_async};

/// 串流事件：一行輸出（stdout/stderr）或一個步驟標記（step）。
#[derive(Clone, Debug, Serialize)]
pub struct CliLine {
    pub stream: String, // "stdout" | "stderr" | "step"
    pub text: String,
}

/// 串流事件接收器（P1c：取代 Tauri `Channel<CliLine>`）。
/// 殼層橋接：桌面 app 以閉包包 `Channel::send`；oserver 未來橋接 SSE／輪詢 ring buffer。
pub type LineSink = Arc<dyn Fn(CliLine) + Send + Sync>;

/// 無輸出的 sink（fire-and-forget 呼叫用——E8 的 dispatch_event→brain_sync 路徑）。
pub fn noop_sink() -> LineSink {
    Arc::new(|_line| {})
}

/// 指令最終結果。
#[derive(Serialize)]
pub struct OpResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub note: Option<L10n>,
}

impl OpResult {
    pub fn from_code(code: i32) -> Self {
        OpResult {
            success: code == 0,
            exit_code: Some(code),
            note: None,
        }
    }
}

/// 寬容解碼：UTF-8 優先，失敗退 BIG5(cp950)，去尾換行。
fn decode_line(bytes: &[u8]) -> String {
    let s = decode_buf(bytes);
    s.trim_end_matches(['\r', '\n']).to_string()
}

/// 子行程環境：PYTHONUTF8=1 + 作用中腦的 GBRAIN_HOME（None=預設腦，不設）。
pub fn env_for_child(cfg: &AppConfig) -> Vec<(&'static str, std::ffi::OsString)> {
    env_for_brain(cfg.active_env_home())
}

/// 跑一個子行程並**捕獲**整段 stdout（不串流），回傳 (exit_code, stdout, stderr)。
/// 給需要解析 JSON 輸出的指令（如 `sources list --json`）用。
pub async fn run_capture(
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

// ── `gbrain config` 子行程包裝（v0.42 DB-plane 讀寫） ────────────────────

/// `gbrain config get <key>`：回傳 (值, 來源 plane)。key 不存在 → None。
pub async fn config_get(
    exe: &str,
    home: Option<&str>,
    key: &str,
) -> Result<Option<(String, String)>, AppError> {
    let env = env_for_brain(home);
    let (code, stdout, _stderr) = run_capture(exe, &["config", "get", key], &env)
        .await
        .map_err(|e| AppError::new("gbrain.configCliFail").p("detail", e.to_string()))?;
    if code != 0 {
        return Ok(None); // key 不存在
    }
    let mut lines = stdout.lines();
    let value = lines.next().unwrap_or("").trim().to_string();
    let source = lines
        .next()
        .and_then(|l| l.split("source:").nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some((value, source)))
    }
}

/// `gbrain config set <key> <value>`：寫入 DB plane。value 不可為空（CLI 會拒絕）。
pub async fn config_set(
    exe: &str,
    home: Option<&str>,
    key: &str,
    value: &str,
) -> Result<(), AppError> {
    let env = env_for_brain(home);
    let (code, _stdout, stderr) = run_capture(exe, &["config", "set", key, value], &env)
        .await
        .map_err(|e| AppError::new("gbrain.configCliFail").p("detail", e.to_string()))?;
    if code != 0 {
        return Err(AppError::new("gbrain.configSetFail")
            .p("key", key)
            .p("detail", stderr));
    }
    Ok(())
}

/// `gbrain config unset <key>`：從 DB plane 移除（讓 file plane 或 default 生效）。
pub async fn config_unset(
    exe: &str,
    home: Option<&str>,
    key: &str,
) -> Result<(), AppError> {
    let env = env_for_brain(home);
    let (code, _stdout, stderr) = run_capture(exe, &["config", "unset", key], &env)
        .await
        .map_err(|e| AppError::new("gbrain.configCliFail").p("detail", e.to_string()))?;
    // unset 不存在的 key 也是成功（冪等）；僅在 CLI 真正報錯時失敗
    if code != 0 && !stderr.contains("not found") {
        return Err(AppError::new("gbrain.configUnsetFail")
            .p("key", key)
            .p("detail", stderr));
    }
    Ok(())
}

/// `gbrain config unset --pattern <prefix>`：批次移除 DB plane 中符合前綴的鍵。
pub async fn config_unset_pattern(
    exe: &str,
    home: Option<&str>,
    pattern: &str,
) -> Result<(), AppError> {
    let env = env_for_brain(home);
    let (code, _stdout, stderr) =
        run_capture(exe, &["config", "unset", "--pattern", pattern], &env)
            .await
            .map_err(|e| AppError::new("gbrain.configCliFail").p("detail", e.to_string()))?;
    if code != 0 {
        return Err(AppError::new("gbrain.configUnsetFail")
            .p("key", pattern)
            .p("detail", stderr));
    }
    Ok(())
}

/// 跑一個子行程，逐行把 stdout/stderr 透過 sink 推出；回傳 exit code。
pub async fn run_child(
    ch: &LineSink,
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

    // stderr 另開一個 task，邊收邊推；sink 可跨 task（Clone）。
    let ch2 = Arc::clone(ch);
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            ch2(CliLine {
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
            ch(CliLine {
                stream: "stdout".into(),
                text,
            });
        }
    }
    let _ = stderr_task.await;
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}

/// git add -A + commit（best-effort：非零退出碼＝無新變更，不視為錯誤）。
/// 用於 sync 前確保 working-tree 變更已進 git（gbrain sync 是 git-based incremental，
/// 未 commit 的變更不會被同步）。回傳 commit 的 exit code；io 層級錯誤（指令啟動失敗）
/// 以 Err 傳播，由呼叫者決定是否中斷。
pub async fn git_add_commit(ch: &LineSink, repo: &Path) -> std::io::Result<i32> {
    ch(CliLine { stream: "step".into(), text: "▶ git add -A".into() });
    // add 失敗不中斷（best-effort）；commit 才回傳結果。
    let _ = run_child(ch, "git", &["add", "-A"], Some(repo), &[]).await;

    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let msg = format!("Operoid sync {stamp}");
    ch(CliLine { stream: "step".into(), text: "▶ git commit".into() });
    let commit_code = run_child(ch, "git", &["commit", "-m", &msg], Some(repo), &[]).await?;
    if commit_code != 0 {
        ch(CliLine {
            stream: "step".into(),
            text: "（無新變更可 commit；仍繼續 sync 已 commit 的差異）".into(),
        });
    }
    Ok(commit_code)
}

/// 確保 `repo` 是「有 commit 的 git repo」：不存在→建立；非 git→git init + 初始 commit。
/// 已是 git repo 則不動。gbrain `sync --repo` 要求目標有 HEAD（至少一個 commit），
/// 否則報 `No commits` 失敗。初始 commit 帶 `-c user.email/name` 防呆（機器可能無 git 身份）。
pub async fn git_init_commit(ch: &LineSink, repo: &Path) -> std::io::Result<()> {
    if repo.join(".git").exists() {
        return Ok(()); // 已是 git repo，不動
    }
    std::fs::create_dir_all(repo)?;
    ch(CliLine { stream: "step".into(), text: "▶ git init".into() });
    let _ = run_child(ch, "git", &["init"], Some(repo), &[]).await;
    ch(CliLine { stream: "step".into(), text: "▶ git add -A".into() });
    let _ = run_child(ch, "git", &["add", "-A"], Some(repo), &[]).await;
    // 初始 commit 帶 identity 防呆；非零（如「nothing to commit」於空目錄）不視為錯誤。
    ch(CliLine { stream: "step".into(), text: "▶ git commit (initial)".into() });
    let _ = run_child(
        ch,
        "git",
        &[
            "-c",
            "user.email=operoid@local",
            "-c",
            "user.name=Operoid",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ],
        Some(repo),
        &[],
    )
    .await;
    Ok(())
}

/// sync 完整流程（作用中腦的 notes repo）：git add+commit → gbrain sync → embed/extract --stale。
async fn run_sync(
    ch: &LineSink,
    exe: &str,
    notes: &Path,
    env: &[(&str, std::ffi::OsString)],
    cfg: &AppConfig,
) -> Result<OpResult, AppError> {
    if !notes.exists() {
        return Err(AppError::new("op.notesNotFound").p("path", notes.display()));
    }
    // git add -A + commit（io 錯誤才中斷；無新變更不中斷）。
    let _ = git_add_commit(ch, notes).await.map_err(|e| e.to_string())?;

    // gbrain sync --repo <notes> [--no-pull] --yes
    let notes_str = notes.to_string_lossy().into_owned();
    let mut sync_args: Vec<String> = vec!["sync".into(), "--repo".into(), notes_str, "--yes".into()];
    if cfg.sync_no_pull {
        sync_args.insert(3, "--no-pull".into());
    }
    let refs: Vec<&str> = sync_args.iter().map(|s| s.as_str()).collect();
    ch(CliLine { stream: "step".into(), text: "▶ gbrain sync".into() });
    let code = run_child(ch, exe, &refs, None, env)
        .await
        .map_err(|e| e.to_string())?;

    // 偵測 defer：sync 大批次會印 "deferring"。這裡以 doctor 檢查 stale 為輔；
    // 簡單起見，sync 後一律補 embed --stale + extract --stale（idempotent、安全）。
    ch(CliLine { stream: "step".into(), text: "▶ gbrain embed --stale".into() });
    let _ = run_child(ch, exe, &["embed", "--stale"], None, env).await;
    ch(CliLine { stream: "step".into(), text: "▶ gbrain extract --stale".into() });
    let _ = run_child(ch, exe, &["extract", "--stale"], None, env).await;

    Ok(OpResult {
        success: code == 0,
        exit_code: Some(code),
        note: Some(L10n::new("op.syncDone")),
    })
}

/// 統一操作分派（core，P1c：cfg/exe 由殼層解析傳入）。`op` ∈
/// stats|sync|extract|embed|ask|think|doctor|orphans|storage|graph-query。
/// `arg` 為 ask/think/graph-query 的查詢或 slug；think 可用 `anchor:<slug>` 前綴。
pub async fn op_run_core(
    cfg: &AppConfig,
    exe: &str,
    ch: &LineSink,
    op: &str,
    arg: Option<&str>,
) -> Result<OpResult, AppError> {
    let env = env_for_child(cfg);
    let notes = cfg.notes_repo_path.clone();
    let notes_path = Path::new(&notes);

    macro_rules! run {
        ($args:expr) => {
            run_child(ch, exe, $args, None, &env).await
        };
    }

    match op {
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
                None => (None, raw.to_string()),
            };
            let mut args: Vec<String> = vec!["think".into(), question];
            // E9 補遺（2026-08-18）：OperationsView 手動 think 比照 GbrainThinkTool 顯式
            // 傳 `--model`（作用中腦 config 的 chat_model），跳過 gbrain fallback 鏈
            // （models.think→default→$GBRAIN_MODEL→opus）。E9 原修復只蓋 agent 路徑，
            // 此處 DB-plane 未設時同樣 fallback 到 opus → synthesis skipped。
            if let Some(m) = crate::gbrain_config::load_for(cfg.active_env_home())
                .ok()
                .and_then(|l| l.config.chat_model)
            {
                args.push("--model".into());
                args.push(m);
            }
            if let Some(a) = anchor {
                args.push("--anchor".into());
                args.push(a);
            }
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let code = run!(&refs).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        "sync" => run_sync(ch, exe, notes_path, &env, cfg).await,
        other => Err(AppError::new("op.unknown").p("op", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// noop_sink 可建立且可作為 LineSink 傳遞（E8 fire-and-forget 前置的型別保證）。
    #[test]
    fn noop_sink_is_a_line_sink() {
        let sink: LineSink = noop_sink();
        sink(CliLine { stream: "step".into(), text: "x".into() }); // 不 panic、不輸出
    }
}
