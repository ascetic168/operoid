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

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app_config::{brain_entry, AppConfig, BrainEntry};
use crate::gbrain_cli::{
    config_set, git_add_commit, git_init_commit, run_capture, run_child, LineSink, OpResult,
};
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

pub fn default_models(c: &AppConfig) -> (String, i64, String) {
    match gbrain_config::load_for(c.active_env_home()).ok() {
        Some(l) if l.exists => (
            l.config
                .embedding_model
                .clone()
                .unwrap_or_else(|| "ollama:embeddinggemma".into()),
            l.config.embedding_dimensions.unwrap_or(768),
            l.config
                .chat_model
                .clone()
                .unwrap_or_else(|| gbrain_config::DEFAULT_CHAT_MODEL.into()),
        ),
        _ => (
            "ollama:embeddinggemma".into(),
            768,
            gbrain_config::DEFAULT_CHAT_MODEL.into(),
        ),
    }
}

/// 新腦建立後，把 chat_model 同步到 DB plane 的 `models.tier.*` + `models.default/think`。
///
/// v0.42：`gbrain init --chat-model` 只寫頂層 chat_model；但 runtime 讀 DB plane 的
/// `models.tier.*`（無則 fallback 到 anthropic claude-*）。若不補，新腦 think/subagent
/// 會跑到 anthropic（跟你要 ANTHROPIC_API_KEY）。故 init 後用 `gbrain config set` 寫 DB plane。
///
/// 僅寫 DB plane（v0.42 權威層）。E3 退役：不再寫 file-plane 的 models.default/think 殘值——
/// runtime 以 DB plane 為準，file-plane 殘值無作用，寫它只會誤導（似有設、實被蓋過）。
pub async fn sync_new_brain_models(
    exe: &str,
    home: &str,
    chat_model: &str,
) -> Result<(), String> {
    let keys = [
        "chat_model",
        "models.default",
        "models.think",
        "models.tier.utility",
        "models.tier.reasoning",
        "models.tier.deep",
        "models.tier.subagent",
    ];
    for k in keys {
        config_set(exe, Some(home), k, chat_model)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// 移除來源核心：`sources remove`。
pub async fn remove_source_core(c: &AppConfig, req: &SourceRef) -> Result<(), AppError> {
    let entry = brain_entry(c, &req.brain_id)?;
    let exe = exe_path(c)?;
    let env = env_for_brain(entry.env_home());
    let (code, _out, err) = run_capture(&exe, &["sources", "remove", &req.source_id], &env)
        .await
        .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.removeFailed").p("code", code).p("detail", err));
    }
    Ok(())
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
}
