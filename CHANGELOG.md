# Changelog

本檔記錄 Operoid 各版本的變動。**發布流程**：

1. 發布前，在此加該版本段落（格式見下方；新版本在上方）。
2. 同步版本號（`package.json`／`Cargo.toml`／`tauri.conf.json`／`Cargo.lock`）。
3. `git tag -a vX.Y.Z` + push → GitHub Actions（`.github/workflows/release.yml`）自動抓取
   本檔中對應 tag 的段落作為 Release 說明，build 各平台安裝包後建立 **draft release**。
4. 到 GitHub Releases 頁審閱 draft，確認無誤後 publish。

> 抓取規則：`release.yml` 找本檔中 `## [vX.Y.Z]` 開頭的段落（到下一個 `## ` 為止）作為該
> 版本的 release body。故**版本段落標題必須含 tag 名**（如 `## [v0.2.5]`）。

---

## [v0.3.0] - 2026-08-19

### 前後端分離完成——Operoid 成為常駐服務＋多前端架構

P1–P5 五階段（計畫 `docs/Operoid-計畫-前後端分離.md`）全部落地。

#### 主要變動

**`ocore` 核心 crate（P1）**

- 全部領域邏輯（domain／runtime／scheduler／event bus／GBrain 能力域）脫離 Tauri——**零 Tauri 依賴**
- `Channel<CliLine>` 串流改 `LineSink` 回呼

**`oserver` 服務 binary（P2–P4）**

- axum HTTP API：agent-os 讀寫面＋GBrain 全域＋ring buffer 主控台輪詢
- `AuthProvider` trait（token 首個實作——企業版帳號插座）
- bind-first＋healthz；SQLite 全走 `spawn_blocking`
- per-brain 序列化待寫入面後續

**桌面前端切換 HTTP（P3–P4）**

- agent-os 與 GBrain 全頁面經 `127.0.0.1:7340`（Bearer token；CORS；wrappers 簽名不變、stores 零改動）
- GUI 正式成為「諸多前端之一」

**服務註冊（P5）**

- **Windows**：SCM（UAC 自我提權；實機驗證）
- **Linux**：systemd system unit（`/etc/systemd/system`——`sudo` 提權安裝，服務以安裝使用者身分執行）
- **macOS**：launchd LaunchDaemon（`/Library/LaunchDaemons`——同上，以安裝使用者身分執行）
- 後兩者 CI 編譯覆蓋、未實機驗證；設定頁「開機自動啟動」開關

**生命週期雙語意**

- 服務已裝 → 開機自啟、GUI 關閉不影響（A1）
- 未裝 → GUI 帶起帶走（A2）

**ingress 併入 oserver（P5）**

- `POST /event`（token＋去重）——GUI 不開，obridge 投件不再掉
- `ingress_server`／`event_bus` 殼層退役

**`src-tauri` 瘦身為殼**

- 視窗＋桌面專屬能力（claude_code／note_view／open dir）＋指令薄層

#### 已知邊界

- E13 員工封存語意、E8 fire-and-forget sync、SSE 串流——列於待處理清單
- Linux/macOS 服務註冊未實機驗證（程式在、CI 編譯覆蓋）

---

## [v0.2.7] - 2026-08-18

### 前後端分離首步（ocore）＋ think 修復＋預設模型改正

#### 主要變動

- **P1a 抽取 `ocore` 核心 crate**（前後端分離計畫首片，`docs/Operoid-計畫-前後端分離.md`）
  - 新 workspace member `ocore`：`domain/`、`agent_state`、`llm`、`outbound`、`i18n`＋`slug`／`gbrain_config`——**零 Tauri 依賴**，桌面殼與未來服務 binary（`oserver`）共用
  - `src-tauri` 以 re-export 保持既有程式碼路徑零改動；介面微調三處（行為零變）
  - 驗證：workspace 測試 137 全綠、0 warning、前端 build 0 error

- **E9 補遺——OperationsView 手動 think 顯式傳 `--model`**
  - E9 原修復只蓋 agent 路徑；手動 think 在 DB-plane models 未設時同樣 fallback 到 anthropic opus → synthesis skipped
  - 比照修復：以作用中腦的 `chat_model` 顯式指定

- **預設 chat model 改 `zhipu:glm-4-flash`**
  - glm-5.x 全系為推理模型（回應含 `reasoning_content`），gbrain think synthesis 解析不相容（`LLM_OUTPUT_NOT_JSON` → 隨機空輸出；coding 端點還會把 5.2 映射成 5.3）
  - glm-4-flash 非推理模型，標準與 coding 端點皆實證可用——新使用者開箱即用

- **obridge 打包**：安裝檔內建 obridge（externalBin＋beforeBundleCommand 自動建置）＋ CI 修復

## [v0.2.6] - 2026-08-17

### Obridge（Email bridge）落地

#### 主要變動

- **E7 步驟 3——Obridge（Operoid Bridge）**：Email 雙向通道（IMAP 收＋SMTP 寄）＋WASM 外掛體系（wasmtime＋component model；HTTP 類通道如 Slack/Teams 未來走外掛）
- **outbound v2（E12）**：完整 send Tool（tool-choice 編排）——外發統一為員工行動；對話回合 tool-loop（think/send/propose/finish）＋自主循環可主動通知
- **obridge 生命週期**：設定檔熱重載（watch toml mtime）＋Operoid 子進程代管（opt-in autostart）；外掛設定傳遞（WIT `init(config)`）
- **零設定**：路徑預設值＋執行檔自動偵測；設定檔為空時自動複製預設範本

---

## [v0.2.5] - 2026-08-13

### 自主循環真實環境打通

v0.2.4（Event 匯流排）以來，聚焦讓承諾驅動自主循環在真實 LLM 環境下真正能完成。

#### 主要變動

- **E2 自主循環可診斷性 + 可收斂性**
  - PLAN／EVAL 診斷軌跡：`record_event` 每輪記 `plan`/`eval` 事件（含 query/done/rationale）——Stalled 時也能看見 LLM 每輪判斷
  - PLAN prompt 強化：看見近期 artifact 內容（消除「PLAN 瞎子」）+ done 判據引導
  - 鬆綁重複偵測 `MAX_REPEAT=2`

- **E9 gbrain think synthesis 缺 LLM 修復**（E2 軌跡揭露的真根因）
  - 根因：think 子行程讀 DB-plane `models.*`（fallback → anthropic opus），不讀 `chat_model` → synthesis 找 `ANTHROPIC_API_KEY` 失敗 → "no LLM available; synthesis skipped"
  - 修法：`GbrainThinkTool` 顯式 `--model <chat_model>`，跳過 fallback 鏈（零 DB-plane 副作用）
  - **承諾驅動自主循環真實環境首度 Satisfied**（`real_run_autonomous`：Stalled 9-10 cycles → 1 cycle Satisfied）

- 收尾 sprint：T1 真實整合測試、文件債清理

#### 測試
`cargo test` — 103 passed; 7 ignored

---

## [v0.2.4] - 2026-08-12

### Event 匯流排架構

Handbook Ch.12 第四種 Trigger（Event-driven）落地：

- factory 寫入 → `InboundEvent` → 腦 → 員工 1:N 路由 → review/提案（Workstream A–E＋G）
- LLM 全域 Semaphore 並發節流（預設 4）
- `run_autonomous` 每輪先清 inbox（修復 doc/impl 不一致）

Webhook 進氣口（F）為 Phase 2 待辦。
