//! Obridge 範例通道外掛（echo）——WASM 外掛體系的最小可行示範。
//!
//! 行為：`source()` = "echo"；`init(config)` 讀設定（示範——如 `greet-name`）；
//! `poll()` 首輪（以 host kv 記錄）產一則事件、之後空；`send()` 一律成功（不實際外發）。
//! 展示 host functions 用法（kv / clock-now）與設定傳遞。
//!
//! 建置：`cargo build -p obridge-plugin-example --target wasm32-wasip2`
//! 佈署：複製產物到 obridge 的 `plugins/` 目錄（檔名慣例 `<name>-<poll_secs>.wasm`）。

wit_bindgen::generate!({
    path: "../obridge/wit",
    world: "channel-plugin",
});

// 設定（init 收到）經 host kv 存取——外掛例為短命實例，跨呼叫狀態靠 kv。
struct EchoComponent;

export!(EchoComponent);

impl exports::operoid::obridge::channel::Guest for EchoComponent {
    fn source() -> String {
        "echo".into()
    }

    fn init(config: String) {
        // 示範設定讀取：{"greet-name": "..."}（無此鍵 → 預設）。
        // 生命週期慣例：init 於每次實例化呼叫（冪等）——把設定寫進 kv 供後續呼叫讀取。
        let name = serde_json::from_str::<serde_json::Value>(&config)
            .ok()
            .and_then(|v| v.get("greet-name").and_then(|n| n.as_str()).map(str::to_string))
            .unwrap_or_else(|| "Obridge".into());
        operoid::obridge::host::kv_set("greet-name", &name);
    }

    fn poll() -> Result<Vec<exports::operoid::obridge::channel::InboundEvent>, String> {
        // host kv 記「是否已打過招呼」——示範外掛持久化（重啟不重發）。
        if operoid::obridge::host::kv_get("greeted").is_some() {
            return Ok(Vec::new());
        }
        operoid::obridge::host::kv_set("greeted", "1");
        let now = operoid::obridge::host::clock_now();
        let name = operoid::obridge::host::kv_get("greet-name").unwrap_or_else(|| "Obridge".into());
        Ok(vec![exports::operoid::obridge::channel::InboundEvent {
            source: "echo".into(),
            brain_id: None,
            employee_id: None,
            title: "echo 外掛上線".into(),
            content: format!("這是 {name} 的範例 WASM 通道外掛事件（host clock-now={now}）。"),
            external_ref: Some(format!("echo-boot-{now}")),
            occurred_at: None,
            reply_to: Some("echo:msg:boot".into()),
        }])
    }

    fn send(_to: String, _employee_id: String, _text: String) -> Result<(), String> {
        Ok(()) // echo：不實際外發，僅示範介面
    }
}
