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

> **為何選 llama.cpp**：截至本文撰寫時，Google 推出的 EmbeddingGemma 2 太新，
> 許多推理引擎尚未全面支援——ollama 提供的 GGUF 檔須有 MLX（Apple Silicon）才能執行，
> Windows 環境走不通；故本專案選回 llama.cpp，並採用
> [當下最新的開發版 build](https://github.com/ggml-org/llama.cpp/releases)。
> 模型 GGUF 下載位置：[ggml-org/embeddinggemma-2-GGUF](https://huggingface.co/ggml-org/embeddinggemma-2-GGUF)
> （上游 [google/embeddinggemma-2](https://huggingface.co/google/embeddinggemma-2)）。

建腦與檢索前須先啟動（下方為手動啟動指令，供首次安裝與除錯；正式環境應設開機自啟——
Windows 登入排程任務／Linux systemd unit——本機參考部署已配置自動啟動）：

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

### 多模態（圖片嵌入）：mm-bridge shim（EmbeddingGemma 2 必需）

gbrain 的圖片匯入管線（圖片檔放進 notes repo → sync 自動建立 `type: image` 頁＋
多模態向量＋圖片搜尋）需要 multimodal embedding。配 EmbeddingGemma 2（本機
llama-server）時，**必須部署部署包內附的 `deploy/local/mm-bridge/` 相容層**，原因有二：

- gbrain 的 `llama-server` recipe 未宣告 multimodal 支援——直接設定
  `embedding_multimodal_model llama-server:<id>` 會被拒。
- gbrain 與 llama.cpp 的 `/v1/embeddings` 多模態請求形狀不同：前者送扁平
  `image_url` parts，後者要求每個 input 元素包 `{content:[…]}`。

mm-bridge 是介於兩者之間的形狀轉譯 shim（bun 單檔、監聽 8090 → 8080），
**不是對 gbrain／llama-server 本體的修改**；上游 gbrain 補上 llama-server recipe 的
`supports_multimodal` 後可整組退役（詳見包內 README）。

安裝（伺服器機，一次性）：

```bash
# 1) 啟動 shim（常駐；Windows 放 shell:startup 或排程任務，Linux 以 systemd unit 常駐）
bun deploy/local/mm-bridge/mm-bridge.mjs        # 監聽 127.0.0.1:8090 → 8080

# 2) schema 前置（僅首次；先確認無 gbrain 行程在跑、已備份腦資料庫；
#    欄位非空時腳本會自我檢查並中止。gbrain self-upgrade 後重跑檢查）
bun deploy/local/mm-bridge/alter-image-dim.mjs  # embedding_image: vector(1024)→vector(768)

# 3) gbrain 設定（文字側 embedding_model 不動、零 reindex；multimodal 走獨立欄位）
gbrain config set provider_base_urls.litellm http://127.0.0.1:8090/v1
gbrain config set embedding_multimodal_model litellm:embeddinggemma-2
gbrain config set embedding_multimodal true
```

驗證：圖片＋同名 `.md` 註解頁放進 notes repo → sync → `gbrain schema stats`
出現 `image` 頁。注意 `image_of` 自動連結在「圖先於註解頁匯入」時會跳過——
PDF 圖檔管線已內建「先 `.md` 後圖檔」順序＋入庫後補一輪冪等 sync 作保險；
手動放置時註解頁請先行。**新腦免手動**（2026-10-18 起）：GUI／API 建立腦時，作用中環境已啟用
multimodal 即自動繼承三鍵（file＋DB 雙 plane），並自動把新腦的 `embedding_image` 由 gbrain
硬編的 vector(1024) 修正為嵌入維度（冪等腳本 `deploy/local/mm-bridge/alter-image-dim.mjs`，
以 bun 執行；找尋順序 `OPEROID_ALTER_IMAGE_SCRIPT` → `~/.bun/install/global/` →
`<cwd>/deploy/local/mm-bridge/`；失敗不擋建立，記於 oserver log）。此步驟前的既有腦仍需手動。**範圍註記**：此 shim 服務的是 gbrain 原生圖片 lane
（拖放圖片與 PDF figures 的 `embedding_image` 向量）；舊 K3 sidecar 已於 v0.5.3
完全退役（見 §10.5）。

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

[factory]                    # 可省；工廠文件轉換的多工
concurrency = 4              # 全域同時轉換的檔案數上限（預設 4）；
                             # 公平調度：飽和時每位遠端使用者至多 1 線程
```

設定優先序：`OSERVER_*` 環境變數 > operoid.toml > app-settings.json（個人模式回落）。
`[llm] env`／`[gbrain]`／`[ingress]`／`[factory]` 改檔熱生效；`host`/`port`/`[tls]`/`frontends_dir` 改後須重啟服務。

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

## 10. PDF 知識管線（MinerU → gbrain 原生 image import）

複雜 PDF（論文/報告）經 MinerU 轉為「章節切塊文字筆記＋圖片筆記」，入 gbrain 索引；
**每圖另發布同名圖檔＋註解頁至 doc 目錄**（`<doc_slug>/fig1-p2.jpg`＋`fig1-p2.md`；
舊 `figures.sqlite` sidecar 已於 v0.5.3 完全退役，試點數據與裁決見 TODO 項目 3）。
圖檔經 `gbrain sync` 自動建立 `type: image` 頁＋多模態向量（`embedding_image`）＋
`image_of` 自動連結，與使用者拖放圖片同軌；查詢端圖路由跨模態圖 arm
（`--cross-modal image`）參與融合。**gbrain 零修改**——文件格式由筆記 body 自帶、
查詢前綴由呼叫端縫入。每頁 frontmatter 帶 `source_pdf`（來源 PDF 出處鏈，
`/api/media/source-pdf` 與 `meta.source_docs` 由此讀取）。

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
- `--mmproj`（可選）：掛上後 `/v1/models` 的 `input_modalities` 含 `image` → gbrain sync
  可為圖檔建立多模態向量（`embedding_image`，純視覺查詢的關鍵）。不掛時 sync 跳過
  圖片——管線不失效，事後補裝 mmproj 後 `gbrain embed --stale` 補嵌即恢復。
- **健康檢查**：`GET /api/knowledge/health`（admin）回報嵌入端點
  （reachable／modalities／vision／n_ctx／**long_input_ok**／dimensions）、chat 端點 VLM
  能力（1 圖試探，按端點快取——決定生成端「讀圖作答」或「定位者」模式）、MinerU 階梯狀態。

### 10.2 MinerU 取用階梯（安裝形態，逐級 fallback）

| 階梯 | 形態 | 設定 |
|---|---|---|
| 1 | **設定覆寫**（首選，venv 安裝） | `[knowledge] mineru_command`＝完整可執行路徑或命令模板。venv `Scripts/*.exe` 是 launcher——**原地以絕對路徑呼叫免 activate**；不可複製 exe 單獨攜帶、venv 搬移即失效（`Fatal error in launcher`）→ 階梯自動降級。`conda run -n m mineru-kit`／`uv run` 皆以模板涵蓋。路徑支援環境變數展開（見下） |
| 1b | **設定目錄**（可攜首選） | `[knowledge] mineru_dir`＝安裝根目錄，程式自動探測 `mineru-kit` 於根／`Scripts\`（Windows venv）／`bin/`（Unix venv、pipx、uv tool）。**支援 `%VAR%`（Windows）／`${VAR}`（Unix）環境變數展開**——寫 `%USERPROFILE%\Documents\python\.mineru` 或 `${HOME}/.mineru` 即同一份設定字串跨機器、跨使用者成立 |
| 2 | **PATH 查找** | `uv tool install mineru`／`pipx install`／pip --user 後 PATH 上有 `mineru-kit` 即命中（Windows 套 PATHEXT；Unix 另驗執行位） |
| 3 | **遠端解析**（egress 分層治理，見 10.3） | `mineru_remote_url`（自架 `mineru-kit api-server`，private）／`mineru_api_key`＋`allow_public_egress=true`（mineru.net，public） |
| 4 | **降級** | pdf_extract naive 文字抽取（無圖、品質降），轉換報告帶品質警告 |

解析檔位：`mineru_tier`＝`flash`／`basic`（預設，CPU 約 10 秒/頁）／`standard`。
路由政策：`pdf_convert_policy`＝`auto`（預設；pdf_extract 毫秒級訊號 S1–S4——無文字層／字形殘片
／圖表引用密度／行長碎片化——任一命中即升級 MinerU）／`always_fast`／`always_mineru`。
判定結果與訊號全量存進轉換輸出的 `conversion-report.json`（可稽核）。
本機階梯探測（含各級落空原因與 PATH 實測）：
`cargo test -p ocore real_ladder_probe -- --ignored --nocapture`。

### 10.3 egress 信任分層（文件外流的治理，K1/K5/K8 共用）

| 層級 | 端點 | 政策 |
|---|---|---|
| `local` | 本機（階梯 1–2 的 MinerU、127.0.0.1 的 llama-server） | 永遠允許 |
| `private` | 使用者／企業自設端點（區網 api-server、內網 LLM 閘道） | **設定本身即同意**（URL 必須明確配置才存在）；敏感場合的合規首選——文件只在自有基礎設施內流動 |
| `public` | 第三方公有雲（mineru.net、zhipu 等） | **預設擋**（`allow_public_egress=false`）；開啟＝opt-in。同一閘門也鎖 K8 VLM 補圖說與 K5 生成端讀圖的 public provider |

每次轉換的 egress 層級與端點記錄於 `conversion-report.json`（稽核面）；企業模式管理員可鎖定政策。

### 10.4 轉換輸出與 gbrain 導入

```
<out>/                                       （轉換暫存目錄；入庫結束即刪）
  notes/<doc_slug>/s00.md … fig1-p2.md …   ← 複製進 gbrain source（git repo；一 chunk 一筆記）
  figures.sqlite                            ← 轉換期 metadata（doc_id/page/caption/…）——
                                              僅供圖檔發布步驟讀取，**不併生產庫**、隨暫存刪除
  conversion-report.json                    ← 路由訊號＋階梯嘗試＋egress 稽核＋警告
  mineru/                                   ← MinerU zip＋解壓（images/ 是圖檔發布來源）
```

筆記與圖檔以 `gbrain sync --no-extract` 導入（extract/facts 對切塊筆記是逐頁 LLM 計費
且無收拾價值）；筆記歸屬工廠 notes repo（federated source）；slug 以文件前綴防碰撞
（`<doc_slug>/sNN`、`<doc_slug>/figN-pP`——歸因靠 metadata 不靠向量）。圖檔發布
（`publish_pdf_figures`）先補強 `.md`（`影像檔：` wikilink＋`原論文 PDF：` 出處＋
匯入 timeline 錨點，冪等）、後落同名圖檔——**順序鐵律**：gbrain 的 `image_of` 在
「圖先於註解頁」時靜默跳過。查詢端由 Operoid 自動縫 `task: search result | query: `
前綴＋停用 expansion（K2）。

**gbrain 版本紀律**：chunking/slug 規則屬 gbrain 內部行為——本管線以 gbrain **0.60.105.0**
驗證（12 查詢 hit@5 12/12、MRR 0.958）；升級 gbrain 後應重跑端到端評測
（`cargo test -p ocore real_k7 -- --ignored --nocapture`）再上線。

### 10.5 sidecar 完全退役（2026-10-17 裁決；v0.5.3 移除）

**repo 內外邊界**：圖檔屬 **repo 內**（`<doc_slug>/figN-pP.<ext>`＋同名註解頁，授權隨
source 成員走；檢索向量由 gbrain `embedding_image` 承擔）。舊 sidecar（`figures.sqlite`
向量/登記庫）及其寫入、查詢、遷移程式路徑已全數移除——不再有任何 sidecar 殘留碼。

- **查詢端圖路**：只有 gbrain 原生圖 arm（`gbrain query --cross-modal image`；注意
  `auto/both` 的文字查詢不會帶出 image 頁，圖必須顯式走此 arm）。
- **doc→PDF 出處鏈**：每頁 frontmatter 的 `source_pdf`（K1 轉換時寫入）取代舊
  `figure_docs` 登記表——`meta.source_docs` 與 `/api/media/source-pdf` 由此讀取；
  來源 PDF 必須位於授權來源 repo 內（包含制）。
- **媒體端點 allowlist**：改為 **repo 包含制**——請求路徑 canonicalize 後必須落在
  principal 授權來源（M1 authorized_sources）的 repo 目錄內＋圖檔副檔名白名單
  （`gbrain sources list` 有 TTL 快取，簽名輪詢不重複 spawn）。
- **K8 補圖說**：改為發布後自動觸發（repo 掃描佇列——doc 目錄的短 caption 圖頁；
  冪等；egress 閘門照舊）。
- **品質基線**（mueller2016 四 lane 試點，TODO 項目 3）：hit@5 全 lane 持平（11/11）；
  MRR sidecar 融合 1.000 vs 圖 arm＋文字融合 0.909——差距為 2/11 查詢的 #1 vs #2/#3
  （純圖向量無 caption），靠註解頁 caption（文字 lane）與選配 OCR 補償。退役後的
  常駐回歸閘：`cargo test -p ocore real_image_lane -- --ignored --nocapture`
  （硬斷言主裁決子集 hit@5 = 9/9）。

## 11. 個人版（桌面）：元件偵測、前置需求與降級行為

桌面安裝檔即是個人版，安裝即用。知識相關功能依賴若干外部元件，全部自動偵測、
缺了會在介面上明說（能力矩陣：個人版在「設定 → 服務」；企業版在
admin／manager／user 三個網頁前端）。

### 11.1 知識圖譜（gbrain）

| 工具 | 用途 | 安裝 |
|---|---|---|
| **git** | sync 流程會在更新圖譜前先 commit | <https://git-scm.com/downloads> |
| **bun** | `gbrain` 透過 bun 安裝與執行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 知識圖譜引擎 | <https://github.com/garrytan/gbrain> |

路徑會自動偵測（Windows 為 `~/.bun/bin/gbrain.exe`）；必要時可在「設定」頁覆寫。

### 11.2 文件管線（嵌入與複雜 PDF）

| 工具 | 用途 | 沒有它 |
|---|---|---|
| **llama-server**（EmbeddingGemma 2） | 本機嵌入後端（檢索用）；啟動旗標見第 10.1 節 | 檢索退為僅關鍵字 |
| **mmproj 投影器**（選配） | 圖片的「圖說＋原圖」聯合向量 | 圖片以圖說文字索引 |
| **MinerU**（選配，**強烈建議**；版本須 > 4.0） | 複雜 PDF 解析（雙欄／掃描／圖表）——`uv tool install "mineru>4.0"` | 僅簡單 PDF（走快速路徑）；文件內圖片無法進入知識庫 |

**MinerU 為何強烈建議**：要解析複雜 PDF，本專案採用 MinerU。不裝它也能跑，但只能
正確處理格式較為簡單的文件，且多模態檢索所需的文件內圖片無法進入知識庫。
egress 信任分層見第 10.3 節。

### 11.3 開機服務（Linux / macOS）

Linux 與 macOS 的開機服務在**任何使用者登入前**就會啟動
（Linux：`/etc/systemd/system` 的 systemd system unit；
macOS：`/Library/LaunchDaemons` 的 launchd LaunchDaemon）。安裝注意事項：

- 請**從你自己的帳號透過 `sudo` 安裝**（例如 `sudo oserver install`）。
  提權只用在寫入系統位置的 unit/plist 檔；服務本身以**安裝者的身分**執行，
  因此資料庫與設定檔的屬主和桌面 app 一致。
- 若安裝程式無法判定安裝使用者（例如直接在 root shell 執行），會直接報錯拒絕——
  請改從你的帳號以 `sudo` 安裝。
- 移除服務：`sudo oserver uninstall`。
- Linux／macOS 的服務路徑已實作但**尚未實機驗證**（Windows 已驗證）。

### 11.4 egress 揭露與責任分界（個人版）

個人版的程序與資料都在本機：腦資料是本機檔案（預設 `~/.gbrain` 的 PGLite）、嵌入
走本機 llama-server（127.0.0.1）、Claude Code handoff 以 **stdio 子程序**啟動
`gbrain serve`（無監聽 port、無第三方遙測）。會離開這台機器的只有模型推論，且
端點皆由使用者自行設定：

| 通道 | 送往哪裡 | 送什麼 |
|---|---|---|
| 內建對話／agent 任務 | 腦設定的 chat model 端點（如 zhipu；gbrain 的 think／advisor 同） | 對話內容＋檢索到的知識片段 |
| 文件轉換（K 管線） | 依 §10.3 信任分層；`public` 層預設擋 | 待轉換文件內容 |
| Claude Code handoff | Claude 服務端點（Anthropic 或 `ANTHROPIC_BASE_URL`） | 對話內容＋檢索到的知識片段 |

**責任分界**：個人版的使用者就是資料擁有者，「能否把資料送上雲端 LLM」屬公司
IT 政策，不是 Operoid 的管制對象——不裝 Operoid，同樣的管道依然存在（本機腦
資料對本機工具本來就是全開的）。Operoid 的義務是**揭露**（本節與啟動對話框的
提示），不是**管制**。管制在企業模式：知識範圍於檢索前強制執行（fail-closed），
且 `claude_code_handoff_enabled=false` 可整個退役 handoff。

**handoff 通道的能力範圍（設計如此）**：啟動的 Claude Code 對所選腦有**完整讀寫**
（stdio MCP 全量操作，含 `put_page`／`delete_page`／`submit_job` 等寫入類），
不受知識範圍限制——對單人本機場景這是合理預設，代價是完整性風險（agent 誤改
腦內容），以 gbrain 版本歷史（`revert_version`）與備份因應。企業場景不應依賴
此通道，請改用受治理的員工能力。

## 12. 升級（更新版本）

**企業版不提供自動更新**——僅在 admin 介面「版本」卡片檢查新版並提示
（查 GitHub Releases，結果快取 24 小時；離線／氣隔環境可於 operoid.toml
設 `[update] check_enabled = false` 關閉對外連線）。**版本間可能不相容**
（本專案不保證向上相容），升級前務必閱讀該版本的 CHANGELOG 發布說明，
特別是標註 breaking／不相容的條目，並**先備份資料目錄**（DB 與設定）。

升級程序（zip 手動部署的標準流程）：

```powershell
# Windows（系統管理員）
sc stop Operoid                          # 或 .\oserver.exe uninstall 後重裝亦可
# 備份資料目錄（--data-dir 指定的目錄）
Copy-Item C:\operoid\data C:\backup\operoid-data-<date> -Recurse
# 解壓新版企業包（operoid-enterprise-windows-x64.zip）覆蓋安裝目錄
#   —— oserver.exe／obridge.exe／frontends\ 全部替換；operoid.toml 在資料
#      目錄（不在部署包內），不受影響
sc start Operoid
.\oserver.exe status                     # 確認 running=true，並核對 /healthz 的 version
```

```bash
# Linux（systemd）
sudo systemctl stop operoid
sudo cp -a /opt/operoid/data /backup/operoid-data-<date>   # 備份資料目錄
# 解壓新版企業包（operoid-enterprise-linux-x64.tar.gz）覆蓋 /opt/operoid
#   —— oserver／obridge／frontends/ 全部替換；operoid.toml 在資料目錄，不受影響
sudo systemctl start operoid
curl -s http://127.0.0.1:7340/healthz                      # 核對 version
```

要點：

- **設定與資料不在部署包裡**：`operoid.toml`、`app-settings.json`、agent DB 皆在
  資料／設定目錄（§2），覆蓋安裝目錄不會動到它們——升級只替換 binaries 與
  `frontends/` 靜態檔。
- **路徑未變即免重裝服務**：服務註冊指向固定執行檔路徑（§5），只要在同一目錄
  覆蓋二進位，`sc stop`／`start`（或 systemctl）即可；若搬移安裝目錄，需先
  `uninstall` 再 `install` 重新註冊。
- **檢查新版**：admin 介面 →「版本」卡片 →「檢查更新」；有新版會顯示橫幅連到
  發布頁（相同版本可按「此版不再提示」）。

**個人版（桌面）**：安裝器直接覆蓋升級（NSIS hooks 會自動停啟 `Operoid` 服務）。
App 內建更新：設定 → 應用程式 →「版本與更新」——「更新提示」預設開（啟動時自動
檢查＋橫幅提示，安裝需按一下）；「自動更新」預設關（開啟後偵測到新版即自動下載
安裝，完成後提示重新啟動）。App 外仍可至
[Releases 頁](https://github.com/ascetic168/operoid/releases) 手動下載安裝檔。

