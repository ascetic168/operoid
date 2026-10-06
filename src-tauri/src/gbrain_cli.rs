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
    // C6（Q7）：檢索 op 改道 KnowledgeService（operator AccessContext；行為不變、留 receipt）。
    if matches!(op.as_str(), "ask" | "query" | "think") {
        let db_path = crate::runtime::agent_db_path(&app)?;
        let access =
            ocore::knowledge::identity::operator_access_context(ocore::runtime::AGENT_WS);
        let kind = if op == "think" {
            ocore::knowledge::backend::RetrieveKind::Think
        } else {
            ocore::knowledge::backend::RetrieveKind::Search
        };
        let svc = ocore::knowledge::service::KnowledgeService::new(&db_path);
        let tctx = ocore::domain::tools::ToolCtx {
            gbrain_exe: exe.clone(),
            gbrain_home: cfg.active_env_home().map(str::to_string),
            chat_model: None,
            mcp: if cfg.gbrain_transport == "mcp" {
                Some(std::sync::Arc::new(ocore::gbrain_mcp::GbrainMcpClient::new(
                    exe.clone(),
                    cfg.active_env_home().map(str::to_string),
                )))
            } else {
                None
            },
            allowed_tools: Default::default(),
            employee_output_root: std::path::PathBuf::from(&cfg.employee_output_path),
            registry: None,
            knowledge: None,
            access,
            turn_max_steps: cfg.turn_max_steps,
            tool_result_max_chars: cfg.tool_result_max_chars,
        };
        let q = arg.clone().unwrap_or_default();
        let out = svc
            .retrieve(&tctx.access, kind, &q, None, 10, &tctx)
            .await
            .map_err(|e| AppError::new("op.runFailed").p("detail", e.to_string()))?;
        sink(CliLine { stream: "stdout".into(), text: out.text });
        return Ok(OpResult::from_code(0));
    }
    op_run_core(&cfg, &exe, &sink, &op, arg.as_deref()).await
}
