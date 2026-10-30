# Appendix B: Developing Operoid

> Applies to: v0.5.11. For enterprise teams who want to **develop their own integrations on Operoid** or **fork and modify** it — assuming you develop with Vibe Coding (Claude Code and other coding agents). The core message: **have the agent read the contracts and the examples before it writes any code**; this appendix gives you a map of where the information lives, plus demonstration prompts you can copy as-is.

## Choose the Route First: Plugin Extension vs Fork Modification

| | Route 1: Obridge plugin | Route 2: fork modification |
|---|---|---|
| Best for | Connecting enterprise IM / internal systems — adding a channel, one ingress/egress interface | Changing core behavior — new APIs, new tools, new governance rules |
| Cost | Low — write a WASM component inside the sandbox, **without touching core code** | High — you maintain the fork and shoulder upstream sync |
| Upgrades | Operoid upgrades as usual; the plugin keeps working unchanged | Every upgrade requires merging upstream |

**Principle: prefer a plugin over a fork wherever possible; if you must fork, keep the changes concentrated and leave the contract layer to upstream.**

## Information Map: What to Read Where

What a coding agent fears most is not being unable to write code, but **reading the wrong places**. Below are the authoritative sources for each kind of task (paths relative to the repository root):

| What you want to do | Read this | Notes |
|---|---|---|
| Channel plugin interface | `obridge/wit/channel.wit` | **The contract**: `channel` (source / init / poll / send) and `host` (make-request / kv / clock-now); TCP/TLS is deliberately not provided |
| How to write a plugin | `obridge-plugin-example/` (the echo example) | wit_bindgen usage, kv persistence, config passing, build commands |
| Ingress/egress contract types | `ocontract/src/lib.rs` | The `InboundEvent` / `SendPayload` fields — a thin shell that both ends map to |
| Operating obridge | `obridge/README.md` | Build and run, **full-chain e2e verification steps**, WASM plugin development, hot reload, managed hosting |
| Config template | `obridge/config.example.toml` | How to write `[[channels]]` |
| Event ingress/egress | This guide's [Chapter 14](14-email-bridge.md), [Chapter 17](17-config-reference.md) | `event_ingress_port/secret`, `event_outbound_url/secret` |
| Employee execution engine | `ocore/src/runtime.rs` | Employee waking, the tool loop, the Commitment lifecycle |
| HTTP API and permissions | `oserver/src/routes.rs`, `oserver/src/rbac.rs` | Endpoint list and the RBAC matrix (74 routes × 4 identities) |
| Config fields | `ocore/src/app_config.rs` | Every field carries a comment explaining the design rationale |
| Desktop IPC | `src-tauri/src/lib.rs` | Every command from desktop → service |
| Concepts and terminology | `handbook/` (Ch.14 Events, Ch.16 Messages, Ch.18/19 SDK) | Why it is designed this way |
| History and context | `CHANGELOG.md`, `JOURNEY.md` | The full story behind every decision |
| Enterprise deployment | `DEPLOYMENT.md` | Server-side configuration and services |

## Fork Modification Guide

### Workspace Structure

| Directory | Role |
|---|---|
| `ocore/` | Core domain logic (Runtime / Store / Tools / GBrain client) — a library with zero Tauri dependencies |
| `oserver/` | The HTTP service (Enterprise mode; the Personal Edition is hosted by the desktop app too) |
| `obridge/` | External channel bridging (Email built in + WASM plugins) |
| `ocontract/` | Event and bridging contract types — **the shared language of both ends; change it with the greatest care** |
| `src-tauri/` + `src/` | Desktop shell (Tauri v2) + Vue 3 frontend |
| `frontends/` | The Enterprise three-role web consoles (pnpm workspace) |
| `ocore/default-skills/` | The built-in skill library (11 plugins, 111 skills) |

### Build and Development Loop

```bash
git clone <你的 fork> && cd Operoid
npm install
npm run tauri dev        # 桌面應用（熱重載）
```

⚠️ **When you change `oserver` or `obridge`, the two must be built together**: `cargo dev-build`. The service launches obridge as a sibling and the two cross-check each other's build id — building only one side reports a mismatch at startup (this is by design, not a bug; see [Chapter 18](18-troubleshooting.md) for the matching troubleshooting).

- Frontend type check + build: `npm run build`; global Rust checks: `cargo check` / `cargo test`.
- CI's `check.yml` runs `cargo check` on all three platforms — run it locally once before pushing a branch.

### Upstream Sync Strategy

- Keep `main` tracking upstream and develop on `feat/<topic>` branches — rebase your branch when upgrading.
- **Do not modify `ocontract`**: the vast majority of integration needs never require touching the contract layer; if you truly need to (e.g. a new channel type) → that is the signal to submit a PR upstream, not to privately extend the contract inside your fork.
- Bug fixes and generally useful governance improvements are also worth PRing upstream — it lowers your long-term maintenance cost.

## A Complete Example: Wiring Up an Enterprise IM

Scenario: the company uses an enterprise IM (Slack / Teams / DingTalk / Lark etc. — **all of them have HTTP APIs**, which is exactly the sweet spot of the plugin system) and wants Employees to send and receive IM messages.

### The Architecture Decision: a Poll Plugin or Direct Webhook Delivery?

| Option | Best for | Approach |
|---|---|---|
| **A: poll plugin** (recommended starting point) | The IM has a "list messages" API | Write a WASM plugin; the host calls your `poll()` every N seconds and you pull the API with `make-request` |
| **B: direct webhook delivery** | The IM only pushes webhooks, or you need low latency | Write a small receiver (a single bun file suffices) that verifies the signature and then sends a `POST /event` to Operoid's ingress |

In both options, Employee replies take the same path: the plugin's `send()` or your receiver delivers the `SendPayload` to the IM API.

### Steps (Option A)

1. **Inventory the IM's API**: the message-list endpoint, the send endpoint, the authentication scheme (bot token?), and whether `page/cursor` can be used for incremental reads.
2. **Scaffold the plugin project**: copy `obridge-plugin-example/` as the starting point and change the crate name in `Cargo.toml`.
3. **Implement the four functions**: `source()` (the registration key, e.g. `"acme-im"`), `init()` (reads the JSON config of `[[channels.acme-im]]` — token, default groups), `poll()` (pull new messages → `InboundEvent`, record the dedup cursor in kv), `send()` (call the send API).
4. **Build and deploy**: `rustup target add wasm32-wasip2` → `cargo build --target wasm32-wasip2` → copy the `.wasm` into the **`plugins/` directory next to the obridge config file**, following the naming convention `<name>-<poll_secs>.wasm`.
5. **Wire it up**: add an `[[channels.acme-im]]` config block to `obridge.toml` (hot-reloads within 2 seconds); Employee routing works the same as email (the `brain_id` / `employee_id` fields or a routes mapping).
6. **Verify**: follow §「全鏈 e2e 手動驗證步驟」 (the full-chain manual e2e verification steps) in `obridge/README.md` — send a message from the IM → a hit appears in `obridge.log` → an In message appears in the Employee's conversation → the Employee replies → the IM receives it.

### Demonstration Prompt 1: Developing a Channel Plugin (copy to Claude Code)

*The prompt template below is provided in Chinese as written; paste it as-is, or ask your coding agent to work in your language.*

```text
你是我們的 Rust 開發代理。請為 Operoid 的 obridge 開發一個企業 IM 通道外掛（WASM component）。
工作目錄：Operoid 倉庫根。請嚴格照以下順序進行。

## 第一步：先讀契約，不要憑空想像
依序讀完再寫碼：
1. obridge/wit/channel.wit        —— 通道介面：channel(source/init/poll/send) 與
                                     host(make-request/kv-get/kv-set/clock-now)。
                                     注意：沙箱刻意不提供 TCP/TLS，一切外部存取走 host 的
                                     make-request（HTTP、10 秒 timeout）。
2. obridge-plugin-example/src/lib.rs —— 官方 echo 範例：wit_bindgen::generate! 寫法、
                                     export! 用法、kv 持久化、init 收 JSON 設定的慣例。
3. obridge/README.md              —— §WASM 外掛開發（建置/部署/檔名慣例）、
                                     §設定熱重載、§全鏈 e2e 手動驗證步驟。
4. obridge/config.example.toml    —— [[channels]] 設定寫法。
5. ocontract/src/lib.rs           —— InboundEvent 欄位（source/brain_id/employee_id/
                                     title/content/external_ref/occurred_at/reply_to）。

## 需求
對接 <IM 名稱> 的 HTTP API（API 文件：<連結>，bot token：<如何取得>）：
1. poll()：用 host 的 make-request 呼叫 <訊息列表端點>，把「上次之後的新訊息」轉成
   inbound-event 清單——source="<im>"、title=頻道/群組名、content=訊息本文、
   external_ref=IM 訊息 id、reply_to=可回覆的錨點（頻道+訊息 id）。
   以 host kv 存去重游標（最後看過的訊息 id），確保 obridge 重啟不重發。
2. send(to, employee_id, text)：用 make-request 呼叫 <發送端點>，把 text 送到 to 指定的
   頻道/群組；任何失敗回 Err(含原因的字串)，不得 panic。
3. init(config)：解析 [[channels.<im>]] 區塊（JSON 字串，至少含 token；未設定收到 "{}"）
   ，寫入 host kv；必須冪等（每次實例化都會被呼叫）。
4. 在 obridge/config.example.toml 加上 <im> 的設定範例（含註解）。

## 硬性限制
- 只能使用 host 能力（make-request/kv/clock-now）；沒有檔案、沒有 TCP/TLS。
- 不得修改 obridge/wit/channel.wit 與 ocontract/——介面以現契約為準。
- 專案結構與 wit_bindgen 用法比照 obridge-plugin-example；新 crate 名 <obridge-plugin-<im>>。

## 建置與測試
- rustup target add wasm32-wasip2
- cargo build -p <crate> --target wasm32-wasip2
- 先跑通官方往返測試：cargo test -p obridge example_plugin -- --ignored --nocapture
- 為新外掛寫單元測試：設定解析（缺 token 應報可讀錯誤）、事件欄位映射、游標去重、
  send 的失敗路徑。

## 驗收條件（全部達成才算完成）
1. .wasm 建置成功，複製到 plugins/<im>-<poll_secs>.wasm 後，obridge 啟動 log 認得此通道。
2. 從 IM 發訊息 → obridge.log 見 poll 命中 → Operoid 對應員工對話出現 In 訊息。
3. 員工回覆 → IM 群組收到訊息。
4. 重啟 obridge 兩次，不重發舊訊息。
5. cargo test 全綠。
```

### Demonstration Prompt 2: Direct Webhook-Delivery Receiver (Option B)

*The prompt template below is provided in Chinese as written; paste it as-is, or ask your coding agent to work in your language.*

```text
你是我們的開發代理。請寫一個「IM webhook → Operoid 進氣口」的小型接收服務。

先讀：
1. obridge/README.md §全鏈 e2e 手動驗證步驟（event_ingress / event_outbound 契約）。
2. ocontract/src/lib.rs 的 InboundEvent（POST /event 的 body 欄位以此為準，snake_case）。
3. 本專案指南 guide/14-email-bridge.md 的「讓員工收發 email：四步」。

需求：
1. bun 單檔（比照 deploy/local/mm-bridge/ 的風格），監聽 http://127.0.0.1:18080/webhook。
2. 驗證 <IM> 的 webhook 簽章（<驗簽方式>），把每則訊息轉成一筆事件：
   POST http://127.0.0.1:<event_ingress_port>/event
   Authorization: Bearer <event_ingress_secret>
   body：{ "kind": ..., "source": "<im>", "title": ..., "content": ...,
           "external_ref": ..., "reply_to": ... }   （欄位名照 InboundEvent）
3. Operoid 不可達時暫存並重試（指數退避，上限 <N> 次）；提供 /healthz。
4. 列出我需要在 app-settings.json 的 app_config 設定的鍵
   （event_ingress_port、event_ingress_secret）與員工回覆外發需要的
   event_outbound_url/secret 對應關係。
驗收：IM 發訊息 → 員工被喚醒（對話出現 In 訊息）；停掉 Operoid 30 秒再恢復，期間的
訊息補投成功、順序正確。
```

### Demonstration Prompt 3: Adding an API Endpoint After Forking

*The prompt template below is provided in Chinese as written; paste it as-is, or ask your coding agent to work in your language.*

```text
你是我們的 Rust 開發代理。請在 Operoid 企業服務新增一個內部報表 API 端點：
GET /api/admin/reports/employee-weekly（僅 admin）。

先讀：
1. oserver/src/routes.rs —— 路由怎麼註冊、回應格式慣例。
2. oserver/src/rbac.rs —— 端點級 RBAC 矩陣怎麼登記（admin 角色）。
3. oserver/src/writes.rs 或相近模組 —— 現有 handler 的錯誤處理慣例。

需求：
1. 回傳近 7 天每位員工：喚醒次數、完成承諾數、待核可提案數（資料來源：SQLite，
   先讀 oserver/src 的 DB 存取慣例）。
2. 遵守現有錯誤/認證慣例；加整合測試（含無 token 401、非 admin 403）。
3. cargo check -p oserver 與 cargo test -p oserver 全綠。
注意：這是 fork 內的私有端點——不要動 ocontract/ 與前端合約；若未來想回饋上游，
先開 issue 討論。
```

## Security Notes

- **The sandbox is the boundary**: plugins get no TCP/TLS/file access — only the host's three capabilities. This is by design: integration needs should push you toward the IM's HTTP API, not around the sandbox.
- **Where credentials live**: the IM token goes in `obridge.toml` (plugins receive it through init); Operoid's ingress/egress secrets go in `app-settings.json` / `operoid.toml`. All are local files — fold the config files into your company's backup and permission management, and never commit them to git.
- **Draw the line for external channels first**: IM / Email are the channels through which Employees reach the outside world. Before go-live, use the [Action Classification Registry](13-governance.md) to decide which tier "Employees automatically replying to external parties" belongs to — if in doubt, keep it in the human-decision tier.
- **Logs**: watch `obridge.log` during development; in Enterprise deployments also watch the service logs (`oserver`'s stderr / platform logs).

---
[← Appendix A](appendix-a-gbrain.md) | [Index](README.md)
