# 第十七章 — Workspace Model 工作空間模型

版本：0.1
狀態：草稿

> 本章定義實體（entity）與其關係。
> 它不是資料庫綱要（schema）。它不規定任何儲存技術、任何資料表、任何查詢語言。
> 無論最終如何儲存，這個模型都必須成立。

---

## 1. 目的

Workspace Model 描述一個 Workspace 之內**存在什麼**，以及**這些事物如何關聯**。

它是每個人 —— 工程師、未來的貢獻者、以及協助開發的 AI 代理 —— 為了理解系統的形狀而查閱的參考。每個實體都有一個目的、一個擁有者、與一組關係。這就是本章定義的全部。

---

## 2. 實體關係總覽

```
Workspace
│
├── owns ──► Employee ──references──► Brain
│              │                         │
│              ├── has ──► Inbox ──► Task
│              ├── has ──► Commitment ──► Task
│              ├── has ──► Memory
│              └── has ──► Metrics
│
├── owns ──► Brain ──references──► Knowledge
│
├── owns ──► Knowledge
│
├── owns ──► Artifact ◄──produced by── Employee
│              │
│              └── belongs to ──► Project / Commitment
│
├── owns ──► Tool
│
└── owns ──► Project ──participates── Employee
                                  │
                                  └──► Artifact, Commitment, Task

Runtime
├── drives ──► Employee (lifecycle)
├── reads ──► Trigger
├── records ──► Event
└── moves ──► Task, Commitment, Memory, Artifact
```

---

## 3. 實體

每個實體：**目的**、**擁有者**、**關係**。

### Workspace
- **目的：** 組織；最外層容器。
- **擁有者：** 自己（頂層）。
- **關係：** 擁有其他每一個實體。Workspace 之外沒有事物存在。

### Employee
- **目的：** 自主的工作者（一個 AI 代理）。
- **擁有者：** Workspace。
- **關係：** 引用一個 Brain。擁有一個 Inbox、Commitment、Memory、Metrics。參與零或多個 Project。產出 Artifact。

### Brain
- **目的：** 可重用的智能。
- **擁有者：** Workspace。
- **關係：** 被一個或多個 Employee 引用。引用知識庫的切片。有版本。

### Knowledge
- **目的：** 組織經策展的知識庫。
- **擁有者：** Workspace。
- **關係：** 被 Brain 引用。是嵌入的來源。有版本。

### Artifact
- **目的：** 工作的持久產出。
- **擁有者：** Workspace。
- **關係：** 由一個 Employee 產出。可屬於一個 Project 或 Commitment。有版本。

### Tool
- **目的：** 透過 Tool Spec 暴露的外部能力。
- **擁有者：** Workspace。
- **關係：** 由 Employee 調用（受權限制約）。獨立於 Brain 與 Knowledge。

### Project
- **目的：** 一個有邊界的倡議。
- **擁有者：** Workspace。
- **關係：** 有參與的 Employee。擁有其範疇內的 Artifact、Commitment 與 Task。

### Task
- **目的：** 一個可執行的目標。
- **擁有者：** 一個 Employee（透過其 Inbox）；可屬於一個 Commitment 或 Project。
- **關係：** 由一個 Commitment 產出。完成時產出一個 Artifact。

### Commitment
- **目的：** 持久的職責。
- **擁有者：** 一個 Employee。
- **關係：** 產生 Task。可屬於一個 Project。一生中可產出 Artifact。

### Trigger
- **目的：** 喚醒 Employee 的東西。
- **擁有者：** Workspace。
- **關係：** 以一個或多個 Employee 為目標。常因一個 Event 而觸發。

### Event
- **目的：** 某件事已發生的不可變紀錄。
- **擁有者：** Workspace。
- **關係：** 由 Employee、Tool、Trigger 或 Runtime 產出。被 Trigger、稽核與指標消費。

### Memory
- **目的：** 一個 Employee 的工作上下文。
- **擁有者：** 單一 Employee。
- **關係：** 由 Runtime 恢復與持久化。有別於 Knowledge 與 Brain 的長期記憶。

---

## 4. 模型化規則

三條規則讓模型保持誠實：

1. **每樣東西都有一個家。** 每個實體都隸屬於恰好一個 Workspace。沒有無所歸屬的狀態。
2. **擁有不等於身份。** 一個 Employee *引用* 一個 Brain；它不擁有 Brain。多個 Employee 可共用一個 Brain。把引用誤當作擁有，是最常見的模型化錯誤。
3. **Spec 與 Status 分離。** 對 Employee（以及任何適用之處），相對固定的定義與持續變化的執行狀態是分開的。這正是版本管理、快照與範本化之所以可能的原因。

---

## 5. 未來擴展

模型未來可能支援：

- **新的實體類型** —— 唯有在架構審查之後（見第二章）。
- **更豐富的關係** —— Artifact 之間的譜系、Employee 之間的委派圖。
- **跨工作空間引用** —— 為聯邦而設的受控連結，且不違反「一個家」的規則。
- **時序模型化** —— 明確追蹤實體與關係如何隨時間變化。
