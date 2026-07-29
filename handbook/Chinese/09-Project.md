# 第九章 — Project 專案

版本：0.1
狀態：草稿

---

## 1. 目的

Project 是一個**有邊界的倡議**。

並非所有工作都是持續的職責。有些工作有目標、有終點：發布一個產品、完成一次遷移、解決一次重大事件、交付一份研究。Project 這個物件，就是把對的 Employee、Artifact、Commitment 與 Task 聚集在這樣一個目標周圍，並給它們一個共享的上下文。

若 Workspace 是組織，Project 就是組織*之內*的一項倡議。

---

## 2. 職責

一個 Project 對以下事項負責：

- **定義範疇** —— 這項倡議包含什麼，以及同樣重要的，不包含什麼。
- **指出目標** —— 「完成」的定義。
- **聚集參與者** —— 協作的 Employee（與人類）。
- **提供共享上下文** —— 一個讓該倡議的 Artifact、Commitment 與 Task 匯聚的單一所在。

Project 是一個框架，不是工作者。它組織工作；它不執行工作。

---

## 3. 擁有

一個 Project 擁有：

- 它的**目標與範疇聲明**。
- 它的**參與者清單** —— 哪些 Employee 與人類參與（參與，而非擁有這些 Employee 本身）。
- 它的**Artifact** —— 為該倡議產出的成果。
- 它的**Commitment 與 Task** —— 範疇在該倡議內的工作。
- 它的**時程** —— 里程碑、期限與狀態。

Project 隸屬於一個 Workspace。它的 Employee 仍屬於該 Workspace；Project 只記錄誰在參與。

---

## 4. 不擁有

一個 Project **不**擁有：

- **那些 Employee** —— 他們是參與；Workspace 才擁有他們。
- **Employee 的其他工作** —— 一個 Employee 可同時服務多個 Project 與多個 Commitment。
- **Brain 或 Knowledge** —— 那些是共享的工作空間資產。
- **執行** —— Project 不做工作；它的參與 Employee 才做。

一個嘗試獨占其 Employee 的 Project，已經變成一個穀倉，而不是協作。

---

## 5. 生命週期

```
Proposed → Active → On Hold → Completed → Archived
              ▲                  │
              └──────────────────┘   (reopened, if needed)
```

- **Proposed（提案）** —— 已定義但尚未開始。
- **Active（進行中）** —— 工作正在進行。
- **On Hold（暫停）** —— 暫停；為恢復而保留。
- **Completed（完成）** —— 目標已達成；Project 可關閉。
- **Archived（封存）** —— 連同其 Artifact 一起以歷史形式保留。

Project 是注定要結束的。一個永遠不結束的 Project，很可能其實是個偽裝的 Commitment。

---

## 6. 未來擴展

Project 概念未來可能支援：

- **範本** —— 預先定義的 Project（一個標準團隊、一組標準 Task），可被複製。
- **組合（Portfolio）** —— 把相關 Project 分組以利監督。
- **相依性** —— 宣告某個 Project 的產出是另一個的輸入。
- **里程碑與報告** —— 對人類與 Employee 都可見的結構化進度。
- **跨工作空間協作** —— 在受控條件下，邀請另一個 Workspace 的 Employee 加入一個 Project。
