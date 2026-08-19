//! gbrain CLI——指令層（P1c 殼）。核心（含 `Channel<CliLine>`→`LineSink` 手術）已搬入
//! `ocore::gbrain_cli`；此處 re-export 保持 `crate::gbrain_cli::*` 路徑零改動，
//! 並留 `op_run` 指令（AppHandle＋Tauri Channel 依賴，sink 橋接）。

pub use ocore::gbrain_cli::*;
pub use ocore::proc::no_console;

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime};

use crate::config;
use crate::i18n::AppError;

/// 把 Tauri Channel 橋接為 ocore [`LineSink`]。
pub fn channel_sink<R: Runtime>(ch: &Channel<CliLine>) -> LineSink {
    let ch = ch.clone();
    Arc::new(move |line: CliLine| {
        let _ = ch.send(line);
    })
}

/// 解析設定與 gbrain exe（exe 不存在 → `gbrain.exeNotFound`）。
pub(crate) fn resolve_gbrain<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(config::AppConfig, String), AppError> {
    let cfg = config::app_config::load(app).map_err(|e| e.to_string())?;
    let exe = cfg.gbrain_exe_path.clone();
    if !std::path::Path::new(&exe).exists() {
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
    let sink = channel_sink::<R>(&on_event);
    op_run_core(&cfg, &exe, &sink, &op, arg.as_deref()).await
}
