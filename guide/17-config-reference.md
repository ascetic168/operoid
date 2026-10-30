# 設定參考

> 本章撰寫中。企業版伺服器設定詳見 [DEPLOYMENT.md](../DEPLOYMENT.md)。

## 預定內容

- 設定檔總覽表：app-settings.json／operoid.toml／obridge.toml／gbrain config.json——各自的角色與位置
- 資料與日誌位置：設定目錄（%APPDATA% 等）、資料目錄（SQLite、skills 樹、obridge.log）
- app-settings.json：重要欄位表（腦登錄、LLM 取樣與失控保險、員工輸出路徑、skills_personal_dir、skills_union_limit、伺服器三項、Obridge 三項、更新兩項……）
- operoid.toml：存在＝企業模式；`[server]`／`[tls]`（半配置拒絕啟動）／`[llm.env]`／`[gbrain]`／`[ingress]`／`[knowledge]`／`[factory]`／`[update]`；啟動時 vs 熱重載欄位
- obridge.toml：`[operoid]`／`[listen]`／`[[channels]]`
- 環境變數：OSERVER_TOKEN、OSERVER_PORT、GBRAIN_HOME、各家 provider API key
- gbrain config.json：chat_model、models tier、provider_base_urls；DB plane 覆寫與「模型」TAB 的關係

---
[← 16 企業版三種角色介面](16-enterprise-consoles.md)｜[索引](README.md)｜[18 疑難排解 →](18-troubleshooting.md)
