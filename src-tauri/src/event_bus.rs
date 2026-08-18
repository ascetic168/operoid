//! Event 匯流排——殼層包裝（P1b，2026-08-18）。實體已搬入 `ocore::event_bus`
//! （不收 AppHandle，改收 state/cfg/db_path）；此處提供 app 版簽名保持
//! 既有呼叫端（ingress_server／殼層 scheduler 接線）零改動。

use tauri::{AppHandle, Manager, Runtime};

use crate::agent_state::{AppState, InboundEvent};
use crate::config::app_config;
use crate::runtime::agent_db_path;

/// 路由並投遞一則外部事件（app 版包裝：載入 cfg/state/db_path 後委派 ocore 版）。
pub async fn dispatch_event<R: Runtime>(app: &AppHandle<R>, ev: InboundEvent) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    let state = app.state::<AppState>();
    ocore::event_bus::dispatch_event(&state, &cfg, &agent_db_path(app)?, ev).await
}
