# 安裝企業版

> 適用版本：v0.5.11。本章是企業版部署的**總覽與名詞對照**；完整逐步流程以 [DEPLOYMENT.md](../DEPLOYMENT.md) 為權威（該文件隨部署包附上，內容同倉庫）。

## 部署包內容

從 [GitHub Releases](https://github.com/ascetic168/Operoid/releases) 下載對應平台的部署包，解壓到伺服器機即可——**伺服器不需要 Node/pnpm**：

| 平台 | 檔案 |
|---|---|
| Windows x64 | `operoid-enterprise-windows-x64.zip` |
| Linux x64 | `operoid-enterprise-linux-x64.tar.gz` |

包內容：`oserver` 執行檔（HTTP 服務＋員工執行引擎）、`obridge` 執行檔（Email／IM 橋接）、三個角色前端靜態檔（`frontends/{admin,manager,user}`）、`DEPLOYMENT.md`、`operoid.example.toml`、`deploy/local/mm-bridge/`（多模態嵌入轉譯層，選配）。

## 伺服器機前置需求

| 項目 | 說明 |
|---|---|
| GBrain | 安裝於伺服器機（`gbrain` CLI）；腦（GBRAIN_HOME）與 notes repo 同機 |
| 嵌入服務 | 本機 llama.cpp `llama-server`（port 8080，EmbeddingGemma 2，768 維）——GBrain 嵌入／查詢**硬依賴**，須常駐 |
| TLS 憑證 | 內網 CA 簽發（首選）或自簽；非 loopback 監聽**必須**配置 TLS，半配置（只有 cert 或只有 key）會被拒絕啟動（fail closed） |
| LLM API keys | 寫在 `operoid.toml` 的 `[llm] env`——服務帳戶看不到使用者環境變數 |
| 多模態（選配） | 圖片嵌入需部署 `deploy/local/mm-bridge/` 轉譯層（bun 單檔，8090 → 8080） |

## 部署設定精靈：`oserver configure`

```bash
./oserver configure --data-dir /opt/operoid/data        # Linux
.\oserver.exe configure --data-dir C:\operoid           # Windows
```

精靈依序進行（Enter 採預設值，`?` 顯示說明，寫入前有摘要確認閘）：

1. **前置檢查**——探測 git／bun／gbrain，缺項給安裝指引。
2. **部署模式**——個人（本機 loopback）或企業（內網服務）。
3. **監聽與前端**——bind host/port、frontends 目錄、TLS cert/key。
4. **知識腦工作目錄**——gbrain 路徑與 `gbrain_home`；可現場 `gbrain init` 建新腦。
5. **master token**——現場生成 64 碼（僅此一次完整顯示）。
6. **管理員帳號**——預設 `admin`／`admin1234`，首次登入**強制**改密碼（≥8 碼）。
7. **系統服務**——可選擇當場註冊開機自啟。

⚠️ 註冊服務前務必先完成精靈——服務等無終端環境首次執行時不會自己彈出精靈，會直接啟動失敗。

## 服務註冊

```bash
./oserver install      # 註冊開機服務（Windows 為 SCM 服務「Operoid」，需系統管理員）
./oserver status       # {"installed": true, "running": true}
./oserver uninstall    # 移除
```

Windows 為已實機驗證路徑；Linux（systemd）／macOS（launchd）已實作，安裝注意事項見 [DEPLOYMENT.md §11.3](../DEPLOYMENT.md)。防火牆需開放 7340 port（指令見 §5）。

## 首次登入

1. 瀏覽器開啟 `https://<伺服器IP>:7340/`——三個前端的入口頁。
2. 用精靈建立的管理員帳號登入 `/admin`；首次登入系統強制設定新密碼。
3. 在「帳號管理」建立 manager／user 帳號；各角色的日常操作見[第 16 章](16-enterprise-consoles.md)。

## 升級

企業版**不自動更新**：admin 介面「版本」卡片可檢查新版（結果快取 24 小時；氣隔環境可以 `operoid.toml` 的 `[update] check_enabled = false` 關閉對外連線）。升級是 zip 覆蓋安裝：停服務 → **備份資料目錄** → 解壓新版覆蓋 → 起服務 → 核對 `/healthz` 的 version。完整程序與注意事項見 [DEPLOYMENT.md §12](../DEPLOYMENT.md)。

## DEPLOYMENT.md 章節對照

| 想做什麼 | 查 |
|---|---|
| 架構總覽 | §1 |
| 伺服器前置、嵌入服務啟動指令 | §2、§10.1 |
| `operoid.toml` 每個欄位 | §3（另見本指南[第 17 章](17-config-reference.md)） |
| TLS 憑證 | §4 |
| 服務註冊、防火牆 | §5 |
| 帳號開通（含不用精靈的手動流程） | §6 |
| obridge 郵件橋接 | §7（本指南[第 14 章](14-email-bridge.md)） |
| 部署後驗收清單 | §8 |
| 部署疑難排解 | §9（本指南[第 18 章](18-troubleshooting.md)） |
| PDF 知識管線、MinerU | §10 |
| 升級 | §12 |

---
[← 03 安裝個人版](03-install-personal.md)｜[索引](README.md)｜[05 快速上手 →](05-quickstart.md)
