//! WASM 外掛載入（wasmtime + component model；WIT 見 `obridge/wit/channel.wit`）。
//!
//! 沙箱邊界：外掛只經 host functions 觸碰外界——`http-request`／`kv`（JSON 檔持久化）／
//! `clock-now`。**刻意不提供 TCP/TLS**（原始協定通道屬內建 native 模組，見 Hybrid 決策）。
//! 掃描 `plugins/*.wasm`：載入失敗僅 log、不炸 host。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ocontract::{EventKind, InboundEvent};
use tokio::sync::mpsc;

use super::channel::Channel;

// 由 WIT 生成 host 端綁定（型別 + Host trait + add_to_linker）。host 函數與外掛呼叫
// 皆 async（host 的 http_request 走 reqwest；外掛呼叫在 wasmtime async store 上）。
wasmtime::component::bindgen!({
    path: "wit",
    world: "channel-plugin",
    imports: { default: async },
    exports: { default: async },
});

// ───────────────── host functions 實作 ─────────────────

/// 外掛的 host 狀態（wasmtime Store data）：HTTP client＋KV（JSON 檔持久化）。
/// Host trait 直接實作在此（`add_to_linker` 以 `HasSelf` 投影）。
pub struct PluginHost {
    kv_path: Option<std::path::PathBuf>,
    kv: Mutex<HashMap<String, String>>,
    // WASI p2（wasm32-wasip2 的 std 需要；能力最小化——builder 預設不開檔案/網路/stdio）
    table: wasmtime::component::ResourceTable,
    wasi: wasmtime_wasi::WasiCtx,
}

impl wasmtime_wasi::WasiView for PluginHost {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl PluginHost {
    fn new(kv_path: Option<std::path::PathBuf>) -> Self {
        let kv = kv_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            kv_path,
            kv: Mutex::new(kv),
            table: wasmtime::component::ResourceTable::new(),
            wasi: wasmtime_wasi::WasiCtxBuilder::new().build(),
        }
    }

    fn persist(&self) {
        if let (Some(path), Ok(kv)) = (&self.kv_path, self.kv.lock()) {
            if let Ok(json) = serde_json::to_vec_pretty(&*kv) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

impl operoid::obridge::host::Host for PluginHost {
    async fn make_request(
        &mut self,
        req: operoid::obridge::host::HttpRequest,
    ) -> Result<operoid::obridge::host::HttpResponse, String> {
        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|e| format!("非法 HTTP method：{e}"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        let mut r = client.request(method, &req.url);
        for (k, v) in &req.headers {
            r = r.header(k, v);
        }
        if let Some(body) = &req.body {
            r = r.body(body.clone());
        }
        let resp = r.send().await.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16() as u16;
        let body = resp.text().await.map_err(|e| e.to_string())?;
        Ok(operoid::obridge::host::HttpResponse { status, body })
    }

    async fn kv_get(&mut self, key: String) -> Option<String> {
        self.kv.lock().unwrap().get(&key).cloned()
    }

    async fn kv_set(&mut self, key: String, value: String) {
        self.kv.lock().unwrap().insert(key, value);
        self.persist();
    }

    async fn clock_now(&mut self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

// ───────────────── 外掛 Channel adapter ─────────────────

/// 一個已載入的 WASM 通道外掛（實作 native 側 [`Channel`]）。
pub struct WasmChannel {
    engine: wasmtime::Engine,
    component: wasmtime::component::Component,
    /// KV 持久化檔（每外掛一個：<plugin-name>-kv.json）。
    kv_path: std::path::PathBuf,
    /// poll 間隔（秒，由外掛目錄慣例：檔名 `<source>-<poll_secs>.wasm`？否則預設 60）。
    poll_secs: u64,
    source: String,
}

impl WasmChannel {
    /// 載入單一 .wasm。
    pub async fn load(wasm_path: &Path, poll_secs: u64, kv_dir: &Path) -> anyhow::Result<Self> {
        let engine = wasmtime::Engine::default();
        let component = wasmtime::component::Component::from_file(&engine, wasm_path)?;
        let name = wasm_path.file_stem().unwrap_or_default().to_string_lossy();
        let kv_path = kv_dir.join(format!("{name}-kv.json"));
        // 讀 source 標籤（呼叫一次 source()）。
        let (source, component) = {
            let mut store = wasmtime::Store::new(&engine, PluginHost::new(Some(kv_path.clone())));
            let mut linker = wasmtime::component::Linker::new(&engine);
            wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        operoid::obridge::host::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |h| h)?;
            let bindings = ChannelPlugin::instantiate_async(&mut store, &component, &linker)
                .await?;
            let s = bindings
                .operoid_obridge_channel()
                .call_source(&mut store)
                .await?;
            (s, component)
        };
        Ok(Self { engine, component, kv_path, poll_secs, source })
    }
}

#[async_trait::async_trait]
impl Channel for WasmChannel {
    fn source(&self) -> &str {
        &self.source
    }

    async fn run_inbound(&self, tx: mpsc::Sender<InboundEvent>) {
        loop {
            if let Err(e) = self.poll_once(&tx).await {
                eprintln!("[obridge:{}] 外掛 poll 失敗（下輪重試）：{e}", self.source);
            }
            tokio::time::sleep(std::time::Duration::from_secs(self.poll_secs.max(1))).await;
        }
    }

    async fn send(&self, to: &str, employee_id: &str, text: &str) -> anyhow::Result<()> {
        let mut store = wasmtime::Store::new(&self.engine, PluginHost::new(Some(self.kv_path.clone())));
        let mut linker = wasmtime::component::Linker::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        operoid::obridge::host::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |h| h)?;
        let bindings =
            ChannelPlugin::instantiate_async(&mut store, &self.component, &linker).await?;
        bindings
            .operoid_obridge_channel()
            .call_send(&mut store, to, employee_id, text)
            .await?
            .map_err(|e| anyhow::anyhow!("外掛 send 失敗：{e}"))
    }
}

impl WasmChannel {
    /// 一輪 poll（獨立方法供測試）。
    pub async fn poll_once(&self, tx: &mpsc::Sender<InboundEvent>) -> anyhow::Result<usize> {
        let mut store = wasmtime::Store::new(&self.engine, PluginHost::new(Some(self.kv_path.clone())));
        let mut linker = wasmtime::component::Linker::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        operoid::obridge::host::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |h| h)?;
        let bindings =
            ChannelPlugin::instantiate_async(&mut store, &self.component, &linker).await?;
        let events = bindings
            .operoid_obridge_channel()
            .call_poll(&mut store)
            .await?
            .map_err(|e| anyhow::anyhow!("外掛 poll 失敗：{e}"))?;
        let mut n = 0;
        for ev in events {
            let _ = tx
                .send(InboundEvent {
                    kind: EventKind::ExternalMessage,
                    source: ev.source,
                    brain_id: ev.brain_id,
                    employee_id: ev.employee_id,
                    title: ev.title,
                    content: ev.content,
                    external_ref: ev.external_ref,
                    occurred_at: ev.occurred_at,
                    reply_to: ev.reply_to,
                    category: None,
                })
                .await;
            n += 1;
        }
        Ok(n)
    }
}

/// 掃描 `plugins_dir` 下的 `*.wasm`，全部載入。單一失敗僅 log。
pub async fn load_all(
    plugins_dir: &Path,
    registry: &mut super::channel::Registry,
) -> anyhow::Result<()> {
    if !plugins_dir.exists() {
        return Ok(()); // 無外掛目錄＝正常
    }
    for entry in std::fs::read_dir(plugins_dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
            continue;
        }
        // 慣例：`<name>-<poll_secs>.wasm`（如 slack-30.wasm）；無後綴 → 60s。
        let poll_secs = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit_once('-'))
            .and_then(|(_, secs)| secs.parse().ok())
            .filter(|&s| s > 0 && s <= 3600)
            .unwrap_or(60);
        match WasmChannel::load(&path, poll_secs, plugins_dir).await {
            Ok(ch) => {
                eprintln!(
                    "[obridge] 外掛載入：{}（source={}，poll={poll_secs}s）",
                    path.display(),
                    ch.source()
                );
                registry.register(Arc::new(ch))?;
            }
            Err(e) => eprintln!("[obridge] 外掛載入失敗（{}，略過）：{e}", path.display()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 載入範例外掛並往返 source/poll/send（需先建置範例外掛：
    /// `cargo build -p obridge-plugin-example --target wasm32-wasip2`，
    /// 產物在 target/wasm32-wasip2/debug/）。
    #[tokio::test]
    #[ignore = "需先建置範例外掛 wasm"]
    async fn example_plugin_roundtrip() {
        let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/wasm32-wasip2/debug/obridge_plugin_example.wasm");
        let kv_dir = std::env::temp_dir();
        let ch = WasmChannel::load(&wasm, 60, &kv_dir).await.expect("載入範例外掛");
        assert_eq!(ch.source(), "echo");
        let (tx, mut rx) = mpsc::channel(10);
        let n = ch.poll_once(&tx).await.unwrap();
        assert_eq!(n, 1, "echo 外掛首輪 poll 應產一則事件");
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.source, "echo");
        assert!(!ev.title.is_empty());
        // send 往返（echo 外掛回 Ok）。
        ch.send("echo:msg:x", "Steve-TW", "測試").await.unwrap();
    }
}
