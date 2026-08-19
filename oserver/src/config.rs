//! oserver 設定（P2 過渡）：**直接讀桌面 app 的 `app-settings.json`**
//! （P2–P4 過渡期決策——零設定重複、與桌面行為完全一致；`operoid.toml` 於 P5
//! 設定遷移時定形）。檔案格式為 tauri-plugin-store 的 JSON：`{"app_config": {...}}`。

use std::path::Path;

use ocore::app_config::AppConfig;

/// 桌面 app 的資料位置解析（與桌面殼一致）：
/// - **設定檔**在 Roaming（tauri-plugin-store 預設 `app_config_dir`）；
/// - **agent DB** 在 Local（P6a 為避 OneDrive 毀 WAL 而遷）。
/// `--data-dir` 同時覆寫兩者（測試隔離用）；None → 各自的桌面預設路徑。
#[derive(Clone)]
pub struct DataDirs {
    pub settings_dir: std::path::PathBuf,
    pub db_dir: std::path::PathBuf,
}

/// 桌面預設兩目錄（Roaming 設定＋Local DB）。
pub fn default_dirs() -> anyhow::Result<DataDirs> {
    resolve_dirs(None)
}

pub fn resolve_dirs(override_dir: Option<&str>) -> anyhow::Result<DataDirs> {
    match override_dir {
        Some(d) => {
            let p = std::path::PathBuf::from(d);
            Ok(DataDirs { settings_dir: p.clone(), db_dir: p })
        }
        None => {
            let settings_dir = dirs::config_dir()
                .map(|d| d.join("com.operoid.studio"))
                .ok_or_else(|| anyhow::anyhow!("無法解析 Roaming AppData 目錄"))?;
            let db_dir = dirs::data_local_dir()
                .map(|d| d.join("com.operoid.studio"))
                .ok_or_else(|| anyhow::anyhow!("無法解析 Local AppData 目錄"))?;
            Ok(DataDirs { settings_dir, db_dir })
        }
    }
}

/// 讀 `app-settings.json`（Roaming）的 `app_config` 鍵 → `AppConfig`。
/// 檔案不存在／缺鍵 → `AppConfig::default()`（與桌面 `app_config::load` 的容錯語意一致）。
pub fn load_config(dir: &Path) -> AppConfig {
    let path = dir.join("app-settings.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!("[oserver] app-settings.json 解析失敗，採預設設定");
        return AppConfig::default();
    };
    match v.get("app_config") {
        Some(cfg_v) => serde_json::from_value::<AppConfig>(cfg_v.clone()).unwrap_or_default(),
        None => AppConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 缺檔 → default；`app_config` 鍵 → 正確反序列化（腦清單可帶過來）。
    #[test]
    fn loads_app_config_key() {
        let tmp = std::env::temp_dir().join(format!("oserver-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // 缺檔
        let c = load_config(&tmp);
        assert!(!c.agent_os_enabled);
        // 有檔
        // notes_repo_path/gbrain_exe_path 無 serde default（必填）——缺了會整體反序列化失敗退 default。
        let json = r#"{"app_config": {"notes_repo_path": "C:/notes", "gbrain_exe_path": "C:/gbrain.exe", "agent_os_enabled": true, "brains": [{"id":"b1","name":"demo","gbrain_home":"C:/x"}]}}"#;
        std::fs::write(tmp.join("app-settings.json"), json).unwrap();
        let c = load_config(&tmp);
        assert!(c.agent_os_enabled);
        assert_eq!(c.brains.len(), 1);
        assert_eq!(c.brains[0].id, "b1");
        assert_eq!(c.brains[0].id, "b1");
        std::fs::remove_dir_all(&tmp).ok();
    }
}

/// 保存 AppConfig 回 `app-settings.json`（與桌面殼同一檔、同一格式）。
/// 原子寫（先寫暫存再改名）——避免半寫狀態。
pub fn save_config(dir: &Path, cfg: &AppConfig) -> anyhow::Result<()> {
    use std::io::Write;
    let path = dir.join("app-settings.json");
    // 保留檔案中其他鍵（目前僅 app_config；如實重建）
    let mut root = serde_json::Map::new();
    root.insert("app_config".into(), serde_json::to_value(cfg)?);
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(root))?;
    let tmp = path.with_extension("json.tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
