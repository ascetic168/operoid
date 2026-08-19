//! `oserver` — Operoid 服務 binary（前後端分離計畫 P2–P5）。
//!
//! 啟動序（三條紀律）：bind-first → healthz（warming→ready）→ 背景 init。
//! 關機觸發：一般模式＝Ctrl+C；服務模式＝SCM Stop 設 `SERVICE_STOP`（等價）。
//! 優雅關機：停 accept → 等 `busy_ids()` 清空（上限 120s）→ 退出。
//!
//! 模式：一般（前景）／`--service`（Windows SCM dispatcher；Linux/macOS 前景同一般）。
//! 子命令：`install`／`uninstall`／`status`（P5）。
//! token：`OSERVER_TOKEN` env **或** `app-settings.json` 的 `server_token`
//! （服務模式無使用者 env——由設定檔提供）。

mod auth;
mod config;
mod gbrain;
mod operations;
mod routes;
mod service;
mod writes;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use auth::TokenProvider;
use ocore::agent_state::AppState;
use ocore::runtime::agent_db_path_in;
use ocore::scheduler;

use crate::routes::ServerState;

/// scheduler 的 AppState 全域暫存（優雅關機查 busy 用；全 Arc 欄位，Clone 同一份）。
static SHARED_STATE: OnceLock<AppState> = OnceLock::new();
/// 服務模式的停止旗標（SCM Stop 設 true——與 Ctrl+C 等價的關機觸發）。
pub(crate) static SERVICE_STOP: AtomicBool = AtomicBool::new(false);

/// 解析 CLI 中與目錄/port 相關的引數（供 run() 與子命令共用）。
struct DirArgs {
    settings_dir: Option<String>,
    db_dir: Option<String>,
    data_dir: Option<String>,
    port: u16,
}

fn parse_args() -> DirArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut a = DirArgs { settings_dir: None, db_dir: None, data_dir: None, port: 7340 };
    if let Ok(p) = std::env::var("OSERVER_PORT") {
        if let Ok(v) = p.parse() {
            a.port = v;
        }
    }
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data-dir" if i + 1 < args.len() => {
                a.data_dir = Some(args[i + 1].clone());
                i += 2;
            }
            "--settings-dir" if i + 1 < args.len() => {
                a.settings_dir = Some(args[i + 1].clone());
                i += 2;
            }
            "--db-dir" if i + 1 < args.len() => {
                a.db_dir = Some(args[i + 1].clone());
                i += 2;
            }
            "--port" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse() {
                    a.port = v;
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    a
}

/// 由引數解析最終兩目錄（--settings-dir/--db-dir 優先 → --data-dir 同覆 → 桌面預設）。
fn resolve_from_args(a: &DirArgs) -> anyhow::Result<config::DataDirs> {
    if let Some(s) = &a.settings_dir {
        let db = a.db_dir.clone().unwrap_or_else(default_db_dir);
        return Ok(config::DataDirs { settings_dir: s.into(), db_dir: db.into() });
    }
    if let Some(d) = &a.db_dir {
        return Ok(config::DataDirs { settings_dir: default_settings_dir().into(), db_dir: d.into() });
    }
    match &a.data_dir {
        Some(d) => Ok(config::DataDirs { settings_dir: d.into(), db_dir: d.into() }),
        None => config::default_dirs(),
    }
}

fn default_settings_dir() -> String {
    config::default_dirs()
        .map(|d| d.settings_dir.to_string_lossy().into_owned())
        .unwrap_or_default()
}
fn default_db_dir() -> String {
    config::default_dirs()
        .map(|d| d.db_dir.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn main() {
    let a = parse_args();

    // ── 子命令分派 ──
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("install") => {
            let dirs = resolve_from_args(&a).expect("解析資料目錄失敗");
            if let Err(e) = service::install(&dirs.settings_dir, &dirs.db_dir) {
                eprintln!("[oserver] install 失敗：{e}");
                std::process::exit(1);
            }
            return;
        }
        Some("uninstall") => {
            if let Err(e) = service::uninstall() {
                eprintln!("[oserver] uninstall 失敗：{e}");
                std::process::exit(1);
            }
            return;
        }
        Some("status") => {
            let installed = service::is_installed().unwrap_or(false);
            let running = std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{}", a.port).parse().expect("addr"),
                Duration::from_secs(1),
            )
            .is_ok();
            println!("{{\"installed\": {installed}, \"running\": {running}}}");
            return;
        }
        _ => {}
    }

    let service_mode = std::env::args().any(|x| x == "--service");
    if service_mode && cfg!(windows) {
        // Windows：SCM dispatcher（阻塞至服務停止）。
        if let Err(e) = service::run_service() {
            eprintln!("[oserver] 服務模式失敗：{e}");
            std::process::exit(1);
        }
        return;
    }
    // 一般模式（含 Linux/macOS 的 --service——前景執行，由 systemd/launchd 託管重啟）。
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("建 tokio runtime 失敗");
    if let Err(e) = rt.block_on(run(&a)) {
        eprintln!("[oserver] 啟動失敗：{e}");
        std::process::exit(1);
    }
}

async fn run(a: &DirArgs) -> anyhow::Result<()> {
    let dirs = resolve_from_args(a)?;
    let cfg = config::load_config(&dirs.settings_dir);

    // token：env 優先，否則設定檔 server_token（服務模式路徑）。
    let token = std::env::var("OSERVER_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| cfg.server_token.clone().filter(|t| !t.trim().is_empty()));
    let Some(token) = token else {
        anyhow::bail!(
            "無 token——設 OSERVER_TOKEN env，或 app-settings.json 需有 server_token（GUI 首次啟動會生成）"
        );
    };
    let db_path = agent_db_path_in(&dirs.db_dir);

    // ── bind-first（單例守衛）──
    let addr = format!("127.0.0.1:{}", a.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("bind {addr} 失敗（已有 oserver 實例？）：{e}"))?;
    eprintln!("[oserver] 監聽 http://{addr}（healthz: /healthz）");

    // AppState（寫入面喚醒＋scheduler 共用；CfgLoader 每次呼叫重讀設定檔——熱生效）。
    let (wake_tx, wake_rx) = tokio::sync::mpsc::channel(64);
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(128);
    let app_state = AppState::new(wake_tx, event_tx, cfg.llm_concurrency);
    let _ = SHARED_STATE.set(app_state.clone());

    let ready = Arc::new(AtomicBool::new(false));
    let state = Arc::new(ServerState {
        auth: Arc::new(TokenProvider::new(token)),
        cfg: cfg.clone(),
        db_path: db_path.clone(),
        ready: Arc::clone(&ready),
        agent_state: Some(app_state.clone()),
        ops: Arc::new(operations::OpRegistry::new()),
        settings_dir: dirs.settings_dir.clone(),
    });

    // 驗 DB（spawn_blocking——rusqlite 同步 API）。
    let db_check = db_path.clone();
    match tokio::task::spawn_blocking(move || ocore::domain::SqliteStore::open(&db_check)).await {
        Ok(Ok(_)) => eprintln!("[oserver] DB 就緒：{}", db_path.display()),
        Ok(Err(e)) => anyhow::bail!("DB 開啟失敗：{e}"),
        Err(e) => anyhow::bail!("DB 檢查任務失敗：{e}"),
    }
    if !cfg.agent_os_enabled {
        eprintln!("[oserver] 注意：agent_os_enabled=false——API 將回 503");
    }

    let dir_for_loader = dirs.settings_dir.clone();
    let load_cfg: scheduler::CfgLoader = Arc::new(move || Ok(config::load_config(&dir_for_loader)));
    scheduler::spawn_loop(app_state, load_cfg, db_path.clone(), wake_rx, event_rx);
    eprintln!("[oserver] scheduler 已啟動（30s tick＋事件/訊息喚醒）");
    ready.store(true, Ordering::SeqCst);
    eprintln!("[oserver] 就緒（healthz → ready）");

    let app = routes::router(Arc::clone(&state))
        .merge(writes::write_routes().with_state(Arc::clone(&state)))
        .merge(gbrain::gbrain_routes().with_state(state));

    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_wait());
    server.await?;
    eprintln!("[oserver] 已退出");
    Ok(())
}

/// 關機觸發：Ctrl+C **或** 服務模式的 SERVICE_STOP（SCM Stop 設）。
async fn shutdown_trigger() {
    let ctrl = tokio::signal::ctrl_c();
    tokio::select! {
        _ = ctrl => { eprintln!("[oserver] 收到 Ctrl+C"); }
        _ = async {
            loop {
                if SERVICE_STOP.load(Ordering::SeqCst) { return; }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        } => { eprintln!("[oserver] 收到服務停止信號"); }
    }
}

/// 優雅關機（graceful_shutdown future）：觸發→等 busy 員工清空（上限 120s）→完成。
async fn shutdown_wait() {
    shutdown_trigger().await;
    eprintln!("[oserver] 等待執行中員工完成（上限 120s）…");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let busy = SHARED_STATE.get().map(|s| s.busy_ids()).unwrap_or_default();
        if busy.is_empty() {
            eprintln!("[oserver] 所有員工已閒置，關機");
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            eprintln!("[oserver] 等待逾時（仍在跑：{busy:?}），強制關機");
            return;
        }
        eprintln!("[oserver] 仍在跑：{busy:?}，續等…");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
