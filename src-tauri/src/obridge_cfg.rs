//! Obridge 設定代管（GUI 編輯 `obridge.toml`）——Operoid 只當**編輯器**：讀寫原始文字，
//! 不解讀內容（通道帳密屬 bridge 職責，契約「抉擇二」：Source 不成 Operoid 一級實體）。
//! 路徑來自 `AppConfig.obridge_config_path`（未設 → 指令回錯，前端不顯示區塊）。
//! 存檔後 **obridge 需重啟**才套用（v1 不做熱重載）。

use tauri::AppHandle;

use crate::config::app_config;
use crate::i18n::AppError;

/// 解析 obridge.toml 路徑（未設定 → Err）。
fn resolve_path(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    let cfg = app_config::load(app)?;
    cfg.obridge_config_path
        .map(std::path::PathBuf::from)
        .ok_or_else(|| AppError::new("obridge.noConfigPath"))
}

/// 讀 obridge.toml 原始內容。
#[tauri::command]
pub fn obridge_config_load(app: AppHandle) -> Result<String, AppError> {
    let path = resolve_path(&app)?;
    std::fs::read_to_string(&path).map_err(|e| {
        AppError::new(format!("obridge.readFailed: {} ({e})", path.display()))
    })
}

/// 寫回 obridge.toml（原子寫：先寫暫存再改名，避免半寫狀態）。
#[tauri::command]
pub fn obridge_config_save(app: AppHandle, content: String) -> Result<(), AppError> {
    let path = resolve_path(&app)?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, content)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|e| AppError::new(format!("obridge.writeFailed: {} ({e})", path.display())))
}

#[cfg(test)]
mod tests {

    /// 原子寫往返：save → load 讀回一致；暫存檔不留。
    #[test]
    fn save_then_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("operoid-obridge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("obridge.toml");
        std::fs::write(&path, "[operoid]\n").unwrap();
        // 直接測核心邏輯（指令的 AppHandle 包裝在 Tauri 環境另測）。
        let content = "[listen]\nport = 17401\n";
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, content)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        assert!(!tmp.exists(), "暫存檔應已改名消失");
        std::fs::remove_dir_all(&dir).ok();
    }
}
