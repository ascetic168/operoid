//! `oserver` — Operoid 服務 binary（前後端分離計畫 P2–P5；遠端化 R1）。
//!
//! 啟動序（三條紀律）：bind-first → healthz（warming→ready）→ 背景 init。
//! 關機觸發：一般模式＝Ctrl+C；服務模式＝SCM Stop 設 `SERVICE_STOP`（等價）。
//! 優雅關機：停 accept → 等 `busy_ids()` 清空（上限 120s）→ 退出。
//!
//! 模式：一般（前景）／`--service`（Windows SCM dispatcher；Linux/macOS 前景同一般）。
//! 子命令：`configure`（首次執行部署精靈）／`install`／`uninstall`／`status`（P5）。
//! token：`OSERVER_TOKEN` env **或** `operoid.toml` `[server].token` **或**
//! `app-settings.json` 的 `server_token`（服務模式無使用者 env——由設定檔提供）。
//! **個人／企業模式**（遠端化 DR-E6）：settings 目錄有 `operoid.toml`＝企業模式
//! （可 bind 外部位址＋rustls TLS＋靜態三前端）；沒有＝個人模式（loopback、行為零變化）。
//! 非 loopback bind 且未啟用 TLS → 拒絕啟動（DR-E5 fail-closed）。

mod accounts;
mod auth;
mod configure;
mod knowledge_admin;
mod config;
mod gbrain;
mod operoid_toml;
mod operations;
mod rbac;
#[cfg(test)]
mod matrix_tests;
mod routes;
mod service;
mod sse;
mod writes;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use auth::PrincipalTokenProvider;
use ocore::agent_state::AppState;
use ocore::runtime::agent_db_path_in;
use ocore::scheduler;

use crate::routes::ServerState;

/// scheduler 的 AppState 全域暫存（優雅關機查 busy 用；全 Arc 欄位，Clone 同一份）。
static SHARED_STATE: OnceLock<AppState> = OnceLock::new();
/// 服務模式的停止旗標（SCM Stop 設 true——與 Ctrl+C 等價的關機觸發）。
pub(crate) static SERVICE_STOP: AtomicBool = AtomicBool::new(false);

/// 解析 CLI 中與目錄/bind 相關的引數（供 run() 與子命令共用）。
struct DirArgs {
    settings_dir: Option<String>,
    db_dir: Option<String>,
    data_dir: Option<String>,
    host: Option<String>,
    frontends_dir: Option<String>,
    port: u16,
    /// port 是否被明示（CLI --port 或 OSERVER_PORT env）——決定 toml port 是否生效。
    port_explicit: bool,
}

fn parse_args() -> DirArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut a = DirArgs {
        settings_dir: None,
        db_dir: None,
        data_dir: None,
        host: None,
        frontends_dir: None,
        port: 7340,
        port_explicit: false,
    };
    if let Ok(p) = std::env::var("OSERVER_PORT") {
        if let Ok(v) = p.parse() {
            a.port = v;
            a.port_explicit = true;
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
            "--host" if i + 1 < args.len() => {
                a.host = Some(args[i + 1].clone());
                i += 2;
            }
            "--frontends-dir" if i + 1 < args.len() => {
                a.frontends_dir = Some(args[i + 1].clone());
                i += 2;
            }
            "--port" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse() {
                    a.port = v;
                }
                a.port_explicit = true;
                i += 2;
            }
            _ => i += 1,
        }
    }
    a
}

/// 遠端化 R1：bind host/port 解析（CLI --host/--port > operoid.toml [server] > 預設
/// 127.0.0.1:7340）。port_explicit 時 toml port 不生效（明示引數最優先）。
fn resolve_bind(a: &DirArgs, t: Option<&operoid_toml::OperoidToml>) -> (String, u16) {
    let host = a
        .host
        .clone()
        .or_else(|| t.and_then(|t| t.server.host.clone()))
        .unwrap_or_else(|| "127.0.0.1".into());
    let port = if a.port_explicit {
        a.port
    } else {
        t.and_then(|t| t.server.port).unwrap_or(a.port)
    };
    (host, port)
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
        Some("configure") => {
            let dirs = resolve_from_args(&a).expect("解析資料目錄失敗");
            if let Err(e) = configure::run_wizard(&dirs) {
                eprintln!("[oserver] configure 失敗：{e}");
                std::process::exit(1);
            }
            return;
        }
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
            // 遠端化 R1：探測位址隨企業模式（operoid.toml [server]）走；解析失敗回落預設。
            let toml_cfg = resolve_from_args(&a)
                .ok()
                .and_then(|d| operoid_toml::load(&d.settings_dir).ok().flatten());
            let (host, port) = resolve_bind(&a, toml_cfg.as_ref());
            let running = std::net::TcpStream::connect_timeout(
                &format!("{host}:{port}").parse().expect("addr"),
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
    // 首次執行（無任何設定檔）：互動終端 → 部署設定精靈；headless（服務／GUI 產生
    // 的行程 stdin=null）不自動彈出——印指引後退出（fail-closed，比缺 token 訊息更具體）。
    if configure::needs_setup(&dirs.settings_dir) {
        use std::io::IsTerminal as _;
        if std::io::stdin().is_terminal() {
            configure::run_wizard(&dirs)?;
        } else {
            let dir_hint = a
                .data_dir
                .as_ref()
                .map(|d| format!(" --data-dir {d}"))
                .unwrap_or_default();
            anyhow::bail!(
                "首次執行（{} 無設定檔）——請先在終端機執行 `oserver configure{dir_hint}` 完成部署設定；服務／GUI 等無終端環境不自動彈出精靈",
                dirs.settings_dir.display()
            );
        }
    }
    // 遠端化 R1（DR-E6）：operoid.toml 存在＝企業模式；解析失敗 → 明確報錯退出（非靜默）。
    let toml_cfg = operoid_toml::load(&dirs.settings_dir)?;
    if toml_cfg.is_some() {
        eprintln!("[oserver] 企業模式：operoid.toml 已載入");
    }
    let cfg = config::load_effective(&dirs.settings_dir, toml_cfg.as_ref())?;
    // 服務行程（LocalSystem）看不到使用者環境——注入殼層快照與 operoid.toml [llm] 的
    // provider keys（llm::complete 與 gbrain 子行程自此都有 key；toml 蓋同名鍵）。
    for (k, v) in &cfg.llm_env {
        std::env::set_var(k, v);
    }
    if !cfg.llm_env.is_empty() {
        eprintln!("[oserver] 已注入 {} 個 LLM 環境變數（設定快照）", cfg.llm_env.len());
    }

    // ── 遠端化 R1：bind/TLS/frontends 解析（CLI > operoid.toml > 預設 127.0.0.1）──
    let (host, port) = resolve_bind(a, toml_cfg.as_ref());
    let tls = match &toml_cfg {
        Some(t) => t.tls.validate()?,
        None => None,
    };
    // DR-E5 fail-closed：非 loopback bind 必須有 TLS（Bearer 不得明文過網路）。
    operoid_toml::ensure_transport_safe(&host, tls.is_some())?;
    let frontends_dir = a
        .frontends_dir
        .clone()
        .or_else(|| toml_cfg.as_ref().and_then(|t| t.server.frontends_dir.clone()));

    // token：env 優先，其次 operoid.toml [server].token，否則設定檔 server_token（服務模式路徑）。
    let token = std::env::var("OSERVER_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            toml_cfg
                .as_ref()
                .and_then(|t| t.server.token.clone())
                .filter(|t| !t.trim().is_empty())
        })
        .or_else(|| cfg.server_token.clone().filter(|t| !t.trim().is_empty()));
    let Some(token) = token else {
        anyhow::bail!(
            "無 token——設 OSERVER_TOKEN env、operoid.toml [server].token，或 app-settings.json 需有 server_token（GUI 首次啟動會生成）"
        );
    };
    let db_path = agent_db_path_in(&dirs.db_dir);

    // ── bind-first（單例守衛）：bind 先行（失敗明確退出）→ serve 隨後。
    // HTTPS：std listener 先 bind（守住單例語意）→ axum-server from_tcp_rustls；HTTP 走既有 axum::serve。
    enum Bound {
        Http(tokio::net::TcpListener),
        Https(axum_server::Server<axum_server::tls_rustls::RustlsAcceptor>),
    }
    let addr = format!("{host}:{port}");
    let bound = if let Some((cert, key)) = &tls {
        let rc = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key)
            .await
            .map_err(|e| anyhow::anyhow!("TLS 憑證載入失敗（cert={cert}, key={key}）：{e}"))?;
        let std_listener = std::net::TcpListener::bind(&addr)
            .map_err(|e| anyhow::anyhow!("bind {addr} 失敗（已有 oserver 實例？）：{e}"))?;
        Bound::Https(axum_server::tls_rustls::from_tcp_rustls(std_listener, rc))
    } else {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| anyhow::anyhow!("bind {addr} 失敗（已有 oserver 實例？）：{e}"))?;
        Bound::Http(listener)
    };
    let scheme = if tls.is_some() { "https" } else { "http" };
    eprintln!("[oserver] 監聽 {scheme}://{addr}（healthz: /healthz）");

    // AppState（寫入面喚醒＋scheduler 共用；CfgLoader 每次呼叫重讀設定檔——熱生效）。
    let (wake_tx, wake_rx) = tokio::sync::mpsc::channel(64);
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(128);
    let app_state = AppState::new(wake_tx, event_tx, cfg.llm_concurrency);
    let _ = SHARED_STATE.set(app_state.clone());

    let ready = Arc::new(AtomicBool::new(false));
    let state = Arc::new(ServerState {
        // C12a：token-per-principal authn（master token→operator；principal token→該身份）。
        auth: Arc::new(PrincipalTokenProvider::new(token, db_path.clone())),
        cfg: cfg.clone(),
        db_path: db_path.clone(),
        ready: Arc::clone(&ready),
        agent_state: Some(app_state.clone()),
        ops: Arc::new(operations::OpRegistry::new()),
        settings_dir: dirs.settings_dir.clone(),
    });

    // 驗 DB（spawn_blocking——rusqlite 同步 API）。
    let db_check = db_path.clone();
    match tokio::task::spawn_blocking(move || {
        let store = ocore::domain::SqliteStore::open(&db_check)?;
        // 崩潰復原（P5）：上次行程中途被殺的孤兒（Working 員工/InProgress task）
        // 救回可掃描狀態——必須在 scheduler 起來之前。
        // C4（D-C4）：冪等建立 operator bootstrap principal（企業身份的最小落點）。
        ocore::knowledge::identity::ensure_operator_principal(&store)?;
        ocore::runtime::recover_stale_runs(&store)
    })
    .await
    {
        Ok(Ok(n)) => eprintln!("[oserver] DB 就緒：{}（啟動復原 {} 名員工）", db_path.display(), n),
        Ok(Err(e)) => anyhow::bail!("DB 開啟失敗：{e}"),
        Err(e) => anyhow::bail!("DB 檢查任務失敗：{e}"),
    }
    // C5（D1/D3）：知識授權 bootstrap（冪等；Q3——既有 sources 全數映射 co-common）。
    // 失敗不擋啟動：gbrain 不可用時檢索本就無法進行；policy 缺→fail closed。
    match ocore::domain::SqliteStore::open(&db_path) {
        Ok(boot_store) => {
            match ocore::knowledge::bootstrap::ensure_knowledge_bootstrap(&boot_store, &cfg).await {
                Ok(ocore::knowledge::bootstrap::BootstrapOutcome::Bootstrapped { sources }) => {
                    eprintln!("[oserver] knowledge bootstrap 完成：co-common ← {sources} 個 source");
                }
                Ok(_) => {}
                Err(e) => eprintln!("[oserver] knowledge bootstrap 失敗（續行；檢索將 fail closed）：{e}"),
            }
        }
        Err(e) => eprintln!("[oserver] knowledge bootstrap 開庫失敗：{e}"),
    }

    if !cfg.agent_os_enabled {
        eprintln!("[oserver] 注意：agent_os_enabled=false——API 將回 503");
    }

    let dir_for_loader = dirs.settings_dir.clone();
    // CfgLoader（scheduler 熱重載）：app-settings.json＋operoid.toml 同一批重讀——
    // [llm] env／[gbrain]／[ingress] 熱生效（host/port/tls/frontends 屬啟動期，改檔須重啟）。
    // operoid.toml 此刻已通過啟動期解析；運行中被人改壞 → Err（scheduler 跳過該輪，不 crash）。
    let load_cfg: scheduler::CfgLoader = Arc::new(move || {
        let t = operoid_toml::load(&dir_for_loader)?;
        config::load_effective(&dir_for_loader, t.as_ref())
    });
    scheduler::spawn_loop(app_state, load_cfg, db_path.clone(), wake_rx, event_rx);
    eprintln!("[oserver] scheduler 已啟動（30s tick＋事件/訊息喚醒）");
    ready.store(true, Ordering::SeqCst);
    eprintln!("[oserver] 就緒（healthz → ready）");

    let app = routes::router(Arc::clone(&state))
        .merge(writes::write_routes().with_state(Arc::clone(&state)))
        .merge(knowledge_admin::knowledge_admin_routes().with_state(Arc::clone(&state)))
        .merge(accounts::account_routes().with_state(Arc::clone(&state)))
        .merge(sse::sse_routes().with_state(Arc::clone(&state)))
        .merge(gbrain::gbrain_routes().with_state(Arc::clone(&state)));
    let app = mount_frontends(app, frontends_dir.as_deref());
    // R2（DR-E4）：RBAC 中介層掛最外層——authn → must_change_password 閘 → 矩陣裁定（403）。
    // OPTIONS preflight 與公開路徑（healthz／登入／靜態頁）直接放行。
    let app = app.layer(axum::middleware::from_fn_with_state(
        Arc::clone(&state),
        rbac::rbac_middleware,
    ));

    match bound {
        Bound::Http(listener) => {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_wait())
                .await?;
        }
        Bound::Https(server) => {
            // axum-server 0.7 的關機是 Handle 式：等 shutdown_wait()（Ctrl+C／SERVICE_STOP
            // → busy 清空）後觸發 graceful——停 accept、等在途連線結束。
            let handle = axum_server::Handle::new();
            tokio::spawn({
                let h = handle.clone();
                async move {
                    shutdown_wait().await;
                    h.graceful_shutdown(None);
                }
            });
            server.handle(handle).serve(app.into_make_service()).await?;
        }
    }
    eprintln!("[oserver] 已退出");
    Ok(())
}

/// 遠端化 R1（DR-E2）：企業模式靜態檔服務——frontends_dir 內 admin/manager/user
/// 子目錄各掛 `/{app}`（SPA fallback index.html）；`/` 為入口頁。個人模式不啟用。
fn mount_frontends(app: axum::Router, dir: Option<&str>) -> axum::Router {
    use tower_http::services::{ServeDir, ServeFile};
    let Some(dir) = dir else { return app };
    let root = std::path::PathBuf::from(dir);
    let mut app = app;
    for name in ["admin", "manager", "user"] {
        let sub = root.join(name);
        if sub.is_dir() {
            let index = sub.join("index.html");
            app = app.nest_service(
                &format!("/{name}"),
                ServeDir::new(&sub).fallback(ServeFile::new(&index)),
            );
            eprintln!("[oserver] 前端「{name}」→ {}（/{name}）", sub.display());
        } else {
            eprintln!("[oserver] 警告：前端目錄不存在——{}", sub.display());
        }
    }
    app.route("/", axum::routing::get(frontends_landing))
}

/// 三前端入口頁（/）：只做導航，不做認證判斷（各 app 自帶登入頁）。
async fn frontends_landing() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!doctype html>
<html lang="zh-Hant">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Operoid</title>
<style>body{font-family:system-ui,sans-serif;display:flex;min-height:100vh;align-items:center;justify-content:center;margin:0;background:#f6f7f9}
main{display:flex;gap:1.5rem}a{display:block;width:11rem;padding:2rem 1.5rem;background:#fff;border:1px solid #e2e5ea;border-radius:.75rem;
text-decoration:none;color:#1a1d23;font-size:1.05rem;font-weight:600;text-align:center;box-shadow:0 1px 3px rgba(0,0,0,.06)}
a:hover{border-color:#7c8cf8;color:#4f5ef0}small{display:block;margin-top:.5rem;font-weight:400;color:#8a8f99;font-size:.8rem}</style></head>
<body><main>
<a href="/admin">系統管理介面<small>admin</small></a>
<a href="/manager">經理人儀表板<small>manager</small></a>
<a href="/user">一般使用者介面<small>user</small></a>
</main></body></html>"#,
    )
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
