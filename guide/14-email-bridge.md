# 電子郵件橋接（Obridge）

> 本章撰寫中。先行參閱：[obridge README](../obridge/README.md)。

## 預定內容

- Obridge 是什麼：員工與外部世界的橋——email 內建（IMAP／SMTP），IM 走 WASM 外掛
- 讓員工收 email：員工實體綁定對外 email（自 v0.5.9 起，obridge.toml 自動同步）
- obridge.toml：`[[channels]]` email-imap 通道設定（帳號、輪詢間隔、routes 位址→員工）
- 設定 → 應用程式：Obridge 設定檔路徑、執行檔、隨 App 啟動
- 熱重載：通道設定 2 秒內生效；listen／operoid 變更觸發重啟
- 日誌：obridge.log 位置與常見錯誤
- 企業 IM 外掛：預覽[附錄 B](appendix-b-development.md)

---
[← 13 治理](13-governance.md)｜[索引](README.md)｜[15 服務與更新 →](15-service-and-updates.md)
