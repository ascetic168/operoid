//! 腦（Brains）管理——指令層（P1c 殼）。核心已搬入 `ocore::brains`
//! （cfg 傳入、串流走 LineSink）；此處負責 AppConfig load/save 與 Channel→sink 橋接。

pub use ocore::brains::*;

use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};

use crate::config::{app_config, AppConfig, BrainEntry, DEFAULT_BRAIN_ID};
use crate::gbrain_cli::{channel_sink, CliLine, OpResult};
use crate::i18n::AppError;

fn cfg<R: Runtime>(app: &AppHandle<R>) -> Result<AppConfig, AppError> {
    app_config::load(app).map_err(Into::into)
}

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
    let c = cfg(&app)?;
    let (c2, entry) = add_brain_core(&c, &req).await?;
    app_config::save(&app, &c2).map_err(|e| e.to_string())?;
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

/// 列出某腦的 sources（live：gbrain sources list --json）。
#[tauri::command]
pub async fn brain_sources<R: Runtime>(
    app: AppHandle<R>,
    brain_id: String,
) -> Result<Vec<GbrainSource>, AppError> {
    let c = cfg(&app)?;
    list_sources(&c, &brain_id).await
}

#[tauri::command]
pub async fn brain_source_add<R: Runtime>(
    app: AppHandle<R>,
    req: SourceAdd,
) -> Result<(), AppError> {
    let c = cfg(&app)?;
    add_source_core(&c, &req).await
}

#[tauri::command]
pub async fn brain_source_remove<R: Runtime>(
    app: AppHandle<R>,
    req: SourceRef,
) -> Result<(), AppError> {
    let c = cfg(&app)?;
    remove_source_core(&c, &req).await
}

/// 綁定 default 來源路徑：確保 `path` 是有 commit 的 git repo（自動 git init），
/// 再跑 `gbrain sync --repo <path>` 將該路徑綁定到腦的 default 來源（存進 DB）。
#[tauri::command]
pub async fn brain_bind_source_path<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<CliLine>,
    brain_id: String,
    path: String,
) -> Result<OpResult, AppError> {
    let c = cfg(&app)?;
    let sink = channel_sink::<R>(&on_event);
    bind_source_path_core(&c, &sink, &brain_id, &path).await
}

/// 同步某腦：scope="all" → sync --all；scope="one" → sync --source <id>。
#[tauri::command]
pub async fn brain_sync<R: Runtime>(
    app: AppHandle<R>,
    on_event: Channel<CliLine>,
    brain_id: String,
    scope: String,
    source_id: Option<String>,
) -> Result<OpResult, AppError> {
    let c = cfg(&app)?;
    let sink = channel_sink::<R>(&on_event);
    sync_brain_core(&c, &sink, &brain_id, &scope, source_id.as_deref()).await
}
