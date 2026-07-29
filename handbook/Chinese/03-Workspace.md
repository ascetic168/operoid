# 第三章 — Workspace 工作空間

版本：0.1
狀態：草稿

---

## 1. 目的

Workspace 是**組織**。

它是 Emploid 中最外層的容器。一切存在的事物 —— 每一個 Employee、Brain、Artifact、知識庫、Tool、Project、Task 與 Commitment —— 都隸屬於恰好一個 Workspace。

Workspace 的存在，是為了執行系統中最重要的邊界：**什麼屬於一起** 的邊界。屬於同一個組織的工作、知識、職權與成果，絕不會外漏到另一個組織。

Workspace 不執行工作。它是工作發生的地方。

---

## 2. 職責

Workspace 對以下事項負責：

- **租戶管理** —— 擁有其中的每一個物件，並確保沒有任何東西無所歸屬。
- **隔離** —— 讓自己的內容與其他每一個 Workspace 保持分離。
- **託管** —— 提供 Employee、Tool 與知識共存的共享環境。
- **組織的生命週期** —— 作為一個整體被建立、暫停與退役。
- **整體的身份** —— 承載組織的名稱、設定與政策。

---

## 3. 擁有

一個 Workspace 擁有：

- **Employee** —— 每一個工作者都隸屬於它。
- **Brain** —— 其員工可用的智能。
- **Knowledge** —— 組織經策展的知識庫。
- **Artifact** —— 在其中產出的每一份持久成果。
- **Tool** —— 為在其中使用而設定的能力。
- **Project** —— 進行中的專案。
- **Commitment 與 Task** —— 工作，包含持久的與即時的。

這些全都從屬於 Workspace。沒有一個能比它的 Workspace 活得更久；當 Workspace 退役時，其內容會隨之封存。

---

## 4. 不擁有

Workspace **不**擁有：

- **工作的動作本身** —— 那屬於 Employee。
- **思考** —— 那屬於 Brain 與 Employee。
- **工具執行的邏輯** —— 那屬於 Tool。
- **排程決策** —— 那屬於 Runtime。

Workspace 是容器與邊界，不是行為者。一個開始做決策的 Workspace，已經越過了它的角色。

---

## 5. 生命週期

```
Created
  │
  ▼
Active  ◄────────┐
  │              │
  ▼              │
Suspended ───────┘   (resume)
  │
  ▼
Archived
```

- **Created（建立）** —— Workspace 被初始化；可以加入第一批物件。
- **Active（運作）** —— 正常運作；工作流動、Employee 執行、Artifact 累積。
- **Suspended（暫停）** —— 暫停運作；沒有 Employee 被喚醒、沒有 Trigger 觸發；內容被保留。
- **Archived（封存）** —— 唯讀退役；Workspace 與其中的一切以歷史形式保留，不再運作。

Workspace 是長壽的。它的生命週期以年為單位，而不是以 session 計。

---

## 6. 未來擴展

Workspace 未來可能支援：

- **聯邦（Federation）** —— 在不合併 Workspace 的前提下，受控地共享知識或成果物。
- **子工作空間** —— 較大組織內的部門或團隊，擁有自己的邊界。
- **範本** —— 預先設定好的 Workspace（一組 Employee、Brain 與 Tool），可為新組織複製。
- **遷移** —— 把一個 Workspace 與其內容搬移到另一個部署。
