# 企業版三種角色介面

> 本章撰寫中。部署見 [DEPLOYMENT.md](../DEPLOYMENT.md)。

## 預定內容

- 登入：密碼（首次登入強制改密碼）、連續 5 次失敗鎖定 15 分鐘；角色 admin ⊃ manager ⊃ user
- `/admin`（管理者）：帳號管理（建立／停用／重設密碼）、模板 CRUD、email bridge 表單、技能共用庫與晉升、知識授權、Version 卡片（更新檢查，可離線關閉）
- `/manager`（經理人儀表板）：團隊員工、提案審核、監看
- `/user`（一般使用者）：自己的員工、聊天、技能提案與採用；可與同事的員工對話（自 v0.5.9 起）
- 與個人版桌面介面的功能對照表

---
[← 15 服務與更新](15-service-and-updates.md)｜[索引](README.md)｜[17 設定參考 →](17-config-reference.md)
