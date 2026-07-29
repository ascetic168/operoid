# 第十三章 — Runtime 執行引擎

版本：0.1
狀態：草稿

> Runtime 管理執行。它從不管理思考。（原則 10。）

---

## 1. 目的

Runtime 是 Emploid 的**引擎**。

它是作業系統核心的對應物：它不做工作，但它讓工作成為可能。Runtime 決定哪個 Employee 執行、何時、多久、用什麼上下文 —— 並確保工作完成時，結果在 Employee 回到睡眠之前被安全地提交。

Part II 與 Part III 其餘章節裡的一切 —— Employee、Task、Commitment、Trigger、Event、Memory —— 都是 Runtime 搬動的*素材*。Runtime 就是那個搬動它們的東西。

這一章只定義單一執行單位的**生命週期**。它刻意不描述排程演算法、並行模型或任何實作技術。那些會變；生命週期不會。

---

## 2. 職責

Runtime 對以下事項負責：

- **喚醒** 對的 Employee（當 Trigger 觸發時）。
- **恢復上下文** —— 給予 Employee 它的 Inbox、Commitment、Memory，以及其 Brain 的相關切片，每次都重新組建。
- **執行** —— 讓 Employee 推理、決策、調用 Tool。
- **強制紀律** —— 逾時、權限、資源限制與隔離。
- **提交成果** —— 確保輸出在 Employee 休眠前被持久化。
- **持久化狀態** —— 儲存 Employee 的 Memory 與狀態，讓下一次喚醒是乾淨的恢復，而非流失。
- **在沒有工作時讓 Employee 回到睡眠**。

---

## 3. 擁有

Runtime 擁有：

- **執行迴圈** —— 喚醒 → 恢復 → 執行 → 提交 → 睡眠 的循環。
- **排程器** —— 哪個 Employee 何時執行、同時跑幾個的政策。
- **生命週期紀律** —— 強制每一次執行都通過同樣定義明確的階段。
- **隔離邊界** —— 讓一個 Employee 的執行不干擾另一個。

---

## 4. 不擁有

Runtime **不**擁有：

- **思考** —— Employee 想什麼，是 Employee 自己的事，來自它的 Brain。Runtime 從不告訴 Employee 該下什麼結論。
- **業務決策** —— 是否核可一張訂單、如何回應一件客訴、報告裡寫什麼：全是 Employee 的。
- **知識或記憶的內容** —— 那屬於 Brain 與 Employee。
- **結果的責任** —— 那屬於 Employee。

這是系統中最重要的邊界。Runtime 一旦開始影響 Employee *該想什麼*，兩件事會同時崩壞：Runtime 變得無法測試，而 Employee 變得無法問責。

---

## 5. 生命週期 —— 執行循環

這是 Runtime 對每一個 Employee、每一次工作都強制的規範循環：

```
       ┌───────────────────────────────────────────────┐
       │                                               ▼
   Trigger ──► Wake ──► Restore Context ──► Execute ──► Commit ──► Sleep
   fires                                       │        Artifact
                                               │
                                          (調用工具、
                                           產出結果)
```

**1. Wake（喚醒）。** 一個 Trigger 觸發。Runtime 選出目標 Employee，把它從睡眠狀態帶出，準備執行。

**2. Restore Context（恢復上下文）。** Runtime 重建 Employee 的工作上下文：它的 Inbox、進行中的 Commitment、工作記憶，以及其 Brain 的相關部分。沒有任何東西被假設「還在記憶體裡」。上下文每次都被刻意重建。（原則 8。）

**3. Execute（執行）。** Employee 推理並行動 —— 讀取 Inbox、做出決策、調用 Tool、產出結果。Runtime 在旁監看、強制限制，並把發生的事記錄為 Event，但它不介入推理。

**4. Commit Artifact（提交成果）。** 在 Employee 可以休息之前，Runtime 確保每一份該持久化的輸出都被安全地提交為 Artifact，並確保 Employee 的 Memory 與狀態被儲存。這是暫時與真實之間的邊界。

**5. Sleep（睡眠）。** 工作已提交、狀態已持久化，Employee 回到它預設的睡眠狀態。它什麼也不駐留。下一次喚醒時它可以被完美地恢復。

---

## 6. 為何上下文是「恢復」而非「記住」

一個長跑行程把狀態留在記憶體裡，一旦崩潰就會失去。Employee 不是長跑行程。

藉由在每次喚醒時恢復上下文、在每次休眠時持久化上下文，Emploid 保證 Employee 的狀態是**持久且可重建的**。崩潰、重啟、換模型、或遷移，絕不會毀掉進行中的工作 —— 因為工作在 Employee 睡眠前就被提交了，上下文也被儲存了。

這正是一個工作空間能容納數千個 Employee、卻只有有工作的那些才曾被喚醒的原因。

---

## 7. 未來擴展

Runtime 未來可能支援：

- **平行與並行執行** —— 許多 Employee 同時工作，且安全隔離。
- **搶佔與背壓** —— 當想執行的 Employee 多過資源所能負荷時，管理負載。
- **協作執行** —— Employee 在一個 Project 內彼此讓步。
- **分散式 Runtime** —— 執行分散到多個節點，狀態依然在每次喚醒時被持久地恢復。
- **可觀測性** —— 為除錯與指標而對執行循環做豐富的追蹤。

無論 Runtime 獲得什麼，核心契約是固定的：**它管理執行的循環，把思考留給 Employee。**
