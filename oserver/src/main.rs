//! `oserver` — Operoid 服務 binary（前後端分離計畫 P2）。
//!
//! 啟動序（三條紀律，計畫 P2）：
//! 1. **bind-first**：bind 127.0.0.1:port → `/healthz` 回 `warming`；
//!    bind 失敗（**單例守衛**——已有實例占用 port）→ 明確報錯退出；
//! 2. 初始化（不阻塞 accept）：驗 DB 可開、建 AppState、起 scheduler loop；
//! 3. 初始化完成 → healthz 轉 `ready`。
//!
//! 優雅關機（Ctrl+C）：停止 accept → 等 `busy_ids()` 清空（員工跑完當前 cycle，
//! 上限 120s）→ 退出。
//!
//! 設定：P2–P4 過渡期**直接讀桌面 app-settings.json**（見 `config.rs`）；
//! port/token 走環境變數 `OSERVER_PORT`（預設 7340）／`OSERVER_TOKEN`（必設）。

mod auth;
mod config;
mod gbrain;
mod routes;
mod operations;
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

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("建 tokio runtime 失敗");
    if let Err(e) = rt.block_on(run()) {
        eprintln!("[oserver] 啟動失敗：{e}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    // ── 參數解析（零依賴手 parse：--data-dir / --port）──
    let args: Vec<String> = std::env::args().collect();
    let mut data_dir_arg: Option<String> = None;
    let mut port: u16 = std::env::var("OSERVER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7340);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data-dir" if i + 1 < args.len() => {
                data_dir_arg = Some(args[i + 1].clone());
                i += 2;
            }
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().unwrap_or(port);
                i += 2;
            }
            _ => i += 1,
        }
    }
    let token = std::env::var("OSERVER_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());
    let Some(token) = token else {
        anyhow::bail!(
            "未設 OSERVER_TOKEN——服務需要共享密鑰才能啟動（PowerShell：$env:OSERVER_TOKEN=\"<任意密鑰>\"）"
        );
    };
    let dirs = config::resolve_dirs(data_dir_arg.as_deref())?;
    let cfg = config::load_config(&dirs.settings_dir);
    let db_path = agent_db_path_in(&dirs.db_dir);

    // ── bind-first（單例守衛：port 占用即明確報錯）──
    let addr = format!("127.0.0.1:{port}");
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

    // ── 初始化（不阻塞 accept；healthz 已可回 warming）──
    // 驗 DB 可開（spawn_blocking——rusqlite 同步 API 不占 async worker）。
    let db_check = db_path.clone();
    match tokio::task::spawn_blocking(move || ocore::domain::SqliteStore::open(&db_check)).await {
        Ok(Ok(_)) => eprintln!("[oserver] DB 就緒：{}", db_path.display()),
        Ok(Err(e)) => {
            // 提前失敗：healthz 停在 warming、log 明示原因。
            anyhow::bail!("DB 開啟失敗：{e}");
        }
        Err(e) => anyhow::bail!("DB 檢查任務失敗：{e}"),
    }
    if !cfg.agent_os_enabled {
        eprintln!("[oserver] 注意：agent_os_enabled=false（app-settings.json）——API 將回 503");
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
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());
    server.await?;
    eprintln!("[oserver] 已退出");
    Ok(())
}

/// 優雅關機：Ctrl+C → 停止 accept → 等 busy 員工清空（上限 120s）→ 退出。
async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("安裝 Ctrl+C 處理失敗");
    eprintln!("[oserver] 收到關機信號，等待執行中員工完成（上限 120s）…");
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
