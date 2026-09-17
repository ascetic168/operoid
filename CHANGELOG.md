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

## [v0.3.4] - 2026-09-18

### 預設模型升回 glm-5.3-flash（解 E15）

- 上游 garrytan/gbrain#4727 已於 **gbrain 0.50.x** 為 zhipu recipe 補上 `thinking_by_default`——gbrain 不再把 GLM 5.x 誤判為非 thinking 模型。實測 think/chat 的 model-aware output cap 恢復 **16000/32000**（`output_tokens` 精確停在 16000 驗證），v0.3.1 回退 glm-4-flash 時的長回應截斷（`LLM_OUTPUT_TRUNCATED`）風險解除。
- `DEFAULT_CHAT_MODEL` 由 `zhipu:glm-4-flash` 升回 `zhipu:glm-5.3-flash`（`ocore/gbrain_config.rs`）；常數文檔與相關測試記錄完整升降級脈絡。
- 前端同步：三語系「主模型」placeholder 與 BrainsView「新增腦」預設 chat model。

## [v0.3.3] - 2026-09-07

> Harness 補強（W0–W3）——計畫 `docs/Operoid-計畫-Harness補強.md`。含兩個**行為變更**（見「⚠️」）。

### 員工可攔截：合作式停止（W1）

- 執行中的員工可被人工停止：`POST /api/employees/{id}/stop` 設旗標，runner 於步驟邊界（PLAN/ACT/EVAL 每輪、對話每步、任務間、承諾間）優雅中止；進行中的單次 LLM 呼叫會跑完（最長約 2 分鐘）。
- 停止語意：承諾維持 `Active`（人可再觸發）、記 `stopped` 事件、員工轉 **`Paused`**——不再被排程器自動喚醒；**人類傳訊息／交辦／核可＝明示恢復**（Paused→Sleeping）。
- 前端：監看 modal「停止」鈕（working 時可用）＋右鍵「停止」；`agent_os.employeeNotRunning`（409）等新錯誤碼三語。

### 員工封存與硬刪除（W1，E13）

- ⚠️ **`DELETE /api/employees/{id}` 語意變更**：預設＝**封存**（軟刪除）——`archived=true`、不再被喚醒／不收新訊息與事件，歷史（對話／事件／產出）完整保留可追溯；`POST /api/employees/{id}/unarchive` 解封。
- `DELETE ...?hard=true`＝**串聯硬刪除**（tasks／messages／events／commitments／artifacts／memory 全清）——僅供開發／測試，無 UI 入口。
- 封存時若員工執行中，順帶合作式停止。前端：右鍵「封存…」取代「刪除」、主列表過濾、「已封存」檢索區＋右鍵解封。

### 恢復力：LLM 重試＋承諾自動重排（W2，T2）

- LLM 層：**5xx 納入可重試**（原僅網路錯誤／429）；指數退避 5s→10s→20s（3 次上限）；4xx 立即失敗。token 用量（usage）best-effort 記入事件（kind `llm`，含 model＋tokens）。
- ⚠️ **承諾自動重試**：`run_autonomous` `Errored` 的承諾自動排程重跑——退避 10min·2^n（上限 4h）、最多 3 次，耗盡後記 `retry_exhausted` 事件待人類；`Satisfied`／人工再觸發歸零。排程器每 tick 只喚醒「退避到期」者——健康承諾不重跑、不出錯不排程（backpressure）。
- 事件匯流排：`dispatch_event` 路由命中時 fire-and-forget `brain_sync`（E8——員工查詢前圖譜已含剛寫入的完整長文；race 由 content 全文兜底）。

### 第一個行動工具：write-note（W3）

- 員工可把完整產出寫成 markdown 筆記：新工具 `write-note`（`ocore/write_note.rs`）——寫到 `employee_output_path`（新設定，預設 `{home}/employee-output`，**在 notes repo 之外、不入圖譜**）；人工 review 後移入 notes repo 走既有 sync 晉升（Handbook Ch.07 §6「衍生知識：產出→驗證→晉升」的最小落地）；收回＝刪檔。
- 防護：僅接受單一 `.md` 檔名（拒路徑分隔／絕對路徑）、拒覆寫、canonicalize 包含檢查；frontmatter 帶 `origin: operoid-employee`＋produced_by 可批次識別。
- 權限閘門 v1（建構期 allowlist）：`Employee.tools`／`EmployeeTemplate.tools`（模板部署繼承；建立模板 API 加 `tools` 參數、表單加核取）；對話迴圈 `write` 動作（寫檔＋Committed artifact）與自主循環 PLAN `tool=write` 分支（Draft、Satisfied 才晉升）；無權限回報員工不中斷。
- 前端：員工右鍵「開啟產出目錄」（新殼層指令 `employee_open_output_dir`）。

**品質**：ocore 152 tests passed（+19）、oserver 4；`cargo check --all-targets --workspace` 0 warning；前端 build 綠燈。

## [v0.3.2] - 2026-09-01（重發布）

### 工廠頁支援 gbrain schema v2（15 類型）＋動態 schema pack

- **單一類型資料表**：新增 `ocore/src/factory_types.rs`，內建 `gbrain-base-v2`（15 型：person/company/media/tweet/social-digest/analysis/atom/concept/source/deal/email/slack/writing/project/note）與 legacy `gbrain-base`（原 6 工廠）兩份定義（目錄、frontmatter 型別/標籤、管線、分類同義詞）。原本散落 5 處的 Rust hardcoded match（`target_dir_of`/`run_core`/`run_textual`/`save_authored_core`/`factory_open_dir`）與 `text_to_md::render` 的型別對照全部改為查表；未知類型明確報錯，移除「誤歸 concepts」的靜默 fallback。
- **動態 schema pack**：pack 名取自 gbrain config file plane 的 `schema_pack`（未設定 → v2、未知 pack → v2 表）。新增 `factory_types` Tauri command 與 `oserver` `GET /api/factories/types` 路由，回傳 pack 資訊＋類型清單＋v1 升級提示。
- **工廠頁主從式改版**：自動分類 hero 卡保留；15+ 類型改為緊湊 chip 清單（左側，動態載入），右側為選中類型的單一拖放/預覽工作區——頁面高度固定不捲頁，未來 pack 增減類型自動跟隨。非 v2 pack 顯示橫幅提示（連到設定頁的 unify-types 按鈕）。
- **分類器 pack 感知**：LLM prompt 的類型枚舉、同義詞正規化（people↔person、meetings↔meeting 等單複數/中文）、heuristic 對應全部改為依作用中 pack 查表；信心分級（高/中自動跑、低交確認）維持不變。v2 無 meeting 型——會議特徵在 v2 pack 交 LLM 判讀。
- **v2 對齊細節**：mentioned_names wikilink 改用該 pack 的 person/company 目錄（v2=`[[person/slug]]`）；wikilink 解析候選目錄（`note_view.rs`）涵蓋 v2+legacy 目錄，跨 pack 舊連結仍可開啟；v2 的 company 頁不再寫 `subtype`（type 已是 company）。
- i18n：三語系補齊 15 類型標題/輸出說明與範本（未知/自訂 pack 類型 fallback 顯示 id、通用範本）。
- 測試：`factory_types`（pack 對應/同義詞/kind 查表）、`classifier`（v2/legacy 雙 pack）、`text_to_md`（v2 render/wikilink 目錄）擴充，全數通過。

### 視窗啟動最大化

- 主視窗（`tauri.conf.json`）加 `"maximized": true`：啟動即最大化，避開 Windows DPI 縮放下 `center: true` 置中計算不準的問題；還原時仍退回 1280×840 置中。

### dev／安裝版的 oserver 舊碼殘留防護

- **dev**：`npm run tauri dev` 的 `beforeDevCommand` 改為新的 `npm run dev:all`（`cargo build -p oserver && npm run dev`）——每次 dev 先確保 `target/debug/oserver.exe` 為最新。此前 `tauri dev` 只編 GUI app crate，不會編 oserver，改過 oserver/ocore 後跑 dev 會出現「GUI 新、服務舊」的路由 404（已遇過一次：背景殘留的舊分離行程 + 未重編的 exe）。
- **升級安裝**：NSIS hooks 新增 PREINSTALL／POSTINSTALL——服務在跑時（以 `sc stop` 回傳碼偵測）先停服務釋放 `oserver.exe` 檔案鎖，裝完自動 `sc start` 重啟；原本已停止或未安裝服務者不受影響。修復「服務模式 + 升級」時舊服務續用記憶體中舊碼／覆寫 exe 失敗的縫隙。反安裝行為不變。

### 操作頁 think：輸出末列出全部引註（可點擊連結）

- 「操作」頁的 think 改以 `gbrain think --json` 執行（`ocore/src/gbrain_cli.rs` 新增 `run_think_json`），取得結構化 citations 後重排為人類可讀格式：問題標題、answer、Gaps、`Model/Pages/Takes/Graph/Citations` footer，最後新增「## 引註（Citations）」清單。
- 引註行採雙括號 wikilink 形式（`[[people/林家豪]]`、`[[林家豪]]（take #3）`）——前端 `OperationsView.linkSegments` 無條件匹配雙括號，不受單括號引註「slug 須含 `/`」規則限制，模型給含前綴或裸標題的 page_slug 皆可點擊開啟筆記（`openNote` 支援裸標題掃描 people/ 等已知目錄）。
- 隱去 `CITATIONS_STRUCTURED_NOT_INLINE`／`CITATIONS_INLINE_NOT_IN_STRUCTURED` 引註比對警告：gbrain 行內標記 regex 僅接受 ASCII slug（中文頁面恆無法判定為行內），且 glm-4-flash 常在本文留下 `[slug#N]` 佔位字面值——兩方向的比對警告對本應用恆為雜訊，引註已由清單完整呈現。其他警告照常顯示。
- 顯示前自 answer 剔除 `[slug#N]` 佔位標記。
- 寬容退路：非零退出碼或 JSON 解析失敗（舊版 gbrain）時逐行原樣輸出 stdout，操作不失敗；stderr（升級提示等）結束後補推不吞訊息。
- 已知取捨：`--json` 模式下 gbrain 整段輸出，answer 不再逐行串流（期間顯示「think：合成中…」step 提示）。
- 測試：新增 `citation_line_matches_link_segment_format`（引註行格式鎖定）。

## [v0.3.1] - 2026-08-31

### gbrain 整合升級——對齊 gbrain v0.46＋模型層重整

**對齊 gbrain v0.46（三項）**

- **MCP provider**：think／query 優先走 `gbrain serve` stdio MCP（rmcp 3.x），失敗自動 fallback CLI 子行程；管理操作仍走 CLI。新設定 `gbrain_transport`（mcp 預設／cli），設定頁可切換。
- **search／think 分離**：新增 `GbrainSearchTool`（`gbrain query` 混合檢索，無 LLM 合成、省 token）與 `GbrainToolset` 複合分派器；操作頁新增 query 按鈕。**注意**：gbrain v0.46 起 `ask` 已是 `query` 的別名（純檢索），帶引用連結的合成回答改由 `think` 提供。
- **schema pack v2 對齊**：factory 寫出的 frontmatter 加 `origin: operoid-factory`；legacy v1 腦顯示遷移橫幅＋unify-types 按鈕。

**模型層重整**

- 預設模型**回退為 `zhipu:glm-4-flash`**：gbrain（≤0.47.x）對 GLM 未視為 thinking-by-default（zhipu recipe 缺 `thinking_by_default`，上游 issue garrytan/gbrain#4727），glm-5.x 的 think／chat 分別被壓在 4000／4096 output tokens，長回應易截斷；glm-4-flash 為非推理模型不受影響。上游修復後可再升回。
- **think（多跳合成）顯式 model 改取 `models.think`**（缺時 fallback `chat_model`）：實測 thinking 模型的推理 token 計入合成 max_tokens 額度，長篇合成穩定 `LLM_OUTPUT_TRUNCATED`→空輸出；think 可獨立指到非推理模型（如 glm-4-flash），chat 維持強模型。手動操作、agent CLI、MCP 三條路徑一致。
- 修復 unify-types 按鈕：補 `--params {target_pack, apply}` 與 `--follow`（缺參數會 permanent fail；PGLite 腦無背景 worker 須 inline 執行）。

**config plane 事實修正（gbrain 0.47.6 實測）**

- gbrain runtime 對 model/tier 鍵採 **file plane（config.json）優先**，DB-plane 值被 shadow（`gbrain config get` 明示）——專案原本「DB plane 權威」的假設與事實相反。
- `set_model`／`set_models_all`（設定頁主模型／tier 編輯）與新腦的 `sync_new_brain_models` 改為**兩 plane 同步寫入**（file 直寫 + `gbrain config set`），不再只寫 DB 而被檔案舊值蓋掉。
- 設定頁 tier 顯示改用 `gbrain config get` 的有效值與來源（file／db 徽章如實呈現）；`db_overrides` 語義改為「檔案無值、由 DB fallback 的鍵」，警告橫幅與提示文案同步修正。

### 對話回合修復

- 對話回合 user prompt 與自主循環 PLAN prompt 注入現在時間（日期感知）。
- 同通道單一回覆保證：同目標一回合僅一則成功送出，抑制 send／finish 重複寫出 Out Message。新增對應測試。

**品質**：ocore 123 tests passed；oserver 4 passed；前端 build 綠燈。

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
