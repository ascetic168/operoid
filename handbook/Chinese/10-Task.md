# 第十章 — Task 任務

版本：0.1
狀態：草稿

---

## 1. 目的

Task 是**一個可執行的目標**。

它是 Employee 在單一連續段內執行的最小工作單位：「草擬這封信」、「查這張訂單」、「產出這份報告」、「更新這筆紀錄」。一個 Task 有明確的擁有者、明確的輸入與明確的輸出 —— 而當輸出被提交時，Task 就完成了。

Task 刻意是**短命的**。它們活在 Employee 的 Inbox 裡，被完成，然後結束。持續數週的工作不是 Task；它是*產生* Task 的 Commitment。

---

## 2. 職責

一個 Task 對以下事項負責：

- **指出它的目標** —— 具體而言，必須完成什麼。
- **指出它的擁有者** —— 哪個 Employee 會執行它。
- **承載它的輸入** —— 做這份工作所需的資訊或 Artifact。
- **產出它的輸出** —— 標記完成的 Artifact 或結果。
- **追蹤它的狀態** —— 它此刻在哪裡。

---

## 3. 擁有

一個 Task 擁有：

- 它的**輸入** —— 參數、來源 Artifact、參考。
- 它的**輸出** —— 產出的 Artifact 或結果。
- 它的**狀態** —— 以及它如何走到這一步的歷史。
- 它的**連結** —— 它所屬的 Commitment 或 Project（若有的話）。

一個 Task 隸屬於恰好一個 Employee（它的擁有者），並活在該 Employee 的 Inbox 裡直到完成。

---

## 4. 不擁有

一個 Task **不**擁有：

- **目的的持久性** —— 那屬於 Commitment。Task 結束；它的理由可能繼續。
- **那個 Employee** —— Task 被指派*給* 一個 Employee；它不控制 Employee。
- **其他 Task** —— Task 之間的排序由 Employee 與 Commitment 管理，而非由個別 Task。

Task 是工作的一個事件，不是一個持續的結構。

---

## 5. 生命週期

```
Created → Assigned → In Progress → Waiting → Completed
                                   │
                                   └─► Failed / Cancelled
```

- **Created（建立）** —— 已定義，等待擁有者。
- **Assigned（指派）** —— 已放入某個 Employee 的 Inbox。
- **In Progress（執行中）** —— Employee 正在執行它。
- **Waiting（等待中）** —— 被外部相依項擋住（回覆、工具結果）。
- **Completed（完成）** —— 輸出已提交；Task 完成。
- **Failed / Cancelled（失敗／取消）** —— Task 無法或不該完成；原因被記錄。

多數 Task 應該很快地從 Assigned 走到 Completed。一個拖延的 Task 是一個訊號：它本該是個 Commitment，或者它被卡住了。

---

## 6. 未來擴展

Task 概念未來可能支援：

- **分解** —— 一個大 Task 拆成子 Task，並可選擇指派給其他 Employee。
- **相依性** —— 宣告某個 Task 在另一個完成前不得開始。
- **範本** —— 為重複性工作設計的可重用 Task 定義。
- **估計** —— 預期的工時與成本，用於規劃與指標。
