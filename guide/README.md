# Operoid 使用指南

繁體中文｜[简体中文](zh-CN/README.md)｜[English](en/README.md)

> **適用版本**：v0.5.11（2026-10）。屬於特定版本才有的功能，各章內會以「自 vX.Y.Z 起」標註。

Operoid 是一套 **AI 員工作業系統**：AI 員工在您自己的電腦或伺服器上，於人類畫定的委任界線內持續工作。本指南是**操作手冊**——帶您從安裝、設定走到每天使用；系統背後的概念與設計推理，請閱讀[架構手冊](../handbook/Chinese/README.md)。

Operoid 一套程式碼、兩種版本：

- **個人版**——桌面應用，裝在自己的電腦（Windows／macOS／Linux），一個人即裝即用。
- **企業版**——部署在公司伺服器，透過瀏覽器以「管理者／經理人／一般使用者」三種角色使用；安裝部署見 [DEPLOYMENT.md](../DEPLOYMENT.md)。

除特別標註外，本指南以**個人版**為主；企業版的使用差異見[第 16 章](16-enterprise-consoles.md)。

## 建議閱讀路徑

第一次使用：**01 → 02 → 03（或 04）→ 05**。之後把[06 桌面應用導覽](06-desktop-tour.md)當地圖，用到哪一頁再查對應章節。

## 章節總覽

### 入門

| 章 | 說明 |
|---|---|
| [01 認識 Operoid](01-introduction.md) | 產品定位、兩種版本、核心概念速覽 |
| [02 系統需求](02-requirements.md) | 平台、硬體、前置元件與能力矩陣 |

### 安裝

| 章 | 說明 |
|---|---|
| [03 安裝個人版](03-install-personal.md) | 下載安裝檔、首次啟動檢查、自動更新 |
| [04 安裝企業版](04-install-enterprise.md) | 部署包內容、`oserver configure` 精靈、服務註冊 |

### 開始使用

| 章 | 說明 |
|---|---|
| [05 快速上手](05-quickstart.md) | 15 分鐘：從啟動到第一次與員工對話 |

### 每天的 Operoid

| 章 | 說明 |
|---|---|
| [06 桌面應用導覽](06-desktop-tour.md) | 介面地圖：每一頁在做什麼 |
| [07 腦與模型設定](07-brains-and-models.md) | 大腦註冊與切換、模型與 API key、服務狀態 |
| [08 員工管理](08-employees.md) | 模板、實體、工具授權、對話與監看 |
| [09 文件工廠](09-factories.md) | 拖放文件 → 轉換 → 寫入知識庫 |
| [10 待辦與事件](10-inbox-and-events.md) | 核可提案、交辦承諾、事件流 |
| [11 技能庫](11-skills.md) | 四區模型、採用／移除、提案與晉升 |
| [12 知識庫與 Claude Code](12-knowledge.md) | 知識查詢、Claude Code 整合 |
| [13 治理：登記表與知識授權](13-governance.md) | 動作類別登記表、知識政策與檢索收據 |
| [14 電子郵件橋接（Obridge）](14-email-bridge.md) | 讓員工收發 email、通道設定 |

### 維運

| 章 | 說明 |
|---|---|
| [15 服務與更新](15-service-and-updates.md) | 本機服務、開機自啟、版本更新 |
| [16 企業版三種角色介面](16-enterprise-consoles.md) | /admin、/manager、/user 日常操作 |

### 參考

| 章 | 說明 |
|---|---|
| [17 設定參考](17-config-reference.md) | 所有設定檔欄位與環境變數 |
| [18 疑難排解](18-troubleshooting.md) | 常見問題、日誌位置、診斷步驟 |

### 附錄

| 章 | 說明 |
|---|---|
| [附錄 A：GBrain 架構說明](appendix-a-gbrain.md) | 給新手：為什麼需要 GBrain？它與一般 RAG、Wiki 搜尋有何不同 |
| [附錄 B：Operoid 開發說明](appendix-b-development.md) | 企業自行開發接口（Obridge 外掛）與 fork 修改——含 Vibe Coding 示範提示詞 |

## 用語約定

本指南沿用應用程式介面用語（以內建「繁體中文」介面為準）：

- **員工（Employee）**——有身份、有職責、持續工作的 AI 代理。
- **腦（Brain）**——由 GBrain 知識引擎管理的知識本體；員工由此查證作答。
- **成果物（Artifact）**——員工工作的產出，版本化、歸工作區所有。
- **承諾（Commitment）**——員工答應完成的事，有完成條件，由人類核可、追蹤、取消。
- **動作類別登記表（Action Registry）**——人類畫定的委任界線：哪些類別的行動可以自動執行。
- **技能（Skill）**——程序性知識（某類工作怎麼做），不是授權；所有行動仍受工具允許清單約束。

更深的概念定義見[架構手冊](../handbook/Chinese/README.md)。

## 文件慣例

- 各章章首標注「適用版本」。
- 介面上實際出現的字串（頁面名、按鈕、狀態）以「」括住，例如「同步全部」「加入」。
- 個人版／企業版差異於章內以段落標明。

## 回饋

問題與建議請到 [GitHub Issues](https://github.com/ascetic168/Operoid/issues) 提出。
