//! 本系統自有的設定（tauri-plugin-store 持久化於 app data）。
//!
//! 只放「GBrain config 沒有、純屬本系統」的東西：notes 內容 repo、gbrain.exe 位置、
//! 選用的 GBRAIN_HOME 覆寫、自動 sync、factory 目標目錄等。GBrain 腦本身的行为
//! （model/embedding/schema/provider...）一律讀 ~/.gbrain/config.json，不在此重抄。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "app-settings.json";
const STORE_KEY: &str = "app_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryTargets {
    pub people: String,
    pub companies: String,
    pub meetings: String,
}

impl Default for FactoryTargets {
    fn default() -> Self {
        Self {
            people: "people".into(),
            companies: "companies".into(),
            meetings: "meetings".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 內容 git repo（GBrain sync --repo 的對象）。預設 ~/notes。
    pub notes_repo_path: String,
    /// gbrain 執行檔路徑。預設 ~/.bun/bin/gbrain.exe（Windows）。
    pub gbrain_exe_path: String,
    /// 選用：覆寫 GBRAIN_HOME（指向 .gbrain 的「父目錄」）。None = 用預設腦。
    #[serde(default)]
    pub gbrain_home_override: Option<String>,
    /// 工廠寫檔後是否自動 commit + sync。
    #[serde(default)]
    pub auto_sync: bool,
    /// sync 是否加 --no-pull（無 remote 的腦建議開）。
    #[serde(default = "default_true")]
    pub sync_no_pull: bool,
    /// 工廠對應的白名單目標子目錄。
    #[serde(default)]
    pub factory_targets: FactoryTargets,
    /// LLM 結構化的取樣溫度。
    #[serde(default = "default_temp")]
    pub llm_temperature: f64,
    /// LLM 結構化的最大輸出 token。
    #[serde(default = "default_max_tokens")]
    pub llm_max_tokens: u32,
}

fn default_true() -> bool {
    true
}
fn default_temp() -> f64 {
    0.2
}
fn default_max_tokens() -> u32 {
    4096
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
        let gbrain_exe = if cfg!(target_os = "windows") {
            format!("{home}\\.bun\\bin\\gbrain.exe")
        } else {
            format!("{home}/.bun/bin/gbrain")
        };
        Self {
            notes_repo_path: format!("{home}/notes"),
            gbrain_exe_path: gbrain_exe,
            gbrain_home_override: None,
            auto_sync: true,
            sync_no_pull: true,
            factory_targets: FactoryTargets::default(),
            llm_temperature: default_temp(),
            llm_max_tokens: default_max_tokens(),
        }
    }
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<AppConfig> {
    let store = app.store(STORE_FILE)?;
    Ok(match store.get(STORE_KEY) {
        Some(v) => serde_json::from_value::<AppConfig>(v).unwrap_or_default(),
        None => AppConfig::default(),
    })
}

pub fn save<R: Runtime>(app: &AppHandle<R>, config: &AppConfig) -> anyhow::Result<()> {
    let store = app.store(STORE_FILE)?;
    store.set(STORE_KEY, serde_json::to_value(config)?);
    store.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = AppConfig::default();
        assert!(c.notes_repo_path.ends_with("notes"));
        assert!(c.sync_no_pull);
        assert_eq!(c.factory_targets.people, "people");
    }

    #[test]
    fn roundtrips_through_json() {
        let c = AppConfig::default();
        let v = serde_json::to_value(&c).unwrap();
        let back: AppConfig = serde_json::from_value(v).unwrap();
        assert_eq!(back.notes_repo_path, c.notes_repo_path);
    }
}
