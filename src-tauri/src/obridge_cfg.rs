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

/// 解析 obridge.toml 路徑。**約定優於設定**：未設 `obridge_config_path` → 預設
/// `<app-data>/obridge/obridge.toml`（載入時自動種範本）——新使用者零設定即可在
/// 設定頁直接編輯範本，不需手改 app-settings.json。
fn resolve_path(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    let cfg = app_config::load(app)?;
    if let Some(p) = &cfg.obridge_config_path {
        return Ok(std::path::PathBuf::from(p));
    }
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::new(format!("obridge.noAppDataDir: {e}")))?;
    Ok(dir.join("obridge").join("obridge.toml"))
}

/// 預設範本（單一來源：obridge crate 的 config.example.toml——兩 crate 共享同一份）。
const TEMPLATE: &str = include_str!("../../obridge/config.example.toml");

/// 設定檔不存在或**空** → 複製範本到該路徑（含父目錄建立）。回傳是否已產生。
fn ensure_config_file(path: &std::path::Path) -> std::io::Result<bool> {
    let empty = std::fs::read_to_string(path)
        .map(|s| s.trim().is_empty())
        .unwrap_or(true); // 讀不到（不存在）視同空
    if !empty {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, TEMPLATE)?;
    Ok(true)
}

/// 讀 obridge.toml 原始內容。檔案不存在或空 → 先自動複製範本到該路徑（使用者設完
/// `obridge_config_path` 重啟後，設定頁直接看到可編輯的範本，不必手動建立檔案）。
#[tauri::command]
pub fn obridge_config_load(app: AppHandle) -> Result<String, AppError> {
    let path = resolve_path(&app)?;
    ensure_config_file(&path).map_err(|e| {
        AppError::new(format!("obridge.writeFailed: {} ({e})", path.display()))
    })?;
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

/// 解析 obridge 執行檔：`obridge_executable` 有設 → 用之；否則**自動偵測** Operoid
/// 執行檔同目錄的 `obridge(.exe)`（workspace 開發環境＝同一 target/debug；正式佈署
/// side-by-side 同目錄）。找不到 → None（autostart 記 log 跳過，使用者可手填路徑）。
fn resolve_executable(cfg: &app_config::AppConfig) -> Option<std::path::PathBuf> {
    if let Some(p) = &cfg.obridge_executable {
        return Some(std::path::PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for name in ["obridge.exe", "obridge"] {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// spawn obridge 子進程（`--config <path>`；Windows 隱藏 console 視窗）。
fn spawn(app: &AppHandle) -> anyhow::Result<()> {
    let cfg = app_config::load(app)?;
    let config_path = resolve_path(app)?;
    let Some(exe) = resolve_executable(&cfg) else {
        anyhow::bail!("找不到 obridge 執行檔——請在設定頁填 obridge_executable（或把 obridge.exe 放在 Operoid 執行檔同目錄）");
    };
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--config").arg(&config_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn {} 失敗：{e}", exe.display()))?;
    eprintln!(
        "[obridge] 子進程已啟動（pid={}，config={}）",
        child.id(),
        config_path.display()
    );
    *CHILD.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))? = Some(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// ensure_config_file：不存在／空 → 複製範本（含父目錄）；有內容 → 不動。
    #[test]
    fn ensure_config_file_seeds_template() {
        let dir = std::env::temp_dir().join(format!(
            "operoid-obridge-seed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let path = dir.join("nested").join("obridge.toml"); // 父目錄也不存在
        // 不存在 → 產生範本（含父目錄）。
        assert!(ensure_config_file(&path).unwrap());
        let seeded = std::fs::read_to_string(&path).unwrap();
        assert!(seeded.contains("[operoid]"), "應為範本內容");
        assert!(seeded.contains("[channels.email_imap.imap]"));
        // 空檔 → 也重灌範本。
        std::fs::write(&path, "   \n").unwrap();
        assert!(ensure_config_file(&path).unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("[operoid]"));
        // 有內容 → 不覆蓋。
        std::fs::write(&path, "[operoid]\ningress_url = \"x\"\n").unwrap();
        assert!(!ensure_config_file(&path).unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("x"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
