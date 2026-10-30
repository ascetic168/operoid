# 安裝個人版

> 適用版本：v0.5.11。個人版＝桌面安裝檔，裝在自己的電腦，無須建置。

## 下載安裝檔

到 [GitHub Releases](https://github.com/ascetic168/Operoid/releases) 頁面，在最新版本的 **Assets** 找對應平台的檔案：

| 平台 | 檔案 | 說明 |
|---|---|---|
| Windows | `Operoid_*_x64-setup.exe` | **建議**——NSIS 安裝精靈；應用內自動更新即以此格式安裝 |
| Windows | `Operoid_*_x64_en-US.msi` | MSI 安裝包，適合企業軟體派送 |
| macOS (Apple Silicon) | `Operoid_*_aarch64.dmg` | M 系列晶片 |
| macOS (Intel) | `Operoid_*_x64.dmg` | Intel 晶片 |
| Linux | `Operoid_*_amd64.deb`、`Operoid-*_x86_64.rpm`、`Operoid_*_amd64.AppImage` | 依發行版擇一 |

其餘資產不需下載：`.sig` 是應用內自動更新的簽章檔，`latest.json` 是更新清單。

## 安裝了什麼

安裝檔內含完整元件，不需要另外準備：

- **Operoid 桌面應用**（Tauri v2）；
- **oserver** 本地服務（員工執行引擎）；
- **obridge** 外部通道橋接（Email／IM）。

## 各平台安裝步驟

### Windows

1. 執行 `.exe`，照安裝精靈完成。
2. 若之前註冊過「開機自動啟動服務」，安裝與解除安裝時精靈會自動停止／恢復該服務（不需手動處理）。

### macOS

1. 開啟 `.dmg`，將 **Operoid** 拖進「應用程式」。
2. 首次執行若出現 macOS 安全性提示，按「打開」確認。

### Linux

```bash
# Debian / Ubuntu
sudo dpkg -i Operoid_*_amd64.deb
# RPM 系
sudo rpm -i Operoid-*_x86_64.rpm
# AppImage
chmod +x Operoid_*_amd64.AppImage && ./Operoid_*_amd64.AppImage
```

Linux 需要先裝好 `libwebkit2gtk-4.1` 等 Tauri v2 系統依賴，清單見 [Tauri 前置需求](https://v2.tauri.app/start/prerequisites/)。

## 首次啟動

1. 開啟 Operoid——它會自動帶起本地服務（`127.0.0.1:7340`，token 自動產生；服務狀態見「設定 → 服務」）。
2. 若缺少 [前置元件](02-requirements.md)，會出現「缺少前置程式」對話框：列出 git／bun／gbrain 的缺項、用途與安裝連結。安裝後重開 Operoid。
3. 主視窗上方可能出現**能力警告橫幅**（MinerU 未裝、嵌入服務未啟等）——缺項不阻斷使用，但影響知識品質上限；對照[第 02 章](02-requirements.md)的表格決定是否處理。

## 更新

「設定 → 應用程式」的**版本與更新**區塊：

| 開關 | 預設 | 行為 |
|---|---|---|
| **更新提示** | 開 | 啟動時（及每 24 小時）自動檢查新版；有新版顯示橫幅，安裝需你按下確認 |
| **自動更新** | 關 | 偵測到新版即自動下載安裝，完成後提示重新啟動 |

⚠️ 本專案**不保證版本間向上相容**——開啟「自動更新」前請評估；版本間的重大變更見 [CHANGELOG](../CHANGELOG.md)。

更新流程：橫幅顯示「有新版 vX.Y.Z 可用」→「前往更新」→ 下載（顯示進度）→「已更新至 vX.Y.Z，重新啟動後生效」→ 按「重新啟動」。也可以不經 App，直接到 [Releases 頁](https://github.com/ascetic168/Operoid/releases)下載安裝檔覆蓋安裝（NSIS 精靈會自動處理服務）。

## 解除安裝

- **Windows**：「設定 → 應用程式 → 安裝的應用程式」解除安裝；解除時會一併移除註冊的服務。
- **macOS／Linux**：刪除應用程式即可（Linux：套件管理器移除）。

設定與資料（腦、員工、對話歷史等）存於使用者資料目錄，解除安裝不會清除；位置清單見[第 17 章](17-config-reference.md)。

## 常見安裝問題

安裝後打不開、服務未啟動、元件偵測不到——見[第 18 章疑難排解](18-troubleshooting.md)。

---
[← 02 系統需求](02-requirements.md)｜[索引](README.md)｜[04 安裝企業版 →](04-install-enterprise.md)
