# Operoid

[English](README.md) | **繁體中文** | [简体中文](README.zh-CN.md)

> **能被公司信任的 AI 員工。**
>
> Operoid 是開源的 **AI Agent 作業系統**：AI 代理——**Employee 員工**——在共享的
> **工作空間（Workspace）**裡承擔持久的職責，並且始終在「由人類畫定、由伺服器
> 強制執行、事後可稽核」的邊界內工作。

以 **Rust**（常駐服務＋Tauri v2 桌面殼）與 **Vue 3 + TypeScript** 打造，跑在你自己的硬體上。
**作者：** [朱國棟 (Charlie Chu)](#關於作者) · **授權：** [MIT](#授權)

---

## 為什麼需要 Operoid？

今天所有主流的 Agent 產品，都已經能在你關掉視窗後繼續工作——排程觸發、持久記憶、常駐執行環境。「持續運作」已經是**標配**。

真正沒有解決的，是每一位管理者真正想問的問題：***我憑什麼讓它做？***

- 它到底被允許做什麼？
- 被允許知道什麼？
- 當它做錯了，誰來負責？

今天的 AI 像一位出色的**顧問**：給完建議就離開。

在組織裡，它：

- 沒有身份
- 沒有授權範圍
- 決策出錯，無從課責

所以 AI 在職場上，始終只能活在小小、被緊盯著的視窗裡。

**員工**不一樣：員工加入組織、承擔成果、持續負責。要讓 AI 做到同樣的事，它需要任何一位新進員工都需要的東西：

- 一個由組織核發——也隨時可收回——的**身份**；
- 一份寫明「不必請示就能做什麼」的**委任**；
- 一張寫明「可以知道什麼」的**權限**；
- 以及它經手的每件事都留下的**紀錄**。

Operoid 把這四件事全部做成**架構，而不是價目表上的企業版加購**——以開源核心、跑在資料所在的地方：你自己的機器，或公司自己的內網。

## 給 AI 代理的作業系統

Operoid 對 AI 代理的關係，就像作業系統對程式：它提供環境，不做程式的工作。

| 作業系統概念 | 在 Operoid 裡 |
|---|---|
| **行程** | **Employee 員工** —— 被排程、執行與暫停的代理 |
| **檔案** | **Artifact 成果物** —— 由工作空間擁有、而非由對話擁有的持久產出 |
| **記憶** | **工作記憶與知識** —— 依需求恢復，而非長期駐留 |
| **裝置** | **Tool 工具** —— 透過受控介面調用的外部能力 |
| **核心（kernel）** | **Runtime 執行引擎** —— 喚醒 Employee、恢復上下文、讓它工作、再讓它休眠 |

Runtime 管理**執行**，從不管理**思考** —— Employee 想什麼，是它自己的事。
這就是為什麼 Operoid 是作業系統，而不是應用程式。

## 治理，做進架構裡

### 人類畫得出的委任界線

「該不該讓 AI 決策？」其實不是一個問題，而是三個：

- 系統能否正確決策（**能力提問**）
- 決策失準時，責任落在誰身上（**課責提問**）
- 這一類決策，是否本來就該由人類作成（**正當性提問**）

只有第一問的答案會隨技術移動。

Operoid 把界線當成憲法——由人類畫定——並以**動作類別登記表（Action Registry）**將它操作化：
授權以**動作類別**為單位，絕不授權給「系統」；**從嚴預設**、會屆期、須重簽，事故後**自動凍結**。

### 知道「誰在問」的知識庫

- 同一個腦、同一個查詢、不同身份 → **不同結果**。
- 授權發生在**檢索之前**，由確定性的伺服器程式碼評估——永不交給 LLM，沒有任何「先取回再遮蔽」。
- 政策缺失或損壞＝**拒絕（fail closed）**。
- 每一次特權檢索都留下一張**收據**：誰、在哪個任務、依哪版政策、看到哪些。

### 每位員工都有身份，也都有紀錄

- 每位 Employee 以自己的 token 認證——身份出自 token 鏈，永不出自呼叫端的自稱。
- 每一件工作成果都是可版本化的 **Artifact**，歸工作空間所有，不歸對話。
- 長期的職責成為 **Commitment**：挺得過重啟、失敗自動重試、連續失敗後**交回人類處理**——而不是默默壞掉。

![經理人收件匣：待核可提案與需要關注的員工](assets/manager-inbox.png)

## Operoid 有什麼不同

Agent 市場分成四類。他們都宣稱「持續運作」；組織治理則是另一回事：

|  | 個人 AI 助理 | 開發者 Agent 工具 | 雲端「AI 員工」SaaS | **Operoid** |
|---|---|---|---|---|
| 為誰而建 | 個人 | 工程師 | 業務團隊 | **公司與團隊** |
| 跑在哪 | 你的機器 | 你的終端機 | 供應商的雲 | **你的機器或公司內網** |
| 不在場也持續工作 | ✓ | 有限 | ✓ | ✓ |
| 委任界線——AI 不必請示能做什麼 | ✗ | ✗ | 企業版加購 | **架構——以動作類別為單位、由人類畫定** |
| 檢索前強制執行的知識權限 | ✗ | ✗ | 部分、僅限雲端 | **fail closed、伺服器強制** |
| 代理身份與稽核軌跡 | ✗ | ✗ | 企業版加購 | **內建——每個特權行為都留收據** |
| 開源 | ✓ | ✓ | ✗ | ✓ · MIT |

## 實際上長什麼樣

Employee 因觸發而醒來：

- 觸發源：排程、進來的 Email、經理人或其他人類員工的訊息
- 恢復上下文，在自己的沙箱工作區裡工作（讀檔、改檔、執行指令、查詢知識，全部在**你**控制的模板級授權之下）
- 然後回去休眠

在對話裡，你看得見它工作：每個工具呼叫、它的參數、花了多久——以及一份乾淨的 Markdown 回覆。當決策超出它的授權——它不猜測：它提案，由人類核可。

![員工的工具呼叫出現在提問與 Markdown 回覆之間的對話畫面](assets/chat-with-process.png)

文件也會變成知識——連圖一起。雙欄論文、掃描報告、滿是圖表的簡報，被解析成章節切塊、可檢索的筆記，每張圖都是獨立的檢索項。
以一篇真實 IEEE 論文為例：

**十二道針對特定圖表的問題，皆可在前五名命中 12/12（MRR 0.90）**

## Operoid 現在能做到什麼

架構手冊的路線圖已**一路實作至 Phase 7**（截至 **v0.5.0**）——五個里程碑全部完成：

1. ✅ **一個真正能工作的 Employee** —— 因 Trigger 喚醒、恢復上下文、調用工具、提交成果物、休眠。
2. ✅ **持久化** —— 工作能挺過完全關機與重啟。
3. ✅ **共享的 Brain 與知識** —— 升級一個 Brain，多個 Employee 同步採用。
4. ✅ **範本與實例** —— 一個範本，多個獨立員工。
5. ✅ **協作** —— 一群 Employee 合作完成一個 Project。

也已完成：看得見過程的對話（每個工具呼叫的參數、耗時、結果）、Email 與 IM 直達同樣的員工（[obridge](obridge/)）、知識圖譜地基、上述文件管線，以及把 [Claude Code](https://claude.com/claude-code) 作為第一個代理入口。
完整全貌——概念、原則、路線圖——見**[架構手冊](handbook/Chinese/README.md)**。

## 一套程式碼，兩種版本

| | 個人版 | 企業版 |
|---|---|---|
| 資產 | 桌面安裝檔 | `operoid-enterprise-*` 部署包 |
| 執行位置 | 使用者自己的電腦 | 公司內網伺服器 |
| 設定 | 安裝即用 | 一條引導式指令：`oserver configure` |
| 存取 | 桌面 app | 瀏覽器：`/admin` `/manager` `/user`——密碼登入＋角色式權限 |

選配能力（知識圖譜引擎、本機嵌入、複雜 PDF 解析）都會被自動偵測，並以能力矩陣呈現在應用程式內；安裝細節見 **[DEPLOYMENT.md](DEPLOYMENT.md)**。

## 開始使用

- **個人版**——到 [**Releases** 頁面](https://github.com/ascetic168/Operoid/releases)下載安裝檔執行即可，無須建置。
- **企業版**——從同一個 Release 下載部署包，執行 `oserver configure`：引導式精靈會檢查前置需求、設定 TLS、建立管理員帳號，並可選註冊開機服務。完整流程見 **[DEPLOYMENT.md](DEPLOYMENT.md)**。
- **從原始碼**——需要 **Rust 工具鏈**與 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)：

  ```bash
  git clone https://github.com/ascetic168/Operoid.git
  cd Operoid
  npm install
  npm run tauri dev    # 執行應用（熱重載）
  ```

  開發指令：`npm run build`（前端型別檢查＋建置）、`cargo test`／`cargo check`（Rust workspace）。改動 `oserver`／`obridge` 時請用 `cargo dev-build` 兩個一起編——服務以 sibling 方式帶起 `obridge`，並會互相核對 build id。

## 深入閱讀

- **[使用指南](guide/README.md)**——從安裝到日常操作的完整說明：安裝、每一頁的操作、技能、治理、設定參考與疑難排解（繁體中文，[简体中文](guide/zh-CN/README.md)、[English](guide/en/README.md) 翻譯進行中）。
- **[架構手冊](handbook/Chinese/README.md)**——這套系統的憲法：概念、原則與背後的推理（[第二十一章——路線圖](handbook/Chinese/21-Roadmap.md)）。
- **[DEPLOYMENT.md](DEPLOYMENT.md)**——前置需求、降級行為，與兩種版本的部署。
- **[界線的形狀](journal/界線的形狀_朱國棟.pdf)**——動作類別登記表背後的論文，從一個真實（已去識別化）的製造業個案蒸餾而來。
- 問題與回饋：開一個 [GitHub issue](https://github.com/ascetic168/Operoid/issues)。

## 關於作者

- 我是專案創始人**朱國棟（Charlie Chu）**。
- 三十年在光電半導體領域，從工程師出身，一路做到公司高階主管。
- 目前是協助企業導入 AI 的顧問。
- Operoid 的治理機制——包括動作類別登記表——正是從製造業現場的真實個案蒸餾而來，不是紙上理論。

## 內建技能圖書館

產品出廠即附策展技能目錄：`ocore/default-skills/` 由 `scripts/import_kwp_skills.py`
自 [anthropics/knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins)
（commit `d9d8e13`，2026-10-27）導入，oserver 啟動時種子進**技能圖書館**
（`<data_dir>/skills/catalog/`，唯讀掛載 `catalog/…`）——一個不佔任何人提示詞的
書架。主體從 UI 挑選技能加入個人區（也可移除）；只有「已挑選技能＋組織共用庫＋
員工草稿」（受 `skills_union_limit` 約束，預設 30）會出現在 AI 員工的提示詞裡。
共 11 個外掛、111 個職能技能（productivity、sales、customer-support、
product-management、marketing、legal、finance、data、enterprise-search、
operations、human-resources）。

上游內容 © Anthropic 及其夥伴，採 **Apache License 2.0**（partner 外掛為 MIT）；
各外掛目錄附其 LICENSE。出處記錄見 `ocore/default-skills/SOURCE.md` 與各外掛目錄
`SOURCE.md`，導入稽核見 `IMPORT-REPORT.md`。這些技能是程序指引、不是額外能力——
技能建議的任何動作仍在員工工具 allowlist 與行為準則內執行。

## 授權

本專案以 **[MIT 授權](LICENSE)** 釋出。
Copyright © 2026 朱國棟 (Charlie Chu)。完整條文見 [LICENSE](LICENSE)。
