# GBrainStudio

[English](README.md) | **繁體中文** | [简体中文](README.zh-CN.md)

**GBrainStudio** 是 [GBrain](https://github.com/garrytan/gbrain) 的友善桌面夥伴。
把日常檔案丟進來，就能得到一張互相連結、可查詢的知識圖譜 —— 不必手寫連結、不必碰命令列。

以 **Tauri v2（Rust）** + **Vue 3 + TypeScript** 打造。
**作者：** 朱國棟 (Charlie Chu) · **授權：** [MIT](#授權)

---

## 為什麼需要 GBrainStudio？

[GBrain](https://github.com/garrytan/gbrain) 是一套強大的知識圖譜引擎 —— 但它以 CLI 為主，
而且要求**手工撰寫筆記**：每個交叉引用都必須是格式正確、命名正確的連結，否則在圖譜裡會
默默斷開。這樣累積語料既慢又容錯率低，日常工作還得背指令、讀終端機輸出。

**GBrainStudio 把這些摩擦拿掉：**

- 📥 **丟個檔案，得到一則已連結的筆記。** 丟進聯絡人 CSV、會議記錄 PDF、或公司介紹 ——
  GBrainStudio 把它轉成合規筆記，所有交叉引用都自動產生，連結確實能在圖譜裡解析。
- 🔗 **永遠不必手寫連結。** 提到某個人或某家公司，就自動接進圖譜；你只管寫白話文，連結交給程式。
- 🖱️ **整套流程都有 GUI。** 跑 sync / ask / think 帶即時串流輸出；點答案裡的任一引用，
  就能在瀏覽器開啟來源筆記。
- 🧠 **多個知識圖譜，視覺化管理。** 建立、登錄、同步多個隔離的腦與其來源，全程不必開終端機。

一言以蔽之：GBrainStudio 把 GBrain 從進階玩家的 CLI，變成一套**好上手**的知識圖譜
建立與探索工具。

## GBrain 是什麼？

[GBrain](https://github.com/garrytan/gbrain) 是底層的知識圖譜引擎：你的筆記會變成一張
可查詢的圖，涵蓋人物、公司、會議與概念。引擎本身的細節請見它的 repo。

> ℹ️ **GBrainStudio 是獨立的搭配 GUI —— 它不是 GBrain。** 它幫你驅動 `gbrain` CLI；
> 引擎、圖譜儲存與檢索都仍是 GBrain 的工作。

## 功能

### 🏭 工廠 —— 把來源檔案變成已連結的筆記

把檔案拖到卡片上（或點擊選檔），即轉換並直接寫入筆記：

| 工廠 | 接受 | 變成 |
|---|---|---|
| **people** | CSV / TXT / MD | 人物筆記（CSV=聯絡人；TXT/MD=一人一檔，LLM 結構化） |
| **companies** | TXT / PDF | 公司筆記（LLM 結構化） |
| **meeting** | TXT / MD / PDF | 會議筆記（LLM 結構化） |
| **inbox** | TXT / MD | 速記 capture |

- **批次拖放：** 一次丟多個檔案；結果清單顯示每個檔的狀態，可點任一檔預覽、編輯後再同步。
- **來源感知：** 筆記寫對位置 —— 作用中腦的作用中來源 repo —— 所以一鍵 **Sync 到腦** 就接得上。
- **手寫編輯器（`+`）：** 從 template 開始自然書寫；存檔時你提到的人名/公司名會自動連進圖譜。

### 🔧 操作 —— 不必開終端機就能用 GBrain

包裝 `gbrain` CLI；輸出即時串流到 console。

- **stats · sync · extract** —— 維持語料健康。
- **ask · think** —— 問問題，得到多跳、附引用的答案（`think` 可用 `anchor:` 聚焦某則筆記）。
- **診斷** —— `doctor`、`orphans`、`storage`、`graph-query`。
- **重建 companies** —— 從人物筆記重新產生公司筆記。
- **可點擊引用：** 答案裡的 `[[people/JLin]]` / `[people/JLin]` 標籤會高亮；點下去就能在瀏覽器讀該筆記。

### 🧠 腦 —— 管理多個知識圖譜

`gbrain` 沒有「列出所有腦」的指令，GBrainStudio 給你一個：

- 登錄既有腦，或建立新腦。
- 每腦可有多個**來源**（git repo），可逐來源或全部同步。
- 切換腦時，config、來源、作用中目標全部跟著走。

### ⚙️ 設定 —— 所有設定集中一處

並排編輯 GBrain 權威的 `config.json`（model、embedding、provider）與本系統自有設定
（路徑、輸出資料夾、sync 旗標、語言）。

### 還有

- **啟動檢查** —— 檢查 `git` / `bun` / `gbrain`，缺漏者直接給安裝連結。
- **三種語言** —— 繁中、簡中、英文，自動偵測並可手動覆寫。

## 前置需求

| 工具 | 用途 | 安裝 |
|---|---|---|
| **git** | sync 流程會在更新圖譜前先 commit | <https://git-scm.com/downloads> |
| **bun** | `gbrain` 透過 bun 安裝與執行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 引擎本體 | <https://github.com/garrytan/gbrain> |

路徑會自動偵測（Windows 為 `~/.bun/bin/gbrain.exe`）；必要時可在「設定」頁覆寫。

## 安裝與執行

> 建置桌面應用需要 **Rust 工具鏈**與 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install          # 安裝依賴
npm run tauri dev    # 執行應用（熱重載）
npm run tauri build  # 建置散布用安裝包
```

僅前端（於 http://localhost:1420 在瀏覽器執行）：`npm run dev`、`npm run build`。

## 快速上手

1. **啟動** GBrainStudio —— 缺漏會以視窗列出。
2. **設定** —— 確認筆記資料夾與 `gbrain` 路徑。
3. **腦** —— 選擇或建立腦，加入一個**來源**（放筆記的 git repo），並設為作用中。
4. **工廠** —— 把檔案拖到對應卡片，再按 **Sync 到腦**。
5. **操作** —— 用一個問題跑 `think`；點任一高亮的人名即可開啟該筆記。

## 設定

經 `tauri-plugin-store` 存於 app data：

| 欄位 | 意義 |
|---|---|
| `notes_repo_path` | 兜底的筆記資料夾；工廠優先用作用中來源的資料夾 |
| `gbrain_exe_path` | `gbrain` 執行檔路徑 |
| `factory_targets` | 輸出子資料夾（`people` / `companies` / `meetings`） |
| `auto_sync` | 工廠寫檔後自動 commit + sync |
| `sync_no_pull` | 加 `--no-pull`（無 remote 的腦建議開） |
| `llm_temperature` / `llm_max_tokens` | 工廠 LLM 結構化的取樣參數 |
| `locale` | 介面語言（`null` = 自動偵測） |

> 一份筆記的檔案位於「作用中腦的作用中來源 repo」。點引用時會先搜該來源、再其他來源、
> 最後才退到兜底資料夾 —— 永遠開對檔案。

## 專案結構

```
src/              Vue 3 前端（views、Pinia stores、i18n、具型別 IPC 包裝）
src-tauri/src/    Rust 後端（config、converters、factories、gbrain_cli、
                  brains、note_view、llm、prereq、i18n）
```

## 技術棧

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router ·
vue-i18n · lucide-vue-next。**後端：** Tauri v2 · Rust。

## 開發

```bash
npm run tauri dev             # 完整應用，熱重載
npm run build                 # 前端型別檢查 + 建置
cd src-tauri && cargo test    # Rust 單元測試
cd src-tauri && cargo check   # 後端快速型別檢查
```

## 疑難排解

- **`think` / `ask` 顯示「(no LLM available)」？** `gbrain think` 的 model 取用順序為
  `models.think → models.default → GBRAIN_MODEL → opus`。`gbrain init --chat-model X` 只設了
  `chat_model` 而**未設** `models.default`，所以 `think` 可能默默退回 Anthropic Opus。修正：
  在該腦上 `gbrain config set models.default <provider:model>`
  （如 `groq:llama-3.3-70b-versatile`）。（GBrainStudio 自有的工廠結構化直接讀 `chat_model`，不受影響。）
- **點引用顯示「找不到筆記」？** 該筆記不在作用中腦的任一來源中，或檔名大小寫不同。
  GBrainStudio 會先精確比對、再大小寫寬容掃描 —— 請確認檔案確實在作用中來源下。

## 授權

本專案以 **[MIT 授權](LICENSE)** 釋出。

Copyright © 2026 朱國棟 (Charlie Chu)。完整條文見 [LICENSE](LICENSE)。
