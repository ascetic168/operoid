# 腦與模型設定

> 適用版本：v0.5.11。「腦」的概念與定位見[附錄 A](appendix-a-gbrain.md)；本章講操作。

## 「腦」頁

一顆腦＝一個 GBrain 知識本體（磁碟上是一個 `.gbrain` 目錄及其父目錄）。員工的知識查詢都發生在其模板對應的腦上。

### 註冊與切換

- **新增** → 二選一：「登錄既有」（指向現有 `.gbrain` 的父目錄）或「建立新腦（gbrain init）」。
- 建立新腦時可指定：名稱、路徑、embedding model 與維度、chat model——留空則用預設（embedding 預設 `llama-server:embeddinggemma-2`，768 維；「設定 → 模型」的「新腦預設 embedding 模型」可改，僅影響新建腦）。
- **設為作用中**——工廠寫入、知識庫操作都以作用中腦為對象。
- 預設腦不可移除。

### 來源（sources）

腦的知識來自一個或多個 **git repo** 來源（工廠寫入的頁面就在來源 repo 裡）：

- 「新增來源」：填來源 id（短名，如 `notes`）與 git repo 路徑。
- 清單欄位：id、路徑、頁數、上次同步；每列可「同步」「設作用中」。
- 「同步全部」：對所有來源執行 sync（commit + sync + embed + extract）。
- 「綁定目錄」：將來源綁到本機路徑。
- **移除來源**時二選一：**封存**（從搜尋隱藏、保留 72 小時，可還原）或**永久刪除**（立即移除全部頁面／向量／檔案，不可逆）。

## 「設定 → 模型」

GBrain 的 `config.json` 是權威來源；這頁的控件會直接寫入它（以及 DB plane）：

| 區塊 | 內容 |
|---|---|
| **主模型** | 一個 `provider:model`（如 `zhipu:glm-5.3-flash`）同步套用到對話與四個 tier |
| **模型路由（tier）** | 依任務分級：Utility（摘要等輕任務）、Reasoning（思考／抽取）、Deep（深度分析）、Subagent（子代理迴圈）。可「全部 tier 用同一個模型」 |
| **Provider base URL** | 直寫 config.json 的端點覆寫（file-plane） |
| **新腦預設 embedding 模型** | `provider:model`＋維度；僅影響新建腦 |
| **gbrain 傳輸** | `MCP`（預設——員工知識工具走 `gbrain serve`，失敗自動退回 CLI）或 `CLI`（純子行程） |
| **LLM 端點解析** | 顯示 API key 狀態（「API key 已設／缺 API key（環境變數）」） |

注意事項：

- **DB plane 覆寫**：部分鍵可能由 DB plane 生效（頁面會列出「由 DB plane fallback 生效」的鍵）——改模型請用本頁控件，不要直接改 `config.json`；「清除所有 DB 覆寫」可歸零。
- **schema pack 提示**：若作用中腦仍是 v1（6 工廠），頁面會建議執行「unify-types」遷移到 v2（15 標準型）——見[第 09 章](09-factories.md)。
- Subagent tier 僅 Anthropic 模型支援 prompt caching；其他 provider 的子代理迴圈成本隨對話長度線性增（頁面會警告）。

## 「設定 → 服務」

- **知識管線能力**：能力矩陣（git／bun／gbrain／MinerU／嵌入端點／多模態／VLM）＋「重新檢查」。MinerU 可在此填**安裝目錄**（`mineru_dir`，支援 `%USERPROFILE%`／`${HOME}` 展開，跨機器可攜），儲存後即時重測。
- **本地服務（oserver）**：port（預設 7340）、運行中／未運行、**開機自動啟動 Operoid 服務**的註冊／解除——見[第 15 章](15-service-and-updates.md)。
- **Obridge**：設定檔路徑、執行檔、隨 App 啟動——見[第 14 章](14-email-bridge.md)。

## 其他設定分頁

- **應用程式**：介面語言、路徑（notes repo、gbrain.exe、GBRAIN_HOME 覆寫）、LLM 溫度／max_tokens、工廠自動 sync、Agent-OS 開關、Obridge、版本與更新——分散見[第 15 章](15-service-and-updates.md)與[第 17 章](17-config-reference.md)。
- **登記表**：動作類別登記表（委任界線）——見[第 13 章](13-governance.md)。
- **知識授權**：知識政策、principals、TTL grants——見[第 13 章](13-governance.md)。
- **進階**：`config.json` 原始編輯器。⚠️ model／tier 鍵可能被 DB plane 覆寫——請改用「模型」分頁的控件編輯。

---
[← 06 桌面應用導覽](06-desktop-tour.md)｜[索引](README.md)｜[08 員工管理 →](08-employees.md)
