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
    runtime.block_on(run(cfg, state_dir));
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

async fn run(cfg: config::Config, state_dir: std::path::PathBuf) {
    let sources: Vec<String> = cfg.channels.iter().map(|c| c.source.clone()).collect();
    eprintln!(
        "[obridge] 啟動：{} 個通道（{sources:?}）→ {}",
        cfg.channels.len(),
        cfg.operoid.ingress_url,
    );

    // 1. 建通道註冊表（send 分派依據）。
    let mut registry = Registry::new();
    for ch_cfg in &cfg.channels {
        match ch_cfg.channel_type.as_str() {
            "email-imap" => {
                let ch: Arc<dyn Channel> = Arc::new(
                    channels::email_imap::EmailImapChannel::new(ch_cfg, &state_dir)
                        .expect("email-imap 通道建構"),
                );
                registry.register(ch).expect("source 標籤唯一（啟動已檢查）");
            }
            other => eprintln!("[obridge] 未知通道類型 {other}（source={}）——跳過", ch_cfg.source),
        }
    }

    // 1b. WASM 外掛（plugins/ 目錄——設定檔同目錄下；無＝正常）。
    let plugins_dir = state_dir.join("plugins");
    if let Err(e) = core::plugins::load_all(&plugins_dir, &mut registry).await {
        eprintln!("[obridge] 外掛掃描失敗（略過）：{e}");
    }

    // 2. 進氣：mpsc 匯流 → 轉 POST ingress。
    let (tx, mut rx) = mpsc::channel::<InboundEvent>(64);
    for source in registry.sources() {
        if let Some(ch) = registry.get(&source) {
            let tx = tx.clone();
            tokio::spawn(async move {
                ch.run_inbound(tx).await;
            });
        }
    }
    drop(tx);
    let operoid = cfg.operoid.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            if let Err(e) = core::ingress::post_event(&operoid, &ev).await {
                eprintln!("[obridge] 事件投遞失敗（{e}）：〈{}〉——下輪 poll 重投", ev.title);
            }
        }
    });

    // 3. 出氣：send endpoint（常駐）。
    core::send_server::serve(cfg.listen.port, cfg.listen.secret, Arc::new(registry)).await;
}
