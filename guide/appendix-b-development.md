# 附錄 B：Operoid 開發說明

> 本章撰寫中。目標讀者：要在 Operoid 上開發自家接口、或 fork 修改的企業團隊（假設以 Vibe Coding／coding agent 開發）。

## 預定內容

- 場景與策略：企業導入後的兩條路——經 obridge 外掛擴充 vs fork 修改
- 資訊地圖：契約（ocontract／obridge WIT）、範例（obridge-plugin-example）、文件（obridge README／handbook／DEPLOYMENT）、原始碼導覽
- 完整範例：接通企業 IM（WASM 通道插件）——從盤點 IM 收發介面到部署驗證
- **示範提示詞**：可直接複製給 Claude Code 的 prompt 範本（含應讀檔案、契約限制、測試與驗收條件）
- Fork 修改：workspace 結構、`cargo dev-build`（build id 互相核對）、上游同步
- 安全注意：token／secret 管理、IM 憑證、日誌

---
[← 附錄 A](appendix-a-gbrain.md)｜[索引](README.md)
