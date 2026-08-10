# Operoid

[English](README.md) | **繁體中文** | [简体中文](README.zh-CN.md)

> Operoid 不是一個聊天應用程式。它是一套 **AI Agent 作業系統** —— 一個作業環境，
> 讓 AI 代理（稱為 **Employee 員工**）在一個共享、持久的工作空間（**Workspace**）裡
> 持續完成有意義的工作。

多數 AI 產品以對話為中心：你問、它答，視窗一關工作就消失了。但真正的工作不是這樣。
採購助理會追蹤一張訂單長達數週；品保工程師會從異常報告、到矯正措施、再到結案一路追蹤。
這些職責需要一個能**持久、能記憶、並在視窗關閉後仍繼續運作**的環境。

Operoid 的存在，就是為了成為那個環境。

以 **Tauri v2（Rust）** + **Vue 3 + TypeScript** 打造。
**作者：** 朱國棟 (Charlie Chu) · **授權：** [MIT](#授權) · **狀態：** v0.2.0 —— 見[目前狀態](#目前狀態)

---

## 為什麼需要 Operoid？

今日的 AI，行為更像一個**顧問**，而不是一個**員工**。顧問給完建議就離開；
員工加入組織、承擔成果、並持續負責。Operoid 是為後者而打造的。

今日的 AI 系統普遍缺乏：

- **持久的職責** —— 對話結束，工作就消失。
- **長期的承諾** —— 沒有「追蹤此事直到完成」的概念。
- **共享的工作空間** —— 沒有一個地方讓多個代理與人類就相同事物協作。
- **組織知識** —— 模型所知，並不等於組織所知。
- **企業角色** —— 代理沒有身份、職權或問責。
- **持續的執行** —— 沒有東西會在相關事件發生時把代理喚醒。

Operoid 把 AI 視為**組織成員，而不是聊天機器人。**

## 什麼是「AI Agent 作業系統」？

傳統作業系統管理行程、記憶、檔案與裝置，讓程式得以執行 —— 它提供環境，不做程式的工作。
Operoid 對 AI 代理做同樣的事：

| 作業系統概念 | 在 Operoid 裡 |
|---|---|
| **行程** | **Employee 員工** —— 被排程、執行與暫停的代理 |
| **檔案** | **Artifact 成果物** —— 由工作空間擁有、而非由對話擁有的持久產出 |
| **記憶** | **工作記憶與知識** —— 依需求恢復，而非長期駐留 |
| **裝置** | **Tool 工具** —— 透過受控介面調用的外部能力 |
| **核心（kernel）** | **Runtime 執行引擎** —— 喚醒 Employee、恢復其上下文、讓它執行、再讓它休眠 |

Runtime 管理**執行**，從不管理**思考** —— Employee 想什麼，是它自己的事。
這就是為什麼 Operoid 被稱為作業系統，而不是應用程式。

## 核心概念

| 概念 | 一句話角色 |
|---|---|
| **Workspace 工作空間** | 組織。一切事物都隸屬於恰好一個。 |
| **Employee 員工** | 工作者。承擔責任的 AI 代理。 |
| **Brain 大腦** | 智能。可重用、可版本化的知識與人格。 |
| **Artifact 成果物** | 成果。工作的產出，歸工作空間所有。 |
| **Knowledge 知識庫** | 組織經策展且持久的記憶。 |
| **Tool 工具** | Employee 可調用的外部能力。它永遠不做決策。 |
| **Project 專案** | 為某個目標而成立的有限度協作。 |
| **Task 任務** | 工作單位。短期、可執行。 |
| **Commitment 長期職責** | 比任務活得更久的持久職責。 |
| **Trigger 觸發器** | 決定何時該喚醒 Employee。 |
| **Runtime 執行引擎** | 管理生命週期的引擎，從不管理思考。 |
| **Event 事件** | 已發生事實的不可變紀錄。 |
| **Memory 工作記憶** | Employee 的工作上下文，每次喚醒時重新恢復。 |

完整的定義 —— 目的、職責、各自擁有什麼、生命週期與未來擴展 —— 見
**[架構手冊](handbook/Chinese/README.md)**，它是這套作業系統的憲法。

## 目前狀態

架構手冊為 **v0.2（草稿）**，而路線圖的里程碑已**一路實作至 Phase 7** ——
手冊中的願景現在是一個運行中的系統，而不只是草稿。

目前版本（v0.2.x）實作了完整的 Agent-OS 技術棧：

- 一個 **Tauri v2 桌面工作空間**（Vue 3 + TypeScript 前端、Rust 後端）。
- 一個以 [GBrain](https://github.com/garrytan/gbrain) 為基礎的**知識圖譜層** ——
  把日常檔案（聯絡人 CSV、會議 PDF、公司介紹）變成互連、可查詢的筆記；
  透過 GUI 而非命令列來同步、提問與推論。
- 完整的 **Agent-OS 執行引擎**（Phase 1–7）：由 Trigger 驅動的 Employee 生命週期引擎；
  持久化於 SQLite 的 **Artifact 成果物**與 **Commitment 承諾**；
  **範本 → 實例**（定義一次、部署多次）；**共享 Brain** 讓一次升級惠及所有 Employee；
  **團隊、專案與任務交接**實現多 Employee 協作；以及**對話層** —— 人機聊天、
  訊息驅動喚醒、即時觀察面板。
- 第一個**代理入口**：在工作空間內啟動並監看 [Claude Code](https://claude.com/claude-code)。

> ℹ️ GBrain 知識圖譜功能是本版本的*知識*層。
> 手冊中定義的 Employee / Runtime / Commitment 架構如今已實作並運行中。

## 技術棧

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**後端：** Tauri v2 · Rust

## 前置需求

要使用目前的知識圖譜功能，桌面應用需要：

| 工具 | 用途 | 安裝 |
|---|---|---|
| **git** | sync 流程會在更新圖譜前先 commit | <https://git-scm.com/downloads> |
| **bun** | `gbrain` 透過 bun 安裝與執行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 知識圖譜引擎 | <https://github.com/garrytan/gbrain> |

路徑會自動偵測（Windows 為 `~/.bun/bin/gbrain.exe`）；必要時可在「設定」頁覆寫。

## 安裝與執行

**一般使用者 —— 直接下載預編譯安裝包即可。** 到
[**Releases** 頁面](https://github.com/ascetic168/Operoid/releases)下載對應平台的最新版本並執行。
除非你要開發 Operoid，否則不需要 `git clone` 或從原始碼建置。

### 開發者（從原始碼建置）

建置桌面應用需要 **Rust 工具鏈**與 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/ascetic168/Operoid.git
cd Operoid
npm install          # 安裝依賴
npm run tauri dev    # 執行應用（熱重載）
npm run tauri build  # 建置散布用安裝包
```

僅前端（於 http://localhost:1420 在瀏覽器執行）：`npm run dev`、`npm run build`。

## 開發

```bash
npm run tauri dev             # 完整應用，熱重載
npm run build                 # 前端型別檢查 + 建置
cd src-tauri && cargo test    # Rust 單元測試
cd src-tauri && cargo check   # 後端快速型別檢查
```

## 專案結構

```
src/              Vue 3 前端（views、Pinia stores、i18n、具型別 IPC 包裝）
                  —— Brains、Factories、Config、員工範本／實體、
                    員工對話、Operations（即時觀察面板）
src-tauri/src/    Rust 後端
                    config · converters · factories · gbrain_cli · claude_code
                    brains · classifier · note_view · llm · prereq · i18n
                    runtime · agent_state · scheduler · note_server
handbook/         架構手冊 —— 憲法（英文 + 中文）
```

## 路線圖

路線圖勾勒於手冊中，依依賴關係排序。五個里程碑皆已**實作至 Phase 7**：

1. ✅ **一個真正能工作的 Employee** —— 因 Trigger 喚醒、恢復上下文、調用工具、提交成果物、休眠。
2. ✅ **持久化與 Commitment** —— 工作能挺過完全關機與重啟（SQLite）。
3. ✅ **共享的 Brain 與知識** —— 升級一個 Brain，多個 Employee 同步採用。
4. ✅ **範本與實例** —— 一個範本，多個獨立員工。
5. ✅ **協作** —— 一群 Employee 合作完成一個 Project（團隊 + 專案 + 任務交接）。

Phase 7 新增了**人機協作層**：交辦承諾給人類、Message 概念、聊天對話與錯誤韌性。
完整說明見[第二十一章 — 路線圖](handbook/Chinese/21-Roadmap.md)。

## 授權

本專案以 **[MIT 授權](LICENSE)** 釋出。
Copyright © 2026 朱國棟 (Charlie Chu)。完整條文見 [LICENSE](LICENSE)。
