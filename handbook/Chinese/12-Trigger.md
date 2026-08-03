# 第十二章 — Trigger 觸發器

版本：0.1
狀態：草稿

---

## 1. 目的

Trigger 是**喚醒 Employee 的東西**。

預設上，Employee 在睡眠。必須有什麼東西判斷有工作要做，並喚醒對的 Employee。那個東西就是 Trigger。

Trigger 把工作*何時* 發生，與工作是*什麼* 解耦開來。Employee 知道要做什麼；Trigger 知道何時該開始。這個分離讓系統能對世界做出反應，而不必讓每個 Employee 永久駐留。

Trigger 從不執行工作。它只發出訊號。

---

## 2. 職責

一個 Trigger 對以下事項負責：

- **監看一個條件** —— 一個事件、一個時間、一則訊息、一道人工指令。
- **辨識目標** —— 哪個（哪些）Employee 該被喚醒。
- **承載上下文** —— Employee 用以理解自己為何被喚醒所需的資訊。
- **通知 Runtime** —— 交班，而非執行。
- **投遞工作（承載為工作時）** —— 當 Trigger 的承載本身是一則待辦，它在通知 Runtime 的同時，把該承載交付為目標 Employee 的 Inbox 裡的一個 Task。最常見的例子：人類的一則訊息（Message-driven Trigger）成為 Inbox 裡的一個 Task，並喚醒該 Employee。

---

## 3. 擁有

一個 Trigger 擁有：

- 它的**條件或規則** —— 它精確監看的事物。
- 它的**目標 Employee** —— 它喚醒誰。
- 它的**承載（payload）** —— 觸發時交付的上下文。
- 它的**啟用狀態** —— 它目前是否武裝。

### Trigger 的種類

```
Trigger
├── Event-driven     （某事發生 —— 一個 Event 抵達）
├── Time-driven      （某個排程或期限到達）
├── Message-driven   （收到一則訊息或請求）
└── Manual           （人類或另一個 Employee 明確地觸發它）
```

---

## 4. 不擁有

一個 Trigger **不**擁有：

- **工作本身** —— 那是 Employee 的。
- **該做什麼的決策** —— Trigger 說「醒來，這是原因」；Employee 決定要怎麼處理。
- **排程政策** —— 優先順序與並行限制屬於 Runtime。

Trigger 是鬧鐘，不是行為者。

---

## 5. 生命週期

```
Defined → Armed → Fired → Reset → (Armed again …)
```

- **Defined（定義）** —— 它的規則與目標已設定。
- **Armed（武裝）** —— 正在監看。
- **Fired（觸發）** —— 條件被滿足；它已帶著承載通知 Runtime。
- **Reset（重置）** —— 準備再次觸發（對重複型 Trigger），或退役（對一次性 Trigger）。

一個 Trigger 可在其生命週期內觸發許多次，或恰好一次，視設計而定。

---

## 6. 未來擴展

Trigger 概念未來可能支援：

- **複合條件** —— 只有當多個條件同時成立時才觸發。
- **過濾與路由** —— 精細的規則，決定哪個 Employee 為哪個情境而醒。
- **去抖動與批次** —— 當許多事件同時抵達時，避免冗餘的喚醒。
- **跨工作空間觸發** —— 一個工作空間裡的事件，在受控條件下喚醒另一個工作空間的 Employee。
