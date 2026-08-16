//! Obridge — Operoid Bridge。
//!
//! 外部通道（Email 現已內建；未來 IM 走 WASM 外掛）與 Operoid 事件匯流排之間的雙向橋：
//! - **進氣**：各通道 `run_inbound` 產出 `InboundEvent` → `POST <operoid>/event`。
//! - **出氣**：`POST /send`（Operoid 的 `event_outbound_url` 指向此）→ 依 `source` 分派通道。
//!
//! 設定：`obridge.toml`（`--config <path>` 指定；預設依序找 cwd／執行檔同目錄）。

mod channels;
mod core;

use std::sync::Arc;

use ocontract::InboundEvent;
use tokio::sync::mpsc;

use core::channel::{Channel, Registry};
use core::config;

fn main() {
    let cfg_path = find_config_path();
    let toml_str = match std::fs::read_to_string(&cfg_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("讀設定檔失敗（{}）：{e}", cfg_path.display());
            std::process::exit(1);
        }
    };
    let cfg = match config::parse(&toml_str) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("設定檔解析失敗：{e}");
            std::process::exit(1);
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let state_dir = cfg_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    runtime.block_on(run(cfg_path, cfg, state_dir));
}

/// `--config <path>` ／ cwd ／ exe 目錄，依序找 `obridge.toml`。
fn find_config_path() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--config") {
        if let Some(p) = args.get(i + 1) {
            return p.into();
        }
    }
    let mut bases = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            bases.push(dir.to_path_buf());
        }
    }
    for dir in bases {
        let p = dir.join("obridge.toml");
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from("obridge.toml")
}

async fn run(cfg_path: std::path::PathBuf, cfg: config::Config, state_dir: std::path::PathBuf) {
    eprintln!(
        "[obridge] 啟動：{} 個通道（{:?}）→ {}（設定熱重載：watch {}）",
        cfg.channels.len(),
        cfg.channels.iter().map(|c| c.source.clone()).collect::<Vec<_>>(),
        cfg.operoid.ingress_url,
        cfg_path.display(),
    );

    // 進氣：單一全域 msc——熱重載重建通道時沿用（通道只管往裡丟事件）。
    let (tx, mut rx) = mpsc::channel::<InboundEvent>(64);
    let operoid = cfg.operoid.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            if let Err(e) = core::ingress::post_event(&operoid, &ev).await {
                eprintln!("[obridge] 事件投遞失敗（{e}）：〈{}〉——下輪 poll 重投", ev.title);
            }
        }
    });

    // 通道註冊表（send 分派依據）——熱重載時整組替換（send endpoint 經 RwLock 讀取）。
    let registry = Arc::new(std::sync::RwLock::new(Registry::new()));
    let mut tasks = tokio::task::JoinSet::new();
    build_channels(&cfg, &state_dir, &tx, &registry, &mut tasks).await;


    // 出氣：send endpoint（常駐；port/secret 變更需重啟——熱重載僅重建通道）。
    let listen_port = cfg.listen.port;
    let listen_secret = cfg.listen.secret;
    tokio::spawn(core::send_server::serve(
        listen_port,
        listen_secret,
        Arc::clone(&registry),
    ));

    // 熱重載：watch 設定檔 mtime（2s 輪詢——簡單、跨平台、無額外依賴）。變更 → 重建通道；
    // 解析失敗 → 記 log、沿用舊通道（下次變更再試）。
    let mut last_modified = file_mtime(&cfg_path);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let m = file_mtime(&cfg_path);
        if m == last_modified {
            continue;
        }
        last_modified = m;
        match std::fs::read_to_string(&cfg_path)
            .map_err(|e| e.to_string())
            .and_then(|s| config::parse(&s).map_err(|e| e.to_string()))
        {
            Ok(new_cfg) => {
                eprintln!(
                    "[obridge] 設定變更——熱重建通道（{:?}）",
                    new_cfg.channels.iter().map(|c| c.source.clone()).collect::<Vec<_>>()
                );
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                build_channels(&new_cfg, &state_dir, &tx, &registry, &mut tasks).await;
            }
            Err(e) => {
                eprintln!("[obridge] 設定變更但解析失敗（沿用現行通道）：{e}");
            }
        }
    }
}

fn file_mtime(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// 依設定建通道（email-imap／wasm＋plugins/ 掃描）→ 註冊 → spawn 進氣 task。
/// 熱重載時整組重跑（先 abort 舊 tasks，見 run()）。
async fn build_channels(
    cfg: &config::Config,
    state_dir: &std::path::Path,
    tx: &mpsc::Sender<InboundEvent>,
    registry: &std::sync::RwLock<Registry>,
    tasks: &mut tokio::task::JoinSet<()>,
) {
    let mut reg = Registry::new();
    for ch_cfg in &cfg.channels {
        match ch_cfg.channel_type.as_str() {
            "email-imap" => {
                match channels::email_imap::EmailImapChannel::new(ch_cfg, state_dir) {
                    Ok(ch) => {
                        let ch: Arc<dyn Channel> = Arc::new(ch);
                        reg.register(ch).expect("source 標籤唯一（啟動已檢查）");
                    }
                    Err(e) => eprintln!("[obridge] email-imap 通道 {} 建構失敗（跳過）：{e}", ch_cfg.source),
                }
            }
            "wasm" => {
                let Some(w) = ch_cfg.wasm.as_ref() else {
                    eprintln!("[obridge] wasm 通道 {} 缺設定（跳過）", ch_cfg.source);
                    continue;
                };
                // plugin 路徑：絕對路徑照用；相對路徑相對於設定檔目錄。
                let plugin_path = std::path::Path::new(&w.plugin);
                let plugin_path = if plugin_path.is_absolute() {
                    plugin_path.to_path_buf()
                } else {
                    state_dir.join(plugin_path)
                };
                let config_json: Option<String> =
                    match w.config.as_ref().map(serde_json::to_string) {
                        Some(Ok(s)) => Some(s),
                        Some(Err(e)) => {
                            eprintln!(
                                "[obridge] 通道 {} 設定序列化失敗（以空設定載入）：{e}",
                                ch_cfg.source
                            );
                            None
                        }
                        None => None,
                    };
                match core::plugins::WasmChannel::load(
                    &plugin_path,
                    w.poll_secs,
                    state_dir,
                    config_json,
                )
                .await
                {
                    Ok(ch) => {
                        let ch: Arc<dyn Channel> = Arc::new(ch);
                        reg.register(ch).expect("source 標籤唯一（啟動已檢查）");
                    }
                    Err(e) => eprintln!("[obridge] wasm 通道 {} 載入失敗（跳過）：{e}", ch_cfg.source),
                }
            }
            other => eprintln!("[obridge] 未知通道類型 {other}（source={}）——跳過", ch_cfg.source),
        }
    }
    // plugins/ 目錄掃描（設定檔同目錄下；無＝正常）。載入失敗僅 log。
    let plugins_dir = state_dir.join("plugins");
    if let Err(e) = core::plugins::load_all(&plugins_dir, &mut reg).await {
        eprintln!("[obridge] 外掛掃描失敗（略過）：{e}");
    }

    // spawn 進氣 tasks（複製註冊表中的通道 Arc）後整組替換註冊表。
    for source in reg.sources() {
        if let Some(ch) = reg.get(&source) {
            let tx = tx.clone();
            tasks.spawn(async move {
                ch.run_inbound(tx).await;
            });
        }
    }
    *registry.write().unwrap() = reg;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 熱重載核心：build_channels 以新設定整組替換註冊表（sources 反映新設定）。
    /// email-imap 建構不連線（poll 才連），離線可測。
    #[tokio::test]
    async fn hot_reload_replaces_registry() {
        let dir = std::env::temp_dir().join(format!(
            "obridge-hotreload-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, _rx) = mpsc::channel(8);
        let registry = std::sync::RwLock::new(Registry::new());
        let mut tasks = tokio::task::JoinSet::new();

        let mk_cfg = |source: &str| {
            let toml = format!(
                r#"[operoid]
                ingress_url = "http://127.0.0.1:1/event"
                ingress_secret = "s"
                [listen]
                port = 1
                secret = "s"
                [[channels]]
                type = "email-imap"
                source = "{source}"
                [channels.email_imap.imap]
                host = "h"
                username = "u"
                password = "p"
                [channels.email_imap.smtp]
                host = "h"
                username = "u"
                password = "p"
                "#
            );
            config::parse(&toml).unwrap()
        };

        build_channels(&mk_cfg("email"), &dir, &tx, &registry, &mut tasks).await;
        assert_eq!(registry.read().unwrap().sources(), vec!["email".to_string()]);
        // 模擬熱重載：abort 舊 tasks → 以新設定重建（source 換名）。
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        build_channels(&mk_cfg("email2"), &dir, &tx, &registry, &mut tasks).await;
        assert_eq!(registry.read().unwrap().sources(), vec!["email2".to_string()]);
        tasks.abort_all();
        std::fs::remove_dir_all(&dir).ok();
    }
}
