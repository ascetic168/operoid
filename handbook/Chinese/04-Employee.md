# 第四章 — Employee 員工

版本：0.1
狀態：草稿

> 在 Operoid 中，Employee 與 AI Agent 是同一個物件。
> 我們稱之為員工，是因為整個系統是模擬真實組織的運作方式。

---

## 1. 目的

Employee 是 Operoid 中**唯一真正在工作的物件**。

Brain 不工作。Tool 不工作。Workspace 不工作。每一件有意義的工作 —— 每一個決策、每一次工具調用、每一份產出的成果、每一項被履行的職責 —— 都是由 Employee 完成的。

因此 Employee 是**一級物件**：Runtime 管理的核心單位、承擔責任的單位、以及組織招募、指派與問責的單位。

Employee **引用** 一個 Brain。它不是一個 Brain。Brain 是*它所知*；Employee 是*那個去知、去決定、去行動的主體*。

---

## 2. 職責

一個 Employee 對以下事項負責：

- **接收工作** —— 將 Task 接入它的 Inbox。
- **做出決策** —— 決定做什麼、順序為何、用哪些工具。
- **調用工具** —— 透過被授權的能力對外界採取行動。
- **產出成果** —— 把工作轉化為持久、已提交的產出。
- **履行職責** —— 在多次喚醒之間承擔持久的責任。
- **維護工作記憶** —— 保留足夠的上下文以便正確地恢復工作。

在 Operoid 裡完成一件事，就是某個 Employee 完成的。出了問題，就是某個 Employee 該負責。

---

## 3. 擁有 —— Employee 模型

一個 Employee 擁有十樣東西。合在一起，構成完整的 Employee。

```
Employee
├── 1. Identity      （識別）
├── 2. Brain         （大腦）
├── 3. Role          （角色）
├── 4. Capability    （能力）
├── 5. Resources     （資源）
├── 6. State         （狀態）
├── 7. Inbox         （收件匣）
├── 8. Commitments   （長期職責）
├── 9. Memory        （工作記憶）
└── 10. Metrics      （績效指標）
```

**1. Identity（識別）** —— Employee 是誰：ID、名稱、頭像、部門、職稱、描述、與擁有者。代理在組織內的對外面貌。

**2. Brain（大腦）** —— 一個*引用*，而非副本。Employee 指向一個持有其知識、人格、提示詞與長期記憶的 Brain。多個 Employee 可共用同一個 Brain。

**3. Role（角色）** —— Employee 的責任定義：使命、職責、職權、KPI、SOP 與政策。Role 定義的是 *Employee 負責什麼*，而不是它知道什麼。

**4. Capability（能力）** —— Employee *能做* 什麼：寄信、查資料庫、瀏覽、跑程式、操作 CAD、呼叫外部服務。Capability 是*能力*，不是工具本身。

**5. Resources（資源）** —— Employee 實際被配置使用的具體工具與系統：那個特定的資料庫、郵件系統、程式執行環境、設計工具。Capability 是抽象的；Resources 是接好線的現實。

**6. State（狀態）** —— Employee 此刻的執行狀態：Idle（待命）、Working（執行中）、Waiting（等待中）、Sleeping（睡眠）、Paused（暫停）或 Error（錯誤）。

**7. Inbox（收件匣）** —— Employee 的工作佇列。每次 Employee 被喚醒，就處理它的 Inbox。這是所有進入工作的前門。工作經由 Trigger 投遞——例如人類的一則訊息（Message-driven Trigger）會成為 Inbox 裡的一個 Task，並喚醒該 Employee。這趟互動本身另以 Message（Ch.16）留存為可回顧的互動紀錄。

**8. Commitments（長期職責）** —— Employee 的長期責任：追蹤這張訂單到交貨、監控這件客訴、讓這次稽核隨時備妥。Commitment 可能持續數週甚至數月，並產生許多 Task。

**9. Memory（工作記憶）** —— Employee 的*工作*記憶，而不是它的知識：今天聯絡了誰、供應商答應了什麼、主管要求優先處理什麼。它是當前工作循環的暫存區。

**10. Metrics（績效指標）** —— 績效資訊：完成任務數、平均回應時間、工具使用量、成本、成功率。衡量 Employee 的依據。

---

## 4. 不擁有

Employee **不**擁有：

- **排程器** —— 何時喚醒是 Runtime 的決定，由 Trigger 驅動。
- **Workspace** —— Employee 活在 Workspace 之內，但不控制它。
- **Project** —— Employee 參與 Project，但不擁有 Project 這個概念。
- **知識庫** —— 知識屬於 Brain；Employee 只是引用它。
- **其他 Employee** —— Employee 是同儕，而非系統內部的監督者。（Employee 之間的協調是協作，不是擁有。）

一個嘗試自我排程、重新設定 Workspace、或改寫共享知識的 Employee，已經越過了它的邊界。

---

## 5. 生命週期

### 5.1 執行狀態

在任何時刻，一個 Employee 恰好處於一個狀態：

```
        Created
           │
           ▼
   ┌─── Idle ◄────────────┐
   │       │              │
   │       ▼              │
   │    Working           │
   │       │              │
   │       ▼              │
   │    Waiting ──────────┘   (resume when unblocked)
   │
   ├──► Sleeping   (預設休憩狀態；上下文已持久化)
   ├──► Paused     （被操作者保留；不會自動喚醒）
   └──► Error      （失敗；需要關注）
```

- **Idle（待命）** —— 清醒、沒有當前工作；會處理下一個 Inbox 項目。
- **Working（執行中）** —— 正在執行。
- **Waiting（等待中）** —— 被某個外部事物擋住（回覆、工具結果、相依項）。
- **Sleeping（睡眠）** —— **預設** 狀態。Employee 在休息，上下文已持久化，未駐留於記憶體。
- **Paused（暫停）** —— 刻意保留；Trigger 不會喚醒它。
- **Error（錯誤）** —— 某件事失敗了；Employee 需要介入才能繼續。

**睡眠是預設。** Employee 只有在 Trigger 觸發、且 Inbox 有工作時才被喚醒。

### 5.2 存在生命週期

除了日常狀態之外，Employee 還有一個壽命：

```
Created → Assigned → Active → … → Retired → Archived
```

一個 Employee 可以被建立、被賦予 Role 與 Brain、工作數月或數年、被重新指派到新 Role，最終被退役。它產出的成果與歷史在退役後依然留存。

---

## 6. Spec 與 Status

為了支援版本管理、快照與部署，Employee 被分為兩層。

**Employee Spec（規格）** —— 相對固定：

- Identity
- Brain（引用）
- Role
- Capability
- Permission / Authority（職權）
- Resources

**Employee Status（運行狀態）** —— 持續變化：

- State
- Inbox
- Current Task
- Commitments
- Working Memory
- Metrics

Spec 回答的是 *「這是怎樣的一個員工？」* Status 回答的是 *「這個員工此刻在做什麼？」* 把兩者分開，才能對 Employee 做版本管理、快照，並把同一份 Spec 部署為多個實例。

---

## 7. Template 與 Instance

為了企業級的部署，Employee 分為兩層：

```
Employee Template（範本）
       │
       ▼
Employee Instance（實例）
```

一個 **Template** 定義某一類員工 —— 例如「採購助理 Steve」。**Instance** 則是該 Template 的具體部署：

- Steve @ 台灣廠
- Steve @ 南京廠
- Steve @ 越南廠

三個 instance **共用** 來自 Template 的 Brain、Role 與 Capability。每個 instance **各自獨立擁有** 自己的 Inbox、Commitments、Memory 與 KPI —— 因為每個廠有自己的訂單、供應商與優先順序。

這就是一個設計良好的員工，如何在不變成一個過載代理的前提下，擴展到整個組織。

---

## 8. 未來擴展

Employee 未來可能支援：

- **多代理協作** —— Employee 之間互相交付 Task、共享上下文，並在一個 Project 內組成團隊。
- **分身（Clone）** —— 為了平行工作產生 Employee 的暫時副本，再合併結果。
- **技能學習** —— Employee 從經驗中改善自己的 SOP 與能力，並回饋到它的 Brain。
- **委派與監督** —— Employee 之間的結構化職權（一個主管 Employee 指揮其他），但不違反「沒有 Employee 擁有另一個」的規則。
- **市集市場** —— Employee Template 可跨 Workspace 發布與匯入。

無論未來加入什麼擴展，規則依然成立：**Employee 是工作者，Brain 是知識，兩者絕不坍縮為一。**
