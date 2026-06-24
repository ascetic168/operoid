//! 本系統自有的設定（tauri-plugin-store 持久化於 app data）。
//!
//! 只放「GBrain config 沒有、純屬本系統」的東西。GBrain 腦本身的行為
//! （model/embedding/schema/provider...）一律讀該腦的 config.json，不在此重抄。
//!
//! 腦（Brains）註冊表：gbrain 沒有「列出所有腦」的指令，故本程式自管一份清單。
//! 每個 `BrainEntry` = 一個腦（`gbrain_home=None` = 預設腦 ~/.gbrain；`Some(parent)` = 隔離腦）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "app-settings.json";
const STORE_KEY: &str = "app_config";

/// 預設腦的固定 id。
pub const DEFAULT_BRAIN_ID: &str = "__default__";

/// 支援的介面語言（與前端 languageConfig 對齊）。
pub const SUPPORTED_LOCALES: &[&str] = &["zh-TW", "zh-CN", "en"];

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

/// 一個註冊的腦。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainEntry {
    /// 穩定 id（前端選取用）。預設腦固定 `__default__`。
    pub id: String,
    /// 顯示名稱。
    pub name: String,
    /// `None` = 預設腦（GBRAIN_HOME 不設，用 ~/.gbrain）；
    /// `Some(parent)` = 隔離腦，parent 指向 .gbrain 的「父目錄」（GBRAIN_HOME 值）。
    #[serde(default)]
    pub gbrain_home: Option<String>,
}

impl BrainEntry {
    pub fn default_brain() -> Self {
        Self {
            id: DEFAULT_BRAIN_ID.into(),
            name: "預設腦".into(),
            gbrain_home: None,
        }
    }
    /// GBRAIN_HOME 環境變數值（給子行程）。None = 不設（用 ~/.gbrain）。
    pub fn env_home(&self) -> Option<&str> {
        self.gbrain_home.as_deref()
    }
    /// `.gbrain` 目錄的絕對路徑（顯示/驗證用）。預設腦 → ~/.gbrain。
    pub fn dot_gbrain_path(&self) -> PathBuf {
        match &self.gbrain_home {
            Some(h) => PathBuf::from(h).join(".gbrain"),
            None => dirs::home_dir().map(|h| h.join(".gbrain")).unwrap_or_default(),
        }
    }
    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_BRAIN_ID
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 內容 git repo（保留欄位；來源感知工廠改用作用中 source 的 local_path）。
    pub notes_repo_path: String,
    /// gbrain 執行檔路徑。預設 ~/.bun/bin/gbrain.exe（Windows）。
    pub gbrain_exe_path: String,
    /// 已退役：舊版單一 GBRAIN_HOME 覆寫。僅保留可讀供 migration，邏輯改用 `brains`。
    #[serde(default)]
    pub gbrain_home_override: Option<String>,
    /// 註冊的腦清單。
    #[serde(default)]
    pub brains: Vec<BrainEntry>,
    /// 作用中腦 id。
    #[serde(default)]
    pub active_brain_id: Option<String>,
    /// 作用中腦內的作用中 source id（給工廠/sync）。
    #[serde(default)]
    pub active_source_id: Option<String>,
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
    /// 介面語言覆寫（None = 依系統語言自動偵測；Some = 使用者手動釘選）。
    #[serde(default)]
    pub locale: Option<String>,
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

/// 由 home 路徑推導穩定 id（內部用，不必吻合 gbrain slug 規則）。
fn brain_id_from_home(home: &str) -> String {
    let s: String = home
        .trim()
        .chars()
        .map(|c| {
            let lc = c.to_ascii_lowercase();
            if lc.is_ascii_alphanumeric() || lc == '_' || lc == '-' {
                lc
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    // 末段（目錄名）較可讀；若全空退回 home 本身
    s.rsplit('-')
        .next()
        .filter(|seg| !seg.is_empty())
        .map(|seg| seg.to_string())
        .unwrap_or_else(|| "brain".to_string())
}

impl AppConfig {
    /// 作用中腦。
    pub fn active_brain(&self) -> Option<&BrainEntry> {
        let id = self.active_brain_id.as_deref()?;
        self.brains.iter().find(|b| b.id == id)
    }
    /// 作用中腦的 GBRAIN_HOME 值（None = 預設腦，不設 env）。
    pub fn active_env_home(&self) -> Option<&str> {
        self.active_brain().and_then(|b| b.env_home())
    }

    /// 一次性、冪等的 migration：種預設腦、吸收舊 gbrain_home_override、修正 active。
    fn normalize(&mut self) {
        // locale 只接受支援值，否則清成 None（前端改回自動偵測）
        if !self
            .locale
            .as_deref()
            .map(|l| SUPPORTED_LOCALES.contains(&l))
            .unwrap_or(true)
        {
            self.locale = None;
        }
        if self.brains.is_empty() {
            self.brains.push(BrainEntry::default_brain());
        }
        // 吸收舊 override
        if let Some(h) = self
            .gbrain_home_override
            .take()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let h_path = PathBuf::from(&h).join(".gbrain");
            let is_default = dirs::home_dir().map(|d| d.join(".gbrain") == h_path).unwrap_or(false);
            if !is_default {
                let id = brain_id_from_home(&h);
                if !self.brains.iter().any(|b| b.gbrain_home.as_deref() == Some(h.as_str())) {
                    let name = Path::new(&h)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| h.clone());
                    self.brains.push(BrainEntry {
                        id: id.clone(),
                        name,
                        gbrain_home: Some(h),
                    });
                }
                // override 原本就是作用中腦 → 沿用
                self.active_brain_id = Some(id);
            }
        }
        // 確保 active 指向存在的 entry，否則退回預設腦
        let active_ok = self
            .active_brain_id
            .as_ref()
            .map(|id| self.brains.iter().any(|b| &b.id == id))
            .unwrap_or(false);
        if !active_ok {
            self.active_brain_id = Some(DEFAULT_BRAIN_ID.into());
            // 預設腦不存在就補一個（防呆）
            if !self.brains.iter().any(|b| b.is_default()) {
                self.brains.push(BrainEntry::default_brain());
            }
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
        let gbrain_exe = if cfg!(target_os = "windows") {
            format!("{home}\\.bun\\bin\\gbrain.exe")
        } else {
            format!("{home}/.bun/bin/gbrain")
        };
        let mut cfg = Self {
            notes_repo_path: format!("{home}/notes"),
            gbrain_exe_path: gbrain_exe,
            gbrain_home_override: None,
            brains: vec![],
            active_brain_id: None,
            active_source_id: None,
            auto_sync: true,
            sync_no_pull: true,
            factory_targets: FactoryTargets::default(),
            llm_temperature: default_temp(),
            llm_max_tokens: default_max_tokens(),
            locale: None,
        };
        cfg.normalize();
        cfg
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_seed_default_brain_and_active() {
        let c = AppConfig::default();
        assert!(c.brains.iter().any(|b| b.is_default()));
        assert_eq!(c.active_brain_id.as_deref(), Some(DEFAULT_BRAIN_ID));
        assert!(c.gbrain_home_override.is_none()); // 已被 normalize 清掉
    }

    #[test]
    fn migrates_override_into_registry() {
        let mut c = AppConfig::default();
        // 模擬舊版：只有 override、無 registry
        c.brains.clear();
        c.active_brain_id = None;
        c.gbrain_home_override = Some("C:/tmp/mybrain".into());
        c.normalize();
        // 種了預設腦 + mybrain
        assert!(c.brains.iter().any(|b| b.is_default()));
        let mine = c
            .brains
            .iter()
            .find(|b| b.gbrain_home.as_deref() == Some("C:/tmp/mybrain"))
            .unwrap();
        // active 沿用 override → 指向 mybrain
        assert_eq!(c.active_brain_id.as_deref(), Some(mine.id.as_str()));
        // override 已清
        assert!(c.gbrain_home_override.is_none());
    }

    #[test]
    fn migration_is_idempotent() {
        let mut c = AppConfig::default();
        c.brains.clear();
        c.active_brain_id = None;
        c.gbrain_home_override = Some("C:/tmp/idem".into());
        c.normalize();
        let n = c.brains.len();
        let active = c.active_brain_id.clone();
        c.normalize(); // 再跑一次
        assert_eq!(c.brains.len(), n);
        assert_eq!(c.active_brain_id, active);
    }

    #[test]
    fn active_brain_resolves() {
        let mut c = AppConfig::default();
        c.brains = vec![
            BrainEntry::default_brain(),
            BrainEntry {
                id: "demo".into(),
                name: "demo".into(),
                gbrain_home: Some("C:/demo".into()),
            },
        ];
        c.active_brain_id = Some("demo".into());
        assert_eq!(c.active_brain().unwrap().id, "demo");
        assert_eq!(c.active_env_home(), Some("C:/demo"));
        // 預設腦 env_home = None
        c.active_brain_id = Some(DEFAULT_BRAIN_ID.into());
        assert_eq!(c.active_env_home(), None);
    }

    #[test]
    fn dot_gbrain_path() {
        let d = BrainEntry::default_brain();
        assert!(d.dot_gbrain_path().ends_with(".gbrain"));
        let iso = BrainEntry {
            id: "x".into(),
            name: "x".into(),
            gbrain_home: Some("C:/parent".into()),
        };
        assert!(iso.dot_gbrain_path().ends_with(".gbrain"));
        assert!(iso.dot_gbrain_path().to_string_lossy().contains("parent"));
    }

    #[test]
    fn roundtrips_through_json() {
        let c = AppConfig::default();
        let v = serde_json::to_value(&c).unwrap();
        let back: AppConfig = serde_json::from_value(v).unwrap();
        assert_eq!(back.active_brain_id, c.active_brain_id);
        assert_eq!(back.brains.len(), c.brains.len());
    }
}
