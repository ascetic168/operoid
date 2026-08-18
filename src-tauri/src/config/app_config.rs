//! App 設定持久化（tauri-plugin-store）。AppConfig 結構本體已搬入 `ocore::app_config`
//! （P1b，2026-08-18）——此處只留 load/save（Tauri 依賴）＋ re-export 保持
//! `crate::config::app_config::*` 路徑零改動。

use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub use ocore::app_config::*;

const STORE_FILE: &str = "app-settings.json";
const STORE_KEY: &str = "app_config";

pub fn load<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<AppConfig> {
    let store = app.store(STORE_FILE)?;
    let mut cfg = match store.get(STORE_KEY) {
        Some(v) => serde_json::from_value::<AppConfig>(v).unwrap_or_default(),
        None => AppConfig::default(),
    };
    cfg.normalize(); // 每次載入都跑（冪等）；吸收舊 override、修正 active
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, config: &AppConfig) -> anyhow::Result<()> {
    let store = app.store(STORE_FILE)?;
    store.set(STORE_KEY, serde_json::to_value(config)?);
    store.save()?;
    Ok(())
}
