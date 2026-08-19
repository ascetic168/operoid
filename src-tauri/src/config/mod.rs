//! Config 模組——指令層（P4 殼）。GBrain 設定核心（視圖組裝／model／provider 編輯）
//! 已搬入 `ocore::gbrain_cfg`；此處留 `#[tauri::command]` 薄層（AppHandle 依賴：
//! 解析作用中腦與 exe 路徑）＋ app 設定持久化（get/save/set_locale）。

pub mod app_config;
// gbrain_config 已搬入 `ocore`（P1a，2026-08-18）；re-export 保持
// `crate::config::gbrain_config::…` 與下方 `pub use` 路徑零改動。
pub use ocore::gbrain_config;

pub use app_config::{AppConfig, BrainEntry, DEFAULT_BRAIN_ID, SUPPORTED_LOCALES};
pub use ocore::gbrain_cfg::GBrainConfigView;

use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::i18n::AppError;

/// 作用中腦的 home（GBRAIN_HOME 值；None=預設腦）。
fn active_home<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app_config::load(app).ok()?.active_env_home().map(|s| s.to_string())
}

/// 解析作用中腦的 gbrain exe 路徑（檔案須存在；不存在回 None——純 file-plane 視圖）。
fn exe_path_opt<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, AppError> {
    let c = app_config::load(app)?;
    if Path::new(&c.gbrain_exe_path).exists() {
        Ok(Some(c.gbrain_exe_path.clone()))
    } else {
        Ok(None)
    }
}

/// 解析作用中腦的 gbrain exe 路徑（檔案須存在——寫入路徑用，缺檔報錯）。
fn exe_path_of<R: Runtime>(app: &AppHandle<R>) -> Result<String, AppError> {
    let c = app_config::load(app)?;
    if Path::new(&c.gbrain_exe_path).exists() {
        Ok(c.gbrain_exe_path.clone())
    } else {
        Err(AppError::new("gbrain.exeNotFound").p("path", &c.gbrain_exe_path))
    }
}

#[tauri::command]
pub async fn get_gbrain_config<R: Runtime>(
    app: AppHandle<R>,
) -> Result<GBrainConfigView, AppError> {
    let home = active_home(&app);
    let exe = exe_path_opt(&app)?;
    ocore::gbrain_cfg::build_config_view(exe.as_deref(), home.as_deref()).await
}

/// 設單一 model/tier 鍵（走 DB plane via `gbrain config set`）。
#[tauri::command]
pub async fn set_gbrain_model<R: Runtime>(
    app: AppHandle<R>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let home = active_home(&app);
    let exe = exe_path_of(&app)?;
    ocore::gbrain_cfg::set_model(&exe, home.as_deref(), &key, &value).await
}

/// 單一模型同步到全部 tier + chat_model + models.default/think（v0.42「勾選同步」用）。
#[tauri::command]
pub async fn set_gbrain_models_all<R: Runtime>(
    app: AppHandle<R>,
    model: String,
) -> Result<(), AppError> {
    let home = active_home(&app);
    let exe = exe_path_of(&app)?;
    ocore::gbrain_cfg::set_models_all(&exe, home.as_deref(), &model).await
}

/// 從 DB plane 移除單一 model/tier 鍵（讓 file plane 或 default 生效）。
#[tauri::command]
pub async fn unset_gbrain_model<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<(), AppError> {
    let home = active_home(&app);
    let exe = exe_path_of(&app)?;
    ocore::gbrain_cfg::unset_model(&exe, home.as_deref(), &key).await
}

/// 清除所有 DB-plane 的 model/tier 覆寫。修復用：一鍵回到 file plane 為準。
#[tauri::command]
pub async fn clear_db_overrides<R: Runtime>(app: AppHandle<R>) -> Result<(), AppError> {
    let home = active_home(&app);
    let exe = exe_path_of(&app)?;
    ocore::gbrain_cfg::clear_db_overrides(&exe, home.as_deref()).await
}

/// 設 provider_base_url（**直寫檔案**，因 gbrain CLI 對此 key no-op）。
#[tauri::command]
pub async fn set_provider_base_url<R: Runtime>(
    app: AppHandle<R>,
    provider: String,
    base_url: Option<String>,
) -> Result<(), AppError> {
    let home = active_home(&app);
    ocore::gbrain_cfg::set_provider_base_url(home.as_deref(), &provider, base_url.as_deref())
}

/// 直寫整份 config.json（file-plane；raw 進階編輯器用）。
#[tauri::command]
pub async fn save_gbrain_config_raw<R: Runtime>(
    app: AppHandle<R>,
    raw_json: serde_json::Value,
) -> Result<(), AppError> {
    let home = active_home(&app);
    ocore::gbrain_cfg::save_raw(home.as_deref(), &raw_json)
}

#[tauri::command]
pub fn get_app_config<R: Runtime>(app: AppHandle<R>) -> Result<AppConfig, AppError> {
    Ok(app_config::load(&app)?)
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> Result<(), AppError> {
    app_config::save(&app, &config)?;
    Ok(())
}

/// 設定介面語言覆寫。`locale=None` 清除覆寫（回到自動偵測）。
#[tauri::command]
pub fn set_locale<R: Runtime>(
    app: AppHandle<R>,
    locale: Option<String>,
) -> Result<Option<String>, AppError> {
    let mut c = app_config::load(&app)?;
    c.locale = locale
        .filter(|l| SUPPORTED_LOCALES.contains(&l.as_str()))
        .map(|s| s.to_string());
    app_config::save(&app, &c)?;
    Ok(c.locale.clone())
}
