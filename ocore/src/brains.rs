//! 腦（Brains）管理核心（P1c 起居於 ocore）——多腦（各 GBRAIN_HOME）+ 每腦多來源。
//!
//! gbrain 沒有「列出所有腦」的指令，故腦清單由系統自管（存於 AppConfig.brains）。
//! 每腦的 sources 則用 gbrain `sources` 即時查詢/增刪/同步。所有 gbrain 呼叫都帶
//! 該腦的 GBRAIN_HOME（非作用中腦也能檢視/操作其來源）。
//!
//! 核心 API 不收 AppHandle：cfg 由呼叫端載入傳入；串流走 [`LineSink`]。
//! 腦清單的持久化（load/save AppConfig）與 `#[tauri::command]` 層在桌面殼。
//! **E8 前置**：`sync_brain_core` 收任意 sink（含 [`noop_sink`]）——dispatch_event
//! 日後可 fire-and-forget 呼叫，不需前端串流 channel。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app_config::{brain_entry, AppConfig, BrainEntry};
use crate::gbrain_cli::{git_add_commit, git_init_commit, run_capture, run_child, LineSink, OpResult};
use crate::proc::env_for_brain;
use crate::gbrain_config;
use crate::i18n::{AppError, L10n};

/// 一個 gbrain source（來自 `sources list --json`）。
///
/// `local_path` 可為 `None`：federated 或剛建立、尚未綁定本地目錄的來源，
/// gbrain 會回傳 `null`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GbrainSource {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub local_path: Option<String>,
    pub federated: bool,
    pub page_count: i64,
    #[serde(default)]
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BrainsList {
    pub brains: Vec<BrainEntry>,
    pub active_id: Option<String>,
    /// 作用中腦的 .gbrain 路徑（前端顯示用）。
    pub active_dot_gbrain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddBrainReq {
    pub name: String,
    /// `Some(parent)` = 隔離腦（parent = .gbrain 的父目錄）；預設腦請勿由此新增。
    pub gbrain_home: Option<String>,
    /// false = 登錄既有；true = 用 gbrain init 建立新腦。
    #[serde(default)]
    pub create: bool,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_dimensions: Option<i64>,
    #[serde(default)]
    pub chat_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceAdd {
    pub brain_id: String,
    pub source_id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct SourceRef {
    pub brain_id: String,
    pub source_id: String,
}

// ── 輔助 ───────────────────────────────────────────────────────────────

pub fn exe_path(c: &AppConfig) -> Result<String, AppError> {
    if Path::new(&c.gbrain_exe_path).exists() {
        Ok(c.gbrain_exe_path.clone())
    } else {
        Err(AppError::new("gbrain.exeNotFound").p("path", &c.gbrain_exe_path))
    }
}

pub fn unique_id(c: &AppConfig, base: &str) -> String {
    if !c.brains.iter().any(|b| b.id == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let cand = format!("{base}-{n}");
        if !c.brains.iter().any(|b| b.id == cand) {
            return cand;
        }
        n += 1;
    }
}

/// 新腦預設 embedding 的最後 fallback（優先序：AppConfig 設定 >
/// 作用中環境 GBrain config > 此常數）。變更時須同步前端預填
/// （`BrainsView.vue`）與 `tests_real.rs` 的 init 參數。
pub const DEFAULT_EMBEDDING_MODEL: &str = "llama-server:embeddinggemma-2";
pub const DEFAULT_EMBEDDING_DIM: i64 = 768;

pub fn default_models(c: &AppConfig) -> (String, i64, String) {
    let cfg_model = c
        .default_embedding_model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let cfg_dim = c.default_embedding_dimensions.filter(|d| *d > 0);
    let gb = gbrain_config::load_for(c.active_env_home())
        .ok()
        .filter(|l| l.exists);
    let chat = gb
        .as_ref()
        .and_then(|l| l.config.chat_model.clone())
        .unwrap_or_else(|| gbrain_config::DEFAULT_CHAT_MODEL.into());
    (
        cfg_model
            .or_else(|| gb.as_ref().and_then(|l| l.config.embedding_model.clone()))
            .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.into()),
        cfg_dim
            .or(gb.as_ref().and_then(|l| l.config.embedding_dimensions))
            .unwrap_or(DEFAULT_EMBEDDING_DIM),
        chat,
    )
}

/// 新腦建立後，把 chat_model 同步到 **兩個 plane**（file + DB）的
/// `models.tier.*` + `models.default/think` + `chat_model`。
///
/// gbrain 0.47.x 實測（2026-08-31 事實修正）：runtime 對 model/tier 鍵採
/// **file/env plane 優先**，DB 值被 shadow（`gbrain config get` 明示）。
/// 舊版只寫 DB plane 會被 `gbrain init --chat-model` 寫入的 file-plane
/// chat_model 蓋掉，tier 甚至 fallback 到 anthropic（跟你要 ANTHROPIC_API_KEY）。
/// 現走 `gbrain_cfg::set_models_all`：file plane 直寫 config.json + DB plane
/// `gbrain config set`（fallback 層），兩 plane 一致。
pub async fn sync_new_brain_models(
    exe: &str,
    home: &str,
    chat_model: &str,
) -> Result<(), String> {
    crate::gbrain_cfg::set_models_all(exe, Some(home), chat_model)
        .await
        .map_err(|e| e.to_string())
}

/// 新增腦核心：驗證/建立（create=true 跑 gbrain init＋models 同步）→ 回傳更新後的
/// AppConfig（新增 entry）與該 entry。**不持久化**——殼層決定 save。
pub async fn add_brain_core(
    c: &AppConfig,
    req: &AddBrainReq,
) -> Result<(AppConfig, BrainEntry), AppError> {
    let home = req
        .gbrain_home
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .ok_or_else(|| AppError::new("brain.needPath"))?
        .to_string();

    let mut c = c.clone();
    if c.brains.iter().any(|b| b.gbrain_home.as_deref() == Some(home.as_str())) {
        return Err(AppError::new("brain.alreadyRegistered").p("path", &home));
    }

    let dot_gbrain = std::path::PathBuf::from(&home).join(".gbrain");
    let config_json = dot_gbrain.join("config.json");

    if req.create {
        // 建立新腦：mkdir + gbrain init
        std::fs::create_dir_all(&dot_gbrain).map_err(|e| e.to_string())?;
        let exe = exe_path(&c)?;
        let (em, dim, cm) = {
            let em = req.embedding_model.clone().unwrap_or_default();
            let dim = req.embedding_dimensions.unwrap_or(0);
            let cm = req.chat_model.clone().unwrap_or_default();
            let (dem, dd, dcm) = default_models(&c);
            (
                if em.is_empty() { dem } else { em },
                if dim == 0 { dd } else { dim },
                if cm.is_empty() { dcm } else { cm },
            )
        };
        let dim_s = dim.to_string();
        let args = vec![
            "init", "--pglite", "--non-interactive",
            "--embedding-model", &em,
            "--embedding-dimensions", &dim_s,
            "--chat-model", &cm,
            "--skip-embed-check",
        ];
        let env = env_for_brain(Some(&home));
        let (code, _out, err) = run_capture(&exe, &args, &env)
            .await
            .map_err(|e| e.to_string())?;
        if code != 0 || !config_json.exists() {
            return Err(AppError::new("brain.initFailed").p("code", code).p("detail", err));
        }
        // gbrain init 只寫 chat_model；v0.42 runtime 讀 DB plane 的 models.tier.*。
        // 同步寫入 DB plane（否則 think/subagent fallback 到 anthropic claude-*）。
        if let Err(e) = sync_new_brain_models(&exe, &home, &cm).await {
            return Err(AppError::new("brain.modelsSyncFailed").p("detail", e));
        }
        // 多模態（圖片嵌入）繼承：作用中環境已啟用 → 三鍵寫入新腦（雙 plane）。
        // 缺此步驟每個 GUI 新腦都得手動重設（2026-10-18 demo 腦回歸）。
        if let Err(w) = inherit_multimodal(&exe, &c, Some(&home)).await {
            eprintln!("[brains] multimodal 繼承失敗（略過）：{w}");
        }
        // provider 端點繼承：gbrain init 只寫 chat provider URL——嵌入 provider
        // （llama-server）漏寫會讓新腦的查詢嵌入全數 Not Found（2026-10-18 demo
        // 腦回歸第二層）。
        if let Err(w) = inherit_provider_urls(&c, &exe, Some(&home)).await {
            eprintln!("[brains] provider_base_urls 繼承失敗（略過）：{w}");
        }
        // embedding_image 維度修正：gbrain 0.60 建 schema 硬編 vector(1024)，
        // 與 768 維嵌入模型錯位（multimodal 開了圖片 sync 仍失敗）——init 後冪等
        // 修正（腳本：deploy/local/mm-bridge/alter-image-dim.mjs；欄位有資料即中止）。
        if let Err(w) = fix_embedding_image_dim(&c, Some(&home), dim).await {
            eprintln!("[brains] embedding_image 維度修正失敗（略過；圖片 sync 會失敗）：{w}");
        }
    } else {
        // 登錄既有：驗 config.json 存在
        if !config_json.exists() {
            return Err(AppError::new("brain.notABrain")
                .p("path", config_json.display())
                .p("home", &home));
        }
    }

    let id = unique_id(
        &c,
        &Path::new(&home)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "brain".into()),
    );
    let entry = BrainEntry {
        id: id.clone(),
        name: req.name.clone(),
        gbrain_home: Some(home),
    };
    c.brains.push(entry.clone());
    Ok((c, entry))
}

/// 新腦 multimodal 繼承：作用中環境的 gbrain config 已啟用 multimodal（且帶
/// litellm URL／模型）→ 三鍵寫入新腦（雙 plane）。來源未啟用 → Ok（不繼承；
/// 工廠頁的能力提示會引導設定）。
async fn inherit_multimodal(exe: &str, c: &AppConfig, home: Option<&str>) -> Result<(), String> {
    let view = crate::gbrain_cfg::build_config_view(Some(exe), c.active_env_home())
        .await
        .map_err(|e| e.to_string())?;
    if !view.embedding_multimodal {
        return Ok(());
    }
    let url = view
        .provider_base_urls
        .get("litellm")
        .and_then(|v| v.as_str())
        .unwrap_or("http://127.0.0.1:8090/v1")
        .to_string();
    let model = view
        .embedding_multimodal_model
        .clone()
        .unwrap_or_else(|| "litellm:embeddinggemma-2".into());
    crate::gbrain_cfg::set_multimodal_setup(exe, home, &model, &url)
        .await
        .map_err(|e| e.to_string())
}

/// 新腦 provider 端點繼承：作用中環境的 `provider_base_urls` 整表複製
/// （init 只寫 chat provider URL；嵌入 provider 漏寫＝新腦查詢/嵌入全 Not Found）。
async fn inherit_provider_urls(c: &AppConfig, exe: &str, home: Option<&str>) -> Result<(), String> {
    let view = crate::gbrain_cfg::build_config_view(Some(exe), c.active_env_home())
        .await
        .map_err(|e| e.to_string())?;
    crate::gbrain_cfg::copy_provider_base_urls(home, &view.provider_base_urls)
        .map_err(|e| e.to_string())
}

/// 新腦 `embedding_image` 維度修正：以 bun 跑 `alter-image-dim.mjs`
/// （PGLITE_DIR 指向新腦；冪等——已為目標維度即 no-op、欄位有資料即中止）。
/// 腳本尋找順序：`OPEROID_ALTER_IMAGE_SCRIPT` → `~/.bun/install/global/` →
/// `<cwd>/deploy/local/mm-bridge/`。
async fn fix_embedding_image_dim(c: &AppConfig, home: Option<&str>, dim: i64) -> Result<(), String> {
    let script = locate_alter_script().ok_or_else(|| {
        "alter-image-dim.mjs 不存在（找過 OPEROID_ALTER_IMAGE_SCRIPT、~/.bun/install/global、<cwd>/deploy/local/mm-bridge）".to_string()
    })?;
    let bun = locate_bun(c).ok_or_else(|| "bun 不在 PATH 也找不到 ~/.bun/bin".to_string())?;
    let pglite = Path::new(home.expect("create=true 呼叫必帶 home"))
        .join(".gbrain")
        .join("brain.pglite");
    let env: Vec<(&'static str, std::ffi::OsString)> = vec![
        ("PGLITE_DIR", pglite.to_string_lossy().into_owned().into()),
        ("TARGET_DIM", dim.to_string().into()),
    ];
    let (_, out, err) = run_capture(&bun, &[script.to_string_lossy().as_ref()], &env)
        .await
        .map_err(|e| e.to_string())?;
    if !out.contains("免改") && !out.contains("改後") {
        return Err(format!(
            "腳本輸出異常：{} {}",
            out.trim().chars().take(200).collect::<String>(),
            err.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

/// `alter-image-dim.mjs` 位置探測（依序：env 覆寫 → gbrain 全域安裝根 → repo 部署包）。
fn locate_alter_script() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OPEROID_ALTER_IMAGE_SCRIPT") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".bun").join("install").join("global").join("alter-image-dim.mjs");
        if p.is_file() {
            return Some(p);
        }
    }
    let deployed = std::env::current_dir()
        .ok()?
        .join("deploy")
        .join("local")
        .join("mm-bridge")
        .join("alter-image-dim.mjs");
    if deployed.is_file() {
        return Some(deployed);
    }
    None
}

/// bun 位置探測：PATH → `~/.bun/bin`（gbrain 同源安裝）。
fn locate_bun(c: &AppConfig) -> Option<String> {
    if let Ok(path_env) = std::env::var("PATH") {
        if let Some(p) = crate::converters::mineru::ladder::find_on_path("bun", &path_env) {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    let home = dirs::home_dir()?;
    for name in if cfg!(windows) { ["bun.exe"] } else { ["bun"] } {
        let p = home.join(".bun").join("bin").join(name);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    let _ = c;
    None
}

/// 列出某腦的 sources（live：gbrain sources list --json）。給 `brain_sources` 指令
/// 與 `note_view`（點擊 wikilink → 在作用中來源 repo 找 .md）共用。
pub async fn list_sources(c: &AppConfig, brain_id: &str) -> Result<Vec<GbrainSource>, AppError> {
    let entry = brain_entry(c, brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());
    let (code, out, err) = run_capture(&exe, &["sources", "list", "--json"], &env)
        .await
        .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.listFailed").p("code", code).p("detail", err));
    }
    let json = extract_json(&out).ok_or_else(|| format!("無法解析 sources JSON：{out}"))?;
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        sources: Vec<GbrainSource>,
    }
    let w: Wrap = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    Ok(w.sources)
}

/// 新增來源核心：先驗 git repo（gbrain 要求）→ `sources add --path`。
pub async fn add_source_core(c: &AppConfig, req: &SourceAdd) -> Result<(), AppError> {
    if !Path::new(&req.path).join(".git").exists() {
        return Err(AppError::new("source.notGitRepo").p("path", &req.path));
    }
    let entry = brain_entry(c, &req.brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());
    let (code, _out, err) = run_capture(
        &exe,
        &["sources", "add", &req.source_id, "--path", &req.path],
        &env,
    )
    .await
    .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.addFailed").p("code", code).p("detail", err));
    }
    Ok(())
}

/// 來源刪除模式（gbrain 0.60 破壞性語意：有資料的來源裸 `remove` 會被拒，
/// 必須 `--confirm-destructive`，或走 `archive` 軟刪——隱藏並保留 72 小時，
/// `sources restore` 可還原）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceRemoveMode {
    /// 預覽影響（`--dry-run`；無副作用）。
    Preview,
    /// 軟刪（保留 72 小時，可還原）。
    Archive,
    /// 永久刪除（不可逆）。
    Destroy,
}

impl SourceRemoveMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "preview" => Some(Self::Preview),
            "archive" => Some(Self::Archive),
            "destroy" => Some(Self::Destroy),
            _ => None,
        }
    }

    fn args(self, source_id: &str) -> Vec<String> {
        match self {
            Self::Preview => vec!["sources", "remove", source_id, "--dry-run"],
            Self::Archive => vec!["sources", "archive", source_id],
            Self::Destroy => vec!["sources", "remove", source_id, "--confirm-destructive"],
        }
        .into_iter()
        .map(str::to_string)
        .collect()
    }
}

/// 刪除／預覽結果：模式＋gbrain 原始輸出（影響預覽盒的文字）。
#[derive(Debug, Serialize)]
pub struct SourceRemoveOutcome {
    pub mode: SourceRemoveMode,
    pub output: String,
}

/// 移除來源核心：`mode` 決定預覽／軟刪／永久刪除（見 [`SourceRemoveMode`]）。
pub async fn remove_source_core(
    c: &AppConfig,
    req: &SourceRef,
    mode: SourceRemoveMode,
) -> Result<SourceRemoveOutcome, AppError> {
    let entry = brain_entry(c, &req.brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());
    let args = mode.args(&req.source_id);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (code, out, err) = run_capture(&exe, &arg_refs, &env)
        .await
        .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.removeFailed").p("code", code).p("detail", err));
    }
    Ok(SourceRemoveOutcome { mode, output: out })
}

/// 綁定 default 來源路徑核心：確保 `path` 是有 commit 的 git repo（自動 git init），
/// 再跑 `gbrain sync --repo <path>` 將該路徑綁定到腦的 default 來源（存進 DB）。
/// 用於新建腦後綁定筆記目錄，或對 local_path 為 null 的 default 來源補綁。
pub async fn bind_source_path_core(
    c: &AppConfig,
    ch: &LineSink,
    brain_id: &str,
    path: &str,
) -> Result<OpResult, AppError> {
    let entry = brain_entry(c, brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());

    // 1. 確保 path 是有 commit 的 git repo（不存在→建立；非 git→init + 初始 commit）。
    let repo = Path::new(path);
    git_init_commit(ch, repo).await.map_err(|e| e.to_string())?;

    // 2. gbrain sync --repo <path> 綁定 default 來源路徑（首次 sync 寫進 DB）。
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: format!("▶ gbrain sync --repo {path} --no-pull"),
    });
    let code = run_child(
        ch,
        &exe,
        &["sync", "--repo", path, "--no-pull", "--yes"],
        None,
        &env,
    )
    .await
    .map_err(|e| e.to_string())?;

    // 3. 補 embed --stale + extract --stale（idempotent；與 sync_brain_core 一致）。
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: "▶ gbrain embed --stale".into(),
    });
    let _ = run_child(ch, &exe, &["embed", "--stale"], None, &env).await;
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: "▶ gbrain extract --stale".into(),
    });
    let _ = run_child(ch, &exe, &["extract", "--stale"], None, &env).await;

    Ok(OpResult {
        success: code == 0,
        exit_code: Some(code),
        note: Some(L10n::new("source.bindDone").p("path", path)),
    })
}

/// 同步某腦核心（**E8 前置**：sink 由呼叫端給——前端串流或 [`crate::gbrain_cli::noop_sink`]）。
/// scope="all" → sync --all；scope="one" → sync --source <id>。
/// 多來源路徑：**不**做 notes-repo 的 git-commit（各 source 自管 repo；但對涉及的
/// source repo 做 best-effort add+commit——工廠頁寫檔後 sync 不漏新檔）；sync 後補 embed/extract。
pub async fn sync_brain_core(
    c: &AppConfig,
    ch: &LineSink,
    brain_id: &str,
    scope: &str,
    source_id: Option<&str>,
) -> Result<OpResult, AppError> {
    let entry = brain_entry(c, brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());

    // sync 前對涉及的 source repo 做 git add+commit（best-effort，不中斷 sync）。
    // gbrain sync 是 git-based incremental，未 commit 的變更不會被同步——工廠頁寫檔後
    // 若直接 sync 會漏掉新檔，故在此確保先進 git。
    let targets: Vec<String> = match list_sources(c, brain_id).await {
        Ok(srcs) => match scope {
            "one" => srcs
                .iter()
                .filter(|s| source_id == Some(s.id.as_str()))
                .filter_map(|s| s.local_path.clone())
                .collect(),
            _ => srcs.iter().filter_map(|s| s.local_path.clone()).collect(),
        },
        Err(_) => Vec::new(),
    };
    for path in &targets {
        let repo = Path::new(path);
        if repo.join(".git").exists() {
            let _ = git_add_commit(ch, repo).await;
        }
    }

    let mut sync_args: Vec<String> = vec!["sync".into()];
    match scope {
        "all" => {
            sync_args.push("--all".into());
        }
        "one" => {
            let sid = source_id.ok_or_else(|| AppError::new("source.needId"))?;
            sync_args.push("--source".into());
            sync_args.push(sid.into());
            if c.sync_no_pull {
                sync_args.push("--no-pull".into());
            }
        }
        other => return Err(AppError::new("source.unknownScope").p("scope", other)),
    }
    sync_args.push("--yes".into());
    let refs: Vec<&str> = sync_args.iter().map(|s| s.as_str()).collect();
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: format!("▶ gbrain {}", refs.join(" ")),
    });
    let code = run_child(ch, &exe, &refs, None, &env)
        .await
        .map_err(|e| e.to_string())?;

    // sync 後補 embed --stale + extract --stale（idempotent；GBRAIN_HOME 同腦）
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: "▶ gbrain embed --stale".into(),
    });
    let _ = run_child(ch, &exe, &["embed", "--stale"], None, &env).await;
    ch(crate::gbrain_cli::CliLine {
        stream: "step".into(),
        text: "▶ gbrain extract --stale".into(),
    });
    let _ = run_child(ch, &exe, &["extract", "--stale"], None, &env).await;

    Ok(OpResult {
        success: code == 0,
        exit_code: Some(code),
        note: Some(L10n::new("source.syncDone").p("scope", scope)),
    })
}

/// 從 stdout 文字中取第一個 `{` 到最後一個 `}` 的 JSON 物件（容忍前後雜訊）。
pub fn extract_json(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end > start {
        Some(s[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_object() {
        let s = "banner\n{\"sources\":[]}\ntail";
        assert_eq!(extract_json(s).unwrap(), "{\"sources\":[]}");
    }

    #[test]
    fn unique_id_avoids_collision() {
        let c = AppConfig::default();
        let a = unique_id(&c, "demo");
        assert_eq!(a, "demo");
        let mut c2 = c.clone();
        c2.brains.push(BrainEntry {
            id: "demo".into(),
            name: "demo".into(),
            gbrain_home: Some("/x".into()),
        });
        assert_eq!(unique_id(&c2, "demo"), "demo-2");
    }

    /// 新腦預設 embedding：AppConfig 設定優先於 GBrain config／內建常數。
    /// 空字串與非正維度視同未設定（不留神按到空白也會回到下層來源）。
    #[test]
    fn default_models_prefers_app_config_embedding() {
        let mut c = AppConfig::default();
        c.default_embedding_model = Some("openai:text-embedding-3-small".into());
        c.default_embedding_dimensions = Some(1536);
        let (em, dim, _) = default_models(&c);
        assert_eq!(em, "openai:text-embedding-3-small");
        assert_eq!(dim, 1536);

        c.default_embedding_model = Some("  ".into());
        c.default_embedding_dimensions = Some(0);
        let (em, dim, _) = default_models(&c);
        // 空字串／0 視同未設定 → 不應採用，落到 GBrain config 或內建 fallback。
        assert_ne!(em, "  ");
        assert_ne!(dim, 0);
    }
}

/// 新腦建立即具備 multimodal（#[ignore]——真實環境：gbrain、bun、alter 腳本與
/// 作用中環境已啟用 multimodal；手動跑）。
/// 跑法：`cargo test -p ocore real_add_brain_multimodal -- --ignored --nocapture`
#[ignore = "真實環境相依：需 gbrain、bun、alter-image-dim.mjs 與已啟用 multimodal 的作用中環境"]
#[tokio::test]
async fn real_add_brain_inherits_multimodal_and_fixes_image_dim() {
    let dir = std::env::temp_dir().join(format!(
        "operoid-brain-mm-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let home = dir.join("newbrain");
    std::fs::create_dir_all(&home).unwrap();
    let c = AppConfig::default();
    let req = AddBrainReq {
        name: "mm-test".into(),
        gbrain_home: Some(home.to_string_lossy().into_owned()),
        create: true,
        embedding_model: None,
        embedding_dimensions: None,
        chat_model: None,
    };
    let (_, _entry) = add_brain_core(&c, &req).await.expect("建腦應成功");

    // 斷言：multimodal 三鍵已繼承（file plane）。
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".gbrain").join("config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cfg["embedding_multimodal"], serde_json::json!(true), "multimodal 應已啟用");
    assert_eq!(
        cfg["embedding_multimodal_model"],
        serde_json::json!("litellm:embeddinggemma-2")
    );
    assert!(cfg["provider_base_urls"]["litellm"].is_string(), "litellm URL 應已繼承");
    assert_eq!(
        cfg["provider_base_urls"]["llama-server"],
        serde_json::json!("http://127.0.0.1:8080/v1"),
        "嵌入 provider URL 應已繼承（缺它＝新腦查詢/嵌入全 Not Found）"
    );

    // 斷言：embedding_image 已為 768（重跑腳本應回報「免改」）。
    let script = locate_alter_script().expect("alter 腳本應可找到");
    let bun = locate_bun(&c).expect("bun 應可找到");
    let env: Vec<(&'static str, std::ffi::OsString)> = vec![
        (
            "PGLITE_DIR",
            home.join(".gbrain").join("brain.pglite").to_string_lossy().into_owned().into(),
        ),
        ("TARGET_DIM", "768".into()),
    ];
    let (code, out, err) = run_capture(&bun, &[script.to_string_lossy().as_ref()], &env)
        .await
        .expect("bun spawn");
    assert_eq!(code, 0, "alter 檢查應成功：{out}{err}");
    assert!(out.contains("免改"), "維度應已修正為 768：{out}");

    std::fs::remove_dir_all(&dir).ok();
}

/// 來源刪除 preview 模式（#[ignore]——真實環境：gbrain 與既有 gbrain-demo 來源；
/// **只跑 preview**（--dry-run 無副作用），絕不執行 archive/destroy）。
/// 跑法：`cargo test -p ocore real_source_remove_preview -- --ignored --nocapture`
#[ignore = "真實環境相依：需 gbrain 與有資料的來源（gbrain-demo）；dry-run 無副作用"]
#[tokio::test]
async fn real_source_remove_preview_reports_impact() {
    let mut c = AppConfig::default();
    c.brains.push(BrainEntry {
        id: "__default__".into(),
        name: "預設腦".into(),
        gbrain_home: None,
    });
    let req = SourceRef {
        brain_id: "__default__".into(),
        source_id: "gbrain-demo".into(),
    };
    let o = remove_source_core(&c, &req, SourceRemoveMode::Preview)
        .await
        .expect("preview 應成功");
    assert!(
        o.output.contains("permanently delete"),
        "影響預覽應含刪除統計：{}",
        o.output
    );
    let sources = list_sources(&c, "__default__").await.unwrap();
    assert!(
        sources.iter().any(|s| s.id == "gbrain-demo"),
        "dry-run 不得真的刪除來源"
    );
}
