# TODO — 待辦與功能分析

> 本文件記錄待實作的功能分析與已知問題。語言慣例比照 JOURNEY.md（繁中）。
> 建立日期：2026-10-17。背景：contacts.csv（2,009 筆）重新匯入後發現 CSV 匯入路徑無 timeline，
> LLM 路徑（txt/md）經 3 頁抽樣 + 464 頁批次驗證有效（詳見各項目）。

---

## 1. CSV 匯入無 Timeline — 智慧 LLM 路由（推薦：heuristic gate + 背景批次富集）

### 現況與問題
- `Pipeline::People` 的 CSV 分支（`csv_people::parse`）為純解析：無 LLM、不產生 timeline。
  txt/md 分支（`text_to_md`）才有 LLM 結構化＋timeline（含 2026-10-16 的日期驗證修正 fe082e1）。
- 對通訊錄而言，Notes 常含「某年某月某日在何處見面」這類高價值事件訊息，純解析全部丟掉很可惜。

### 使用者提議
匯入時辨識內文是否含日期文字（YYYY/MM/DD、YYYY-MM-DD、X月Y日…），需要者才走 LLM。

### 推薦做法
- **Heuristic gate**：CSV 解析完成後，掃描 Notes（與其他自由文字欄位）比對日期 pattern；
  命中者併入「LLM 富集佇列」，未命中者維持純解析頁面。
- 建議 pattern（由保守到寬鬆分期）：
  1. `YYYY-MM-DD`、`YYYY/MM/DD`、`YYYY年M月D日`（年月日齊全，誤判率最低）
  2. `M月D日`（補推斷年份，需註明推斷）
  3. `民國YYY年`、孤立 `YYYY` 年份＋事件動詞線索（會面/拜訪/meet/簽約…）
- **不阻塞匯入**：匯入本身照舊秒級完成（純解析先落盤）；富集佇列背景批次跑（每列一次 LLM 呼叫）。
- LLM 呼叫沿用 `text_to_md` 的 prompt 與 YYYY-MM-DD 驗證（dropped 計數入摘要）。
- 成本估算（實測，2026-10-17）：464 頁批次約 25 分鐘（併發 4、ZhipuAI），失敗主因為
  供應商內容過濾器誤判（2/467，重試無效）——需在摘要回報並列出失敗清單。
- 富集覆寫與凍結保護相容：已驗證 factory-owned 頁可被 LLM 路徑覆寫（enriched 標記頁不受影響）。

### 驗收條件
- CSV 匯入後，含日期 Notes 的聯絡人頁自動帶 `<!-- timeline -->`（YYYY-MM-DD 合法），
  摘要顯示「待富集 N 列／已富集 M／失敗 K（清單）」。

---

## 2. 匯入註解（Import Annotation）→ 自動 Timeline 錨點

### 使用者提議
CSV 匯入時要求（或預設）註解，如「從 Google Contacts 匯入的聯絡人」；Operoid 自動補上
匯入日期＋註解作為 timeline 事件。引申：任何文件匯入時，使用者可「選擇性」給註解。

### 推薦做法
- UI：工廠拖放流程加「註解」欄——一般工廠選填；people CSV 建議帶預設值
  「從 Google Contacts 匯入」（可改）。
- 寫入格式：timeline 區塊首位插入 `### YYYY-MM-DD — 匯入：<註解>`（gbrain timeline parser 相容）。
- 保證每頁至少一筆 timeline 錨點（即使無任何日期事件），gbrain timeline 視圖不空白，
  且可追溯資料來源與匯入批次。
- **與凍結保護的互動需設計**：重匯時舊頁已含上次「匯入」事件，直接覆寫會遺失舊錨點。
  建議：匯入事件以「(匯入來源, 日期)」內容去重，重匯時保留舊錨點、只補新錨點；
  或視為 enriched 的一種（首匯後標記，重匯 freeze）——擇一，需討論。

### 驗收條件
- 帶註解匯入後，每頁 timeline 含「匯入」事件；重複匯入不產生重複錨點、不遺失舊錨點。

---

## 3. 圖片匯入（現況：完全不支援）

### 現況確認（2026-10-17 查核）
- `media` 工廠 = `Pipeline::Textual`：只收 txt/md/pdf（新聞剪報等「文字文件」），非圖片檔。
- 全工廠／轉換器無 png/jpg/jpeg 等副檔名支援；`gbrain capture` 僅文字（inline/file/stdin）。
- Operoid 有 `blobs/` 目錄可放附件；gbrain 有 figures 概念（PDF 內嵌圖萃取），但無獨立圖片頁型別。

### 推薦做法（分期）
- **Phase 1（最小可行）**：
  - 圖片檔落 `blobs/`（以內容雜湊命名防重），產生 `media/<slug>.md` 頁，
    frontmatter 帶附件相對路徑（欄位名稱需與 gbrain schema pack v2 對齊，避免自訂欄位衝突——待調查）。
  - **強制註解**（使用者標題/描述）作為頁面 lead line＋timeline 錨點（搭配項目 2 機制）。
- **Phase 2**：視覺 LLM 自動描述圖片內容寫入 body（明確標注 AI-generated）；
  EXIF 拍攝日期 → timeline 事件。
- **Phase 3**：批次拖放（資料夾）、內容雜湊去重、與 gbrain figures 整合。
- 待調查：gbrain 對附件/圖片的 schema 演進方向（避免自訂 frontmatter 未來被打臉）。

---

## 4. 附帶 Bug（本次遷移發現，建議優先修）

1. **oserver `/api/factories/run`（無 target）寫錯 repo**：
   - API 無 target 時以 `cfg.notes_repo_path` 為寫入根；GUI 則用「作用中來源 local_path」。
     本機 `notes_repo_path` 殘留舊值（指向 gbrain-demo），API 批次 460 頁誤寫 demo repo
     （已清理、無 sync 過、demo repo 淨復原）。
   - 建議：API 無 target 時改用作用中來源路徑（與 GUI 一致），或至少在回應中明示實際寫入根目錄。
   - `app-settings.json` 的 `notes_repo_path` 已手動修正為 `C:\Users\charl\notes`（2026-10-17）。
2. **dev 流程帶起舊 oserver.exe**：`npm run tauri dev` 重編後 `target/debug/oserver.exe`
   可能仍是舊檔（本次為 8/30 舊檔，缺新路由 → 前端「本地服務通訊失敗」）。
   建議：`ensure_server` spawn 前比對 `deps/oserver.exe` 與 `oserver.exe` mtime/大小，
   不一致即拒絕帶起並提示重 build；或 build 腳本在連結後強制 replace。

---

## 相關紀錄（2026-10-17 contacts.csv 重包實測）

- CSV 匯入：2,009 列 → 合併 44 群 → 寫入 1,963 頁（`person/`，gbrain-base-v2；
  舊 `people/` 同為合法 path_prefix，gbrain sync 以 rename 處理遷移）。
- LLM 富集（txt/md 管線，經 `/api/factories/run`，每批 40、併發 4）：
  464/467 成功（99.6%），2 檔遭 ZhipuAI 內容過濾器誤判（1301，內容無害、重試無效），
  1 檔重試後成功；dropped 非法日期事件全批次為 0。
- 輸入檔產生器與批次 runner：`C:\Users\charl\Documents\operoid-llm-enrich\`
  （`run_batches.py`；同名聯絡人已合併為單一輸入檔）。
