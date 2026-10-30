# 電子郵件橋接（Obridge）

> 適用版本：員工 email 綁定自 v0.5.9 起。Obridge（Operoid Bridge）是員工與外部世界的雙向橋：**Email 內建**（IMAP 收／SMTP 發），其他通道（企業 IM 等）走 WASM 外掛——開發見[附錄 B](appendix-b-development.md)。

## 它怎麼運作

```
外部寄件者 ──email──▶ [obridge: IMAP 輪詢] ──POST /event──▶ Operoid（喚醒員工）
Operoid（員工回覆）──POST /send──▶ [obridge: SMTP] ──email──▶ 原收件者（回同一 thread）
```

- **進**：obridge 定時輪詢 IMAP 信箱，比對 routes 把信投遞給對應員工——員工被喚醒，整封信出現在它的對話裡。
- **出**：員工的回覆經 obridge 以對應的 From 身分寄出，**還原 In-Reply-To**——外部收信者看到的是正常的同一串往來。

## 讓員工收發 email：四步

1. **綁定對外 email**：員工實體部署時填「對外 email」，或事後在實體上修改——這是唯一要記的員工端設定（單一真相存在員工資料上）。
2. **設定事件進出口**（`app-settings.json` 的 `app_config`，位置見[第 17 章](17-config-reference.md)）：
   ```json
   "event_ingress_port": 17341,
   "event_ingress_secret": "<密碼A>",
   "event_outbound_url": "http://127.0.0.1:17401/send",
   "event_outbound_secret": "<密碼B>"
   ```
3. **設定 obridge.toml**（模板見 `obridge/config.example.toml`）：
   - `[operoid]`：`ingress_url`（指到 Operoid 的 `/event`）＋`ingress_secret`（密碼A）；
   - `[listen]`：`port`（如 17401）＋`secret`（密碼B）；
   - `[[channels.email_imap]]`：IMAP／SMTP 帳號、輪詢間隔、routes（收件地址→員工）、senders（員工→From 地址）。
   - 員工綁定 email 後，**routes／senders 會自動同步**進 obridge.toml（自 v0.5.9 起），通常不用手改。
4. **啟動 obridge**：最簡單是交給 Operoid 代管——「設定 → 應用程式」設 **obridge.toml 路徑**＋**obridge 執行檔**＋勾「啟動時自動帶起」；安裝檔自帶 obridge.exe（與主程式同目錄，路徑可留空自動偵測）。之後 Operoid 啟動時帶起、退出時帶走。

## 熱重載

obridge 每 2 秒檢查 `obridge.toml`——`[[channels]]` 與外掛設定**改了即生效**；`[listen]`／`[operoid]` 變更需重啟（代管模式下，設定頁存檔會自動重啟子進程）。

「設定 → 應用程式」的 Obridge 區塊也可直接**編輯 obridge.toml 原文**（Operoid 只當編輯器，通道帳密屬 obridge 職責）。

## 驗證（全鏈走一遍）

1. 從外部信箱寄信到 route 命中的地址。
2. obridge 日誌應見投遞；員工被喚醒，對話頁出現 In 訊息（整封信序列化）。
3. 與員工對話；外部信箱應收到**回原 thread** 的回信（寄件者為 senders 對應的 From 身分）。

## 日誌與排解

- 日誌：資料目錄下的 **`obridge.log`**（obridge 的 stderr 全部在這）；位置見[第 17 章](17-config-reference.md)。
- 員工沒收到信：先看 routes 是否命中（地址拼寫、大小寫）、IMAP 帳密、輪詢間隔；再看到沒有對應員工（已封存的員工不會被喚醒）。
- 收不到回信：看 `event_outbound_url` 是否已設（未設時員工的 send 會顯示「外發未啟用」）、SMTP 憑證。
- 更多：[第 18 章](18-troubleshooting.md)。

> 治理提醒：email 是**對外通道**——讓員工自動處理來信前，先用[登記表](13-governance.md)的圍欄（例如「禁對外發送」或人裁決層）畫好界線。

---
[← 13 治理](13-governance.md)｜[索引](README.md)｜[15 服務與更新 →](15-service-and-updates.md)
