# Operoid 企業模式部署指引（遠端化 R1–R8）

> 適用場景：oserver 部署在公司內網伺服器，使用者以瀏覽器經三個角色前端（`/admin`、`/manager`、`/user`）連入。
> 個人模式（桌面 GUI＋本機 oserver）**不需要本文件**——安裝桌面版即用，行為不受企業模式影響。

---

## 1. 架構總覽

```
公司伺服器機（內網）                        使用者電腦（瀏覽器，零安裝）
┌─────────────────────────────┐          ┌──────────────────────────┐
│ oserver（Windows 服務/systemd）│          │  /admin   系統管理者介面    │
│  HTTPS（rustls, :7340）       │ ◀──────  │  /manager 經理人儀表板      │
│  /admin /manager /user 前端   │  Bearer  │  /user    一般使用者介面    │
│  SQLite（operoid.db）         │  token   └──────────────────────────┘
│  GBrain（stdio＋服務帳戶）      │
│  operoid.toml（服務組態）      │
└─────────────────────────────┘
```

安全模型（不可妥協項）：

- **非 loopback bind 必須啟用 TLS**——oserver 啟動時強制檢查（DR-E5 fail-closed），否則拒絕啟動。
- Bearer token 不落瀏覽器 storage（記憶體持有）；24h 會話 TTL＋refresh 輪替。
- 端點級 RBAC：admin ⊃ manager ⊃ user，伺服器端 403（fail-closed）；知識檢索另有 scope/clearance/grant 授權（M1 鐵律）。

---

## 2. 前置需求（伺服器機）

| 項目 | 說明 |
|---|---|
| oserver 執行檔 | `cargo build --release -p oserver`，或 CI release 產物 |
| 前端靜態檔 | `cd frontends && pnpm install && pnpm build` → `frontends/apps/{admin,manager,user}/dist` |
| GBrain | 安裝於**伺服器機**（`gbrain` CLI 於 PATH 或 operoid.toml 指定）；腦（GBRAIN_HOME）與 notes repo 同機 |
| TLS 憑證 | 內網 CA 簽發（首選）或自簽（見 §4） |
| LLM API keys | 服務端自填（operoid.toml `[llm] env`） |

---

## 3. operoid.toml（服務組態）

放於 `<資料目錄>/operoid.toml`（**存在即企業模式**；個人模式無此檔、行為零變化）：

```toml
[server]
host = "0.0.0.0"            # 內網位址或 0.0.0.0；非 loopback ⇒ [tls] 必填
port = 7340
token = "<master-token>"    # admin 全權 master token（CSPRNG；离線產生：openssl rand -hex 32）
frontends_dir = "/opt/operoid/frontends"   # 內含 admin/ manager/ user/ 三子目錄

[tls]
cert = "/opt/operoid/certs/server.crt"     # PEM
key  = "/opt/operoid/certs/server.key"

[llm.env]                    # 服務端自填 provider keys（服務行程看不到使用者環境）
ZHIPUAI_API_KEY = "..."
# OPENAI_API_KEY = "..."

[gbrain]                     # 可省；覆寫 gbrain 執行檔路徑
exe_path = "/home/gbrain/.bun/bin/gbrain"

[ingress]                    # 可省；obridge 投遞口
port = 7341
secret = "<ingress-secret>"
```

設定優先序：`OSERVER_*` 環境變數 > operoid.toml > app-settings.json（個人模式回落）。
`[llm] env`／`[gbrain]`／`[ingress]` 改檔熱生效；`host`/`port`/`[tls]`/`frontends_dir` 改後須重啟服務。

---

## 4. TLS 憑證

**首選：內網 CA**。簽發含伺服器 IP/DNS 的 SAN 憑證，使用者機已信任內網 CA → 瀏覽器無警告。

**自簽（快速內網可用）**：

```bash
openssl req -x509 -newkey rsa:2048 -keyout server.key -out server.crt \
  -days 365 -nodes -subj "//CN=operoid-server" \
  -addext "subjectAltName=IP:192.168.0.2,DNS:operoid.internal"
```

自簽憑證首次連線時瀏覽器會警示（預期行為）；正式部署建議內網 CA。

---

## 5. 服務註冊

### Windows（SCM，已實機驗證）

```powershell
# 系統管理員
oserver.exe install --data-dir C:\operoid    # 註冊 Windows 服務（開機自啟）
oserver.exe status                            # {"installed": true, "running": true}
# 移除：oserver.exe uninstall
```

服務以 LocalSystem 執行——**LLM keys 必須寫在 operoid.toml `[llm] env`**（服務看不到使用者環境變數）。

### Linux（systemd）

```ini
# /etc/systemd/system/operoid.service
[Unit]
Description=Operoid server
After=network-online.target

[Service]
ExecStart=/opt/operoid/oserver --data-dir /opt/operoid/data
Restart=on-failure
User=operoid
# 專用服務帳戶（security model 部署基線）：GBRAIN_HOME 與資料目錄權限 0700

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload && sudo systemctl enable --now operoid
```

> Linux 服務路徑為骨架＋CI 編譯覆蓋；**實機驗證待有 Linux 環境**（Windows 為已驗證路徑）。

### 防火牆

```powershell
# Windows（系統管理員）
netsh advfirewall firewall add rule name="Operoid oserver 7340" dir=in action=allow protocol=TCP localport=7340
```

```bash
# Linux（ufw 例）
sudo ufw allow 7340/tcp
```

---

## 6. 首次啟動與帳號開通

1. 啟動服務，確認 `https://<伺服器IP>:7340/healthz` 回 `{"status":"ready","version":"..."}`。
2. 瀏覽器開 `https://<伺服器IP>:7340/`——三前端入口頁。
3. 以 **master token**（operoid.toml `[server].token`）開通第一個 admin 帳號：

   ```bash
   curl -k -X POST https://<伺服器IP>:7340/api/accounts \
     -H "authorization: Bearer <master-token>" -H "content-type: application/json" \
     -d '{"login_name":"admin1","roles":["admin"]}'
   # 回應含 temporary_password（僅此一次顯示）——交給該使用者
   ```

4. 該使用者開 `/admin` 登入（臨時密碼）→ 系統**強制**設定新密碼（≥12 碼）→ 之後在「帳號管理」頁建立 manager/user 帳號。
5. （可選）服務間整合用長期 token：管理者介面「知識授權」→ 簽發 token（TTL 留空＝不過期）。

---

## 7. obridge（外部事件進氣）

obridge 佈署在能觸達事件的機器，`obridge.toml` 的 `[operoid]` 指向伺服器：

```toml
[operoid]
ingress_url = "http://<伺服器IP>:7340/event"   # 企業模式：主 port /event
secret = "<ingress-secret>"                     # 與 operoid.toml [ingress].secret 一致
```

> `[ingress].port` 有設時 ingress 走獨立 port；未設則共用主 port 的 `/event`（Bearer＝master token 或 ingress secret）。

---

## 8. 驗收清單（部署後）

- [ ] 第二台機器瀏覽器 `https://<伺服器IP>:7340/healthz` → ready（自簽憑證需手動信任）。
- [ ] admin 建號 → 該帳號登入 → 首登強改密 → 三前端各自功能正常。
- [ ] user 帳號呼叫管理功能被 403（如直接打 `/api/brains`）——伺服器端真的擋。
- [ ] 統一 GUI（個人模式桌面版）行為不變——企業部署不影響個人模式。
- [ ] **整機重啟**：服務自啟、前端可登入、員工狀態完整、待核可提案仍在。
- [ ] （有真實 GBrain＋LLM 時）員工部署→交辦→喚醒→產出→核可全閉環＋知識檢索身份裁定（M1 鐵律）。

---

## 9. 疑難排解

| 症狀 | 原因與處置 |
|---|---|
| 啟動失敗「非 loopback 須 TLS」 | operoid.toml 補 `[tls]`（cert＋key 齊備），或改 bind 127.0.0.1 |
| 頁面載入但資料全空 | 看瀏覽器 Console：CSP／CORS／憑證問題；`/healthz` 的 `version` 對比預期（**舊版服務誤用**的辨識點——殺掉舊行程重啟） |
| 登入回 429 | 連續 5 次密碼錯誤鎖定 15 分鐘（行程內狀態，重啟服務即解除）；admin 可用「重設密碼」 |
| 服務模式 LLM 失效 | 服務帳戶看不到使用者 env——keys 必須寫 operoid.toml `[llm] env`，改後重啟 |
| 員工喚醒無反應 | GBrain 安裝於伺服器機了嗎？`/api/prereq`（admin）檢查 git/bun/gbrain |
