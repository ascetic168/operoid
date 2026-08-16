//! Obridge 範例通道外掛（echo）——WASM 外掛體系的最小可行示範。
//!
//! 行為：`source()` = "echo"；`poll()` 首輪（以 host kv 記錄）產一則示範事件、之後空；
//! `send()` 一律成功（不實際外發）。展示 host functions 用法（kv / clock-now）。
//!
//! 建置：`cargo build -p obridge-plugin-example --target wasm32-wasip2`
//! 佈署：複製產物到 obridge 的 `plugins/` 目錄（檔名慣例 `<name>-<poll_secs>.wasm`）。

wit_bindgen::generate!({
    path: "../obridge/wit",
    world: "channel-plugin",
});

struct EchoComponent;

export!(EchoComponent);

impl exports::operoid::obridge::channel::Guest for EchoComponent {
    fn source() -> String {
        "echo".into()
    }

    fn poll() -> Result<Vec<exports::operoid::obridge::channel::InboundEvent>, String> {
        // host kv 記「是否已打過招呼」——示範外掛持久化（重啟不重發）。
        if operoid::obridge::host::kv_get("greeted").is_some() {
            return Ok(Vec::new());
        }
        operoid::obridge::host::kv_set("greeted", "1");
        let now = operoid::obridge::host::clock_now();
        Ok(vec![exports::operoid::obridge::channel::InboundEvent {
            source: "echo".into(),
            brain_id: None,
            employee_id: None,
            title: "echo 外掛上線".into(),
            content: format!("這是 Obridge 範例 WASM 通道外掛的事件（host clock-now={now}）。"),
            external_ref: Some(format!("echo-boot-{now}")),
            occurred_at: None,
            reply_to: Some("echo:msg:boot".into()),
        }])
    }

    fn send(_to: String, _employee_id: String, _text: String) -> Result<(), String> {
        Ok(()) // echo：不實際外發，僅示範介面
    }
}
