# 技能庫

> 適用版本：技能系統自 v0.5.10 起、技能圖書館（catalog）自 v0.5.11 起。技能是程序性知識，**不是授權**——技能建議的任何動作仍在員工工具允許清單與行為準則內執行。

## 四區模型

「技能庫」頁由四個區組成，權限各自不同：

| 區 | 位置 | 誰能寫 | 會進員工提示詞嗎 |
|---|---|---|---|
| **員工草稿區**（drafts） | 工作區 `skills/` | 員工自己 | 會 |
| **個人區**（personal） | 個人版預設 `~/.agents/skills`（與 Claude Code／ZCode 等生態共用同一目錄） | 只有你（員工唯讀） | 會 |
| **公司共用庫**（library） | `<data_dir>/skills/library/` | （企業版）管理員晉升後組織共用 | 會 |
| **技能圖書館**（catalog） | `<data_dir>/skills/catalog/` | 唯讀——出廠內建書架 | **不會**——採用前不佔任何提示詞 |

個人區設為空字串（`skills_personal_dir = ""`）＝企業模式：個人區改為 `<data_dir>/skills/users/<principal>/`，逐主體隔離。

## 技能圖書館：瀏覽與採用

出廠即附策展技能目錄：**11 個外掛、111 個職能技能**（productivity、sales、customer-support、product-management、marketing、legal、finance、data、enterprise-search、operations、human-resources），內容導入自 [anthropics/knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins)。oserver 啟動時自動種子進圖書館；你刪掉的外掛不會被復活。

操作：

- 頁面依**外掛分組**（可摺疊）列出：drafts → 個人區 → 公司共用庫 → 技能圖書館（自 v0.5.11 的排序）。
- 點技能卡可**直接檢視 SKILL.md 內容**——先看內容再決定。
- **「加入」**：把技能從圖書館複製進你的個人區——只有「加入」後，AI 員工才會載入它。
- **「移除」**：從個人區移除（兩次點擊確認）。

## union 上限：`skills_union_limit`

單一員工實際載入的技能數（草稿＋個人區＋共用庫合計）不得超過上限，**預設 30**（`0`＝不設限，不建議）。這是提示詞面積的硬閘：到達上限時，任何「新增技能」動作——員工建草稿、你採用、管理員晉升、提案核可——都會被拒絕，直到先移除其他技能。圖書館書架**不計入**（未採用的技能不佔提示詞）。

## 技能是怎麼誕生的：提案流程

只有人類會移動技能檔案——員工永遠不能直接發佈：

1. **員工寫草稿**（在工作區 `skills/<名稱>/SKILL.md`）。
2. **員工提案**（`propose_skill` 工具）：系統先跑**確定性 linter**（格式檢查），再由**獨立子代理預審**。
3. **主體（你）核可**：提案出現在「待辦」，核可後伺服器把技能複製到你的個人區。
4. （企業版）**管理員晉升**：把個人區技能晉升到公司共用庫，全組織可用。

技能格式：一個目錄＋`SKILL.md`（YAML front matter 的 `name`／`description`＋本文＋選附腳本）。它也可以直接放進 `~/.agents/skills` 與其他工具共用——同一份技能，Claude Code／ZCode 與 Operoid 的員工都能讀。

---
[← 10 待辦與事件](10-inbox-and-events.md)｜[索引](README.md)｜[12 知識庫與 Claude Code →](12-knowledge.md)
