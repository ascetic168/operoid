# 疑難排解

> 適用版本：v0.5.11。原則：**先看橫幅與狀態，再查日誌**。企業部署類問題另見 [DEPLOYMENT.md §9](../DEPLOYMENT.md)。

## 診斷第一步

1. 主視窗上方的**警告橫幅**說了什麼？（能力缺項會在這裡明說）
2. 「設定 → 服務」的**能力矩陣**——哪一項紅？按「重新檢查」。
3. 本地服務狀態：同頁「本地服務（oserver）」是否「運行中」。
4. 相關日誌：`obridge.log`（Email 問題）、來源 repo 的 `conversion-report.json`（PDF 轉換問題）、`oserver status`／`oserver version`（服務問題）。

## 個人版常見症狀

| 症狀 | 原因與處置 |
|---|---|
| 啟動出現「缺少前置程式」 | git／bun／gbrain 缺項——按對話框連結安裝後**重開 App**；路徑不對時到「設定 → 應用程式」修正 `gbrain.exe` 路徑 |
| 「Operoid 本地服務未啟動」 | 重新開啟 App（會自動帶起服務）；或手動執行 `oserver`。port 被佔用時可用 `OSERVER_PORT` 換 port |
| 橫幅「未偵測到 MinerU」 | 複雜 PDF 會降級為快速模式——安裝：`uv tool install "mineru>4.0"`；或在「設定 → 服務」填 MinerU 安裝目錄（`mineru_dir`） |
| 橫幅「嵌入模型未啟用多模態」 | 圖片僅以文字說明索引——llama-server 啟動旗標加 `--mmproj`（投影器 GGUF 下載見 [DEPLOYMENT.md §2](../DEPLOYMENT.md)） |
| 橫幅「llama-server 缺 -b/-ub 8192 旗標」 | 長 chunk 被（預設 512）拒收、知識檢索品質下降——啟動指令補 `-b 8192 -ub 8192` 後重測。企業版可用 `GET /api/knowledge/health` 的 `embedding.long_input_ok` 核對 |
| PDF 轉換警告「降級 pdf_extract」 | MinerU 取用階梯全級落空——`conversion-report.json` 的 `ladder.attempts` 記錄每級落空原因（路徑／權限／版本） |
| 員工喚醒無反應 | 查三前置：git／bun／gbrain；模型 API key 是否已設（「設定 → 模型」的端點解析）；「事件」頁看有沒有被觸發 |
| 員工收不到 email | 見[第 14 章](14-email-bridge.md)排解段落（routes 命中、IMAP 憑證、`obridge.log`） |
| 服務模式 LLM 突然失效 | 服務帳戶看不到使用者環境變數——**換 API key 後要重啟 App／重註冊服務**讓快照更新 |
| 更新後設定／資料不見了 | 不會——資料在設定／資料目錄（[第 17 章](17-config-reference.md)），更新只替換程式。若真異常，檢查是否以不同使用者帳戶登入系統 |

## 企業版常見症狀

| 症狀 | 原因與處置 |
|---|---|
| 啟動失敗「非 loopback 須 TLS」 | `operoid.toml` 補齊 `[tls]`（cert＋key **齊備**），或改 bind 127.0.0.1 |
| 頁面載入但資料全空 | 看瀏覽器 Console（CSP／CORS／憑證）；核對 `/healthz` 的 `version`——常見是**舊版服務還在跑**，殺掉舊行程重啟 |
| 登入回 429 | 連續 5 次密碼錯誤鎖定 15 分鐘（重啟服務即解除）；admin 可用「重設密碼」 |
| obridge 行為像舊版 | 新舊二進位混用——`/api/obridge/status` 的 `exe_build_match=false` 即是；oserver 與 obridge 必須出自同一建置（同一部署包／`cargo dev-build`） |
| 圖片嵌入失敗 | 多模態需要 mm-bridge shim（8090→8080）——見 [DEPLOYMENT.md §2](../DEPLOYMENT.md)「多模態」 |

## FAQ

**Q：忘記管理員密碼？** 另一位 admin 可重設；或以 master token 經 API 重建帳號（[DEPLOYMENT.md §6](../DEPLOYMENT.md) 手動流程）。

**Q：可以兩台電腦共用一顆腦嗎？** 腦是本機目錄；跨機請走企業版（一台伺服器、大家連上去），不要直接共享資料夾。

**Q：怎麼完全重來？** 刪除腦（「腦」頁移除來源可選永久刪除）＋刪除設定／資料目錄（[第 17 章](17-config-reference.md)位置表）。先備份。

**Q：哪裡回報問題？** [GitHub Issues](https://github.com/ascetic168/Operoid/issues)——附上 `oserver version` 輸出與相關橫幅／日誌摘錄。

---
[← 17 設定參考](17-config-reference.md)｜[索引](README.md)｜[附錄 A →](appendix-a-gbrain.md)
