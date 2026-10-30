# 服務與更新

> 本章撰寫中。

## 預定內容

- 個人版架構：桌面 App 自動帶起 oserver（127.0.0.1:7340，自動產生 token）；關閉 App 即停止
- 「開機自動啟動 Operoid 服務」（設定→服務）：註冊後關閉 App 員工仍持續工作；解除註冊
- `oserver install / uninstall / status` 命令列等價操作
- 版本與更新（設定→應用程式）：檢查更新、更新提示（預設開）、自動更新（預設關——不保證向上相容）
- 更新流程：通知→下載（進度）→重新啟動生效；企業版：Version 卡片＋手動升級（DEPLOYMENT.md §12）
- 服務模式下的 API key：須寫進設定檔（LocalSystem 看不到使用者環境變數）

---
[← 14 電子郵件橋接](14-email-bridge.md)｜[索引](README.md)｜[16 企業版三種角色介面 →](16-enterprise-consoles.md)
