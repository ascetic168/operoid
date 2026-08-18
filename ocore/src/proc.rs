//! 子行程小工具（原 src-tauri `gbrain_cli` 的純函式；P1b 上移 ocore 供 runtime 使用）。

/// 由顯式 home（GBRAIN_HOME 值；None=預設腦）組子行程環境：PYTHONUTF8=1＋GBRAIN_HOME。
pub fn env_for_brain(home: Option<&str>) -> Vec<(&'static str, std::ffi::OsString)> {
    let mut env: Vec<(&'static str, std::ffi::OsString)> = vec![("PYTHONUTF8", "1".into())];
    if let Some(h) = home.map(str::trim).filter(|h| !h.is_empty()) {
        env.push(("GBRAIN_HOME", h.into()));
    }
    env
}

/// 寬容解碼整段 buffer：UTF-8 優先，失敗退 BIG5(cp950)。
pub fn decode_buf(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::BIG5.decode(bytes);
            cow.into_owned()
        }
    }
}

/// 壓制 Windows console 子視窗。release build 為 GUI 子系統，spawn 子行程時 Windows 會為其
/// 配置新 console 造成黑色視窗閃現；設 `CREATE_NO_WINDOW` 避免。非 Windows 為 no-op。
///
/// `std::process::Command` 的 `creation_flags` 來自 std 的 `CommandExt` trait；
/// `tokio::process::Command` 則是自帶 inherent method，故分兩函式。
#[cfg(windows)]
pub fn no_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt as _;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
#[allow(unused_variables)]
pub fn no_console(cmd: &mut std::process::Command) {}

/// 同上，給 `tokio::process::Command`（串流/捕獲子行程用）。
#[cfg(windows)]
pub fn no_console_async(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
#[allow(unused_variables)]
pub fn no_console_async(_cmd: &mut tokio::process::Command) {}
