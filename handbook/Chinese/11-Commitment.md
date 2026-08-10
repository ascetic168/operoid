# 第十一章 — Commitment 長期職責

版本：0.1
狀態：草稿

> Commitment 比 Task 活得更久。（原則 9。）

---

## 1. 目的

Commitment 是**持久的職責**。

Task 是一個會結束的單一可執行目標；而 Commitment 是一項持續的職責，直到一個完成條件被滿足才結束。「追蹤這張採購單直到收貨。」「監控這件客訴直到結案。」「讓這次稽核隨時備妥直到結束。」

Commitment 是 Operoid 用來表達組織中*真正重要* 工作的方式：那些跨越數日、數週或數月、任何單一對話都無法承載的責任。

Commitment 不是一個比較大的 Task。它是另一種物件 —— 一個會在其生命週期中*產生* Task 的物件。

---

## 2. 職責

一個 Commitment 對以下事項負責：

- **宣告它的完成條件** —— 使它被滿足的確切狀態。
- **擁有一個 Employee** —— 那個為把它完成而負責的人。
- **產生 Task** —— 一而再、再而三地，把這項長期責任拆解為可執行的工作。
- **跨越喚醒而續存** —— 撑過每一次休眠、每一個 session、每一次模型替換，直到被滿足。
- **記錄它的歷史** —— 為它做過什麼、還剩什麼。

---

## 3. 擁有

一個 Commitment 擁有：

- 它的**完成條件** —— 「完成」的定義。
- 它的**擁有 Employee** —— 誰該負責。
- 它**產生的 Task** —— 它所產出的可執行工作。
- 它的**狀態與歷史** —— 進度、里程碑、事件。
- 它的**連結** —— 連向 Artifact、Project，以及它所涉及的人或系統。

---

## 4. 不擁有

一個 Commitment **不**擁有：

- **逐一刻的執行** —— 那是 Employee 的事，一次一個 Task。
- **Employee 的全部注意力** —— 一個 Employee 可同時承擔多個 Commitment。
- **所需的知識** —— 那是從 Brain 與知識庫引用來的。

Commitment 定義的是*什麼必須隨時間保持為真*；它不執行讓它保持為真的工作。

---

## 5. 生命週期

```
Proposed → Active ──────────────► Satisfied → Archived
   │            │                     ▲
   └─► Rejected ├─► Suspended ─────────┘   (resumed)
                │
                └─► spawns Tasks repeatedly throughout its life
```

- **Proposed（提案）** —— Employee 在對話中識別出一件該長期追蹤的事，主動提案（含完成條件）。等待人類核可。未核可前不運行（不產生 Task、不喚醒）。
- **Active（運作中）** —— 人類核可（或直接交辦）後，Employee 正在處理它，視需要產生 Task。
- **Suspended（暫停）** —— 刻意暫停；但未被遺忘。
- **Satisfied（已滿足）** —— 完成條件已達成。
- **Rejected（拒絕）** —— 人類拒絕了員工的提案；未進入 Active 即終止。
- **Archived（封存）** —— 連同完整歷史保留。

一個 Commitment 的壽命是對著它的**完成條件**來衡量，而不是對時間。當條件被滿足，Commitment 就結束 —— 無論為此花了多少個 Task。

---

## 6. 未來擴展

Commitment 概念未來可能支援：

- **層級** —— 一個 Commitment 把子職責委派給其他 Employee。
- **滿足證據** —— 結構化的證據，證明完成條件確實被滿足。
- **升級** —— 當一個 Commitment 停滯時，自動路由給人類或主管 Employee。
- **模式** —— 為常見組織職責（訂單追蹤、稽核備妥、事件監控）設計的可重用 Commitment 範本。
