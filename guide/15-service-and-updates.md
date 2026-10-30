# 服務與更新

> 適用版本：v0.5.11。本章說明個人版的本地服務（oserver）生命週期與版本更新；企業版的服務部署見[第 04 章](04-install-enterprise.md)。

## 本地服務是怎麼跑的

Operoid 桌面應用是外殼；真正執行員工的是**本地服務 oserver**（監聽 `127.0.0.1:7340`，token 自動產生並保存）。兩種執行模式，由「設定 → 服務」的**「開機自動啟動 Operoid 服務」**開關決定：

| 模式 | 行為 |
|---|---|
| **未註冊**（預設） | 開啟 App 時啟動服務，關閉 App 時一併停止——背景不常駐 |
| **已註冊**（開機服務） | 開機自啟、常駐背景；關閉 App 不影響運行——**員工持續工作**（排程、email 進來的任務照樣處理） |

狀態隨時可在「設定 → 服務 → 本地服務（oserver）」查看（運行中／未運行、port）。

命令列等價操作（進階）：

```bash
oserver install     # 註冊開機服務（Windows 需系統管理員；會走 UAC 提權）
oserver status      # {"installed": true, "running": true}
oserver uninstall   # 移除
oserver version     # 版本＋build hash＋build time
```

平台細節：Windows 為 SCM 服務「Operoid」（已實機驗證）；Linux 為 systemd、macOS 為 launchd（以 `sudo oserver install` 安裝，服務以安裝者身分執行，見 [DEPLOYMENT.md §11.3](../DEPLOYMENT.md)）。升級／解除安裝時，安裝精靈會自動停止／恢復服務。

## 服務模式下的 API key

開機服務以系統服務帳戶執行，**看不到你帳戶的環境變數**。Operoid 會在啟動／註冊服務時，把當時存在於你環境中的 provider API key 快照進設定檔——所以：**更換或新增 API key 後，重啟 App（或重註冊服務）**才會生效。企業版則直接寫在 `operoid.toml` 的 `[llm] env`。

## 版本與更新

「設定 → 應用程式 → 版本與更新」：

| 開關 | 預設 | 行為 |
|---|---|---|
| **更新提示** | 開 | 啟動時（及每 24 小時）檢查 [GitHub Releases](https://github.com/ascetic168/Operoid/releases)；有新版顯示橫幅，**安裝一律需你按下確認** |
| **自動更新** | 關 | 偵測到新版即背景下載安裝（minisign 簽章驗證），完成後提示重新啟動 |

⚠️ 本專案**不保證版本間向上相容**——「自動更新」預設關就是這個原因；開啟前請評估，並習慣查閱 [CHANGELOG](../CHANGELOG.md) 的 breaking 條目。

更新未就緒時（例如新版發布管線尚未備妥），頁面會提示稍後再試或到 Releases 頁手動下載。

### 企業版差異

企業版**不自動更新**：`/admin` 的「版本」卡片可檢查新版（結果快取 24 小時；氣隔環境可 `operoid.toml` 設 `[update] check_enabled = false` 關閉對外連線）。升級為手動覆蓋安裝（先備份資料目錄），完整程序見 [DEPLOYMENT.md §12](../DEPLOYMENT.md)。

---
[← 14 電子郵件橋接](14-email-bridge.md)｜[索引](README.md)｜[16 企業版三種角色介面 →](16-enterprise-consoles.md)
