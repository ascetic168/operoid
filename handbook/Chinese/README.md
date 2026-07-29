# Emploid 架構手冊

**版本：** 0.1
**狀態：** 草稿

> Emploid 不是一個 AI 聊天應用程式。
> 它是一個作業環境，讓 AI 代理（稱為 Employee 員工）在共享的工作空間中持續完成有意義的工作。

---

## Emploid 是什麼

Emploid 是一套 **AI Agent 作業系統**。

它是主控平台（主控平台），AI 代理在其中被招募、被賦予角色與職權、有工作時被喚醒、沒工作時被休眠。每一個代理都隸屬於恰好一個 Workspace（工作空間）。代理產出的每一份 Artifact（成果物）都屬於組織，而不屬於某段對話。每一項職責都跨越模型替換、session 邊界與工具更換而持續存在。

**Agent（代理）** 與 **Employee（員工）** 是同一件事。我們使用「員工」這個詞，是因為整個系統是模擬真實組織的運作方式：人們擔任角色、承擔責任、使用工具、產出成果，並且在工作之中持續存在。Emploid 把這個模型帶到 AI 身上。

名字也由此而來：一個 AI 員工 —— 一個「似員工」的存在 —— 就是一個 **emploid**。平台是 Emploid，而在其中工作的代理就是 emploid。至於本手冊全書使用的正式架構術語，仍維持 **Employee**。

這份手冊，就是這套作業系統的**憲法**。

---

## 本手冊是什麼 —— 以及不是什麼

本手冊定義 Emploid 的**核心抽象**：系統由哪些物件組成、它們之間的關係，以及貫穿一切的设计哲學。

它**不是** API 參考文件，**不是**技術選型報告。它不規定必須使用哪種程式語言、資料庫、模型或工具協定。

核心原則很簡單：

> 架構應該比技術活得更久。

今天提供語言模型的那一套，三年後可能被替換。今天使用的儲存引擎，五年後可能被替換。但如果 *Employee、Brain、Workspace、Artifact、Commitment* 這些概念依然成立，這套架構就是成功的。這份手冊的存在，就是要讓這些概念保持穩定。

如果一個被提議的功能無法用這裡定義的概念來表達，答案不是先改程式碼 —— 而是先回頭檢視架構。

---

## 如何閱讀

本手冊分為四個部分。

**Part I — 願景**：說明 Emploid *為何* 存在，以及所有決策都必須遵守的原則。

**Part II — 核心概念**：定義每一個一級物件 —— 它是什麼、擁有什麼、不擁有什麼、如何誕生與結束、以及如何演進。

**Part III — 執行期**：定義工作實際如何流動 —— 任務、長期職責、觸發器、執行引擎、事件與記憶。

**Part IV — 平台**：定義資料模型、代理與工具介面、安全，以及路線圖。

每一個概念章節都遵循同一個嚴謹的結構：

1. **Purpose 目的** —— 這個物件為何存在。
2. **Responsibilities 職責** —— 它對什麼負責。
3. **Owns 擁有** —— 什麼屬於它。
4. **Doesn't Own 不擁有** —— 什麼明確不在它的範圍內。
5. **Lifecycle 生命週期** —— 它如何誕生、變化與結束。
6. **Future Extension 未來擴展** —— 它如何在不破壞架構的前提下成長。

請先讀 Part I。然後依序讀 Part II，因為後面的概念建立在前面之上。Part III 與 Part IV 則預設讀者已經理解核心概念。

> **譯註：** 本中文版與英文版內容一致。手冊中的核心架構詞（Workspace、Employee、Brain 等）保留英文原文作為正式術語，並在首次出現時附上中文釋義；這些詞對應到程式碼識別字，保留英文可確保中英文版與實作一一對應。圖表中的標籤亦保留英文，以維持與英文版一致的版面。

---

## 目錄

### Part I — 願景

- [01 — 願景 Vision](01-Vision.md)
- [02 — 設計哲學 Design Philosophy](02-Design-Philosophy.md)

### Part II — 核心概念

- [03 — Workspace 工作空間](03-Workspace.md)
- [04 — Employee 員工](04-Employee.md)
- [05 — Brain 大腦](05-Brain.md)
- [06 — Artifact 成果物](06-Artifact.md)
- [07 — Knowledge 知識庫](07-Knowledge.md)
- [08 — Tool 工具](08-Tool.md)
- [09 — Project 專案](09-Project.md)

### Part III — 執行期

- [10 — Task 任務](10-Task.md)
- [11 — Commitment 長期職責](11-Commitment.md)
- [12 — Trigger 觸發器](12-Trigger.md)
- [13 — Runtime 執行引擎](13-Runtime.md)
- [14 — Event 事件](14-Event.md)
- [15 — Memory 工作記憶](15-Memory.md)

### Part IV — 平台

- [16 — Workspace Model 工作空間模型](16-Workspace-Model.md)
- [17 — Agent SDK 代理介面](17-Agent-SDK.md)
- [18 — Tool SDK 工具介面](18-Tool-SDK.md)
- [19 — Security 安全](19-Security.md)
- [20 — Roadmap 路線圖](20-Roadmap.md)

---

## 概念圖

```
Workspace
│
├── Employee  (the autonomous worker; an AI agent)
│      ├── Identity
│      ├── Brain          ← referenced, shared
│      ├── Role
│      ├── Capability
│      ├── Resources
│      ├── State
│      ├── Inbox          ← Tasks arrive here
│      ├── Commitments    ← long-term responsibilities
│      ├── Memory         ← working memory
│      └── Metrics
│
├── Brain     (reusable intelligence: knowledge, persona, prompt, memory)
├── Knowledge (curated organizational knowledge)
├── Artifact  (outputs of work; first-class citizens)
├── Tool      (external capability; never decides)
├── Project   (a bounded initiative)
│
└── Runtime   (wakes, restores, executes, commits, sleeps)
        ├── Task        (one executable objective)
        ├── Commitment  (persistent responsibility)
        ├── Trigger     (what wakes an Employee)
        └── Event       (immutable record of what happened)
```

**一句話角色：**

| 概念 | 一句話角色 |
|---------|---------------|
| Workspace | 組織。一切事物都隸屬於恰好一個 Workspace。 |
| Employee | 工作者。承擔責任的 AI 代理。 |
| Brain | 智能。可重用、可版本化的知識與人格。 |
| Artifact | 成果。工作的產出，歸工作空間所有。 |
| Knowledge | 組織的記憶。經策展且持久的知識。 |
| Tool | 能力。Employee 可調用的外部力量。 |
| Project | 專案。為某個目標而成立的有限度協作。 |
| Task | 工作單位。短期、可執行。 |
| Commitment | 長期戰。比任務壽命更長的持續職責。 |
| Trigger | 鬧鐘。決定何時該喚醒 Employee。 |
| Runtime | 引擎。管理生命週期，從不管理思考。 |
| Event | 紀錄。已發生事實的不可變紀錄。 |
| Memory | 暫存區。Employee 的工作上下文，每次喚醒時重新恢復。 |

---

## 本手冊如何演進

這是一份活的文件。變更遵循兩條規則：

1. **新增概念** 必須經過架構審查。一級概念的集合是刻意保持小的，也必須維持小。
2. **修改原則** 必須先改手冊，再改程式碼 —— 絕不能反過來。

手冊版本紀錄於本檔案最上方。當概念改變時，版本也隨之改變。
