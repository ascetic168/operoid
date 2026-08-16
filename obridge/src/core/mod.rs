//! Obridge core：設定、通道介面/註冊表、ingress client、send endpoint。
//! （WASM 外掛載入 `plugins` 於後續步驟加入。）

pub mod channel;
pub mod config;
pub mod ingress;
pub mod plugins;
pub mod send_server;
