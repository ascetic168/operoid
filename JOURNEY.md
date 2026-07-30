# Emploid 實踐藍圖與進度（Journey & Progress）

**Version:** 0.1
**Status:** Living document — 持續更新
**建立日期：** 2026-07-30
**最後更新：** 2026-07-30

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
  - [ ] 一個 Template 產出多個獨立 Instance（「每座廠一個 Steve」），各追蹤各自的現實。

## Phase 5 — 協作（≡ Handbook Milestone 5）

- **目標：** 多 Employee 在 Project 內合作：交接 Task、共享 context、產出共享 Artifact，彼此不互相擁有。
- **退出條件：**
  - [ ] 一個 Project 由一隊 Employee 並行＋循序完成。

## Horizon — v1 之後

Skill learning、cloning/parallelism、marketplace、federation、distributed runtime、人機團隊——詳見 `handbook/20-Roadmap.md §7`。現階段**不展開**；重點是：這些都不需要新核心概念，是對既有概念的延伸。這正是好架構的檢驗。

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
| 2026-07-30 | Phase 3 驗證 | 真實 `real_shared_brain`：兩員工（emp-a／emp-b）共用 demo 腦各跑 think → Graph 1、Pages 21、Citations 12／1、groq；各產 artifact、memory 獨立。共用腦＋獨立狀態端到端正確。 | Phase 4（Template 與 Instance） | D3（延後） |

**目前所在：** Phase 3 完成（程式碼＋測試＋真實 gbrain 驗證）。下一步 = Phase 4（Template 與 Instance）。
