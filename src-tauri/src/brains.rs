//! 腦（Brains）管理 — 多腦（各 GBRAIN_HOME）+ 每腦多來源（gbrain sources）。
//!
//! gbrain 沒有「列出所有腦」的指令，故腦清單由本程式自管（存於 AppConfig.brains）。
//! 每腦的 sources 則用 gbrain `sources` 即時查詢/增刪/同步。所有 gbrain 呼叫都帶
//! 該腦的 GBRAIN_HOME（非作用中腦也能檢視/操作其來源）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};

use crate::config::{
    app_config,
    gbrain_config::{self},
    AppConfig, BrainEntry, DEFAULT_BRAIN_ID,
};
use crate::gbrain_cli::{env_for_brain, run_capture, run_child, CliLine, OpResult};
use crate::i18n::{AppError, L10n};

/// 一個 gbrain source（來自 `sources list --json`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GbrainSource {
    pub id: String,
    pub name: String,
    pub local_path: String,
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

fn cfg<R: Runtime>(app: &AppHandle<R>) -> Result<AppConfig, AppError> {
    app_config::load(app).map_err(Into::into)
}

fn exe_path(c: &AppConfig) -> Result<String, AppError> {
    if Path::new(&c.gbrain_exe_path).exists() {
        Ok(c.gbrain_exe_path.clone())
    } else {
        Err(AppError::new("gbrain.exeNotFound").p("path", &c.gbrain_exe_path))
    }
}

fn brain_entry<'a>(c: &'a AppConfig, brain_id: &str) -> Result<&'a BrainEntry, AppError> {
    c.brains
        .iter()
        .find(|b| b.id == brain_id)
        .ok_or_else(|| AppError::new("brain.notFound").p("id", brain_id))
}

fn unique_id(c: &AppConfig, base: &str) -> String {
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

fn default_models(c: &AppConfig) -> (String, i64, String) {
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
                .unwrap_or_else(|| "groq:llama-3.3-70b-versatile".into()),
        ),
        _ => (
            "ollama:embeddinggemma".into(),
            768,
            "groq:llama-3.3-70b-versatile".into(),
        ),
    }
}

// ── 指令 ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn brains_list<R: Runtime>(app: AppHandle<R>) -> Result<BrainsList, AppError> {
    let c = cfg(&app)?;
    Ok(BrainsList {
        active_dot_gbrain: c.active_brain().map(|b| {
            b.dot_gbrain_path().to_string_lossy().into_owned()
        }),
        brains: c.brains.clone(),
        active_id: c.active_brain_id.clone(),
    })
}

#[tauri::command]
pub async fn brains_add<R: Runtime>(
    app: AppHandle<R>,
    req: AddBrainReq,
) -> Result<BrainEntry, AppError> {
    let home = req
        .gbrain_home
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .ok_or_else(|| AppError::new("brain.needPath"))?
        .to_string();

    let mut c = cfg(&app)?;
    if c.brains.iter().any(|b| b.gbrain_home.as_deref() == Some(home.as_str())) {
        return Err(AppError::new("brain.alreadyRegistered").p("path", &home));
    }

    let dot_gbrain = PathBuf::from(&home).join(".gbrain");
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
        let env_ref: Vec<(&str, std::ffi::OsString)> = env;
        let (code, _out, err) = run_capture(&app, &exe, &args, &env_ref)
            .await
            .map_err(|e| e.to_string())?;
        if code != 0 || !config_json.exists() {
            return Err(AppError::new("brain.initFailed").p("code", code).p("detail", err));
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
        name: req.name,
        gbrain_home: Some(home),
    };
    c.brains.push(entry.clone());
    app_config::save(&app, &c).map_err(|e| e.to_string())?;
    Ok(entry)
}

#[tauri::command]
pub fn brains_remove<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), AppError> {
    if id == DEFAULT_BRAIN_ID {
        return Err(AppError::new("brain.cannotRemoveDefault"));
    }
    let mut c = cfg(&app)?;
    let before = c.brains.len();
    c.brains.retain(|b| b.id != id);
    if c.brains.len() == before {
        return Err(AppError::new("brain.notFound").p("id", &id));
    }
    if c.active_brain_id.as_deref() == Some(id.as_str()) {
        c.active_brain_id = Some(DEFAULT_BRAIN_ID.into());
        c.active_source_id = None;
    }
    app_config::save(&app, &c).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn brains_set_active<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), AppError> {
    let mut c = cfg(&app)?;
    if !c.brains.iter().any(|b| b.id == id) {
        return Err(AppError::new("brain.notFound").p("id", &id));
    }
    c.active_brain_id = Some(id);
    c.active_source_id = None; // 新腦未必有舊 source，重設
    app_config::save(&app, &c).map_err(|e| e.to_string())?;
    Ok(())
}

/// 設作用中來源（作用中腦內）。前端選 source 時呼叫。
#[tauri::command]
pub fn brains_set_active_source<R: Runtime>(
    app: AppHandle<R>,
    source_id: Option<String>,
) -> Result<(), AppError> {
    let mut c = cfg(&app)?;
    c.active_source_id = source_id;
    app_config::save(&app, &c).map_err(|e| e.to_string())?;
    Ok(())
}

/// 列出某腦的 sources（live：gbrain sources list --json）。給 `brain_sources` 指令
/// 與 `note_view`（點擊 wikilink → 在作用中來源 repo 找 .md）共用。
pub(crate) async fn list_sources<R: Runtime>(
    app: &AppHandle<R>,
    brain_id: &str,
) -> Result<Vec<GbrainSource>, AppError> {
    let c = cfg(app)?;
    let entry = brain_entry(&c, brain_id)?;
    let exe = exe_path(&c)?;
    let env = env_for_brain(entry.env_home());
    let env_ref: Vec<(&str, std::ffi::OsString)> = env;
    let (code, out, err) = run_capture(app, &exe, &["sources", "list", "--json"], &env_ref)
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

/// 列出某腦的 sources（live：gbrain sources list --json）。
#[tauri::command]
pub async fn brain_sources<R: Runtime>(
    app: AppHandle<R>,
    brain_id: String,
) -> Result<Vec<GbrainSource>, AppError> {
    list_sources(&app, &brain_id).await
}

#[tauri::command]
pub async fn brain_source_add<R: Runtime>(
    app: AppHandle<R>,
    req: SourceAdd,
) -> Result<(), AppError> {
    // 先驗 git repo（gbrain 要求）
    if !Path::new(&req.path).join(".git").exists() {
        return Err(AppError::new("source.notGitRepo").p("path", &req.path));
    }
    let c = cfg(&app)?;
    let entry = brain_entry(&c, &req.brain_id)?;
    let exe = exe_path(&c)?;
    let env = env_for_brain(entry.env_home());
    let env_ref: Vec<(&str, std::ffi::OsString)> = env;
    let (code, _out, err) = run_capture(
        &app,
        &exe,
        &["sources", "add", &req.source_id, "--path", &req.path],
        &env_ref,
    )
    .await
    .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.addFailed").p("code", code).p("detail", err));
    }
    Ok(())
}

#[tauri::command]
pub async fn brain_source_remove<R: Runtime>(
    app: AppHandle<R>,
    req: SourceRef,
) -> Result<(), AppError> {
    let c = cfg(&app)?;
    let entry = brain_entry(&c, &req.brain_id)?;
    let exe = exe_path(&c)?;
    let env = env_for_brain(entry.env_home());
    let env_ref: Vec<(&str, std::ffi::OsString)> = env;
    let (code, _out, err) =
        run_capture(&app, &exe, &["sources", "remove", &req.source_id], &env_ref)
            .await
            .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(AppError::new("source.removeFailed").p("code", code).p("detail", err));
    }
    Ok(())
}

/// 同步某腦：scope="all" → sync --all；scope="one" → sync --source <id>。
/// 多來源路徑：**不**做 git-commit（各 source 自管 repo）；sync 後補 embed/extract。
#[tauri::command]
pub async fn brain_sync<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<CliLine>,
    brain_id: String,
    scope: String,
    source_id: Option<String>,
) -> Result<OpResult, AppError> {
    let c = cfg(&app)?;
    let entry = brain_entry(&c, &brain_id)?;
    let exe = exe_path(&c)?;
    let env = env_for_brain(entry.env_home());
    let env_ref: Vec<(&str, std::ffi::OsString)> = env.clone();

    let mut sync_args: Vec<String> = vec!["sync".into()];
    match scope.as_str() {
        "all" => {
            sync_args.push("--all".into());
        }
        "one" => {
            let sid = source_id.ok_or_else(|| AppError::new("source.needId"))?;
            sync_args.push("--source".into());
            sync_args.push(sid);
            if c.sync_no_pull {
                sync_args.push("--no-pull".into());
            }
        }
        other => return Err(AppError::new("source.unknownScope").p("scope", other)),
    }
    sync_args.push("--yes".into());
    let refs: Vec<&str> = sync_args.iter().map(|s| s.as_str()).collect();
    let _ = on_event.send(CliLine {
        stream: "step".into(),
        text: format!("▶ gbrain {}", refs.join(" ")),
    });
    let code = run_child(&app, &on_event, &exe, &refs, None, &env_ref)
        .await
        .map_err(|e| e.to_string())?;

    // sync 後補 embed --stale + extract --stale（idempotent；GBRAIN_HOME 同腦）
    let _ = on_event.send(CliLine {
        stream: "step".into(),
        text: "▶ gbrain embed --stale".into(),
    });
    let _ = run_child(&app, &on_event, &exe, &["embed", "--stale"], None, &env_ref).await;
    let _ = on_event.send(CliLine {
        stream: "step".into(),
        text: "▶ gbrain extract --stale".into(),
    });
    let _ = run_child(&app, &on_event, &exe, &["extract", "--stale"], None, &env_ref).await;

    Ok(OpResult {
        success: code == 0,
        exit_code: Some(code),
        note: Some(L10n::new("source.syncDone").p("scope", &scope)),
    })
}

/// 從 stdout 文字中取第一個 `{` 到最後一個 `}` 的 JSON 物件（容忍前後雜訊）。
pub(crate) fn extract_json(s: &str) -> Option<String> {
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
