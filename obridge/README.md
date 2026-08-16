# Obridge — Operoid Bridge

Operoid 與外部通道（Email 現已內建；未來 IM 走 WASM 外掛）之間的雙向橋接器。
架構設計見 `../docs/Operoid-設計-統一事件ingress契約.md` §十三。

## 建置與執行

```
cargo build -p obridge
cp obridge/config.example.toml obridge/obridge.toml   # 填入帳號與密鑰
cargo run -p obridge            # 或直接跑執行檔（自動找同目錄 obridge.toml）
```

## 全鏈 e2e 手動驗證步驟（Email 通道）

1. **Operoid 端設定**（`app-settings.json` 的 `app_config`）：
   - `"event_ingress_port": 17341`、`"event_ingress_secret": "<密A>"`
   - `"event_outbound_url": "http://127.0.0.1:17401/send"`、`"event_outbound_secret": "<密B>"`
2. **obridge 端設定**（`obridge.toml`）：`ingress_url` 指向 `http://127.0.0.1:17341/event`、
   `ingress_secret=<密A>`；`[listen] port=17401`、`secret=<密B>`；IMAP/SMTP 帳號、
   routes（收件地址→employee_id）、senders（employee_id→From 地址）。
3. `npm run tauri dev`（Operoid）＋ `cargo run -p obridge`。
4. 從外部信箱寄信到 route 命中的地址 → obridge log 應見投遞；Operoid 端對應員工被喚醒、
   聊天頁出現 In 訊息（整封信序列化）。
5. 與員工對話（員工的 `send` 動作缺省回錨點）→ 外部信箱應收到**回原 thread** 的回信
   （In-Reply-To 還原），寄件者為 senders 對應的 From 身分。
6. （外發未設 `event_outbound_url` 時）員工的 send 結果應顯示「外發未啟用」。

## 真實信箱 smoke 測試（#[ignore]）

```
OBRIDGE_IMAP_HOST=... OBRIDGE_IMAP_USER=... OBRIDGE_IMAP_PASS=... \
  cargo test -p obridge real_imap -- --ignored --nocapture
OBRIDGE_SMTP_HOST=... OBRIDGE_SMTP_USER=... OBRIDGE_SMTP_PASS=... OBRIDGE_SMTP_TO=... \
  cargo test -p obridge real_smtp -- --ignored --nocapture
```

## WASM 外掛開發

- 介面：`wit/channel.wit`（匯出 source/poll/send；host 提供 make-request/kv/clock——
  刻意不含 TCP/TLS）。
- 範例：`../obridge-plugin-example`（echo）。
  建置：`cargo build -p obridge-plugin-example --target wasm32-wasip2`
  （需 `rustup target add wasm32-wasip2`）。
- 佈署：複製 `.wasm` 到 obridge 設定檔同目錄的 `plugins/`，檔名慣例 `<name>-<poll_secs>.wasm`。
- 往返測試：先建置範例外掛後
  `cargo test -p obridge example_plugin -- --ignored --nocapture`。
