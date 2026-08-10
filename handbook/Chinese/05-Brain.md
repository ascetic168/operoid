# 第五章 — Brain 大腦

版本：0.1
狀態：草稿

---

## 1. 目的

Brain 是**可重用的智能**。

它持有 Employee *所知* 與 *傾向如何回應* 的一切 —— 與 Employee *是誰*、*負責什麼* 區分開來。把智能抽離成獨立的物件，讓 Operoid 可以讓知識獨立於使用它的工作者之外，被共享、版本化、升級與替換。

Brain 回答的問題是：*「在這個情境下，具備這份專長的人應該如何思考、如何回應？」*

Employee 回答的是：*「我就是在這個情境中的人，而我為結果負責。」*

Brain 從不執行。它是被諮詢的。

---

## 2. 職責

一個 Brain 負責提供：

- **Persona（人格）** —— Employee 說話的性格與語氣。
- **Knowledge（知識）** —— Employee 可引用的經策展專長。
- **Long-term Memory（長期記憶）** —— 跨 session 累積的經驗，有別於 Employee 的短期工作記憶。
- **Prompt（提示詞）** —— 形塑 Employee 推理方式的指令與傾向。
- **Preferred Models（偏好模型）** —— 對哪個語言或嵌入模型最適合此 Brain 的指引。
- **Embedding（嵌入表示）** —— 讓 Brain 能檢索相關知識的向量表示。
- **Skills（技能）** —— Brain 教導其 Employee 的、具名且可重用的能力（如何解讀採購單、如何摘要一份稽核）。

Brain 也負責被**版本化** —— 它的知識與提示詞隨時間以可追蹤的方式演進。

---

## 3. 擁有

一個 Brain 擁有：

- 它的 **Persona** 與 **Prompt**。
- 它的 **Knowledge 引用** —— 它所引用的知識庫切片。
- 它的 **Long-term Memory** —— 依附於這份專長的累積經驗。
- 它的 **Embedding** —— 讓其知識可被檢索的向量。
- 它的 **模型偏好**。
- 它的 **版本歷史** —— Brain v1、v2、v3，每個都是一個連貫的快照。

一個 Brain 可被**多個 Employee** 引用。共享正是重點：一個 Brain 可以服務一整個部門、訓練背景相似的 Employee。

---

## 4. 不擁有

一個 Brain **不**擁有：

- **執行** —— 它從不執行、調用工具或提交成果。那是 Employee 的工作。
- **責任** —— Brain 不為結果負責；使用它的 Employee 才負責。
- **Inbox 或 Commitment** —— 那些屬於 Employee。
- **某個特定任務的工作記憶** —— 那屬於 Employee。
- **職權** —— Brain 不授予任何權限。Employee 的 Role 才授予。

Brain 是專長，不是代理。一個開始執行或承擔職責的 Brain，是被誤用了。

---

## 5. 生命週期

```
Draft → Published (v1) → … → vN → Deprecated → Archived
                          ▲
                          │
                    (Employees upgrade)
```

- **Draft（草稿）** —— 正在撰寫；尚不可用。
- **Published（發布）** —— 一個編號版本上線，可被 Employee 引用。
- **Upgrade（升級）** —— 新版本（v2、v3…）取代舊版本。Employee 可升級到新版本，並自行選擇何時採用。
- **Deprecated（棄用）** —— 新的 Employee 不可再引用；既有者被鼓勵升級。
- **Archived（封存）** —— 以歷史形式保留；不再運作。

因為 Brain 被版本化，組織可以改善集體智能，同時仍能理解某個 Employee 在舊版 Brain 下做過什麼。

---

## 6. 未來擴展

Brain 未來可能支援：

- **技能學習** —— 從使用它的 Employee 的經驗中累積新 Skills，讓專長與時俱進。
- **特化** —— 把一個通用 Brain 分支出特定領域的特化版本。
- **組合** —— 將多個 Brain（或來自不同 Brain 的 Skills）組合成更豐富的專長。
- **市集市場** —— 跨 Workspace 發布 Brain 以供匯入。
- **評測** —— 在升級前對 Brain 的品質與可靠度做正式評分。

貫穿這一切，規則依然成立：**Brain 是知識，絕不是工作者。**
