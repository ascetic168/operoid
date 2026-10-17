# TODO — 待辦與功能分析

> 本文件記錄待實作的功能分析與已知問題。語言慣例比照 JOURNEY.md（繁中）。
> 建立日期：2026-10-17。背景：contacts.csv（2,009 筆）重新匯入後發現 CSV 匯入路徑無 timeline，
> LLM 路徑（txt/md）經 3 頁抽樣 + 464 頁批次驗證有效（詳見各項目）。

---

## 1. ✅ CSV 匯入無 Timeline — 智慧 LLM 路由（已實作 2026-10-17）

> 實作差異：富集為同 run 兩階段（階段一純解析落盤 → 階段二對命中頁跑 LLM），非跨行程背景佇列——ocore 無佇列基礎設施；真背景列為後續增強。

### 現況與問題
- `Pipeline::People` 的 CSV 分支（`csv_people::parse`）為純解析：無 LLM、不產生 timeline。
  txt/md 分支（`text_to_md`）才有 LLM 結構化＋timeline（含 2026-10-16 的日期驗證修正 fe082e1）。
- 對通訊錄而言，Notes 常含「某年某月某日在何處見面」這類高價值事件訊息，純解析全部丟掉很可惜。

### 使用者提議
匯入時辨識內文是否含日期文字（YYYY/MM/DD、YYYY-MM-DD、X月Y日…），需要者才走 LLM。

### 推薦做法
- **Heuristic gate**：CSV 解析完成後，掃描 Notes（與其他自由文字欄位）比對日期 pattern；
  命中者併入「LLM 富集佇列」，未命中者維持純解析頁面。
- 狀態：**使用者同意（2026-10-17）**。
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

### 實測附記（2026-10-17 手動批次富集 464/467 頁）
- **Timeline 命中率**：484 筆含 Notes 中僅 152 頁（31%）由 LLM 產生 timeline（共 155 事件）——
  其餘 Notes 無明確日期，LLM 依規則不生成。智慧路由的 date-pattern gate 需對齊這個比例，
  避免對無日期列白呼叫 LLM。
- **Slug 發散問題**：LLM 標題加辨識詞（「穎昌-曾小姐」）或錯字（孫啟閩→孫啉閩）會產生
  新 slug，與既有頁形成重複（本次 23 頁，已手動收斂：2 頁改名回原 slug、28 個被取代舊頁刪除）。
  實作時應**固定 slug 來源**（沿用 CSV 匯入既有的 slug），LLM 標題僅供 frontmatter title/disambiguation，
  不得改變檔名；同名多紀錄合併輸入時尤需如此。
- 供應商內容過濾器誤判具隨機性（同一檔跨批次成敗不同，蘇世賢上批成功本批失敗）；
  失敗重試應跨批次收斂，最終失敗清單輸出供人工處理。

---

## 2. ✅ 匯入註解（Import Annotation）→ 自動 Timeline 錨點（已實作 2026-10-17）

### 使用者提議
CSV 匯入時要求（或預設）註解，如「從 Google Contacts 匯入的聯絡人」；Operoid 自動補上
匯入日期＋註解作為 timeline 事件。引申：任何文件匯入時，使用者可「選擇性」給註解。

### 推薦做法
- UI：工廠拖放流程加「註解」欄——一般工廠選填；people CSV 建議帶預設值
  「從 Google Contacts 匯入」（可改）。
- 寫入格式：timeline 區塊首位插入 `### YYYY-MM-DD — 匯入：<註解>`（gbrain timeline parser 相容）。
- 保證每頁至少一筆 timeline 錨點（即使無任何日期事件），gbrain timeline 視圖不空白，
  且可追溯資料來源與匯入批次。
- **重匯策略：採去重（使用者決定，2026-10-17）**——保留舊錨點、捨去重複、只補新錨點。
  去重鍵建議用（錨點類型「匯入」＋來源/批次識別）而非日期，避免同日重匯不同來源時誤判。

### 凍結保護互動——現況的精確描述（2026-10-17 查核）

凍結判定（`factories.rs::write_page_guarded`）：目標頁已存在時，
**(a)** frontmatter `origin ≠ operoid-factory`（人手/外來頁），或
**(b)** 帶 `enriched: true`／`<!-- enriched -->` 標記 → 跳過不覆寫；其餘（factory-owned 未標記）→ 無條件覆寫。

三個必須知道的事實：

1. **gbrain 不會寫這兩個標記。** gbrain 自己的富集狀態記在 DB 欄位
   （`enriched_at`/`enriched_by`，見 gbrain `src/commands/enrich.ts`），不動 frontmatter；
   在 gbrain GUI 裡加 timeline 列只改頁面內容。所以 factory-owned 頁上的手工增補
   （含 gbrain 寫入的 timeline 列）**今天就不受凍結保護**——重匯會蓋掉。標記慣例目前
   純靠使用者/工具自律（companies 的 1,029 頁之所以安全，是因為它們沒有 origin 欄）。
2. **去重策略會順便補掉這個缺口（僅限 timeline 區塊）**：寫入路徑從盲目覆寫改為
   「讀舊頁 timeline → 合併（保留所有非本次來源的列）→ 寫回」。人手/gbrain 加的
   timeline 列從此在重匯時存活。但 **body 的手工編輯仍會被覆寫**——那是 enriched
   標記的職責，不在本項目範圍。
3. **凍結頁完全不碰，新錨點也不會加上**——凍結頁的匯入錨點停留在它最後一次被工廠
   寫入的時間。這是刻意取捨（凍結=不動），代價是該頁錨點可能過時。另：編輯器存檔與
   使用者確認的預覽寫回不受凍結保護（fe082e1 明確保留無條件覆寫，明示動作優先）。

### 驗收條件
- 帶註解匯入後，每頁 timeline 含「匯入」事件；重複匯入不產生重複錨點、不遺失舊錨點，
  也不遺失人手/gbrain 加的其他 timeline 列。

---

## 3. ✅ 圖片匯入（已實作 2026-10-17：image 工廠＋企業版對話框＋multimodal 前置檢查）

### 調查結果（2026-10-17；gbrain 原始碼 checkout v0.42.52 ＋ 本機安裝版 0.60.262.0）
- **gbrain 原生圖片管線已存在**（v0.27.1「cherry」系列，非未來計畫而是現有功能）：
  - 觸發：圖片檔放進 notes repo → `gbrain sync`（`sync.ts`：.md 或——multimodal 啟用時——
    圖片副檔名皆 syncable）；另有 `gbrain import <dir>` 目錄匯入、`gbrain files`／`storage` 查詢。
  - 支援格式：png/jpg/jpeg/gif/webp/heic/heif/avif，單檔上限 20MB（Voyage multimodal 限制）。
  - 每張圖自動處理：SHA256 去重（未變即 skip）、HEIC/AVIF 轉通用編碼、EXIF 進 frontmatter、
    OCR（選用）入 chunk_text、multimodal embedding 寫入 `embedding_image` 欄（→ 圖片搜尋）。
  - 建立 `type: image` 資產頁（engine 內建型別，不在 schema pack 20 型清單；位元組由 gbrain
    儲存層管理，Operoid 無須自建 blobs）。
  - **路徑鄰近自動連結**（cherry-3）：`foo.png` 與同目錄 `foo.md` → 自動加 `image_of` edge。
- **前提條件**：需設定 multimodal embedding 模型（`embedding_multimodal_model`；Voyage 或
  OpenAI-compat）。本機目前**未設定**（config 查無此鍵）→ 未設定時 sync 直接跳過圖片。
- **Operoid 側現況**：工廠只收 txt/md/pdf/csv；`gbrain capture` 明確拒絕二進位（含圖片，
  原始碼註記「not yet supported」）；Operoid 也不會替使用者把圖片放進 repo 或生成註解頁。

### 推薦做法（修正版——不自建儲存，Operoid 做編排層）
原 Phase 1「自建 blobs/＋自訂附件 frontmatter」**作廢**：gbrain 已內建儲存/去重/轉檔/EXIF/
OCR/向量搜尋，重造只會與其演進衝突。改為：
- **Operoid 加 image 管線**：拖放（單/批）圖片 → 落來源 repo（與註解頁同目錄、同名主檔，
  以觸發 `image_of` 自動連結）→ **強制註解**（使用者決定）：生成同名 .md 註解頁
  （lead line＝使用者標題/描述，搭配項目 2 的匯入錨點機制）→ sync 時 gbrain 自動建
  image 頁並連結。
- **後續**：設定 multimodal embedding 後即得圖片搜尋；EXIF 拍攝日期可由 Operoid 轉為
  註解頁的 timeline 事件。
- **待辦細節**：批次的 OCR/embed 費用估算與併發；把 doctor 的 multimodal 設定檢查接進
  Operoid 前置檢查（未設定時提前提示，而非 sync 靜默跳過）。
- **試點結果（2026-10-17，多模態圖片嵌入 × PDF figures 統一性評估）**：
  mueller2016 的 8 張 MinerU 圖以筆記同名放進 demo repo doc 目錄 → sync →
  `type: image` 頁＋768 維 `embedding_image` 向量＋`image_of` 連結全數落地；
  視覺語意查詢「magnetic field cross section inductor」→ 圖片頁排名 #1（0.9495）。
  EmbeddingGemma 2 + mmproj 經 llama.cpp 原生 content 形狀直接服務（sidecar 的
  嵌入形狀與 llama.cpp 相同，**不需要 mm-bridge**）。
  **方向決議候選**：PDF figures 改道 gbrain 原生 image import（與使用者拖放同軌）、
  sidecar 向量/檢索退役——遷移前先以 mueller2019 規模做檢索品質對照
  （K3 聯合向量 vs gbrain 圖向量＋OCR/旁頁文字的取捨需實測）。
- **檢索品質對照（2026-10-17，`cargo test -p ocore real_figures_lane -- --ignored`）**：
  mueller2016、11 條帶圖查詢（embed-exp 12 條中 8 條帶 fig 目標＋K3 pixel-only 3 條；
  主裁決子集 9 條＝IMG_QUERIES 6＋pixel 3。「mueller2019 規模」查無此語料，
  經使用者核准以 mueller2016 完成）。四 lane：A＝sidecar 聯合向量單臂、
  B＝gbrain `--cross-modal image` 純圖向量單臂、A′＝現行產品視圖（RRF k=60：
  sidecar 列＋文字列）、C＝候選產品視圖（圖列＋文字列）。
  - 單臂：A hit@5 11/11 MRR 1.000；B hit@5 11/11 MRR 0.894（Q3 fig5→#2、
    Q11 fig9→#3——caption 不在圖向量輸入，視覺相似圖表互搶排名）。
  - 產品視圖：A′ hit@5 11/11 MRR 1.000；C hit@5 11/11 MRR 0.909（文字 lane 的
    同名註解頁把 Q11 從 #3 補到 #2）。
  - **裁決：hit@5 全 lane 持平（目標圖必進前 5）＝核心檢索成功指標不變；
    MRR 差距僅 2/11 查詢的 #1 vs #2/#3，成因明確且架構內有補償（註解頁 caption
    走文字 lane）與再補強（註解頁含完整 caption、選配 embedding_image_ocr）。
    退役收益為架構統一（拖放與 MinerU 同軌、免 sidecar 儲存/嵌入/簽章/遷移面）。
    → 決定改道＋退役；再測條款：管線改寫後重跑本測試，若 C 再退化重議。**
  - **產品整合發現**：`gbrain query --cross-modal auto/both` 的文字查詢不會帶出
    image 頁（both＋limit 20 實測僅回文字頁）——圖片必須顯式走
    `--cross-modal image` arm。sidecar 退役時 service route 2 需改接此 arm，
    否則產品查詢靜默失去圖片。
- **✅ 改道實作（2026-10-17，commits 4d65e65／7b523c5／18066b0）**：
  - 寫入端：`publish_pdf_figures`（工廠與 ingest 共用）——`figN-pP.md` 補強
    （影像檔 wikilink＋原論文 PDF 出處＋匯入錨點，冪等）＋同名圖檔落 doc 目錄
    （先 .md 後 .jpg）；sidecar 寫入全數移除（轉換器 work figures.sqlite 保留為
    metadata 載體）。
  - 查詢端：service route 2 在 sidecar 缺席時改用 `gbrain query --cross-modal
    image` 圖 arm（錨文件歸因同鐵律；`sidecar_config` 對不存在的檔回 None）。
  - 遷移：`POST /api/knowledge/migrate-figures`（Admin）——已授權列搬進 source
    repo（未授權列不遷，fail-closed 保留）、sidecar 封存 `.retired-<date>`、
    figures.assets 留待手動清理；查詢端開檔一律 `open_existing`。
  - **✅ 本機遷移已執行（2026-10-17）**：`MIGRATE_FIGURES_DB=… cargo test -p ocore
    real_migrate_local -- --ignored`——9 列全授權（mueller2016 → gbrain-demo repo）、
    8 圖已在 repo 全數略過（0 重複）、9 註解頁補強（wikilink＋原論文出處＋匯入錨點；
    fig3 caption-only 只加錨點）、git commit＋sync×2＋embed --stale 零警告、
    sidecar 封存 `gbrain-demo.figures.sqlite.retired-2026-10-17`（查詢端自動切換
    圖 arm；`figures.assets/` 留待手動清理）。遷移後重跑 `real_figures_lane`：
    四 lane 數據與試點一致（再測條款通過，C 未退化）。
- **工具選項：Unsloth（2026-10-17 調查，未採用）**：Unsloth 是微調/量化框架＋模型發布管線，
  **不是** ollama/llama-server 的替代（Unsloth Studio 底層就是拉 llama.cpp 當推論後端，
  只差管理 UI）。對本專案的實際價值是**微調 EmbeddingGemma 2**：官方支援
  （`FastSentenceTransformer`，embedding 微調指南；EmbeddingGemma 2 有專頁與
  [unsloth/embeddinggemma-2-GGUF](https://huggingface.co/unsloth/embeddinggemma-2-GGUF)），
  用自有語料做領域適配後匯出 GGUF 經 llama-server 服務，gbrain 端零改動（維度一致即可）。
  gbrain 原始碼（0.60.262.0 與 checkout）無任何 unsloth 專屬整合——支援是間接的
  （任何 OpenAI 相容端點皆可用）。候選增強，非必要。

### 設定方案（2026-10-17 深度調查：全本地棧，EmbeddingGemma 2）

使用者現況：文字 embedding 走本機 llama-server 的 `embeddinggemma-2`（768d）。
**EmbeddingGemma 2**（Google DeepMind 2026-10-06，~740M，Apache 2.0）原生多模態：
文字+圖片+視頻幀+音訊 → 統一 768 維。llama.cpp **支援圖片 embedding**：
`llama-server --mmproj <mmproj.gguf>` 後 `/v1/embeddings` 接受 multimodal content 陣列
（官方 GGUF：ggml-org/embeddinggemma-2-GGUF）。

**gbrain 側的路由事實（安裝版 0.60.262.0 原始碼查核）**：
- `supports_multimodal` 僅 voyage（自有 /multimodalembeddings 端點）與 **litellm-proxy**
  （標準 /embeddings + OpenAI image_url content 陣列，**無模型白名單——信任使用者的模型 id**）。
- `llama-server` recipe **未宣告** multimodal → `embedding_multimodal_model llama-server:<id>`
  會被拒（"does not support multimodal embedding"）。
- 多模態與文字 embedding 的模型路由**互相獨立**（`embedding_multimodal_model` 優先，
  fallback 才是 `embedding_model`）；向量走獨立的 `embedding_image` 欄 → 文字側零 reindex。

**設定步驟（文字維持本機、圖片走同機 litellm-recipe 別名）**：
1. llama-server 帶 mmproj 重啟：`llama-server -m embeddinggemma-2.gguf --mmproj <mmproj.gguf>`
   （ggml-org/embeddinggemma-2-GGUF 取件；注意 llama.cpp #19040 的 GGUF 精度回報）。
2. multimodal 模型經 litellm recipe 指向同一台 llama-server（recipe 信任模型 id、
   端點即標準 /v1/embeddings）：
   `gbrain config set provider_base_urls.litellm http://localhost:8080/v1`
   `gbrain config set embedding_multimodal_model litellm:embeddinggemma-2`
   `gbrain config set embedding_multimodal true`
3. （選用）OCR：`gbrain config set embedding_image_ocr true` +
   `embedding_image_ocr_model <多模態 chat 模型>`（expansion touchpoint；zhipu GLM-4V 或
   llama-server 上的視覺模型）→ OCR 文字進 chunk_text，文字側即可全文檢索。
4. **請求形狀實測結果（2026-10-17，已完成）**：gbrain 的形狀
   （`input:[{type:'image_url',…}]` + `input_type`）直接打 llama.cpp 會 400——
   llama.cpp 要求每個 input 元素包 `{content:[…]}`（chat 式 parts，官方 README 格式）。
   **已寫好形狀轉譯 shim 並實測通過**，正式收錄於
   **`deploy/local/mm-bridge/`**（shim＋啟動器＋schema 變更腳本＋README；
   部署位置 `C:\Users\charl\AppData\Local\llama-cpp\`，127.0.0.1:8090 → 8080）。
   同步完成 schema 前置：`content_chunks.embedding_image` 由 vector(1024) ALTER 成
   **vector(768)**（0.60 硬編 1024、無設定鍵；欄位當時全空、無損遷移；腦已備份
   `brain.pglite.bak-20261017`；變更腳本 `~/.bun/install/global/alter-image-dim.mjs`，
   gbrain self-upgrade 後若被重置可冪等重跑）。
   gbrain 設定已套用：`provider_base_urls.litellm=http://127.0.0.1:8090/v1`、
   `embedding_multimodal_model=litellm:embeddinggemma-2`、`embedding_multimodal=true`
   （multimodal 經 litellm recipe 別名指向同一台 llama-server；文字側
   `llama-server:embeddinggemma-2` 不動）。
5. **端到端煙霧測試（通過）**：`images/mm-bridge-smoke-test.png` + 同名註解頁 → sync →
   image 頁建立、`image_asset` chunk 的 `embedding_image` 實存 **768 維向量**、
   註解頁 text chunks 照常、timeline 1 筆萃取。
6. **剩餘事項**：
   - ~~shim 開機自啟未掛~~ **已完成（2026-10-17）**：Startup 資料夾 + VBS 隱藏包裝
     （`shell:startup\mm-bridge.vbs`，免 admin；部署副本 `deploy/local/mm-bridge/mm-bridge-autostart.vbs`）。
     S4U 排程任務版（`register-mm-bridge-task.ps1`）因需提權未採用，留作替代。
     llama-server 本體的重啟（含 --mmproj）仍是手動流程。
   - `image_of` 自動連結在「圖先於註解頁匯入」時會靜默跳過（gbrain best-effort）——
     Operoid 實作時應先寫註解頁再丟圖，或匯入後跑連結補算。
   - gbrain `self-upgrade`／未來 migration 可能重置 image 欄維度——升級後重跑
     `alter-image-dim.mjs` 驗證（冪等）。
   - 上游正道：給 gbrain 的 llama-server recipe 補 `supports_multimodal` 宣告
     （openai-compat multimodal 路徑上游已存在，是小改動），可免除 shim。
   - ~~Operoid 工廠的 image 管線仍未實作~~ **已實作（2026-10-17，commit f601657）**：
     新增 `image` 工廠（v2 pack 第 16 型、`Pipeline::Image`）——拖放圖片（單/批）→
     強制逐張註解對話框 → 圖落 `media/<名稱>.<ext>`＋同名註解頁（`type: note`＋
     匯入 timeline 錨點，即項目 2 的圖片版）→ sync 時 gbrain 自動多模態嵌入與
     `image_of` 連結。sha256 去重（同內容重用、同名異容 `-2/-3`）、凍結保護適用於
     註解頁、無 LLM。API：`/api/factories/run` 與桌面 command 增 `annotations`
     平行陣列（強制非空）；scoped（circle）目標暫不支援 image 工廠。分類器以
     副檔名高信心直達 image 工廠。剩：企業版 user 前端的註解對話框 UI 移植
     （後端已就緒，缺 UI 時會收到明確的 per-file 錯誤）、doctor multimodal
     前置檢查整合。

---

## 4. ✅ 附帶 Bug（已修復 2026-10-17，commit 9493658 之後）

1. **oserver `/api/factories/run`（無 target）寫錯 repo**：
   - API 無 target 時以 `cfg.notes_repo_path` 為寫入根；GUI 則用「作用中來源 local_path」。
     本機 `notes_repo_path` 殘留舊值（指向 gbrain-demo），API 批次 460 頁誤寫 demo repo
     （已清理、無 sync 過、demo repo 淨復原）。
   - 建議：API 無 target 時改用作用中來源路徑（與 GUI 一致），或至少在回應中明示實際寫入根目錄。
   - `app-settings.json` 的 `notes_repo_path` 已手動修正為 `C:\Users\charl\notes`（2026-10-17）。
   - **操作知識（同日二修）**：桌面 GUI 退出時會以記憶體內的 app_config 回寫檔案——
     在 GUI 還開著時手動改檔會被蓋掉（當日 `notes_repo_path` 修正即被回寫沖銷一次，
     `active_brain_id`/`active_source_id` 同批回寫為 demo 腦）。修檔必須在 GUI 關閉後進行；
     修復行為本身也應涵蓋「exit-save 不得以過期記憶體值覆寫檔案內已變更的鍵」。
2. **dev 流程帶起舊 oserver.exe**：`npm run tauri dev` 重編後 `target/debug/oserver.exe`
   可能仍是舊檔（本次為 8/30 舊檔，缺新路由 → 前端「本地服務通訊失敗」）。
   建議：`ensure_server` spawn 前比對 `deps/oserver.exe` 與 `oserver.exe` mtime/大小，
   不一致即拒絕帶起並提示重 build；或 build 腳本在連結後強制 replace。
   **✅ 已修＋根因定位（2026-10-17，commit c4f7d28/4ed3ade）**：根因是 `tauri.conf`
   的 `externalBin` 指向 `src-tauri/binaries/<name>-<triple>.exe`（未被 git 追蹤的
   本機 artifact，8/30 殘留）——每次 `cargo check/build -p operoid` 時 tauri-build
   把該檔複製蓋寫 `target/debug/oserver.exe`（蓋掉 cargo 的新建置）。已修：
   `dev:all` 接上 `scripts/sync-sidecars.mjs`（建置後同步 sidecar 來源）＋
   `ensure_server` 與 deps 逐位元組比對、不一致拒絕帶起。

---

## 相關紀錄（2026-10-17 contacts.csv 重包實測）

- CSV 匯入：2,009 列 → 合併 44 群 → 寫入 1,963 頁（`person/`，gbrain-base-v2；
  舊 `people/` 同為合法 path_prefix，gbrain sync 以 rename 處理遷移）。
- LLM 富集（txt/md 管線，經 `/api/factories/run`，每批 40、併發 4）：
  464/467 成功（99.6%），2 檔遭 ZhipuAI 內容過濾器誤判（1301，內容無害、重試無效），
  1 檔重試後成功；dropped 非法日期事件全批次為 0。
- 輸入檔產生器與批次 runner：`C:\Users\charl\Documents\operoid-llm-enrich\`
  （`run_batches.py`；同名聯絡人已合併為單一輸入檔）。
