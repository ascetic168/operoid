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
