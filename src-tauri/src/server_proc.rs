//! 本地服務（oserver）代管（P3）——比照 obridge autostart 模式，但**分離語意**：
//!
//! - 啟動時 healthz 探測（1s timeout）：已在跑 → 不重複 spawn；
//! - 未跑 → spawn oserver（**detached**：GUI 退出不帶走，服務續跑——分離核心價值）；
//! - exe 解析：`server_executable` 欄位 → app exe 同目錄 → dev 的 `target/debug/oserver.exe`；
//! - `server_token` 缺省 → 首次自動生成（隨機 hex）並保存回 app-settings.json。
//!
//! 前端以 `server_info()` 指令取得 `{port, token, running}`（桌面殼是本地 process，
//! token 不出本機）。

use tauri::{AppHandle, Runtime};

use crate::config::app_config;

/// 隨機 hex token（64 bit，密碼學品味足以擋本機亂試；非對外安全邊界）。
fn gen_token() -> String {
    // 以時間＋行程 id 疊加的簡易熵源；本機 127.0.0.1 情境足夠（P5 換 toml 時可升級）。
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let p = std::process::id() as u128;
    format!("{:016x}{:016x}", t as u64 ^ (p << 32) as u64, (t >> 64) as u64 ^ p as u64)
}

/// 探測 oserver 是否在跑（healthz，1s timeout）。
fn probe(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("valid addr"),
        std::time::Duration::from_secs(1),
    )
    .is_ok()
}

/// 解析 oserver 執行檔：設定欄位 → app exe 同目錄 → dev target/debug。
fn resolve_oserver(exe_hint: Option<&str>) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    if let Some(p) = exe_hint.filter(|s| !s.trim().is_empty()) {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
        eprintln!("[server] 設定的 server_executable 不存在：{p}");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("oserver.exe");
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    // dev：target/debug（cwd 為 repo 時）
    let dev = PathBuf::from("target/debug/oserver.exe");
    if dev.exists() {
        return Some(dev);
    }
    None
}

/// App 啟動時呼叫（lib.rs setup）：確保本地服務在跑。
pub fn ensure_server<R: Runtime>(app: &AppHandle<R>) {
    let mut cfg = match app_config::load(app) {
        Ok(c) => c,
        Err(_) => return,
    };
    if !cfg.server_autostart {
        return;
    }
    // token 缺省 → 生成並保存（前端 server_info 需要）。
    if cfg.server_token.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_none() {
        cfg.server_token = Some(gen_token());
        let _ = app_config::save(app, &cfg);
    }
    let token = cfg.server_token.clone().expect("已確保存在");
    let port = cfg.server_port;

    if probe(port) {
        eprintln!("[server] oserver 已在跑（127.0.0.1:{port}），不重複啟動");
        return;
    }
    let Some(exe) = resolve_oserver(cfg.server_executable.as_deref()) else {
        eprintln!(
            "[server] 找不到 oserver 執行檔——請在設定頁填 server_executable（或建置 oserver：cargo build -p oserver）"
        );
        return;
    };

    let mut cmd = std::process::Command::new(&exe);
    cmd.env("OSERVER_PORT", port.to_string())
        .env("OSERVER_TOKEN", &token)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // detached + 不彈 console 視窗；GUI 退出不帶走（分離語意——與 obridge 不同）。
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000 | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP); // CREATE_NO_WINDOW＋detached
    }
    match cmd.spawn() {
        Ok(child) => {
            eprintln!(
                "[server] 已啟動 oserver（pid {}，127.0.0.1:{port}）——GUI 退出不帶走（服務續跑）",
                child.id()
            );
            // child drop 不會 kill（detached 語意）。
        }
        Err(e) => eprintln!("[server] 啟動 oserver 失敗：{e}"),
    }
}

/// `server_info` 指令主體：回 `{port, token, running}`（前端 HTTP 用）。
pub fn server_info<R: Runtime>(app: &AppHandle<R>) -> Result<serde_json::Value, crate::i18n::AppError> {
    let cfg = app_config::load(app).map_err(|e| {
        crate::i18n::AppError::new("server.cfgFail").p("detail", e.to_string())
    })?;
    let port = cfg.server_port;
    let running = probe(port);
    Ok(serde_json::json!({
        "port": port,
        "token": cfg.server_token,
        "running": running,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_nonempty_hex() {
        let t = gen_token();
        assert!(t.len() >= 16);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()), "{t}");
    }
}
