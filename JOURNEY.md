# Emploid 實踐藍圖與進度（Journey & Progress）

**Version:** 0.1
**Status:** Living document — 持續更新
**建立日期：** 2026-07-30
**最後更新：** 2026-08-16

> Handbook 是「憲法」（說明 *what* 與 *why*）；本文件是「工程日誌」（說明 *how*、*when*、*走到哪*）。
> 兩者衝突時，以 Handbook 為準——改本文件或改提案，絕不靜默改程式碼來違背憲法。

---

## 0. 這份文件是什麼、怎麼用

Emploid 的目標很遠：從一個「GBrain 知識圖譜操作 GUI」演進成 Handbook 描繪的「AI Agent Operating System」。這段路不可能一步走完，所以需要一份**可重複回顧**的藍圖，隨時確認：

1. 目標態是什麼（來自 Handbook，不重抄，只引用）。
2. 現況是什麼（程式碼實際長怎樣）。
3. Gap 在哪。
4. 分幾階段走、每階段的**退出條件**是什麼、**這階段不做什麼**（克制）。
5. 現在走到哪（見文末「進度紀錄」）。

**用法：** 每次開工先讀「進度紀錄」與「待決問題」；每完成一個段落，回來更新進度紀錄、勾掉退出條件。方向偏了就回來對照 Handbook 原則。

---

<div style="page-break-after: always; break-after: page;"></div>

# 一、目標態（摘自 Handbook）

> 完整內容以 `handbook/` 為準；此處僅壓縮，便於隨時對齊。

**Emploid 是 AI Agent Operating System**——AI agent 以 **Employee** 之姿（Employee ≡ Agent）活在持久 的 **Workspace** 裡，承擔責任、被喚醒工作、做完沉睡，跨模型／工具／session 皆存續。

**五個設計目標**（Ch.01）：
1. 知識活過模型替換。
2. 責任活過對話關閉。
3. Artifact 歸於 workspace、是 first-class。
4. Employee 之間（含人類）協作。
5. Workspace 持久存續。

**OS 類比**（Ch.01 §6）：process→Employee、file→Artifact、memory→Working Memory/Knowledge、device→Tool、kernel→Runtime。Runtime 管 **execution**，不管 **reasoning**。

**十原則**（Ch.02，順序即依賴序）：知識≠工作者 / Employee 擁有責任 / Artifact first-class / 一切在 Workspace 內 / Tool 不決策 / Brain 可共享 / Employee 預設沉睡 / context 還原而非記憶 / Commitment 活過 Task / Runtime 管執行不管推理。

**Handbook 既定的五里程碑順序**（Ch.20，依依賴排列）：
1. **一個真正能運作的 Employee**（wake→restore→read inbox→invoke tool→commit artifact→sleep）。
2. **持久化與 Commitment**（跨 session 存活、Artifact first-class、memory 無瑕還原）。
3. **共享 Brain 與 Knowledge**（一個 Brain 服務多 Employee）。
4. **Template 與 Instance**（一個 Template 部署多獨立 Instance）。
5. **協作**（多 Employee 在 Project 內並行＋循序合作）。

**核心概念一句話角色**：Workspace＝組織；Employee＝工作者；Brain＝可共享的智慧；Artifact＝工作產出（first-class）；Knowledge＝組織的記憶；Tool＝不決策的外部能力；Project＝有界倡議；Task＝短命可執行單位；Commitment＝長期責任；Trigger＝喚醒 Employee 的鬧鐘；Runtime＝生命週期引擎；Event＝不可變事實紀錄；Memory＝每次喚醒還原的工作脈絡。

---

<div style="page-break-after: always; break-after: page;"></div>

# 二、現況（程式碼實際是什麼）

> 誠實盤點，作為起點。以 2026-07-30 的程式碼為準。

Emploid（app）目前是一個 **GBrain 知識圖譜操作 GUI**：管理多個 GBrain 腦、用 factory 把雜亂檔案結構化成筆記、對腦做 sync/think/ask。整個後端圍繞 GBrain CLI 打轉。

**指令面（`lib.rs` 約 30 個 Tauri command）：** 環境探測、gbrain config 讀寫、`op_run`（跑 sync/think/ask）、factory pipeline（file→md、分類、寫檔）、brain 與 source 註冊表、開啟筆記、啟動外部 Claude Code。

**持久化：** `tauri-plugin-store` 的 `app-settings.json`，**只存** GBrain 腦清單（`BrainEntry`）、作用中腦/source、factory 目標目錄、LLM 採樣參數、locale、Claude Code 終端偏好。沒有任何「工作」「責任」「產出」「事件」的持久狀態。

**前端：** `ConfigView` / `BrainsView` / `FactoriesView` / `OperationsView` + `ClaudeCodeDialog`。全 brain-centric。

**概念對照（現況 vs Handbook）：**

| Handbook 概念 | 現況 | 說明 |
|---|:---:|---|
| Workspace | 🟡 隱性 | 無顯式 Workspace 實體；整個 app 即單一隱性工作區 |
| Employee | ❌ 無 | 無 identity / role / authority / memory / commitment |
| Brain | 🟡 部分 | GBrain 腦＝知識圖譜；缺 Handbook Brain 的 persona＋prompt＋memory 組成 |
| Knowledge | ✅ 有 | notes repo ＋ graph（factory 生成 + gbrain sync）|
| Artifact | ❌ 偽 first-class | factory 產出只是 notes repo 裡的 md 檔，無 provenance / version / ownership |
| Tool | 🟡 部分 | gbrain CLI 硬接於程式碼，非註冊式 Tool、無權限邊界 |
| Project | ❌ 無 | — |
| Task | ❌ 無 | — |
| Commitment | ❌ 無 | — |
| Trigger | ❌ 無 | 目前全人工觸發（點按鈕） |
| Runtime | ❌ 無 | 無 wake/restore/execute/commit/sleep 引擎 |
| Event | ❌ 無 | — |
| Memory | ❌ 無 | 無可還原的工作記憶 |

**一句話 headline：** 程式碼是「以 Brain 為中心的知識圖譜 GUI」；Handbook 要的是「以 Employee 為中心、Runtime 驅動的 Agent OS」。這是核心 drift，也是整段旅程要收斂的落差。

**現有資產（不要丟）：** GBrain 知識圖譜＋factory 管線＋think/ask 是真實且有價值的能力——在 Handbook 語彙裡，它們對應到 **Knowledge** 與 **Brain 的知識層**，並且天生適合作為第一個 Employee 的「能力來源」。旅程不是砍掉重練，而是把這層能力「降格」為 Agent OS 的一個後端。

---

<div style="page-break-after: always; break-after: page;"></div>

# 三、策略定調

D1／D2／D4 已於 2026-07-30 定案（見 §七）；D3（GUI 演進）刻意延後，做到 Phase 1+ 再想。
若日後出現更好選項，回來修這裡＋§七，再動程式碼。

1. **GBrain 不重來，降為第一個 Knowledge/Brain 後端（provider）。**
   現有 factory、sync、think/ask 保留，未來包裝成 Employee 可呼叫的能力／Tool。Agent OS 層**疊在**現有 GUI 之上，先用 feature flag 隔離，**不破壞**既有 GBrain 工作流。

2. **資料模型先立在持久化之上、獨立於儲存技術。**
   先沿用 `tauri-plugin-store`（JSON）起步，但 domain types 設計成**不綁死** JSON store——預留遷移到 SQLite 的空間（見待決 D2）。

3. **由內而外：先能把「一個 Employee 一圈」跑起來，再談多工與協作。**
   完全呼應 Handbook Ch.20 的依賴順序。不提前做 Template、不做 collaboration、不做 marketplace。

4. **Employee 的「推理」第一版優先複用既有路徑**（gbrain think/ask，或接 Claude Code），而非從零拉新 LLM 管線（見待決 D4）。先把「骨架」跑通，再換引擎。

---

<div style="page-break-after: always; break-after: page;"></div>

# 四、分階段藍圖

> 每階段列：**目標 / 對應 Handbook / 退出條件（可驗）／這階段不做（克制）**。
> 退出條件沒達標，不進下一階段。

## Phase 0 — 地基（Foundation）

- **目標：** 把核心資料模型與持久化骨架立起來，讓第一個 Employee 有地方「站」。**不碰 runtime、不碰 LLM。**
- **對應：** 為 Handbook Milestone 1 鋪路；把 Workspace 從隱性提升為**顯式概念**（Principle 4）。
- **範圍：** 定義 domain types（`Workspace`、`Employee`、`BrainRef`、`Artifact`、`Commitment` 等）＋持久化（先 JSON store，介面預留可換 SQLite）＋基礎 CRUD（建立／列出／持久化一個 Workspace、一個指向某 GBrain 腦的 Employee 記錄、一個 Artifact 記錄）。全程 feature flag 隔離。
- **退出條件：**
  - [x] 能建立並持久化 Workspace / Employee / Artifact，**重啟後仍在**。（Phase 0 以 `JsonStore` 單測證：put → 重開 store → 資料仍在；app 層的「建立／列出」入口屬 Phase 1。）
  - [x] 既有 GBrain GUI 行為完全不變（flag off 時零影響）。（無新 Tauri 指令、無前端改動；`agent_os_enabled` 預設 false 且 `#[serde(default)]`；`cargo build` 0 warning。）
  - [x] domain types 不綁死特定儲存後端（有 trait 介面）。（`Store` trait 為純 Rust、無 Tauri；`JsonStore` 為其檔案式實作，可換 SQLite。）
- **不做：** runtime、trigger、LLM 推理、多 Employee、任何 UI 大改。
- **狀態：✅ 完成（2026-07-30）。** 落點：`src-tauri/src/domain/{mod,models,store}.rs` ＋ `AppConfig.agent_os_enabled`。72 tests 全綠（含 12 新 domain 測試）。

## Phase 1 — 一個能跑完一圈的 Employee（≡ Handbook Milestone 1）

- **目標：** 單一 Employee 走完 **wake → restore context → read inbox → invoke tool → commit artifact → sleep**。第一版**人工觸發**即可。
- **對應：** Handbook Milestone 1；驗證最小 Runtime、Tool 邊界（Principle 5）、context 還原（Principle 8）。
- **範圍：** 最小 Runtime（排程一個 Employee 跑一輪）＋把 **gbrain think/ask 註冊成第一個 Tool**（D4 已定：第一版推理固定走 gbrain think/ask；故第一版 Brain ≈ GBrain 腦的 think/ask 能力）＋Artifact commit＋context 的 persist/restore。
- **退出條件：**
  - [x] 一個 Employee 可靠跑完一圈，產出 Artifact。（`run_cycle` 走完 wake→restore→execute→commit→sleep，產出 Committed Artifact。stub-Tool 測試＋真實 gbrain `agent_run`。）
  - [x] **重啟／閒置**後，不丟**進度**（context 可還原）。（`context_restored_across_restart` 測試：重開 store 後 artifact／memory 仍在、第二輪累積。）〔註：本條原寫「不丟 Commitment 與進度」；Commitment 持久化屬 Phase 2，故修為「進度」。〕
  - [x] Tool 永遠不替 Employee 做決策（邊界測試）。（`Tool` trait 結構上只有 `invoke`；`tool_never_acts_unless_invoked` 測試。）
- **不做：** 多 Employee 並行、Template、共享 Brain 切換、自動 Trigger。
- **狀態：✅ 完成（2026-07-30，含真實 gbrain 驗證）。** 落點：`domain/{tools.rs, models.rs(Task/Memory), store.rs}` ＋ `runtime.rs`（`run_cycle`／`GbrainThinkTool`／`agent_run`／`agent_seed`）＋ `lib.rs` 註冊兩指令（執行期以 `agent_os_enabled` 把關）。76 tests 全綠。真實 gbrain 端到端驗證通過（`real_gbrain_think_cycle` ignored 測試：Graph 1、Pages 21、Model groq、4 citations，產出含多場會議的合成 artifact）。

## Phase 2 — 持久化與 Commitment（≡ Handbook Milestone 2）

- **目標：** Commitment 跨 session 存活；Artifact 真 first-class（identity / provenance / version / ownership）；Memory 無瑕還原。
- **對應：** 驗證 Principle 3（Artifact first-class）、8（context restored）、9（commitment 活過 task）。
- **退出條件：**
  - [x] 系統**完全關機重啟**後，進行中的工作正確接續。（`commitment_spans_tasks_across_restart` 測試：重開同一 SQLite db 後 commitment/task/artifact 皆在、第二輪接續。持久化＝SQLite；接續＝手動 wake，見 D3／Phase 3+ 的自動喚醒。）
  - [x] Artifact 在產生它的 Employee／session 結束後仍存續、可追溯。（完整 provenance：produced_by＋source_task_id＋source_commitment_id；版本歷史：`revise_artifact_keeps_history`——舊版 Superseded、新版 Committed、revised_from 鍊保留。）
- **不做：** 多 Employee 共享 Brain、Template、協作。
- **狀態：✅ 完成（2026-07-30，含真實 gbrain 驗證）。** 落點：`domain/sqlite_store.rs`（`SqliteStore`，D2 已遷 SQLite）＋ Commitment 活化（`agent_create_commitment`／`agent_satisfy_commitment`）＋ Artifact provenance／版本（`revise_artifact`）＋ run_cycle 繫結 commitment ＋ `agent_revise_artifact`／`agent_list_state` 指令。81 tests 全綠；真實 gbrain 在 SQLite 上跑通（Graph 1、Pages 21、Citations 10）。

## Phase 3 — 共享 Brain 與 Knowledge（≡ Handbook Milestone 3）

- **目標：** 一個 Brain 服務多 Employee；Knowledge 可策展、可版本、可檢索。
- **對應：** 驗證 Principle 1（知識≠工作者）、6（Brain 可共享）。**GBrain 給我們起跑優勢**——一個 graph 本就服務多次查詢，這階段把它做實成「Brain 可被多 Employee 引用」。
- **退出條件：**
  - [x] 升級一個 Brain，多 Employee 採用，**不失身份、不失進行中工作**。（結構上早已可行：`Employee.brain` 是共享 `BrainRef`，員工狀態各自獨立於 SQLite。Phase 3 把它**證明**出來——`shared_brain_two_employees_independent`＋`brain_upgrade_preserves_inflight_work`（stub v1→v2：舊員工 v1 進行中工作不失、新員工採 v2）＋真實 `real_shared_brain`（兩員工共用 demo 腦、Graph>0、各自 artifact/memory）。）
- **狀態：✅ 完成（2026-07-30，含真實 gbrain 驗證）。** 落點：`agent_recruit` 指令（招募員工、可共用腦）、`agent_list_state` 加 employees。83 tests 全綠；真實兩員工共用 demo 腦跑通（Graph 1、Pages 21、Citations 12／1）。**無模型變更**（Brain 維持＝GBrain 後端，D1；Knowledge＝GBrain 圖譜）。

## Phase 4 — Template 與 Instance（≡ Handbook Milestone 4）

- **目標：** Employee Template 部署成多個獨立 Instance，共享 Brain 與 Role，各自擁有獨立 Inbox 與 Commitment。
- **退出條件：**
  - [x] 一個 Template 產出多個獨立 Instance（「每座廠一個 Steve」），各追蹤各自的現實。（`template_deploys_independent_instances`：`steve` template → 部署 Steve-TW／NJ／VN 三 Instance，皆共享 brain_id＋role＋template_id；各跑一圈產獨立 artifact／memory；一 Instance 的 commitment 不影響另兩者。）
- **狀態：✅ 完成（2026-07-30）。** 落點：`EmployeeTemplate` 實體＋`Employee.template_id` 溯源；Store 的 template collection（JsonStore＋SqliteStore）；`agent_create_template`／`agent_deploy_instance` 指令（`deploy_instance` helper）。84 tests 全綠、`cargo build` 0 warning。**無真實 gbrain 測試**（部署是資料層；Instance 執行已由 Phase 3 `real_shared_brain` 覆蓋）。

## Phase 5 — 協作（≡ Handbook Milestone 5）

- **目標：** 多 Employee 在 Project 內合作：交接 Task、共享 context、產出共享 Artifact，彼此不互相擁有。
- **退出條件：**
  - [x] 一個 Project 由一隊 Employee 並行＋循序完成。（`team_runs_concurrently`：3 員工併發、產共享 project artifact；`handoff_task_between_employees`：A→B 任務交接＋B 接手；`concurrent_cycles_overlap_in_time`：**併發實證**——3×800ms 併發 ≈1.1s（非 2.4s）；`real_team_concurrent`：兩員工在 demo 腦上併發 think、皆 Graph>0。）
- **狀態：✅ 完成（2026-07-31）。** 落點：`Project` 實體＋`Artifact`/`Task`＋`project_id`；`agent_create_project`／`agent_run_team`（**併發** `futures::join_all`，各員工腦各自解析、gbrain 子行程真並行）／`agent_handoff_task`／`agent_run_task`。87 tests 全綠、`cargo build` 0 warning。**併發模型**：同 process 內併發 tokio task（Option A；OS-process／分散式為 Horizon）。

## Phase 6 — 員工生命週期（Trigger 驅動的自主運行）

> v1（Phase 0–5）完成了**資料模型＋單發人工循環**，但員工「不會自己跑」。Phase 6 兌現 Handbook
> 里程碑 1 的**完整循環**（因 Trigger 喚醒→還原→**讀 Inbox**→調用 Tool→提交 Artifact→睡眠）與 Runtime
> 排程器——把員工從「CRUD 列」變成「會醒來、持續工作到完成才睡」的工作者。**紅線**：Trigger 驅動
> 非常駐（守原則 7／8）。範圍決策：**承諾驅動（完整願景）**。

### 6a — Runtime 地基（排程器 + busy-lock + 收件匣喚醒）

- **目標**：帶 Assigned task 的 Sleeping 員工，啟動時／收到喚醒信號時自己醒來處理、做完睡回；與前端 `agent_run` 競態安全。
- **退出條件：**
  - [x] Sleeping＋待辦 task → 自動喚醒處理完睡回（`run_inbox_drains_assigned_tasks`、`real_inbox_wake` ignored）。
  - [x] 同一員工被排程器與指令並發 → busy-lock 擋下（`try_acquire_serializes_same_employee`）。
  - [x] WAL 多連線並發寫入不鎖死（`wal_concurrent_writers_with_busy_timeout`）。`cargo test` 91 全綠、6 ignored。
- **狀態：✅ 完成（2026-08-03）。** 落點：Store 加 5 個 list-by-owner 方法＋WAL/index；`agent_state.rs`（AppState/busy-lock/WakeSignal）；`scheduler.rs`（常駐排程器：mpsc＋30s tick＋啟動掃描）；`runtime.rs` 抽 `restore_memory`／`commit_artifact` 共用 helper＋新增 `run_inbox`／`build_tool_ctx`／`agent_db_path`；DB 搬 Local AppData（避 OneDrive／網域同步毀 WAL）＋一次性遷移；`agent_run`／`agent_run_task` 接 busy-lock。

### 6b — ReasoningTool + 承諾驅動（agent loop）

- **目標**：員工憑一個 Active commitment 自主運行（規劃→行動→評估→直到 Satisfied 或卡住 Suspended）。
- **退出條件：**
  - [x] Handbook 先改後碼：Ch.13 §4 加「自主執行與完成評估（修訂）」、Ch.12 §2 加「投遞工作」、Ch.04 Inbox 補訊息投遞（**中英六檔鏡像**）。
  - [x] `Reasoner` trait＋`LlmReasoner`（包既有 `llm::complete`，結構化 JSON）＋`build_reasoner`（解析腦的 endpoint，缺 key → `llm.noApiKey`）。
  - [x] `run_autonomous`：plan→act→evaluate 循環；`done`→`Satisfied`；0 產出卡住→`Suspended`；硬錯→`Error`。`completion_condition` 終於被評估。
  - [x] 排程器：啟動一次承諾掃描（不在每次 tick 重跑，免燒 LLM）＋每次 tick／信號的 Inbox 掃描。`cargo test` 93 全綠。
- **狀態：✅ 完成（2026-08-03）。** 落點：Handbook 六檔；`domain/tools.rs` 加 `Reasoner`／`parse_json_value`；`runtime.rs` 加 `LlmReasoner`／`build_reasoner`／`run_autonomous`／`CycleBudget`／`AutonomousOutcome`；`scheduler.rs` 拆 `scan_inbox`（tick）／`scan_commitments`（啟動）。**v1 簡化**：未新增 Memory/Commitment 欄位（用既有 `Suspended` 狀態＋`memory.notes` 進度脈絡）；承諾僅啟動喚醒（每次 tick 重跑列為未來，搭配 backpressure）。
- **待補**：真實 gbrain+llm 的 `run_autonomous` ignored 測試——**已補 `real_run_autonomous`（2026-08-12，`#[ignore]`），待手動驗證**（需 demo 腦＋LLM API key）。

### 6c — 溝通（Message-driven Trigger）

- **目標**：右鍵員工 → 溝通 → 輸入訊息 → 員工被喚醒處理。
- **退出條件：**
  - [x] `agent_send_message(employee_id, text, commitment_id?)`：訊息 → Inbox 裡一個 `Assigned` Task ＋ push `WakeSignal`（不搶 busy-lock、不執行員工）。6a 的 `scan_inbox`/`run_inbox` 會消化——訊息無 commitment 也會被處理。
  - [x] 前端：`tauri.ts` wrapper、`stores/agent.ts` action、`EmployeeInstanceView` 右鍵「溝通…」項＋訊息 modal（textarea）；i18n 三語（`instances.message*`）＋補齊 `agent_os.*` 錯誤鍵。`npm run build`（vue-tsc＋vite）0 error、`cargo check` 0 error。
- **狀態：✅ 完成（2026-08-03）。** 這是讓 6a/6b 引擎在 UI 可見的觸發源——訊息投遞 Inbox、排程器喚醒、`run_inbox` 產 artifact。**待 6d**：即時觀察（訊息送出後員工醒來跑一圈，目前需手動刷新才看得到狀態變化）。

### 6d — 監看 + 生命週期事件

- **目標**：右鍵員工 → 監看 → 看見目前正在進行的工作與歷程（即時）。
- **退出條件：**
  - [x] 生命週期事件（Ch.14 啟發，append-only）：`Event` model＋`events` 表＋`put_event`／`list_events_by_employee`（最新在前）；`record_event` 在 `commit_artifact`／`run_inbox`／`run_autonomous`（wake／satisfied／stalled／errored）記錄。
  - [x] `agent_watch(employee_id)` 指令（取代 over-fetch 的 `agent_list_state`）：回傳 state＋commitments＋tasks＋近期 artifacts＋memory＋近期 events。
  - [x] 監看 modal：每 1.5s 輪詢，顯示狀態色／承諾／當前 task／近期產出＋捲動事件 log（仿 OperationsView console）；關閉即停。i18n 三語。
  - [x] **順帶修 6a 的 bug**：`agent_list_state`／`agent_revise_artifact` 等 **11 個指令**仍讀舊 Roaming DB（6a 已遷 Local）→ 過時資料；全改 `agent_db_path`。
  - [x] `cargo test` 94 全綠、`cargo check`／`npm run build` 0 error。
- **狀態：✅ 完成（2026-08-03）。** 落點：`domain/{models,store,sqlite_store,mod}.rs`（Event）；`runtime.rs`（record_event＋記錄點＋agent_watch＋11 處 DB-path 修正）；`tauri.ts`／`EmployeeInstanceView.vue`／i18n。**Phase 6 至此全數完成**。

---

## Phase 7 — 人機協作（聊天＋交辦＋提案承諾）

> Phase 6 讓員工會自己醒來工作，但人機介面仍單發（溝通＝一次 Q&A、承諾無 UI 入口、員工不會發問／提案）。Phase 7 升級為**雙向多次對話**＋**交辦承諾**＋**員工提案承諾待人類核可**。範圍決策：**新增 Message 一級概念**（接受 Handbook 修訂）＋**分階段 7a→7b→7c**。

### 7a — 交辦承諾 ＋ 立即喚醒（無需改手冊）

- **目標**：右鍵交辦 → 員工立即自主跑該承諾，不必重啟 app。
- **退出條件：**
  - [x] `agent_create_commitment` 建立後**背景立即喚醒**（busy-lock 把關、非阻塞）。
  - [x] 前端：右鍵「交辦…」modal（標題＋完成條件）、wrapper、store action、i18n 三語。
  - [x] `cargo test` 94 全綠、`npm run build` 0 error。
- **狀態：✅ 完成（2026-08-03）。** 落點：`runtime.rs` 抽 `run_commitments_for_employee` helper（鎖＋tool/ctx/reasoner＋清 Inbox＋每個 Active commitment 跑 run_autonomous），`agent_create_commitment` 與 `scheduler::scan_commitments` 共用（排程器掃描簡化為候選＋委派）；前端 `tauri.ts`／`stores/agent.ts`／`EmployeeInstanceView.vue`（交辦 modal）／i18n。

### 7b — Message 概念 ＋ 聊天 ＋ 對話迴圈 ＋ 員工發問（含 Handbook 修訂）

- **目標**：雙向多次聊天介面；員工 Reasoner 驅動回覆（可反問）；新增 Message 一級概念。
- **退出條件：**
  - [x] **先手冊後碼**：Handbook 新增 **Ch.16 Message**（中英）＋**Part IV 重編 16→17–21**（10 檔改名＋header＋Security→Tool-SDK 交叉引用）＋README（v0.2、TOC、概念表、概念圖）＋Ch.04 Inbox／Ch.18 Agent-SDK 封閉清單／Ch.21 Roadmap §7 交叉引用。明文調和反聊天立場（Message 是互動層，durable 仍是 Artifact／Commitment）。
  - [x] `Message` model＋`MessageDirection{In,Out}`＋Store（put_message／list_messages_by_employee，trait+Json+Sqlite＋`messages` 表）。
  - [x] `agent_send_message` 同時寫 `Message{In}`；`run_conversational_turn`（知識檢索→Reasoner 回覆答案／反問→`Message{Out}`＋artifact）；`run_inbox` 注入 `Option<&Reasoner>`，訊息 task 走對話回合（無 reasoner 退回 gbrain 單發，守 6c）；`scan_inbox` best-effort 建 reasoner；`agent_watch` 加 `messages`。
  - [x] 前端 `EmployeeChatView`（`/instances/:id/chat`，氣泡列表 In 左／Out 右、輪詢 1.5s、auto-scroll、Enter 送出）；右鍵「對話…」入口；i18n 三語。
  - [x] `cargo test` 95 全綠（新增 conversational reply 測試）＋`npm run build` 0 error。
- **狀態：✅ 完成（2026-08-04）。** 落點：`handbook/*`（Message 章＋重編＋多處，中英）；`domain/{models,store,sqlite_store,mod,tools(?)} .rs`；`runtime.rs`（Message＋run_conversational_turn＋run_inbox Option reasoner＋agent_send_message In＋agent_watch messages）；`scheduler.rs`（scan_inbox reasoner）；`EmployeeChatView.vue`(新)／`router.ts`／`tauri.ts`／`EmployeeInstanceView.vue`／i18n。

### 7c — 員工提案承諾 ＋ 人類核可（含 Ch.11／Ch.20 修訂）

- **目標**：員工在對話中判斷某事該成為承諾時，主動提案（Proposed），待人類核可後成立（Active）。
- **退出條件：**
  - [x] **先手冊後碼**：Ch.11 加 `Proposed`／`Rejected` 生命週期＋Ch.20 §5（Security）「提案-核可」通用化到承諾成立（中英）。
  - [x] `CommitmentStatus::Proposed`／`Rejected`；`agent_propose/approve/reject_commitment`；`agent_watch` 加 `proposals`；`run_conversational_turn` 擴充 Reasoner schema（`kind: propose` → 建 Proposed 承諾 ＋ Out Message 徵求同意）。
  - [x] 前端：聊天頁 Out 氣泡下方**內嵌 [核可]／[拒絕] 鈕**（commitment_id 比對 proposals）；wrapper；i18n 三語。
  - [x] `cargo test` 95 全綠 ＋ `npm run build` 0 error。
- **狀態：✅ 完成（2026-08-06）。** **Phase 7 全部完成**（7a–7c）。

---

## Horizon — v1 之後

Skill learning、cloning/parallelism、marketplace、federation、distributed runtime、人機團隊——詳見 `handbook/21-Roadmap.md §7`。現階段**不展開**；重點是：這些都不需要新核心概念，是對既有概念的延伸。這正是好架構的檢驗。

---

<div style="page-break-after: always; break-after: page;"></div>

# 五、第一個具體落點（等你 ready 時）

> 現在只規劃，不動手。這是 Phase 0 的第一小步，刻意限縮範圍。

**寫核心 domain types 與持久化 trait，背後 feature flag，不動現有 GUI：**

1. 新增 `src-tauri/src/domain/` 模組，定義 `Workspace` / `Employee` / `BrainRef` / `Artifact` / `Commitment` 等型別（先以 Handbook 定義為本，只放最精簡欄位）。
2. 定義儲存抽象 trait（`load` / `save` / `list` …），實作一份 JSON-store 版本。
3. 加 feature flag（例如 `agent_os`），預設關閉；flag off 時現有 app 零變化。
4. 補最小 unit test（建立→持久化→重載→相等）。

**這一步刻意不包含：** Runtime、Tool 註冊、任何 LLM 呼叫、任何新 UI。先把「骨架型別＋能存」做紮實，是整趟旅程的地基。

---

<div style="page-break-after: always; break-after: page;"></div>

# 六、不變的守則（Guardrails）

每個階段、每個 feature，都拿這幾條自問：

1. **是否守住十原則？** 衝突時——改設計，或先修 Handbook，**絕不靜默改程式碼**違背原則（Ch.02）。
2. **架構活過技術。** 不在核心概念章節綁死 Rust/SQLite/MCP/特定模型（Ch.01 §「A Note on Technology」）。
3. **概念集保持小。** 新增 first-class 概念需架構審查；能由既有概念表達的，就用既有概念（Ch.20 §1）。
4. **先 Handbook，後程式碼。** 提案表達不出來→回頭修架構，不是先改碼。
5. **The standing question（Ch.20 §8）：** *Does it honor the ten principles?* 是→做；否→改提案或先修 Handbook。

---

<div style="page-break-after: always; break-after: page;"></div>

# 七、決策紀錄（Decisions）

> 定案後搬進對應 Phase 的範圍；日後翻案就回來改這裡＋相關 Phase，再動程式碼。

## 已定案（2026-07-30）

- **D1 — Knowledge/Brain 後端邊界：✅ 先以 GBrain 為唯一後端，但透過 trait 抽象**（provider 介面），避免日後抽換困難。第二個 backend 等真有需求再做。
- **D2 — 持久化技術：✅ Phase 2 已遷 SQLite**（`SqliteStore`，rusqlite bundled；`Store` trait 仍保 `JsonStore` 對照）。原 JSON store 起步、需求出現時遷移——已於 Phase 2 落地。
- **D4 — Employee 推理引擎（第一版）：✅ 先複用 gbrain think/ask 跑通一圈。** 故第一版 Brain ≈ GBrain 腦的 think/ask 能力；直接 LLM API／Claude Code 路徑等迴圈穩了再評估。

## 延後

- **D3 — GUI 演進策略：⏸ 刻意延後，做到 Phase 1+ 再想。** 目前唯一共識：「feature flag 隔離、先不動既有 GBrain 頁面」；新頁面何時上線，等 Phase 1 有東西可展示再決定。
- **D6 — GUI 與 oserver 跨機器分離部署：⏸ 延後（2026-08-22 記錄）。** v0.3.0 刻意限定 oserver 僅本機（bind 127.0.0.1、前端寫死 127.0.0.1、token 明文本機假設）。未來跨機器時有四個必改點，**含最易漏的 CSP connect-src**（2026-08-22 NSIS 安裝版事件：CSP 只在打包版生效、dev 不套用——dev 正常、安裝版待辦全空）。完整清單見待處理清單 E14；屆時以專檔計畫展開。

## 已定案（2026-08-18）

- **D5 — 前後端分離（後端服務化）：✅ 全部完成（2026-08-18～20，P1–P5，v0.3.0）**（專檔 `docs/Operoid-計畫-前後端分離.md`，即待處理清單 R2 的展開）。六決策：單機服務先行（預留團隊升級）／全部搬（GBrain 能力域後搬）／shared token／階段混合啟動且 `oserver install` 服務註冊為交付物／同 repo workspace（新 `ocore`＋`oserver`，與 obridge 同構）／`operoid.toml`＋SQLite。本輪刻意不做：SSE、多帳號/RBAC、第二前端、多租戶。核心動機：Runtime 壽命不再綁架 GUI（原則 7 系統側）、「不熄的燈」劇本首次真實成立。

---

<div style="page-break-after: always; break-after: page;"></div>

# 八、進度紀錄（Progress Log）

> 每次推進回來追加一條；格式：日期｜階段｜做了什麼｜下一步｜open issues。

| 日期 | 階段 | 做了什麼 | 下一步 | Open issues |
|---|---|---|---|---|
| 2026-07-29 | —（前置） | 專案更名 GBrainStudio→Emploid 完成（repo/code/identifier/Cargo.lock 皆已更名）；Handbook 更新為 Emploid。 | — | package-lock.json 仍殘留舊名（npm install 自動修） |
| 2026-07-30 | —（前置） | 找回並修正舊 Claude 記憶→寫入 Emploid 記憶目錄；刪除舊 GBrainStudio 專案資料夾。 | — | — |
| 2026-07-30 | 規劃 | 撰寫本藍圖（JOURNEY.md）；盤點現況與 Gap；定 Phase 0–5。 | 等 D1–D4 定調後，展開 Phase 0 第一小步 | D1, D2, D3, D4 |
| 2026-07-30 | 決策 | D1/D2/D4 定案（GBrain 唯一後端＋trait 抽象；JSON store 起步、Phase 1 末／2 初遷 SQLite；推理首版用 gbrain think/ask）；D3 延後。feature flag 機制定為 runtime（AppConfig 欄位）。 | 展開 Phase 0 | D3（延後） |
| 2026-07-30 | **Phase 0 ✅** | 新增 `src-tauri/src/domain/`（models＋Store trait＋JsonStore＋id/timestamp helpers）；`AppConfig.agent_os_enabled`（runtime flag，預設 false）；`lib.rs` 加 `mod domain;`（不接指令）。72 tests 全綠（含 12 新測），`cargo build` 0 warning。 | Phase 1 | D3（延後）；package-lock.json 仍殘留舊名 |
| 2026-07-30 | **Phase 1 ✅** | 新增 `domain/tools.rs`（`Tool` trait＋Spec/Input/Output/Ctx，純 Rust、boxed Send future）、`runtime.rs`（`run_cycle` 走 wake→restore→execute→commit→sleep、`GbrainThinkTool`、`agent_run`／`agent_seed` 指令）；models 加 Task／Memory、Store 加對應方法。觸發方式：測試驅動 stub-Tool（不加 UI，D3）。76 tests 全綠（含循環／重啟還原／Tool 邊界），`cargo build` 0 warning。 | 真實 gbrain 驗證 | D3（延後） |
| 2026-07-30 | Phase 1 驗證 | 真實 gbrain 端到端通過：`real_gbrain_think_cycle`（ignored 測試）對 demo 腦跑 think → Graph 1、Pages 21、Model groq（非 opus）、4 citations，合成「晶瀚半導體會議」答案並 commit 為 artifact。一圈 wake→restore→execute→commit→sleep 全走通。 | Phase 2 | D3（延後） |
| 2026-07-30 | **Phase 2 ✅** | `domain/sqlite_store.rs`（`SqliteStore`，D2 遷 SQLite；`Store` trait 保 JsonStore 對照）；Commitment 活化（create／satisfy）、Artifact provenance＋版本（`revise_artifact`）、run_cycle 繫結 commitment、新指令 `agent_create_commitment`／`agent_satisfy_commitment`／`agent_revise_artifact`／`agent_list_state`。81 tests 全綠（SQLite round-trip／重啟、commitment 跨重啟多 task、artifact revise 歷史），`cargo build` 0 warning。 | Phase 3（共享 Brain 與 Knowledge） | D3（延後）；package-lock.json 仍殘留舊名 |
| 2026-07-30 | Phase 2 驗證 | 真實 gbrain 在 **SQLite** 上跑通：`real_gbrain_think_cycle` 改用 SqliteStore → Graph 1、Pages 21、Citations 10、Model groq。SQLite 持久化＋commitment／artifact 流程端到端正確。 | Phase 3 | D3（延後） |
| 2026-07-30 | **Phase 3 ✅** | 共享 Brain 與 Knowledge（Milestone 3）。關鍵認知：共享/升級腦結構上早已可行（BrainRef 共享、員工狀態獨立），故 Phase 3 以**證明**為主、無模型變更。加 `agent_recruit`（招募員工、可共用腦）、`agent_list_state` 加 employees。83 tests 全綠（shared_brain 兩員工、upgrade_preserves_inflight_work）。 | Phase 4 | D3（延後） |
| 2026-07-30 | Phase 3 驗證 | 真實 `real_shared_brain`：兩員工（emp-a／emp-b）共用 demo 腦各跑 think → Graph 1、Pages 21、Citations 12／1、groq；各產 artifact、memory 獨立。共用腦＋獨立狀態端到端正確。 | Phase 4 | D3（延後） |
| 2026-07-30 | **Phase 4 ✅** | Template 與 Instance（Milestone 4）。加 `EmployeeTemplate` 實體（name/brain/role 可重用定義）＋`Employee.template_id` 溯源；Store template collection（JsonStore＋SqliteStore）；`agent_create_template`／`agent_deploy_instance`（`deploy_instance` helper 抄 brain/role、各自獨立）。`template_deploys_independent_instances`：一 template 部署 3 Instance、共享 brain/role、各自獨立。84 tests 全綠、`cargo build` 0 warning。 | Phase 5 | D3（延後） |
| 2026-07-31 | **Phase 5 ✅ — v1 抵達** | 協作（Milestone 5，最終）。`Project` 實體＋`Artifact`/`Task`＋`project_id`；`agent_create_project`／`agent_run_team`（**併發** `futures::join_all`）／`agent_handoff_task`／`agent_run_task`。併發模型＝同 process 內併發 tokio task（Option A）。測試：team_runs_concurrently、handoff_task、**concurrent_cycles_overlap_in_time**（併發實證 3×800ms≈1.1s）、real_team_concurrent（兩員工 demo 腦）。87 tests 全綠、0 warning。 | **Handbook 五里程碑盡數達成**；後續為 Horizon | D3（延後） |
| 2026-08-01 | **D3 GUI 首版** ✅ | Agent-OS 首份可見 UI（D3 實作）。後端補能：`Store` 加 delete_template/delete_employee；新指令 ensure_workspace／list_templates／list_employees／delete_*／rename_*。前端：`tauri.ts` agent_* wrappers＋`AppConfig.agent_os_enabled`；`stores/agent.ts`；`components/ContextMenu.vue`（右鍵選單）；`EmployeeTemplateView`（master/detail＋腦 picker＋CRUD）、`EmployeeInstanceView`（**視窗風格卡片網格＋右鍵管理＋部署**）。側欄加「員工模板／員工實體」；**啟動預設頁改為員工實體**；ConfigView 加 Agent-OS 開關；i18n 三語。`cargo test` 87 全綠、`npm run build`（vue-tsc＋vite）0 error。 | 視覺驗證（npm run tauri dev）＋ Horizon | — |
| 2026-08-03 | **Phase 6a ✅** | Runtime 地基——員工生命週期首部曲。`domain/store.rs`＋`sqlite_store.rs` 加 5 個 list-by-owner 方法＋WAL/busy_timeout＋新 index；新模組 `agent_state.rs`（AppState／每員工 busy-lock RAII guard／WakeSignal mpsc）、`scheduler.rs`（常駐排程器：`async_runtime::spawn`＋select! 於 mpsc 喚醒與 30s tick＋啟動掃描，只喚醒 Sleeping＋有待辦 task 者）；`runtime.rs` 抽 `restore_memory`／`commit_artifact` 共用 helper、新增 `run_inbox`（收件匣驅動喚醒）、`build_tool_ctx`、`agent_db_path`（**DB 搬 Local AppData** 避 OneDrive 毀 WAL＋一次性遷移）；`agent_run`／`agent_run_task` 接 busy-lock。`cargo test` **91 全綠**（新增 inbox-drain／empty-inbox／WAL 並發／busy-lock 序列化）、6 ignored（含新 `real_inbox_wake`）。 | 6b（ReasoningTool＋承諾驅動，含 Handbook P10／訊息-Inbox） | 預存 `csv_people.rs` 一條 unused-assignment 警告（非本 Phase 引入） |
| 2026-08-03 | **Phase 6b ✅** | 承諾驅動——員工憑 Active commitment 自主運行。**先手冊後碼**：Handbook Ch.13 §4 加「自主執行與完成評估（修訂）」（Runtime 可主理多步推理循環＋諮詢 Brain 評估完成，因「何時睡」屬生命週期）、Ch.12 §2 加「投遞工作」、Ch.04 Inbox 補訊息投遞——**中英六檔鏡像**。`domain/tools.rs` 加 `Reasoner` trait＋`parse_json_value`；`runtime.rs` 加 `LlmReasoner`（包既有 `llm::complete`，結構化 JSON）、`build_reasoner`（缺 API key → `llm.noApiKey`）、`run_autonomous`（plan→act→evaluate；done→`Satisfied`、0 產出卡住→`Suspended`、硬錯→`Error`；`CycleBudget` 限流）、`AutonomousOutcome`；`scheduler.rs` 拆 `scan_inbox`（tick）／`scan_commitments`（**啟動一次**，免每次 tick 燒 LLM）。`cargo test` **93 全綠**（新增 satisfies／suspends-on-no-progress）。 | 6c 溝通（Message-driven Trigger＋前端，讓 6a/6b 在 UI 可見） | 承諾僅啟動喚醒（每次 tick 重跑列未來）；真實 gbrain+llm 的 `run_autonomous` ignored 測試待補 |
| 2026-08-03 | **Phase 6c ✅** | 溝通——Message-driven Trigger 的 UI 觸發源。`agent_send_message(employee_id, text, commitment_id?)`：訊息→Inbox 一個 `Assigned` Task＋push `WakeSignal`（不搶 busy-lock／不執行員工；6a `scan_inbox`／`run_inbox` 消化，訊息無 commitment 也處理）；`lib.rs` 註冊。前端：`tauri.ts` wrapper、`stores/agent.ts` `sendMessage` action、`EmployeeInstanceView` 右鍵「溝通…」＋訊息 modal（textarea）；i18n 三語 `instances.message*`＋補齊 `agent_os.*` 錯誤鍵（employeeBusy 等）。`npm run build`（vue-tsc＋vite）0 error、`cargo check` 0 error。 | 6d 監看（事件 log＋觀察面板，讓訊息後的喚醒可即時看見） | 訊息送出後狀態變化需手動刷新（6d 加輪詢） |
| 2026-08-03 | **Phase 6d ✅ — Phase 6 完成** | 監看＋生命週期事件。`Event` model（Ch.14 啟發，append-only）＋`events` 表＋`put_event`／`list_events_by_employee`（最新在前）；`record_event` 在 `commit_artifact`／`run_inbox`／`run_autonomous`（wake／satisfied／stalled／errored）記錄。`agent_watch(employee_id)`（state＋commitments＋tasks＋近期 artifacts＋memory＋events）。前端監看 modal：1.5s 輪詢、捲動事件 log（仿 OperationsView console）。i18n 三語。**順帶修 6a bug**：`agent_list_state`／`agent_revise_artifact` 等 **11 個指令**仍讀舊 Roaming DB → 全改 `agent_db_path`。`cargo test` 94 全綠、`npm run build` 0 error。 | **Phase 6 全部完成**（6a–6d） | 真實 gbrain+llm 的 `run_autonomous` ignored 測試待補；csv_people 預存警告 |
| 2026-08-04 | **Phase 7a ✅** | 交辦承諾＋立即喚醒。抽 `run_commitments_for_employee` helper（busy-lock→tool/ctx/reasoner→清 Inbox→每個 Active commitment 跑 run_autonomous），`agent_create_commitment` 與 `scheduler::scan_commitments` **共用**（掃描簡化為候選＋委派）；`agent_create_commitment` 建立後**背景非阻塞喚醒**（`async_runtime::spawn`，不必重啟 app）。前端：右鍵「交辦…」modal（標題＋完成條件）、`agentCreateCommitment`/`agentSatisfyCommitment` wrapper、`createCommitment` action、i18n 三語 `instances.delegate*`。`cargo test` 94 全綠、`npm run build` 0 error。 | 7b Message 概念（Handbook 修訂）＋聊天＋對話迴圈 | — |
| 2026-08-04 | **Phase 7b ✅** | Message 概念＋聊天＋對話迴圈。**先手冊後碼**：Handbook 新增 **Ch.16 Message**（中英）＋**Part IV 重編 16→17–21**（10 檔改名＋header＋Security→Tool-SDK 交叉引用）＋README v0.2（TOC／概念表／概念圖）＋Ch.04／Ch.18（封閉清單：Out message 由 Runtime 代發）／Ch.21 §7 交叉引用；明文調和反聊天立場。後端：`Message`／`MessageDirection`＋Store（`messages` 表）＋`run_conversational_turn`（知識檢索→Reasoner 回覆答案／反問→`Message{Out}`）＋`run_inbox` 注入 `Option<&Reasoner>`（訊息走對話回合，無 reasoner 退回 gbrain）＋`agent_send_message` 寫 `Message{In}`＋`agent_watch` 加 messages。前端：`EmployeeChatView`（`/instances/:id/chat`，氣泡 In 左／Out 右、1.5s 輪詢、auto-scroll）＋右鍵「對話…」＋i18n 三語。`cargo test` **95 全綠**、`npm run build` 0 error。 | 7c 員工提案承諾＋人類核可（Ch.11 修訂） | — |
| 2026-08-10 | —（品牌） | **專案更名 Emploid→Operoid**（與 `pixquilly/emploid` Python 套件品牌碰撞）。深度查證：GitHub 0 衝突、npm/PyPI 可用、USPTO 商標空白、`operoid.io`/`.co` 網域可用。機械替換涵蓋 48 檔（README×3、handbook×42、Cargo.toml/tauri.conf/lib/main/runtime 等 Rust 全域）；`agent_db_path` 移除 Roaming→Local 遷移死碼，DB 檔名 `operoid.db`（可重建，不遷移）；`lib.rs` 加 `migrate_app_data_dir`——identifier `com.emploid.studio→com.operoid.studio` 一次性目錄遷移（app-settings.json 無痛延續）；handbook 詞源句改寫為 operation 詞根（EN＋中）。docs/ 4 檔重命名（含 PDF 劇本）。 | GitHub repo 重命名；發新 release | JOURNEY 歷史文字保留 Emploid 不動（如實記錄） |
| 2026-08-12 | —（Event 匯流排） | **Event 匯流排架構**（Workstream A–E＋G，commit a9ab669）。實作 Handbook Ch.12 第四種 Trigger——**Event-driven**（前三種 Message/Time/Manual 已在 Phase 6 落地）。核心：factory 寫入→`InboundEvent`→mpsc channel→`dispatch_event`（腦→員工 1:N 路由）→重用 `agent_send_message` 內部邏輯（`Message{In}`+`Task{Assigned}`+`wake()`）→下游 `run_inbox`/`run_conversational_turn`/propose 零改動接手。附帶：①LLM 全域 Semaphore 節流（`LlmReasoner` 持 permit，涵蓋對話/PLAN/EVAL 全路徑，預設 4 並發）；②`run_autonomous` 每輪先清一個 inbox（修復 doc/impl 不一致——承諾 session 期間的訊息不再苦等 session 結束）；③`process_one_inbox_task` 共用 helper 抽取。新檔 `event_bus.rs`；`AppConfig` 加 `llm_concurrency`/`event_review_enabled`。`cargo test` **103 全綠**（+2 新測：`run_autonomous_processes_inbox_first`、`list_employees_by_brain`）。詳見 `docs/Operoid-計畫-Event匯流排架構.md`。 | F（webhook 進氣口，Phase 2）—見待處理清單 E7 | brain_sync fire-and-forget 未做（E8，靠 summary 預覽兜底） |
| 2026-08-12 | —（收尾 sprint） | **收尾＋驗證 sprint**。清債：T3（JOURNEY 第 183／228 行 `.claude/plans/soft-tickling-swan.md` 懸空引用移除）、`csv_people.rs` L620 unused-assignment（`let mut only_disk = 0usize`→`let only_disk: usize`）、package-lock 版本同步（0.2.2→0.2.4，`npm install`）。補測：`real_run_autonomous`（`#[ignore]`，**第一個需真實 LLM API key 的測試**——真實 demo 腦＋LLM 跑完 plan→act→evaluate，實證斷言 Satisfied／Stalled／Errored，不硬斷言必然 Satisfied）。清單校正：T4（`20-Roadmap` 過時路徑）、E1 doc、T5 package-lock 舊名三項皆為**誤報**（早已完成），已於待處理清單標正。驗證：`cargo test` **103 全綠＋7 ignored**（+1 新測試）、`cargo check --all-targets` csv_people 警告消失（唯一殘留為既有 `ExternalMessage` dead-code，屬 E7 webhook）、`npm run build` 0 error。 | 手動驗證：跑 `real_run_autonomous` ignored 測試（需 demo 腦＋API key）＋ GUI 視覺驗證（`npm run tauri dev`） | 真實 LLM 測試待手動跑；GUI 待視覺驗證 |
| 2026-08-12 | —（自主循環修復） | **修復 `run_autonomous` 真實 LLM 下 Stalled 的雙根因**（`real_run_autonomous` 揭露）。① **`Ok(false)` fall-through**：PLAN 階段 LLM 回 `done:true` 但 `evaluate_done` 判 false 時，原 `Ok(false) => {}` 直接 fall through 取 `next_query`（但 PLAN 既回 done 就不會給 query）→ 必然 Stalled「未給出 next_query」；改為留 note（「評估未過，請由新角度繼續」）＋ `continue` 重新 PLAN。② **`evaluate_done` 看不見 artifact 內容**：評估者 prompt 原本只拿 artifact id 列表，無從判斷成果 → 傾向回 false；加 `store` 參數，取最近 3 個 artifact 各截 400 字放進 prompt。三個 stub 測試不變（皆走 `Ok(true)` 或不進 evaluate）。`cargo test` 103 全綠、`cargo check --all-targets` 無新 warning。 | 重跑 `real_run_autonomous` 驗證 Stalled→Satisfied（需 demo 腦＋API key） | — |
| 2026-08-12 | —（E2） | **自主循環可收斂性＋可診斷性**（E2 重新框架）。原始訴求（query 動態化）早已由 `run_autonomous` PLAN 滿足——重新聚焦真實測試揭露的收斂問題。① **診斷軌跡**：`record_event` 每輪記 plan/eval（含 rationale）；`evaluate_done` 回傳 `(bool, String)`；`real_run_autonomous` 印每輪軌跡——Stalled 時也能看見 LLM 每輪判斷。② **PLAN 強化**：抽 `recent_artifact_summaries` helper（PLAN/EVAL 共用），PLAN prompt 加 artifact 摘要（消除「PLAN 瞎子」不對稱）＋ done 判據引導。③ **鬆綁重複偵測** `MAX_REPEAT=2`（首次重複留 note 換角度、不浪費 ACT）。④ `runtime.rs:68` 過時註解修正。`cargo test` 103 全綠、`cargo check --all-targets` 無新 warning。 | 重跑 `real_run_autonomous` 驗證軌跡可見＋收斂改善 | Satisfied 非硬保證（LLM 非確定性） |
| 2026-08-13 | —（E9） | **修復 gbrain think synthesis 缺 LLM**（E2 軌跡揭露的真根因，v1 核心能力阻斷）。根因：think 子行程讀 DB-plane `models.*`（`--help` 明載解析鏈→fallback `opus`=anthropic），不讀頂層 `chat_model`；demo 腦 DB-plane 全空→synthesis 找 `ANTHROPIC_API_KEY` 失敗→"no LLM available; synthesis skipped"→自主循環 EVAL 恆 false 無法 Satisfied。修法（路徑 A，零副作用）：`ToolCtx` 加 `chat_model` 欄位＋`build_tool_ctx`/`agent_run_team` 用 `load_for` 取值＋`GbrainThinkTool.invoke` 加 `--model <chat_model>` 顯式指定（跳過 fallback 鏈，不動 DB-plane）。4 個真實測試 ToolCtx 同步。驗證：`think --model zhipu:glm-5.2` synthesis 有完整內容（2824 字、12 筆會議）；`cargo test` 103 全綠。附帶修 `ZHIPU_API_KEY`→`ZHIPUAI_API_KEY` 筆誤。 | 使用者重跑 `real_run_autonomous` 確認端到端 Satisfied | — |
| 2026-08-14 | —（T1 驗證） | **T1 `real_run_autonomous` 真實環境驗證通過**——v1 核心首度端到端實證。實跑（demo 腦＋zhipu:glm-5.2＋`ZHIPUAI_API_KEY`）：**Satisfied、2 artifacts、124 秒**。PLAN／EVAL 軌跡（4 筆）實證循環會自我修正——第 1 輪 think 回空（查詢「晶瀚半導體 會議 記錄 重點」）→EVAL 正確判「僅標題、未完成」→PLAN 換詞「晶瀚半導體 會議」→第 2 輪撈出 11 場會議、EVAL 判 done。產出品質高（蝕刻品質事件會議總覽，含與會者／時間軸／根因深化歷程）。驗證揭露三觀察（非阻斷）：①think 對查詢詞檢索敏感（回空但循環自癒）；②頁尾恆 `Graph:0/Citations:0` 即使有內容（gbrain v0.42 計數器瑕疵）→循環無「檢索是否成功」機讀訊號；③第 1 個空 artifact 也被 commit（無剪枝）。 | A（循環魯棒性）＋ B（Artifact 生命週期）——見待處理清單 | 真實環境驗證已完成；A/B 待實作 |
| 2026-08-14 | —（A＋B） | **自主循環魯棒性（A）＋ Artifact 生命週期（B）落地**——T1 三觀察的對應修復。**A**：新增 `is_barren_think_output`（以 think 頁尾 `\nModel:` signature 把關、量主體非空白字元 < `BARREN_MIN_CHARS=30` 判貧瘠；缺 signature 的 stub／其他工具 fail-safe 不判）；`run_autonomous` ACT 後偵測貧瘠→不 commit、記 `barren` 事件、留 note 換詞重 PLAN，連續 `MAX_BARREN=3` 才 Stalled。貧瘠產出從此不入庫（不靠 EVAL 事後發現）。**B**：`commit_artifact` 重構為 `commit_artifact_with_status` 核心＋Committed 包裝；自主循環探索期產出 commit 為 **Draft**，唯承諾 **Satisfied** 才 `promote_artifacts_to_committed` 晉升 Committed（Handbook Ch.06「工作成真的一刻」）；Stalled／Errored 的探索產出維持 Draft（不曾成真，與已完成工作可區別）。單發路徑（run_cycle／對話）仍走 Committed。`cargo test` **107 全綠**（+4 新測：is_barren 分類、貧瘠 Stalls、貧瘠後恢復、Stalled 維持 Draft）、7 ignored、0 新 warning。 | 可選：重跑 `real_run_autonomous` 觀察 A/B 在真實環境行為 | — |
| 2026-08-14 | —（A/B 真實環境複驗） | **重跑 `real_run_autonomous` 觀察 A/B**：Satisfied、**1 cycle、93.6s**（較 run 1 的 2 cycle／124s 快——LLM 第一輪即選對查詢，非 A 功勞）。**B 確認**：artifact 印出 `[Committed]`——探索期 Draft→Satisfied 晉升 Committed 正確。**A 本次未在 live loop 觸發**（LLM 選了好查詢）。直接重跑舊的「回空」查詢 `晶瀚半導體 會議 記錄 重點`：gbrain **今早回真空（0 body）、現在回冗長負向答案**（「未找到…最接近的是華晶集團聚晶半導體」＋Gaps，~200 字、Citations:3）——證實 gbrain 檢索**非確定性**。此發現驗證 A 的層次劃分正確：A 抓 **0-body 空**（便宜結構捷徑：不入庫、跳 EVAL、換詞）；**冗長負向**（語意「其實沒找到」）流到 EVAL 判（不複製語意分析）。兩者互補。A 由 3 個測試覆蓋（含 SwitchingStub 恢復）。 | — | A/B 真實行為已觀察；gbrain 檢索非確定性屬 gbrain 側 |
| 2026-08-14 | —（E3） | **退役 deprecated `sync_models_to_chat`（file-plane 殘值同步）**。R1 的 DB-plane 路徑（`set_gbrain_models_all`／`sync_new_brain_models` step 1）早已完全取代它——v0.42 runtime 以 DB plane 為準，file-plane 的 `models.*` 是無作用的殘值。移除：①`sync_models_to_chat` 函式＋doc＋`#[deprecated]`（gbrain_config.rs）；②`save_gbrain_config_raw` 的呼叫＋`#[allow(deprecated)]`（config/mod.rs）——raw 編輯器現如實存使用者輸入；③`sync_new_brain_models` 的 file-plane step 2（brains.rs），僅留 DB-plane step 1；④4 個 `sync_*` 測試＋測試模組 `#![allow(deprecated)]`。**刻意保留**：`models_default_of`（設定頁顯示用 read helper）、`tier_of`（dead-code）、ConfigView raw 警告（E3 後仍正確）。原列「anthropic base URL hard-code」風險經查為 gbrain 自身 fallback（非 Operoid 碼），E3 不碰 URL。`cargo test` **103 全綠**（-4 測試）、7 ignored、0 新 warning、無殘留 `#[deprecated]`／`allow(deprecated)`。 | — | — |
| 2026-08-14 | —（E7 步驟 1＋2） | **外部事件 ingress：契約擴充 ＋ E7 endpoint 落地**（設計見 `docs/Operoid-設計-統一事件ingress契約.md` v0.2）。**步驟 1（契約）**：`InboundEvent` 收斂為薄外殼——`summary`→`content`（持整封原生訊息，bridge 序列化 From/To/Cc/Bcc/本文/上下文）、加 `external_ref`（去重鍵）／`occurred_at`／`Deserialize`、`review_prompt` 依 kind 分枝。討論關鍵：BCC＋IM 流證明結構式通用欄位是 lowest-common-denominator 陷阱 → 來源特有資訊全併入 content、員工（LLM）讀了判斷（見 `docs/Operoid-設計-事件bridge與來源差異.md`）。factory 兩處建構點同步；連帶消滅 `ExternalMessage` dead-code 警告。**步驟 2（E7）**：新檔 `ingress_server.rs`（鏡像 `note_server`）——`POST /event`＝Bearer 認證→`Json<InboundEvent>`→`(source,external_ref)` 去重→`dispatch_event`。`AppConfig` 加 `event_ingress_port`／`event_ingress_secret`（皆 None＝停用、opt-in）；`AppState` 加 `seen_external_refs`（session 內、邊界化 8192）；`lib.rs` setup 接線（port＋secret 皆有設才啟動）。E7 三待決定案：固定 port（127.0.0.1）／shared-secret／完整 body。`cargo test` **110 全綠**（+7：review_prompt 兩分枝、JSON 反序列化、check_auth 四情境）、7 ignored、0 新 warning。元件層已測；**完整 HTTP 路徑待手動 e2e**（curl 步驟見待處理清單 E7）。 | 步驟 3：Email bridge（外部程式）＋ 手動 e2e 驗證 | ingress server 未經 GUI app 實跑驗證 |
| 2026-08-15 | —（E7 outbound 補） | **outbound 補「可不回覆」**（使用者提問 1 的缺口）：reasoner schema 加 `kind=none`——員工判斷訊息無需回應（純通知／與職責無關）→ 不寫 Out message、不外發（即使 outbound 已啟用）、記 `silent` 事件、task 照常完成。修前形狀：answer/ask/propose 三 kind 一律回覆外發，Email bridge 上線後每封群發信都會被回。「要不要回」是內容判斷（Principle 10），歸員工。`cargo test` **115 全綠**（+1：none 不回不發）。使用者提問 2（群發回覆其他收件人）討論後留給 E12 send Tool。 | Email bridge（步驟 3）＋ E12 | — |
| 2026-08-15 | —（E7 outbound） | **E7 outbound v1 落地（回覆式自動觸發、免核可）**。使用者決策：**不需人類核可**——outbound 是通用通道（Email／IM／未來 ERP/MES），逐一核可失去自動化效益（推翻原「傾向核可」記載）。契約：`InboundEvent` 加 `reply_to`（bridge 自訂不透明錨點，機讀、不進 prompt）＋ `Task.external_reply_to/external_source` 成對（serde default，零 schema 遷移）；`dispatch_event` 填入。實作：新檔 `outbound.rs`（`send_reply` POST `{source, reply_to, employee_id, text}` 給 bridge、Bearer、10s timeout；`OutboundConfig` 沿 runtime 傳遞鏈下傳）；`run_conversational_turn`（含 no-reasoner fallback）寫完 Out message 自動外發，記 `outbound_sent/failed` 事件、失敗不重試不阻斷；`AppConfig.event_outbound_url/secret` opt-in。多員工並發正確性：每事件各帶錨點→各 Task 帶回，e2e（axum stub bridge）實證兩員工兩訊息回覆無交叉。**附帶修既有 bug**：`record_event` 事件 id 只在該員工範圍去重→跨員工撞 id 互覆（Json/SqliteStore 皆然），id 前綴改帶 employee_id。`cargo test` **114 全綠**（+4）、7 ignored、0 warning；`npm run build` 0 error。Outbound v2（完整 send Tool＋tool-choice）→ E12，排 Email bridge 之後。 | Email bridge（步驟 3）＋ E12 | — |
| 2026-08-16 | —（Obridge 生命週期） | **obridge 啟動時機＋設定生效機制（A+B，使用者定案）**。**B 熱重載**：obridge 內建 watch `obridge.toml` mtime（2s 輪詢，零依賴）——變更即 abort 舊通道 tasks＋整組重建（Registry 改 `Arc<RwLock>` 供 send endpoint 每請求重讀；進氣 mpsc 全域共用；`[[channels]]` 即改即生效；listen/operoid 區段需重啟；解析失敗沿用舊設定）。**A 子進程代管（opt-in）**：`AppConfig.obridge_autostart`＋`obridge_executable`——Operoid 啟動 spawn obridge（Windows CREATE_NO_WINDOW）、`RunEvent::Exit` 帶走；設定頁存檔後自動重啟子進程（涵蓋非熱重載區段）；未開 → 使用者自管。前端：設定頁 app 區塊加 autostart 開關＋執行檔/設定檔路徑輸入（i18n 三語）。`cargo test` **136 全綠**（+1：熱重載整組替換註冊表）、0 warning、npm build 0 error。 | 真實信箱手動驗證 | — |
| 2026-08-16 | —（Obridge 補） | **外掛設定傳遞＋GUI 代管 obridge.toml**（使用者討論定案：補設定＋GUI 代管）。①WIT `channel` 加 `init(config)`——host 每次實例化傳入 `[[channels.wasm.config]`（任意 TOML→JSON，薄外殼：結構由外掛自訂）；`[[channels]] type="wasm"`（plugin 路徑＋poll_secs＋config）聲明式配置外掛通道；範例 echo 外掛示範 init 讀 `greet-name`（往返測試實證設定生效）。②Operoid 設定頁「Obridge 設定」區塊：`AppConfig.obridge_config_path` 指向 obridge.toml → 原始文字編輯＋原子寫（`obridge_cfg.rs` 兩指令；Operoid 只當編輯器不解讀內容——守抉擇二）；未設路徑顯示指引；存檔需重啟 obridge（無熱重載）。i18n 三語。`cargo test` **135 全綠**（+2）、0 warning、`npm run build` 0 error。 | 真實信箱手動驗證 | — |
| 2026-08-16 | —（E7 步驟 3） | **Obridge（Operoid Bridge）落地——Email bridge ＋ Hybrid 模組架構＋WASM 外掛機制**。討論定案：同 repo workspace member（非另開專案——契約型別共享、編譯期防漂移）／Rust／雙向一次做／錨點無狀態／**WASM 外掛體系（wasmtime 47＋component model）＋email 內建 native（Hybrid）**——原始協定（IMAP/SMTP）內建、HTTP API 類通道（Slack/Teams/Graph-Exchange）未來全走外掛（host 僅 make-request/kv/clock，刻意不含 TCP/TLS）／名稱 obridge。實作：**workspace 升級**（root Cargo.toml，members 四 crate，零破壞）；**`ocontract`** 共享契約 crate（InboundEvent/EventKind 上移＋SendPayload；src-tauri re-export 零改動）；**`obridge`**（config toml＋Channel trait/Registry＋ingress client＋axum send endpoint（Bearer、依 source 分派）＋email_imap 通道（imap crate＋lettre；MailSource/MailSink 抽象供離線測試；UIDVALIDITY+last_uid 去重狀態檔；錨點 `<source>:msg:<msgid>?to=<addr>` URL 編碼）＋WASM 外掛載入（wasi-p2 最小能力沙箱））；**`obridge-plugin-example`**（echo 外掛，wasm32-wasip2）＋往返測試通過（載入→poll→send）。`cargo test` **133 全綠**（ocontract 1＋obridge 9＋src-tauri 123）＋3 ignored（真實信箱×2＋wasm 往返）、0 warning；`npm run build` 0 error。設定範本 `obridge/config.example.toml`。 | 真實信箱手動驗證（smoke＋全鏈 e2e：Operoid↔obridge↔信箱） | ingress/outbound 之真實通道未驗 |
| 2026-08-16 | —（E12） | **Outbound v2 落地——完整 send Tool（tool-choice 編排），提前於 Email bridge 實作（stub 驗證）**。使用者討論定案六點：tool-choice 編排／payload `to` 自由字串（薄外殼）／**統一走 send Tool（v1 自動外發移除）**／對話回合 tool-loop／守門僅「未啟用回報員工」／提前做＋stub e2e。實作：`ToolInput` 加 `params`（工具特有參數，既有工具零影響）；`outbound.rs` 加 `SendTool`（`send-external-message`，每回合 capture 來源＋錨點；`to` 缺省回退錨點、明示即新目標；主動發送需明示 `source`；未啟用→回報員工非靜默）＋payload `reply_to` 正名 `to`；`run_conversational_turn` 重寫為**回合內 tool-loop**（think/send/propose/finish，上限 6 步；silent 語意保留＝finish 無 text 且未 send；propose 後 finish 掛 proposed_commitment_id 核可鈕相容）；`run_autonomous` PLAN 可選 send（主動通知人類）→ ACT 走 SendTool、記 note、循環續跑。無 Reasoner 退化路徑仍自動外發（runtime 代發的既有退化語意）。`cargo test` **123 全綠**（+8）、7 ignored、0 warning。多員工錨點不交叉保證不變（e2e 改經 send 動作實證）。Handbook 檢查：不需改（send 走 Tool 符合 Ch.08）。設計見契約文件「十二」。 | Email bridge（E7 步驟 3；上線後手動複驗 outbound v2） | — |
| 2026-08-14 | —（E7 e2e 驗證） | **E7 ingress 手動 e2e 驗證通過**。設 `app-settings.json` 的 `event_ingress_port=17341`／`event_ingress_secret`（測後已還原）→ `npm run tauri dev` → log 見 `[ingress] 進氣口就緒：127.0.0.1:17341/event`。curl 三情境全綠：①有效認證首投 `202 accepted`、②同 `external_ref` 重投 `200 duplicate; ignored`（去重）、③無 Authorization `401 bad credentials`。dispatch 亦正確觸發（`事件〈RE: E-07 良率〉路由命中 0 名員工`——runtime DB 之 brain 上無員工，屬資料條件非缺陷）。**E7 ingress HTTP 路徑（新碼）端到端驗證通過**；下游 dispatch→喚醒→對話為既有已測碼，未重驗（需 seed 員工＋打 LLM）。收尾：關 app、移除進程、還原 config。 | Email bridge（步驟 3）＋ outbound 設計（自動寄出 vs 核可後寄出） | — |
| 2026-08-18 | —（D5 規劃） | **前後端分離計畫定案**（待處理清單 R2 展開為專檔 `docs/Operoid-計畫-前後端分離.md`）。使用者四項定調＋三項推薦採納＝六決策（DR1–DR6）：單機服務先行／全部搬／shared token／階段混合啟動（P5 `oserver install` 服務註冊為交付物）／同 repo workspace（`ocore`＋`oserver`）／`operoid.toml`＋SQLite。P1 抽 `ocore`（純重構，含 `Channel<CliLine>` 手術＝E8 前置）→ P2 `oserver` 骨架＋讀取面 API → P3 寫入面＋前端切 HTTP → P4 GBrain 能力域 → P5 服務註冊＋ingress 整併＋設定遷移。刻意不做：SSE／RBAC／第二前端／多租戶。守則自檢：不需改 Handbook。 | P1 抽 `ocore` | — |
| 2026-08-18 | **P1–P4 ✅** | **前後端分離 P1–P4 一日完成**（詳見專檔進度紀錄）。P1a–d 抽 `ocore`（零 Tauri 依賴；`Channel<CliLine>`→`LineSink` 手術＝E8 前置；137→141 測試全綠）；P2 `oserver` 骨架（bind-first/healthz/AuthProvider trait＋TokenProvider/讀取面 API；設定直讀 app-settings.json——Roaming 設定與 Local DB 兩路徑）；P3 寫入面（ocore 13 核心函式；`employeeBusy`→409、交辦→202 背景喚醒；GUI 不再自跑 scheduler；前端 19 wrappers 切 HTTP；**e2e 實證無 GUI 下員工被喚醒自主運行**）；P4 GBrain 能力域（gbrain_cfg 核心＋operations ring buffer 輪詢主控台＋全套 endpoints；前端 24 wrappers 切 HTTP——`src-tauri` 僅剩殼）。過程修復：CORS preflight（POST 誤報 offline）、啟動競態重試。 | P5 | — |
| 2026-08-19 | **P5 ✅ — 分離完成（v0.3.0）** | 服務註冊三平台（Windows SCM＋UAC 自我提權＋install 自啟，實機驗證；Linux systemd/macOS launchd——CI 編譯覆蓋、未實機驗證）；生命週期雙語意 A1（服務自啟）/A2（GUI 帶起帶走）；設定頁「本地服務」開關；ingress 併入 `POST /event`（token＋去重）——GUI 不開 obridge 投件不再掉；修訂：operoid.toml 延後（app-settings.json 單一來源）、note_server 留殼。**Windows 實機 e2e 全過**（install/UAC→自啟→ready→API→ingress 202/duplicate→uninstall 乾淨）。版號 v0.3.0＋CHANGELOG＋tag（tag 因 CI 跨平台 bug 待重打，見 08-20 列）。 | 使用者驗收＋release | — |
| 2026-08-20 | —（v0.3.0 收尾 sprint） | **v0.3.0 收尾系列**。① **CI 跨平台修復**：oserver Cargo.toml 的 `[target.'cfg(windows)']` 誤插 `[dependencies]` 後——全部依賴被吞成 windows-only（Windows 恰好通過；macOS/Linux 全滅）→重排修復（`7f5920d`）。② **prereq 翻轉設計**（使用者定調：路徑已知者不該「執行看看」證明）：bun/gbrain 以檔案存在判斷、版本走 `prereq_cache` 快取背景刷新——實證 **1000ms→60ms**，gbrain（bun 冷啟可達 20s）不再是啟動成本（`ecbd2fd`）；先修的 LocalSystem PATH fallback 隨之退役為次要路徑（`9e3b9d5`）。③ 白底閃爍：視窗 backgroundColor #09090b（`5ab175d`）。④ **obridge 上線**：使用者本機測試 email server——範本佔位主機（imap.corp.com→DNS 陷阱 127.0.53.53）改 mail.example.test、ingress 指向 7340/event＋server_token、自簽憑證加 `tls_insecure` 選項（`25bea38`）——Email 全鏈路（收信→事件→喚醒）通。 | 使用者驗收→重打 tag v0.3.0→三平台 CI→release publish | gitea 間歇逾時待補推；Linux/macOS 服務未實機驗證 |

**目前所在：** 🎉 **Phase 7（人機協作）完成——7a 交辦＋7b 聊天＋7c 員工提案核可全數落地。** 人機介面從單發 Q&A 升級為完整協作：雙向多次聊天（Reasoner 驅動回覆、可反問）、交辦承諾（立即喚醒自主跑）、員工在對話中主動提案承諾（Proposed）待人類核可（Active）。Handbook 新增 Message 一級概念（Ch.16，Part IV 重編）＋修訂 Ch.11（Proposed/Rejected）＋Ch.20 §5（提案-核可通用化）。**Phase 6＋7 全部完成**——員工生命週期＋人機協作的完整願景已兌現。

**Event 匯流排（2026-08-12～15）**：實作 Handbook Ch.12 第四種 Trigger——**Event-driven**。factory 寫入（會議記錄/people/companies）完成後，具備對應腦的員工自動被喚醒 review，產生回應或提案承諾（待人類核可）。附帶 LLM 全域並發節流（Semaphore）＋ inbox 佇列延遲修復。**E7 ingress（2026-08-14）**：`POST /event` HTTP 進氣口（Bearer＋去重）落地並 e2e 驗證——Email/IM bridge 的統一投遞 API。**Outbound v2（2026-08-16，E12）**：完整 send Tool（tool-choice 編排）——外發統一是員工的行動：對話回合 tool-loop（think/send/propose/finish）＋自主循環可主動通知人類；payload `to` 缺省回退錨點、明示即新目標（薄外殼）。Email bridge（步驟 3）為後續。

**自主循環真實環境打通（2026-08-12～14，E2＋E9＋T1）**：E2 加 PLAN／EVAL 診斷軌跡（`record_event` 每輪記 plan/eval）＋ PLAN prompt 強化（看見 artifact、鬆綁重複偵測）；軌跡揭露真根因——gbrain think synthesis 因 DB-plane model fallback 到 anthropic 而缺 LLM（E9）。修法：`GbrainThinkTool` 顯式 `--model`（零副作用）。**T1 端到端驗證通過（2026-08-14）**——`real_run_autonomous` 實跑 Satisfied、2 artifacts、124 秒，軌跡實證循環自我修正（think 回空→換詞→收斂）；產出品質高（11 場會議總覽）。v1 核心能力的最後阻斷清除並經真實環境確認。驗證亦揭露三觀察（A＝循環魯棒性、B＝Artifact 生命週期）：think 檢索對查詢詞敏感且無「檢索成功」機讀訊號、貧瘠產出仍入庫無剪枝。

**前後端分離完成（2026-08-18～20，P1–P5，v0.3.0）**：`ocore`（純 Rust 核心，零 Tauri）＋`oserver`（常駐服務：axum HTTP API、token 認證、agent-os 讀寫面、GBrain 全域、ring-buffer 主控台、`/event` ingress、三平台服務註冊）＋桌面殼（視窗＋桌面專屬能力，前端全走 HTTP）。生命週期雙語意：裝服務→開機自啟（GUI 關閉不影響）；未裝→GUI 帶起帶走。計畫全貌見 `docs/Operoid-計畫-前後端分離.md`（DR1–DR6＋版次策略：個人/企業差異在發佈與組態層，認證 provider 插座已種）。

**D3 GUI 首版（2026-08-01）**：Agent-OS 首次有可見 UI——員工模板（1:1 綁腦、CRUD）、員工實體（視窗卡片＋右鍵管理、個別命名如 Steve@TW）。待你 `npm run tauri dev` 視覺驗證。

**CSP 事件與分離部署記錄（2026-08-22）**：NSIS 安裝版啟動後待辦全空、按鍵凍結——根因是 `tauri.conf.json` CSP 無 `connect-src`，打包版擋掉前端對 oserver 的 `http://127.0.0.1` fetch（dev 模式不套用 CSP，故 dev 正常）。修法：CSP 補 `connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:*`，重建安裝檔。教訓已轉為待處理清單 **E14**（GUI 與 oserver 跨機器分離部署的四個必改點，含最易漏的 CSP），並登錄為決策 D6（延後）。
