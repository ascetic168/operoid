//! Obridge 設定代管（GUI 編輯 `obridge.toml`）＋子進程代管（opt-in）。
//!
//! - **編輯器**：讀寫原始文字，不解讀內容（通道帳密屬 bridge 職責，契約「抉擇二」）。
//!   路徑來自 `AppConfig.obridge_config_path`。obridge 自行 watch 檔案 mtime **熱重載**
//!   通道設定；`[listen]`/`[operoid]` 區段變更需重啟。
//! - **子進程代管**（`obridge_autostart`）：Operoid 啟動時 spawn obridge、退出時帶走；
//!   設定頁存檔後自動重啟子進程（讓非熱重載區段也生效）。未開 → obridge 使用者自管。

use std::process::Child;
use std::sync::Mutex;

use tauri::AppHandle;

use crate::config::app_config;
use crate::i18n::AppError;

/// 代管的 obridge 子進程（None＝未代管）。
static CHILD: Mutex<Option<Child>> = Mutex::new(None);

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
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::new(format!("obridge.readFailed: {} ({e})", path.display())))
}

/// 寫回 obridge.toml（原子寫：先寫暫存再改名，避免半寫狀態）。
/// 若 autostart 開啟且已代管子進程 → 順手重啟（套用 listen/operoid 等非熱重載區段；
/// 通道區段 obridge 自己會熱重載，重啟一併覆蓋、無害）。
#[tauri::command]
pub fn obridge_config_save(app: AppHandle, content: String) -> Result<(), AppError> {
    let path = resolve_path(&app)?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, content)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|e| AppError::new(format!("obridge.writeFailed: {} ({e})", path.display())))?;
    if let Err(e) = restart_if_managed(&app) {
        eprintln!("[obridge] 設定已存檔，但重啟 obridge 子進程失敗：{e}");
    }
    Ok(())
}

/// App 啟動時呼叫（lib.rs setup）：`obridge_autostart` 開啟 → spawn 子進程。
pub fn autostart(app: &AppHandle) {
    let Ok(cfg) = app_config::load(app) else { return };
    if !cfg.obridge_autostart {
        return;
    }
    if let Err(e) = spawn(&app) {
        eprintln!("[obridge] autostart 失敗：{e}");
    }
}

/// App 退出時呼叫：帶走代管的子進程（best-effort）。
pub fn kill_managed() {
    if let Ok(mut guard) = CHILD.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// 已代管 → 重啟（kill＋respawn）；未代管（autostart 關或尚未 spawn）→ no-op。
fn restart_if_managed(app: &AppHandle) -> anyhow::Result<()> {
    let managed = CHILD.lock().map(|g| g.is_some()).unwrap_or(false);
    if !managed {
        return Ok(());
    }
    kill_managed();
    spawn(app)
}

/// spawn obridge 子進程（`--config <path>`；Windows 隱藏 console 視窗）。
fn spawn(app: &AppHandle) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    let (Some(exe), Some(config_path)) = (&cfg.obridge_executable, &cfg.obridge_config_path)
    else {
        anyhow::bail!("obridge_autostart 需要 obridge_executable 與 obridge_config_path 皆已設定");
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--config").arg(config_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn {exe} 失敗：{e}"))?;
    eprintln!(
        "[obridge] 子進程已啟動（pid={}，config={config_path}）",
        child.id()
    );
    *CHILD.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))? = Some(child);
    Ok(())
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
