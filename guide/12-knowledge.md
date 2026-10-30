# 知識庫與 Claude Code

> 適用版本：v0.5.11。圖表級檢索自 v0.5.0 起。GBrain 的架構說明見[附錄 A](appendix-a-gbrain.md)。

## 「知識庫」頁

這頁把 gbrain 指令包裝成按鈕、串流輸出——绕過員工、直接操作作用中腦：

### 查詢

| 操作 | 做什麼 |
|---|---|
| **ask** | 混合檢索——對腦提問，取回相關頁面 |
| **query** | 混合檢索（ask 的新一代指令） |
| **think** | 多跳合成——可用 `anchor:slug` 換行後接問題，以指定頁為錨點推理 |

檢索不只回文字：命中含圖頁面時，回覆會帶上**檢索到的圖表**與**來源 PDF**（圖表向量＋VLM 圖說的成果，見[第 02 章](02-requirements.md)）。

### 維護與診斷

| 操作 | 做什麼 |
|---|---|
| **sync** | commit + sync + embed + extract——寫入後的完整同步 |
| **extract** | `extract --stale`——補抽落後的圖譜 |
| **stat** | 腦統計 |
| **doctor** | 健康檢查 |
| **orphans** | 找出無入邊的孤立頁 |
| **storage** | 層狀態 |
| **graph-query** | 輸入 slug，看某實體的圖譜邊 |
| **重建 companies** | 從 people 重建公司頁（可加 `--clean`） |
| **unify-types** | schema pack v1→v2 遷移（見[第 09 章](09-factories.md)） |

## Claude Code 整合

左側導覽底部的「開啟 Claude Code」按鈕，會以**當前腦的 MCP 伺服器**啟動 [Claude Code](https://claude.com/claude-code)——讓你用終端機裡的 coding agent 直接查寫這顆腦：

- **腦**：選哪顆腦（think / search 涵蓋此腦全部來源）。
- **終端機**：Windows Terminal／cmd／PowerShell，或自訂指令範本（`{cwd}`／`{cmd}` 佔位字元）。
- **工作目錄**：Claude Code 編輯檔案的位置（不影響腦的知識）。
- 啟動前會自動檢查：`claude` CLI 是否在 PATH、gbrain 是否就緒。

⚠️ **重要披露**（對話框內原文）：「此通道可讀寫整顆腦（不受知識範圍限制）；對話與檢索到的知識內容會送往雲端的 Claude 端點。」——Claude Code 對所選腦有**完整讀寫**（含刪頁等寫入操作），誤改可靠 gbrain 版本歷史回復。個人版這是合理預設；**企業場景不應依賴此通道**（可以 `claude_code_handoff_enabled=false` 整個退役），請改用受治理的員工能力。

## 員工怎麼查知識

員工的知識工具（think／search）走同一顆腦，但**經過治理**：企業模式依知識政策在檢索前授權（見[第 13 章](13-governance.md)）。傳輸方式由「設定 → 模型 → gbrain 傳輸」決定：MCP（預設，`gbrain serve` 子行程，失敗自動退回 CLI）或純 CLI。對話裡的每次知識查詢都會顯示在工具呼叫軌跡中（見[第 08 章](08-employees.md)）。

企業模式的主控台檢索操作可被 `ops_retrieval_enabled=false` 停用——檢索一律走員工工具，管理面不繞過治理。

---
[← 11 技能庫](11-skills.md)｜[索引](README.md)｜[13 治理 →](13-governance.md)
