# 附錄 B：Operoid 開發說明

> 適用版本：v0.5.11。給要**在 Operoid 上開發自家接口**或 **fork 修改**的企業團隊——假設你們以 Vibe Coding（Claude Code 等 coding agent）開發。核心訊息：**先讓 agent 讀契約與範例，再動手**；本附錄給你資訊地圖與可直接複製的示範提示詞。

## 先選路線：外掛擴充 vs fork 修改

| | 路線一：Obridge 外掛 | 路線二：fork 修改 |
|---|---|---|
| 適合 | 接企業 IM／內部系統——加一個通道、一個進出接口 | 改核心行為——新 API、新工具、新治理規則 |
| 成本 | 低——沙箱內寫一個 WASM 元件，**不碰核心碼** | 高——維護 fork、承擔上游同步 |
| 升級 | Operoid 升級照常，外掛原樣可用 | 每次升級要合併上游 |

**原則：能外掛就不 fork；要 fork，把改動集中、把契約層留給上游。**

## 資訊地圖：該讀哪裡

Coding agent 最怕的不是不會寫，而是**讀錯地方**。以下是每類任務的權威資訊源（路徑相對於倉庫根）：

| 要做什麼 | 讀這裡 | 說明 |
|---|---|---|
| 通道外掛介面 | `obridge/wit/channel.wit` | **契約**：`channel`（source／init／poll／send）與 `host`（make-request／kv／clock-now）；刻意不提供 TCP/TLS |
| 外掛怎麼寫 | `obridge-plugin-example/`（echo 範例） | wit_bindgen 用法、kv 持久化、設定傳遞、建置指令 |
| 進出接口契約 | `ocontract/src/lib.rs` | `InboundEvent`／`SendPayload` 欄位——薄外殼，兩端都對映它 |
| obridge 操作 | `obridge/README.md` | 建置執行、**全鏈 e2e 驗證步驟**、WASM 外掛開發、熱重載、代管 |
| 設定範本 | `obridge/config.example.toml` | `[[channels]]` 怎麼寫 |
| 事件進出口 | 本指南[第 14 章](14-email-bridge.md)、[第 17 章](17-config-reference.md) | `event_ingress_port/secret`、`event_outbound_url/secret` |
| 員工執行引擎 | `ocore/src/runtime.rs` | 員工喚醒、工具迴圈、承諾生命週期 |
| HTTP API 與權限 | `oserver/src/routes.rs`、`oserver/src/rbac.rs` | 端點清單與 RBAC 矩陣（74 路由 × 4 身份） |
| 設定欄位 | `ocore/src/app_config.rs` | 每個欄位都有註解說明設計理由 |
| 桌面 IPC | `src-tauri/src/lib.rs` | 桌面 → 服務的全部指令 |
| 概念與術語 | `handbook/Chinese/`（Ch.14 事件、Ch.16 訊息、Ch.18/19 SDK） | 為什麼這樣設計 |
| 歷史與脈絡 | `CHANGELOG.md`、`JOURNEY.md` | 每個決策的來龍去脈 |
| 企業部署 | `DEPLOYMENT.md` | 伺服器端組態與服務 |

## Fork 修改指南

### Workspace 結構

| 目錄 | 角色 |
|---|---|
| `ocore/` | 核心域邏輯（Runtime／Store／Tools／GBrain 客戶端）——零 Tauri 依賴的函式庫 |
| `oserver/` | HTTP 服務（企業模式；個人版也由桌面代管） |
| `obridge/` | 外部通道橋接（Email 內建＋WASM 外掛） |
| `ocontract/` | 事件與橋接契約型別——**兩端共用的語言，改它要最慎重** |
| `src-tauri/` ＋ `src/` | 桌面殼（Tauri v2）＋ Vue 3 前端 |
| `frontends/` | 企業版三角色網頁（pnpm workspace） |
| `ocore/default-skills/` | 內建技能圖書館（11 外掛 111 技能） |

### 建置與開發迴圈

```bash
git clone <你的 fork> && cd Operoid
npm install
npm run tauri dev        # 桌面應用（熱重載）
```

⚠️ **改 `oserver` 或 `obridge` 時，兩個要一起編**：`cargo dev-build`。服務以 sibling 方式帶起 obridge，並互相核對 build id——只編一邊會在啟動時報不匹配（這是設計，不是 bug；對應排解見[第 18 章](18-troubleshooting.md)）。

- 前端型別檢查＋建置：`npm run build`；Rust 全域檢查：`cargo check`／`cargo test`。
- CI 的 `check.yml` 會在三平台跑 `cargo check`——推分支前本地先過一遍。

### 上游同步策略

- `main` 跟上游、開發在 `feat/<主題>` 分支——升級時 rebase 你的分支。
- **不修 `ocontract`**：絕大多數接口需求用不著改契約層；真需要（例如新的通道型別）→ 這是該向上游提 PR 的訊號，而不是在 fork 裡私有擴充。
- 修 bug、通用的治理改進——也建議 PR 回上游，降低長期維護成本。

## 完整範例：接通企業 IM

場景：公司用某企業 IM（Slack／Teams／釘釘／飛書等，**都有 HTTP API**——這正是外掛體系的甜蜜點），要讓員工收發 IM 訊息。

### 架構決定：poll 外掛還是 webhook 直投？

| 方案 | 適合 | 做法 |
|---|---|---|
| **A：poll 外掛**（推薦起點） | IM 有「列訊息」API | 寫 WASM 外掛，host 每 N 秒呼叫你的 `poll()`，你用 `make-request` 拉 API |
| **B：webhook 直投** | IM 只推 webhook、或要低延遲 | 寫一個小接收器（bun 單檔即可），驗簽後 `POST /event` 到 Operoid 的進氣口 |

兩方案員工回覆都走同一條路：外掛的 `send()` 或你的接收器把 `SendPayload` 送到 IM API。

### 步驟（方案 A）

1. **盤點 IM 的 API**：訊息列表端點、發送端點、鑑權方式（bot token？）、能不能用 `page/cursor` 做增量。
2. **產生外掛專案**：複製 `obridge-plugin-example/` 為起點，改 `Cargo.toml` 的 crate 名。
3. **實作四個函式**：`source()`（註冊鍵，如 `"acme-im"`）、`init()`（讀 `[[channels.acme-im]]` 的 JSON 設定——token、預設群組）、`poll()`（拉新訊息→`InboundEvent`，kv 記游標去重）、`send()`（呼叫發送 API）。
4. **建置與部署**：`rustup target add wasm32-wasip2` → `cargo build --target wasm32-wasip2` → 複製 `.wasm` 到 **obridge 設定檔同目錄的 `plugins/`**，檔名慣例 `<name>-<poll_secs>.wasm`。
5. **接線**：`obridge.toml` 加 `[[channels.acme-im]]` 設定區塊（2 秒內熱重載）；員工路由同 email（`brain_id`／`employee_id` 欄位或 routes 對映）。
6. **驗證**：照 `obridge/README.md` §「全鏈 e2e 手動驗證步驟」——IM 發訊息 → `obridge.log` 見命中 → 員工對話出現 In 訊息 → 員工回覆 → IM 收到。

### 示範提示詞一：開發通道外掛（複製給 Claude Code）

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

### 示範提示詞二：webhook 直投接收器（方案 B）

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

### 示範提示詞三：fork 後加一個 API 端點

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

## 安全注意

- **沙箱即邊界**：外掛拿不到 TCP/TLS/檔案——只能經 host 的三個能力。這是設計：接口需求應推著你用 IM 的 HTTP API，而不是繞過沙箱。
- **憑證落點**：IM token 寫 `obridge.toml`（外掛經 init 收到）；Operoid 進出密鑰寫 `app-settings.json`／`operoid.toml`。皆為本機檔案——把設定檔納入公司的備份與權限管理，不要提交進 git。
- **對外通道先畫線**：IM／Email 是員工觸及外部世界的通道。上線前用[動作類別登記表](13-governance.md)決定「員工自動回覆外部」屬哪一層——拿不準就留在人裁決層。
- **日誌**：開發期盯 `obridge.log`；企業部署加看服務日誌（`oserver` 的 stderr／平台日誌）。

---
[← 附錄 A](appendix-a-gbrain.md)｜[索引](README.md)
