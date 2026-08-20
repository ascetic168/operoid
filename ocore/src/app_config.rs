//! Operoid 自有設定（AppConfig）——結構、預設值、冪等 migration（P1b 起居於 ocore，純 Rust）。
//!
//! 只放「GBrain config 沒有、純屬本系統」的東西。GBrain 腦本身的行為
//! （model/embedding/schema/provider...）一律讀該腦的 config.json，不在此重抄。
//!
//! 腦（Brains）註冊表：gbrain 沒有「列出所有腦」的指令，故本程式自管一份清單。
//! 每個 `BrainEntry` = 一個腦（`gbrain_home=None` = 預設腦 ~/.gbrain；`Some(parent)` = 隔離腦）。
//! 持久化（tauri-plugin-store 的 load/save）留在桌面殼 `src-tauri/src/config/app_config.rs`。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 預設腦的固定 id。
pub const DEFAULT_BRAIN_ID: &str = "__default__";

/// 支援的介面語言（與前端 languageConfig 對齊）。
pub const SUPPORTED_LOCALES: &[&str] = &["zh-TW", "zh-CN", "en"];

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
    /// LLM 結構化的取樣溫度。
    #[serde(default = "default_temp")]
    pub llm_temperature: f64,
    /// LLM 結構化的最大輸出 token。
    #[serde(default = "default_max_tokens")]
    pub llm_max_tokens: u32,
    /// 介面語言覆寫（None = 依系統語言自動偵測；Some = 使用者手動釘選）。
    #[serde(default)]
    pub locale: Option<String>,
    /// 「開啟 Claude Code」最近用過的工作目錄（最多 3 個，前端維護）。
    #[serde(default)]
    pub recent_claude_cwds: Vec<String>,
    /// 「開啟 Claude Code」偏好終端機 profile id（如 "wt"/"cmd"/"powershell"；或 "custom"）。
    #[serde(default)]
    pub claude_terminal: Option<String>,
    /// 自訂終端機指令範本（claude_terminal == "custom" 時使用；含 {cwd}/{cmd} 佔位字元）。
    #[serde(default)]
    pub claude_terminal_template: Option<String>,
    /// Agent-OS 子系統開關（runtime feature flag）。Phase 0 不讀取；Phase 1 起的指令據此啟用。
    /// 預設關閉；既有 app-settings.json 因 `#[serde(default)]` 可無痛載入。
    #[serde(default)]
    pub agent_os_enabled: bool,
    /// LLM 並發上限（全域 Semaphore permit 數；節流「全部喚醒」的尖峰並發 LLM 呼叫）。
    #[serde(default = "default_llm_concurrency")]
    pub llm_concurrency: usize,
    /// 工廠寫檔後是否觸發員工 review（Event 匯流排開關；Phase 7c）。
    #[serde(default = "default_true")]
    pub event_review_enabled: bool,
    /// 外部事件 ingress HTTP port（E7 進氣口）。`None` → 不啟動 ingress server（預設、最安全）。
    /// 設為某 port → 啟動 `POST /event`（127.0.0.1:port），供外部 bridge（Email/IM/…）投遞事件。
    /// 見 `docs/Operoid-設計-統一事件ingress契約.md`。
    #[serde(default)]
    pub event_ingress_port: Option<u16>,
    /// ingress server 共用密鑰（`Authorization: Bearer <secret>`）。port 有設但此為 None →
    /// server 不啟動（避免無認證暴露）。bridge 端需知道此密鑰。
    #[serde(default)]
    pub event_ingress_secret: Option<String>,
    /// outbound（E7 外發）bridge 的 send endpoint URL（如 `http://127.0.0.1:7342/send`）。
    /// `None` → 不外發（回覆僅留在 Operoid 對話歷史；預設）。設置後，源自外部事件的
    /// 對話回合回覆會自動 POST 給 bridge（免人類核可——見待處理清單 E7 決策紀錄）。
    #[serde(default)]
    pub event_outbound_url: Option<String>,
    /// outbound bridge 共用密鑰（`Authorization: Bearer <secret>`）；有設才帶。
    #[serde(default)]
    pub event_outbound_secret: Option<String>,
    /// Obridge（Operoid Bridge）設定檔 `obridge.toml` 的路徑。`None`（預設）→ 設定頁
    /// 不顯示 Obridge 區塊。設定後，設定頁可讀寫該檔（原始文字——Operoid 只當編輯器，
    /// 不解讀內容；通道帳密屬 bridge 職責，契約「抉擇二」）。obridge 會 watch 此檔並熱重載
    /// 通道設定。
    #[serde(default)]
    pub obridge_config_path: Option<String>,
    /// Operoid 啟動時自動把 obridge 帶起為子進程（關閉時帶走；設定頁存檔後自動重啟它）。
    /// 需 `obridge_config_path`＋`obridge_executable` 皆有設才生效。`false`（預設）→
    /// obridge 由使用者自行管理（手動啟動）。
    #[serde(default)]
    pub obridge_autostart: bool,
    /// obridge 執行檔路徑（autostart 用）。
    #[serde(default)]
    pub obridge_executable: Option<String>,
    /// 本地服務（oserver）自動帶起（P3）：GUI 啟動時 healthz 無回應則 spawn（detached——
    /// GUI 退出**不帶走**，服務續跑：分離核心價值）。預設 true。
    #[serde(default = "default_true")]
    pub server_autostart: bool,
    /// 本地服務 port（oserver 監聽 127.0.0.1）。預設 7340。
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    /// 本地服務 shared token（前端 HTTP 認證用）。None → 殼層首次自動生成並保存。
    #[serde(default)]
    pub server_token: Option<String>,
    /// oserver 執行檔路徑（代管用；缺省 → 自動偵測：app exe 同目錄 → dev 的 target/debug）。
    #[serde(default)]
    pub server_executable: Option<String>,
    /// 前置程式版本字串快取（P5：探測不 spawn——版本背景刷新後回寫於此）。
    #[serde(default)]
    pub prereq_cache: Option<crate::prereq::PrereqCache>,
    /// LLM provider API key 環境變數快取（P5：服務以 LocalSystem 跑，看不到使用者
    /// 環境——殼層於帶起/安裝服務時快照於此；oserver 啟動時注入自身環境，
    /// gbrain 子行程自動繼承）。僅鍵存在於殼層環境者會被快照。
    #[serde(default)]
    pub llm_env: std::collections::BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
}
fn default_temp() -> f64 {
    0.2
}
fn default_llm_concurrency() -> usize {
    4
}
fn default_server_port() -> u16 {
    7340
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
    /// LLM 採樣參數切片（P1a：`ocore::llm::complete` 不收 AppConfig，由此轉換）。
    pub fn llm_sampling(&self) -> crate::llm::SamplingParams {
        crate::llm::SamplingParams {
            temperature: self.llm_temperature,
            max_tokens: self.llm_max_tokens,
        }
    }

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
    pub fn normalize(&mut self) {
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
            llm_temperature: default_temp(),
            llm_max_tokens: default_max_tokens(),
            locale: None,
            recent_claude_cwds: Vec::new(),
            claude_terminal: None,
            claude_terminal_template: None,
            agent_os_enabled: false,
            llm_concurrency: default_llm_concurrency(),
            event_review_enabled: true,
            event_ingress_port: None,
            event_ingress_secret: None,
            event_outbound_url: None,
            event_outbound_secret: None,
            obridge_config_path: None,
            obridge_autostart: false,
            obridge_executable: None,
            server_autostart: true,
            server_port: default_server_port(),
            server_token: None,
            server_executable: None,
            prereq_cache: None,
            llm_env: std::collections::BTreeMap::new(),
        };
        cfg.normalize();
        cfg
    }
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


/// 由註冊表查找腦（原 src-tauri `brains::brain_entry`；P1b 上移 ocore 供 runtime 使用）。
pub fn brain_entry<'a>(c: &'a AppConfig, brain_id: &str) -> Result<&'a BrainEntry, crate::i18n::AppError> {
    c.brains
        .iter()
        .find(|b| b.id == brain_id)
        .ok_or_else(|| crate::i18n::AppError::new("brain.notFound").p("id", brain_id))
}
