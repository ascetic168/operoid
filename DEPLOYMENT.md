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

**首選：從 GitHub Release 下載企業部署包。** 推送 `v*` 標籤後，CI 會在同一個 Release 附上：

- 桌面安裝包（`.msi`／`.exe`／…）——**個人版**：裝在使用者電腦，開箱即用、零設定。
- `operoid-enterprise-windows-x64.zip`／`operoid-enterprise-linux-x64.tar.gz`——**企業版部署包**：內含
  `oserver` 執行檔、三個前端靜態檔（`frontends/{admin,manager,user}`）、`DEPLOYMENT.md`、`operoid.example.toml`。
  下載解壓到伺服器機即可照本文件部署，**伺服器機不需要 Node/pnpm**。

| 項目 | 說明 |
|---|---|
| 企業部署包 | GitHub Release 下載（上表）；或自行建置：`cargo build --release -p oserver`＋`cd frontends && pnpm install && pnpm build` |
| GBrain | 安裝於**伺服器機**（`gbrain` CLI 於 PATH 或 operoid.toml 指定）；腦（GBRAIN_HOME）與 notes repo 同機 |
| Embedding 服務 | 本機 [llama.cpp](https://github.com/ggml-org/llama.cpp/releases) `llama-server`（port 8080），模型 EmbeddingGemma 2（768 維）；GBrain 嵌入／查詢**硬依賴**，須常駐（見下方啟動指令） |
| TLS 憑證 | 內網 CA 簽發（首選）或自簽（見 §4） |
| LLM API keys | 服務端自填（operoid.toml `[llm] env`） |

**Embedding 服務（EmbeddingGemma 2）啟動方式**：從 llama.cpp Releases 下載對應平台 build，
模型 GGUF（`llama-server:embeddinggemma-2`，Apache 2.0）取自
[ggml-org/embeddinggemma-2-GGUF](https://huggingface.co/ggml-org/embeddinggemma-2-GGUF)
（上游 [google/embeddinggemma-2](https://huggingface.co/google/embeddinggemma-2)）。
建腦與檢索前須先啟動：

```bash
llama-server --embedding \
  -m /opt/llama-cpp/models/embeddinggemma-2.gguf \
  --alias embeddinggemma-2 --host 127.0.0.1 --port 8080 \
  -c 32768 -b 8192 -ub 8192
```

> `-c 32768` 為 context 上限、`-b/-ub 8192` 為實體批次大小——低於此值時長 chunk
> （>512 tokens）會被 llama-server 拒絕。`--alias` 必須與腦設定
> `embedding_model: llama-server:embeddinggemma-2` 冒號後的名稱一致。
> Windows 個人版可用登入排程任務常駐（S4U、隱藏視窗）；Linux 以 systemd unit 為宜。
> **多模態嵌入（可選）**：同 repo 另附 `mmproj-embeddinggemma-2-{BF16,Q8_0}.gguf`
> 影像／音訊投影器；下載後於啟動指令加
> `--mmproj /opt/llama-cpp/models/mmproj-embeddinggemma-2-BF16.gguf`
> 即支援圖片／音訊嵌入（輸出同為 768 維），`/v1/embeddings` 的每個 prompt 以
> `{"content": [{"type": "image_url", ...}, ...]}` 傳入。腦的純文字嵌入用不到，
> 省 RAM 可不加（BF16 版約多佔 0.9 GB，Q8_0 約 0.5 GB）。

**新腦預設 embedding 模型（可選）**：伺服器上**新建腦**的 embedding 初始值可於
`<data-dir>/app-settings.json` 的 `app_config` 區塊設定（改後重啟 oserver；企業版亦可以
admin 身分在 web 端腦頁設定）：

```json
{
  "app_config": {
    "default_embedding_model": "llama-server:embeddinggemma-2",
    "default_embedding_dimensions": 768
  }
}
```

優先序：此設定 > 腦環境的 GBrain `config.json` > 內建預設（`llama-server:embeddinggemma-2` / 768）。
僅影響**新建腦**；既有腦換模型一律走 `gbrain migrate embeddings`（破壞性全量重嵌，先備份）。

> ⚠️ **伺服器機上不要開桌面 GUI（operoid 桌面程式）。** 它會嘗試沿用企業服務，但手上是個人版 token，
> 會整片離線造成混淆；桌面 GUI 屬個人版形態，裝在使用者電腦。

---

## 3. operoid.toml（服務組態）

放於 `<資料目錄>/operoid.toml`（**存在即企業模式**；個人模式無此檔、行為零變化）。
部署包內附 `operoid.example.toml` 範本，複製改名後改值即可：

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

[knowledge]                  # 可省；K1 PDF 知識管線（MinerU 階梯＋egress 分層）——見 §10
mineru_command = "/home/svc/.mineru/Scripts/mineru-kit.exe"
allow_public_egress = false  # 預設（public 第三方雲預設擋）

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
# 系統管理員（oserver.exe 來自企業部署包解壓目錄，如 C:\operoid）
cd C:\operoid
.\oserver.exe install --data-dir C:\operoid    # 註冊 Windows 服務（開機自啟）
.\oserver.exe status                            # {"installed": true, "running": true}
# 移除：.\oserver.exe uninstall
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

**首選：部署設定精靈（`configure`）。** 在伺服器機的終端機執行：

```bash
./oserver configure --data-dir /opt/operoid/data   # Windows：.\oserver.exe configure --data-dir C:\operoid
```

精靈依序進行（Enter 採用 [預設值]，`?` 顯示說明，寫入前有摘要確認閘）：

1. **前置檢查**——探測 git／bun／gbrain；缺項顯示用途與安裝指引（可 `r` 重新檢查）。
2. **部署模式**——個人（本機 loopback）或企業（內網服務，寫入 operoid.toml）。
3. **監聽與前端**——bind host/port、frontends 目錄（即時檢查三子目錄）、TLS cert/key
   （非 loopback 必填，與啟動期 DR-E5 檢查同規則）。
4. **知識腦工作目錄**——gbrain 執行檔（自動探測 PATH／`~/.bun/bin`）與 `gbrain_home`；
   目錄不存在可現場 `gbrain init` 建新腦；另可填 notes repo 與 LLM keys。
5. **master token**——現場 CSPRNG 生成 64 碼（僅此一次完整顯示）或貼上既有值。
6. **管理員帳號**——預設 `admin`／`admin1234`；`must_change_password=true`，**首次登入
   系統強制設定新密碼（≥8 碼）**，改密前除改密／登出外全部 API 拒絕（伺服器端閘）。
7. **系統服務**——可選擇當場註冊開機自啟（等同 `install` 子命令）。

產出：企業模式寫 `<data-dir>/operoid.toml`＋`app-settings.json`（腦清單）；個人模式僅
`app-settings.json`。管理員帳號直接建於 `operoid.db`。

> ⚠️ 服務／GUI 等**無終端環境**遇首次執行不會彈出精靈——服務啟動會失敗並提示先跑
> `configure`。**註冊服務前務必先完成精靈。**

手動替代流程（不用精靈，適合自動化佈署）：

1. 啟動服務，確認 `https://<伺服器IP>:7340/healthz` 回 `{"status":"ready","version":"..."}`。
2. 瀏覽器開 `https://<伺服器IP>:7340/`——三前端入口頁。
   （對照：桌面安裝包＝**個人版**——裝在使用者電腦直接用，與本文件無關；企業部署＝本文件。）
3. 以 **master token**（operoid.toml `[server].token`）開通第一個 admin 帳號：

   ```bash
   curl -k -X POST https://<伺服器IP>:7340/api/accounts \
     -H "authorization: Bearer <master-token>" -H "content-type: application/json" \
     -d '{"login_name":"admin1","roles":["admin"]}'
   # 回應含 temporary_password（僅此一次顯示）——交給該使用者
   ```

4. 該使用者開 `/admin` 登入（臨時密碼）→ 系統**強制**設定新密碼（≥8 碼）→ 之後在「帳號管理」頁建立 manager/user 帳號。
5. （可選）服務間整合用長期 token：管理者介面「知識授權」→ 簽發 token（TTL 留空＝不過期）。

---

## 7. obridge（外部事件進氣——郵件橋接）

企業包內含 `obridge` 執行檔（與 `oserver` 同目錄）＋ `obridge.example.toml` 範本。
oserver **子行程代管** obridge：admin 開 `/admin` →「郵件橋接」頁，以表單設定
IMAP／SMTP 帳號與路由，存檔即寫入 `<settings-dir>/obridge/obridge.toml`（先經
`obridge --check` 驗證、原子寫入），並自動啟動／重啟子行程（stderr 導入同目錄
`obridge.log`）。密碼／密鑰欄**留空＝保留現值**；同機代管時 ingress 位址與密鑰
（server token）由伺服器自動寫入，表單不需填。

兩種佈署模式：

- **同機代管（預設）**：obridge 與 oserver 同機。表單「由本服務代管」開啟＋
  存檔後 oserver 自動帶起 obridge；回信方向（`event_outbound_url`/`_secret` 指向
  obridge send endpoint）也一併自動同步。收信去重狀態檔為
  `<settings-dir>/obridge/<source>-state.json`——刪除會重掃信（伺服器端
  `(source, external_ref)` 去重擋重複事件）。
- **遠端橋接**：obridge 佈署在能觸達郵件伺服器的另一台機器。表單切「遠端橋接」
  填 oserver 的 `/event` 位址與密鑰，存檔後把該 `obridge.toml` 與 `obridge` 執行檔
  複製到該機，自行以 systemd unit／排程啟動：
  `obridge --config /path/to/obridge.toml`（頻道設定變更會被 obridge 自身 2 秒
  mtime 熱載入；`[listen]`/`[operoid]` 變更需重啟）。注意 obridge send endpoint
  **僅綁 127.0.0.1**——遠端橋接模式下回信方向不可跨機，郵件為單向進氣。

> `[ingress].port` 有設時 ingress 走獨立 port；未設則共用主 port 的 `/event`（Bearer＝master token 或 ingress secret）。安全：`obridge.toml` 內含明文密碼，務必限服務帳戶可讀；設定檔變更歷程見 obridge.log。

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
| obridge 行為像舊版（新旗標無效、log 格式不同） | 新舊二進位混用——啟動 log 各有 build id，`/api/obridge/status` 的 `exe_build_match=false` 即是；oserver 帶起時也會警告。兩者必須出自同一建置（`cargo dev-build`／同一部署包） |
| 知識檢索品質突降／長 chunk 消失 | llama-server `-b/-ub 8192` 旗標回歸（預設 512 拒收長 chunk）——`GET /api/knowledge/health` 的 `embedding.long_input_ok=false` 即是；修正啟動旗標後重測 |
| PDF 轉換品質警告「降級 pdf_extract」 | MinerU 取用階梯全級落空——見 §10 的四種安裝形態；`conversion-report.json` 的 `ladder.attempts` 可稽核每級落空原因 |

---

## 10. PDF 知識管線（MinerU → gbrain ＋圖向量 sidecar）

複雜 PDF（論文/報告）經 MinerU 轉為「章節切塊文字筆記＋圖片筆記」，入 gbrain 純文字索引；
每圖另記入 `figures.sqlite` sidecar（後續以多模態嵌入補圖向量，K3）。**gbrain 零修改**——
文件格式由筆記 body 自帶、查詢前綴由呼叫端縫入。

### 10.1 嵌入服務紀律（必帶旗標）

```bash
llama-server --embedding \
  -m /opt/llama-cpp/models/embeddinggemma-2.gguf \
  --alias embeddinggemma-2 --host 127.0.0.1 --port 8080 \
  -c 32768 -b 8192 -ub 8192 \
  --mmproj /opt/llama-cpp/models/mmproj-embeddinggemma-2-BF16.gguf   # 多模態嵌入（可選，見下）
```

- `-b/-ub 8192` **不可省**：預設 512 會拒收長 chunk（>512 tokens）—— chunk 連同 caption
  直接從索引消失。健康檢查會以約 2,200 tokens 的長輸入實測此紀律。
- `--mmproj`（可選）：掛上後 `/v1/models` 的 `input_modalities` 含 `image` → sidecar 可做
  「caption＋原圖」聯合向量（純視覺查詢的關鍵）。不掛時 sidecar 降級為純文字（caption/
  章節/頁碼）向量——管線不失效，事後補裝 mmproj 只需重嵌 sidecar（每文件僅 ~9–25 列，秒級）。
- **健康檢查**：`GET /api/knowledge/health`（admin）回報嵌入端點
  （reachable／modalities／vision／n_ctx／**long_input_ok**／dimensions）、chat 端點 VLM
  能力（1 圖試探，按端點快取——決定生成端「讀圖作答」或「定位者」模式）、MinerU 階梯狀態。

### 10.2 MinerU 取用階梯（四種安裝形態，逐級 fallback）

| 階梯 | 形態 | 設定 |
|---|---|---|
| 1 | **設定覆寫**（首選，venv 安裝） | `[knowledge] mineru_command`＝完整可執行路徑或命令模板。venv `Scripts/*.exe` 是 launcher——**原地以絕對路徑呼叫免 activate**；不可複製 exe 單獨攜帶、venv 搬移即失效（`Fatal error in launcher`）→ 階梯自動降級。`conda run -n m mineru-kit`／`uv run` 皆以模板涵蓋 |
| 2 | **PATH 查找** | `uv tool install mineru`／`pipx install`／pip --user 後 PATH 上有 `mineru-kit` 即命中 |
| 3 | **遠端解析**（egress 分層治理，見 10.3） | `mineru_remote_url`（自架 `mineru-kit api-server`，private）／`mineru_api_key`＋`allow_public_egress=true`（mineru.net，public） |
| 4 | **降級** | pdf_extract naive 文字抽取（無圖、品質降），轉換報告帶品質警告 |

解析檔位：`mineru_tier`＝`flash`／`basic`（預設，CPU 約 10 秒/頁）／`standard`。
路由政策：`pdf_convert_policy`＝`auto`（預設；pdf_extract 毫秒級訊號 S1–S4——無文字層／字形殘片
／圖表引用密度／行長碎片化——任一命中即升級 MinerU）／`always_fast`／`always_mineru`。
判定結果與訊號全量存進轉換輸出的 `conversion-report.json`（可稽核）。

### 10.3 egress 信任分層（文件外流的治理，K1/K5/K8 共用）

| 層級 | 端點 | 政策 |
|---|---|---|
| `local` | 本機（階梯 1–2 的 MinerU、127.0.0.1 的 llama-server） | 永遠允許 |
| `private` | 使用者／企業自設端點（區網 api-server、內網 LLM 閘道） | **設定本身即同意**（URL 必須明確配置才存在）；敏感場合的合規首選——文件只在自有基礎設施內流動 |
| `public` | 第三方公有雲（mineru.net、zhipu 等） | **預設擋**（`allow_public_egress=false`）；開啟＝opt-in。同一閘門也鎖 K8 VLM 補圖說與 K5 生成端讀圖的 public provider |

每次轉換的 egress 層級與端點記錄於 `conversion-report.json`（稽核面）；企業模式管理員可鎖定政策。

### 10.4 轉換輸出與 gbrain 導入

```
<out>/
  notes/<doc_slug>/s00.md … fig1-p2.md …   ← gbrain source（git repo；一 chunk 一筆記）
  figures.sqlite                            ← sidecar 來源資料：doc_id/page/image_path/
                                              caption/section/image_md5（vec 由 K3 回填；
                                              image_md5 為去重鍵——重轉換不重嵌）
  conversion-report.json                    ← 路由訊號＋階梯嘗試＋egress 稽核＋警告
  mineru/                                   ← MinerU zip＋解壓（images/ 是 K3 嵌入來源，保留）
```

筆記以 `gbrain sync --no-extract` 導入（extract/facts 對切塊筆記是逐頁 LLM 計費且無收
拾價值）；筆記歸屬工廠 notes repo（federated source）；slug 以文件前綴防碰撞
（`<doc_slug>/sNN`、`<doc_slug>/figN-pP`——歸因靠 metadata 不靠向量）。
查詢端由 Operoid 自動縫 `task: search result | query: ` 前綴＋停用 expansion（K2）。

**gbrain 版本紀律**：chunking/slug 規則屬 gbrain 內部行為——本管線以 gbrain **0.60.105.0**
驗證（12 查詢 hit@5 12/12、MRR 0.958）；升級 gbrain 後應重跑端到端評測
（`cargo test -p ocore real_k7 -- --ignored --nocapture`）再上線。
