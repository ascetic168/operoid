# mm-bridge — gbrain 多模態（llama-server + EmbeddingGemma 2）本地相容層

> 個人版本地部署的相容性 shim。**不是對 gbrain / llama-server 本體的修改**——
> 它是一個介於兩者之間的小型 HTTP 轉譯服務，加上一個一次性的 schema 前置變更。
> 上游（gbrain）補上 llama-server recipe 的 `supports_multimodal` 後可整組退役
> （已回報 upstream：[garrytan/gbrain#7094](https://github.com/garrytan/gbrain/issues/7094)，2026-10-20）。

## 解決什麼

gbrain 0.60.x 的圖片匯入管線（sync 自動 import、multimodal embedding、圖片搜尋）
在 `embedding_multimodal_model` 指向 **llama-server** 時無法使用，原因有二：

1. **Recipe 缺宣告**：gbrain 只在 `voyage` 與 `litellm-proxy` 兩個 recipe 上標了
   `supports_multimodal`；`llama-server` recipe 沒標 → 設定會被
   "does not support multimodal embedding" 拒絕。
2. **請求形狀不合**：gbrain 的 openai-compat multimodal 路徑送
   `input:[{type:'image_url',image_url:{…}}]`（扁平 parts）；
   llama.cpp 的 `/v1/embeddings` 要求每個 input 元素包一層
   `{content:[…parts…]}`（chat 式 parts）。直接打會 400。

本 shim 以 **litellm recipe 別名**接在 gbrain 與 llama-server 之間：
litellm recipe 信任使用者填的模型 id、走標準 `/embeddings`，
shim 把 gbrain 的形狀轉成 llama.cpp 的 `{content:[…]}` 形狀，回應攤平轉正。

## 檔案

| 檔案 | 作用 |
|---|---|
| `mm-bridge.mjs` | 轉譯 shim（bun 單檔、零依賴）。監聽 `127.0.0.1:8090` → 上游 `127.0.0.1:8080`。`UPSTREAM`/`PORT` 環境變數可覆寫 |
| `mm-bridge-autostart.vbs` | 登入自啟（複製到 `shell:startup`；WScript.Shell.Run 隱藏視窗啟動 shim） |
| `start-mm-bridge.cmd` | 手動啟動器（前景、可見視窗，除錯用） |
| `alter-image-dim.mjs` | 一次性 schema 前置：`content_chunks.embedding_image` 由 vector(1024)（硬編，為 Voyage 設計）改成 vector(768)（EmbeddingGemma 2 維度）。冪等，gbrain self-upgrade 後若被重置可重跑 |
| `register-mm-bridge-task.ps1` | （替代方案）S4U 登入排程任務註冊——需管理員權限；未採用，留作參考 |

## 設定步驟（全新機器）

前提：`llama-server --embedding -m embeddinggemma-2.gguf --mmproj <mmproj.gguf>`
已在 8080 跑起（要帶 `--mmproj`，否則圖片 embedding 會失敗）；`bun` 可用。

1. 啟動 shim 並設為登入自啟：把 `mm-bridge-autostart.vbs` 複製到
   `shell:startup`（`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\`）；
   手動除錯用 `start-mm-bridge.cmd`（可見視窗）。
2. **schema 前置**（僅首次；欄位必須全空，腳本會自我檢查非空即中止；
   執行前備份 `~/.gbrain/brain.pglite`，且確認沒有 gbrain 行程在跑）：
   ```
   bun alter-image-dim.mjs
   ```
   機器相關路徑用環境變數覆寫：`PGLITE_DIR`（預設 `C:/Users/charl/.gbrain/brain.pglite`）、
   `GBRAIN_SRC`（預設本機 bun global 的 gbrain 原始碼路徑）、`TARGET_DIM`（預設 768）。
3. gbrain 設定（文字側 `llama-server:embeddinggemma-2` 不動、零 reindex；
   multimodal 走獨立的 `embedding_image` 欄）：
   ```
   gbrain config set provider_base_urls.litellm http://127.0.0.1:8090/v1
   gbrain config set embedding_multimodal_model litellm:embeddinggemma-2
   gbrain config set embedding_multimodal true
   ```
4. 驗證：丟一張圖＋同名 `.md` 註解頁進 notes repo → sync →
   `gbrain schema stats` 應出現 `image` 頁；DB 直查
   `vector_dims(embedding_image)` 應為 768。

## 已知限制與注意事項

- **llama-server 必須帶 `--mmproj`**，否則圖片請求失敗。
- **升級後重跑**：`gbrain self-upgrade`／未來 migration 可能把 image 欄重置回
  vector(1024)——升級後重跑 `alter-image-dim.mjs`（冪等，會先自我檢查）。
- **`image_of` 自動連結**：gbrain 在「圖比註解頁先匯入」時會靜默跳過連結
  （best-effort）。註解頁先行或在同一批匯入可避開。
- shim 是前景 process；開機自啟靠 `start-mm-bridge.cmd` 的捷徑/排程。
- 維度差異的數學面：EmbeddingGemma 2 與 Voyage multimodal 維度不同（768 vs 1024），
  因此需要 ALTER 欄位；shim 不做任何維度補墊，向量原樣入庫。

## 退役條件

gbrain 上游給 llama-server recipe（或任何 openai-compat recipe）補上
`supports_multimodal` 並（若形狀仍未對齊）在 `embedMultimodalOpenAICompat`
處理 llama.cpp 的 content 包裝後，即可退役本 shim：還原
`embedding_multimodal_model` 為 `llama-server:<id>`、停掉 shim、
（若上游把欄位維度改為可設定）重跑對應遷移。
