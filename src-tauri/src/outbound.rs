//! Outbound 殼層——實體已搬入 `ocore::outbound`（P1a，2026-08-18）。
//!
//! 此處 re-export 之餘，補回需要 `AppHandle`／`AppConfig` 的載入器
//! （原 `OutboundConfig::load(app)`；ocore 零 Tauri，無法住在核心 crate）。

pub use ocore::outbound::*;

use tauri::{AppHandle, Runtime};

use crate::config::app_config;

/// 從 App 設定載入外發組態（對應 P1a 前的 `OutboundConfig::load(app)`）。
pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> OutboundConfig {
    match app_config::load(app) {
        Ok(c) => OutboundConfig {
            url: c.event_outbound_url,
            secret: c.event_outbound_secret,
        },
        Err(_) => OutboundConfig::default(),
    }
}
