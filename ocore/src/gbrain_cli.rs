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

use serde::{Deserialize, Serialize};
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
#[derive(Serialize, Deserialize, Clone)]
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

// ── think --json：結構化輸出解析＋人類可讀重排 ─────────────────────────────

/// `gbrain think --json` 的 citations 元素（cite-render.ts ParsedCitation）。
#[derive(Deserialize)]
struct ThinkCitation {
    page_slug: String,
    row_num: Option<u64>,
}

/// `gbrain think --json` 輸出的最小欄位集（缺項一律寬容為預設）。
#[derive(Deserialize)]
struct ThinkJson {
    #[serde(default)]
    question: String,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    citations: Vec<ThinkCitation>,
    #[serde(default)]
    gaps: Vec<String>,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default, rename = "modelUsed")]
    model_used: Option<String>,
    #[serde(default, rename = "pagesGathered")]
    pages_gathered: Option<u64>,
    #[serde(default, rename = "takesGathered")]
    takes_gathered: Option<u64>,
    #[serde(default, rename = "graphHits")]
    graph_hits: Option<u64>,
}

fn push_stdout(ch: &LineSink, text: &str) {
    ch(CliLine { stream: "stdout".into(), text: text.to_string() });
}

/// 執行 `gbrain think --json`：stderr 即時串流；stdout 整段捕獲後解析，
/// 以人類可讀格式重排推送，並在最後條列全部引註（`[dir/slug]` 形式，
/// 前端 OperationsView.linkSegments 會渲染成連結）。解析失敗或非零退出
/// 時逐行原樣輸出 stdout（等同舊行為的寬容退路）。
async fn run_think_json(
    ch: &LineSink,
    program: &str,
    args: &[&str],
    env: &[(&'static str, std::ffi::OsString)],
) -> i32 {
    ch(CliLine { stream: "step".into(), text: "think：合成中…".into() });
    let (code, stdout, stderr) = match run_capture(program, args, env).await {
        Ok(r) => r,
        Err(e) => {
            ch(CliLine { stream: "stderr".into(), text: format!("think: spawn 失敗：{e}") });
            return -1;
        }
    };
    // run_capture 無法邊跑邊串流 stderr（gbrain 的進度／升級提示走 stderr），
    // 結束後補推，確保不吞訊息。
    for line in stderr.lines() {
        if !line.is_empty() {
            ch(CliLine { stream: "stderr".into(), text: line.to_string() });
        }
    }
    if code != 0 {
        for line in stdout.lines() {
            if !line.is_empty() { push_stdout(ch, line); }
        }
        return code;
    }
    let parsed: ThinkJson = match serde_json::from_str(&stdout) {
        Ok(p) => p,
        Err(e) => {
            // 非 JSON（舊版 gbrain、上游格式變更）→ 原樣逐行輸出，不讓操作失敗。
            ch(CliLine {
                stream: "stderr".into(),
                text: format!("think: --json 輸出解析失敗（{e}），改以純文字顯示"),
            });
            for line in stdout.lines() {
                if !line.is_empty() { push_stdout(ch, line); }
            }
            return code;
        }
    };

    push_stdout(ch, &format!("# {}", parsed.question));
    if !parsed.answer.is_empty() {
        // 模型（尤其 glm-4-flash）常把行內引註退化成 `[slug#N]` 佔位字面值，
        // 對讀者毫無資訊——從顯示本文中剔除（引註以清單呈現）。
        let cleaned = regex::Regex::new(r"\[slug(?:#\d+)?\]")
            .expect("static regex")
            .replace_all(&parsed.answer, "")
            .trim_end()
            .to_string();
        for line in cleaned.lines() {
            push_stdout(ch, line);
        }
    }
    if !parsed.gaps.is_empty() {
        push_stdout(ch, "## Gaps");
        for g in &parsed.gaps {
            push_stdout(ch, &format!("- {g}"));
        }
    }
    push_stdout(ch, "---");
    push_stdout(ch, &format!(
        "Model: {} | Pages: {} | Takes: {} | Graph: {} | Citations: {}",
        parsed.model_used.unwrap_or_default(),
        parsed.pages_gathered.unwrap_or(0),
        parsed.takes_gathered.unwrap_or(0),
        parsed.graph_hits.unwrap_or(0),
        parsed.citations.len(),
    ));
    // 引註清單：頁面級 `[dir/slug]`、take 級附 row。slug 中的 `/` 保留在括號內，
    // 符合 linkSegments 的單括號引註格式（不可含空白）。
    if !parsed.citations.is_empty() {
        push_stdout(ch, "## 引註（Citations）");
        for c in &parsed.citations {
            push_stdout(ch, &format!("- {}", citation_line(c)));
        }
    }
    // 隱去行內/結構化引註比對警告：中文 slug 不符合 gbrain 行內標記的 ASCII
    // regex，且 glm-4-flash 常在本文留下 `[slug#N]` 佔位標記，兩個方向的
    // 比對警告對本應用恆為雜訊；引註已由下方清單完整列出。
    let warnings: Vec<&str> = parsed
        .warnings
        .iter()
        .map(|w| w.as_str())
        .filter(|w| {
            !w.contains("CITATIONS_STRUCTURED_NOT_INLINE") && !w.contains("CITATIONS_INLINE_NOT_IN_STRUCTURED")
        })
        .collect();
    if !warnings.is_empty() {
        ch(CliLine {
            stream: "stderr".into(),
            text: format!("Warnings: {}", warnings.join(", ")),
        });
    }
    code
}

/// 單筆引註行：雙括號 `[[slug]]`（wikilink 形式，linkSegments 無條件匹配，
/// 且不受 slug 需含 `/` 的單括號規則限制——模型給的 page_slug 可能是
/// `people/林家豪` 也可能是裸標題 `林家豪`）；take 級附 row 註記。
fn citation_line(c: &ThinkCitation) -> String {
    match c.row_num {
        Some(n) => format!("[[{}]]（take #{n}）", c.page_slug),
        None => format!("[[{}]]", c.page_slug),
    }
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
/// stats|sync|extract|embed|ask|query|think|doctor|orphans|storage|graph-query|unify-types。
/// `arg` 為 ask/query/think/graph-query 的查詢或 slug；think 可用 `anchor:<slug>` 前綴。
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
        // query＝混合檢索（向量＋關鍵字＋RRF 融合，無 LLM 合成；gbrain v0.46 的 `search`
        // 只是 tsvector 關鍵字搜尋，混合檢索叫 `query`）。
        "query" => {
            let q = arg.ok_or_else(|| AppError::new("op.needArg").p("op", "query"))?;
            let code = run!(&["query", &q]).map_err(|e| e.to_string())?;
            Ok(OpResult::from_code(code))
        }
        // schema pack v1（gbrain-base）→v2（gbrain-base-v2）遷移：提交 unify-types Minion
        // job（retype 舊 24 型→15 標準型）。僅供設定頁的提示按鈕手動觸發，不自動執行。
        // job handler 必帶 `target_pack`，缺參數會 permanent fail（實測 gbrain 0.46）；
        // `--follow` 讓 PGLite 腦 inline 執行（背景 worker 需要 Postgres，PGLite 無法跑）。
        "unify-types" => {
            let code = run!(&[
                "jobs",
                "submit",
                "unify-types",
                "--params",
                r#"{"target_pack":"gbrain-base-v2","apply":true}"#,
                "--follow",
            ])
            .map_err(|e| e.to_string())?;
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
            // 傳 `--model`，跳過 gbrain fallback 鏈（models.think→default→$GBRAIN_MODEL→opus）。
            // E9 原修復只蓋 agent 路徑，此處 DB-plane 未設時同樣 fallback 到 opus → synthesis skipped。
            // 2026-08-30 改取 models.think（缺時 fallback chat_model）：thinking 模型
            // 長合成會 LLM_OUTPUT_TRUNCATED，think 應走 config 指定的合成專用模型。
            if let Some(m) = crate::gbrain_config::load_for(cfg.active_env_home())
                .ok()
                .and_then(|l| l.config.think_model().map(|s| s.to_string()))
            {
                args.push("--model".into());
                args.push(m);
            }
            if let Some(a) = anchor {
                args.push("--anchor".into());
                args.push(a);
            }
            // 以 --json 取得結構化 citations（純文字輸出只在 footer 顯示數量、
            // 不列出被引頁面），再重排成人類可讀格式。引註以 [dir/slug] 條列，
            // OperationsView 的 linkSegments 會將其渲染成可點擊連結。
            args.push("--json".into());
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let code = run_think_json(ch, exe, &refs, &env).await;
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

    /// 引註行格式：雙括號 `[[slug]]`（linkSegments 無條件匹配，支援無 `/` 的
    /// 裸標題 slug）；take 級附 row。
    #[test]
    fn citation_line_matches_link_segment_format() {
        let page = ThinkCitation { page_slug: "people/林家豪".into(), row_num: None };
        assert_eq!(citation_line(&page), "[[people/林家豪]]");
        let bare = ThinkCitation { page_slug: "林家豪".into(), row_num: None };
        assert_eq!(citation_line(&bare), "[[林家豪]]");
        let take = ThinkCitation { page_slug: "meetings/2026-06-15".into(), row_num: Some(3) };
        assert_eq!(citation_line(&take), "[[meetings/2026-06-15]]（take #3）");
    }
}
