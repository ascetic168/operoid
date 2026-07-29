# 第十七章 — Agent SDK 代理介面

版本：0.1
狀態：草稿

> 本章定義的是**介面**，不是實作。
> 這裡沒有程式語言。重點在於為 Employee 暴露的作業命名，讓任何未來的執行引擎都能遵守它們。

---

## 1. 目的

Agent SDK 是 **Runtime 與 Employee 之間的契約**。

它定義了可對 Employee 提出什麼要求 —— 啟動、暫停、恢復、取消 —— 以及可代 Employee 對工作空間提出什麼要求。透過在概念層級固定這份契約，Emploid 確保 Employee 與 Runtime 能各自獨立演進：新的 Brain、新的模型、新的工具協定，沒有一個需要打破這份介面。

---

## 2. Employee 生命週期介面

這些是 Employee 對 Runtime 暴露的作業：

```
Employee
│
├── Run()        — 開始工作：處理 Inbox、推理、行動
├── Pause()      — 暫停執行，保留狀態以供恢復
├── Resume()     — 從 Pause 停下之處繼續
└── Cancel()     — 永久停止，並記錄原因
```

- **Run** 是入口點。一個 Trigger 觸發，Runtime 恢復上下文，然後呼叫 Run。
- **Pause** 與 **Resume** 讓 Runtime 在不失去 Employee 進度的前提下，管理競爭與等待。
- **Cancel** 乾淨地結束當前執行，並附帶一個被記錄的原因。

這四個就是生命週期。Employee 做的其他一切 —— 推理、調用 Tool、提交 Artifact —— 都發生在 Run *之內*。

---

## 3. 工作空間介面

這些是工作空間*對 Employee* 暴露的作業，讓 Employee 能改變世界，而不只是思考世界：

```
Workspace (as seen by an Employee)
│
├── OpenProject()     — 進入某個倡議的上下文
├── CommitArtifact()  — 把一份產出持久化為一級 Artifact
└── PublishEvent()    — 宣告某件事發生了
```

- **OpenProject** 把 Employee 的工作範疇劃定到某個特定倡議，給予它正確的上下文。
- **CommitArtifact** 是把暫時性的工作轉化為持久組織資產的動作。在 Employee 提交之前，沒有什麼是真的。
- **PublishEvent** 讓 Employee 對工作空間的其餘部分發出訊號 —— 這可能透過 Trigger 喚醒其他 Employee。

一個不能提交 Artifact 或發布 Event 的 Employee，只能空談。它無法工作。

---

## 4. 工具調用介面

透過工作空間，Employee 可以調用 Tool：

```
Tool
│
└── Execute()         — 在權限制約下，執行一個已定義的操作
```

- **Execute** 把 Employee 的意圖帶給一個 Tool。Tool 行動；Employee 擁有結果。Runtime 在呼叫周圍強制權限與限制。

---

## 5. 設計規則

Agent SDK 遵循三條規則：

1. **介面是最小的。** Run、Pause、Resume、Cancel、OpenProject、CommitArtifact、PublishEvent、Execute。如果某個作業不在這份清單上，Employee 就不該能直接做它。
2. **介面是穩定的。** 這些名字應該比任何實作都活得久。新增作業需要架構審查。
3. **介面遵守邊界。** 沒有任何作業讓 Employee 能重新設定 Workspace、改寫共享知識、自我排程、或控制另一個 Employee。那些權力不屬於 Employee。

---

## 6. 未來擴展

Agent SDK 未來可能支援：

- **委派** —— 一個 Employee 透過一個結構化作業，請求另一個 Employee 承擔一個 Task。
- **串流** —— 在 Run 期間對長跑工作提供漸進式輸出。
- **自省** —— 讓 Employee 檢視自身狀態與 Commitment 的作業。
- **協商** —— 協作的 Employee 之間的結構化交握。

無論新增什麼，規則依然成立：**SDK 暴露的是生命週期與工作空間作業；它絕不暴露逃離 Employee 邊界的能力。**
