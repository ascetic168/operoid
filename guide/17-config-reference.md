# 設定參考

> 適用版本：v0.5.11。本章是所有設定檔與環境變數的速查表；企業版伺服器組態的逐步說明以 [DEPLOYMENT.md §3](../DEPLOYMENT.md) 為權威。

## 設定檔總覽

| 檔案 | 位置 | 格式 | 誰用 |
|---|---|---|---|
| `app-settings.json` | 設定目錄（見下） | JSON | 個人版＋企業版共用的基底設定（`{"app_config": {...}}`） |
| `operoid.toml` | 同設定目錄 | TOML | **存在＝企業模式**（個人模式無此檔、行為零變化） |
| `obridge.toml` | 你指定（`obridge_config_path`） | TOML | obridge 通道設定；GUI 可當編輯器直接改 |
| `config.json` | 每顆腦的 `.gbrain` 旁 | JSON | GBrain 的模型設定——「設定 → 模型」的權威來源 |

**設定目錄**：Windows `%APPDATA%\com.operoid.studio`；macOS `~/Library/Application Support/com.operoid.studio`；Linux `~/.config/com.operoid.studio`。

## app-settings.json 常用欄位

以下為 `app_config` 區塊中與日常使用相關的欄位（完整結構見 `ocore/src/app_config.rs`）：

| 欄位 | 預設 | 說明 |
|---|---|---|
| `agent_os_enabled` | `false` | 員工子系統總開關（員工模板／實體） |
| `brains` ＋ `active_brain_id` ＋ `active_source_id` | — | 腦登錄與作用中對象（GUI「腦」頁管理） |
| `gbrain_exe_path` | `~/.bun/bin/gbrain.exe` | gbrain 執行檔路徑 |
| `gbrain_transport` | `"mcp"` | 員工知識查詢傳輸：`mcp`（失敗自動退 CLI）或 `cli` |
| `default_embedding_model` ＋ `_dimensions` | 內建 768 維 | **新建腦**的 embedding 初始值 |
| `employee_output_path` | `~/employee-output` | 員工「寫入筆記」的沙箱根目錄 |
| `auto_sync` ＋ `sync_no_pull` | `true` ＋ `true` | 工廠寫檔後自動 commit+sync；sync 加 `--no-pull` |
| `llm_temperature` ／ `llm_max_tokens` | 0.2 ／ 4096 | LLM 結構化取樣 |
| `turn_max_steps` | 40 | 對話回合工具迴圈保險絲 |
| `autonomy_max_cycles` ／ `autonomy_max_minutes` | 25 ／ 15 | 自主工作預算 |
| `subagent_max_steps` | 20 | 子代理步數保險絲 |
| `skills_personal_dir` | `~/.agents/skills` | 技能個人區；**空字串＝企業模式**（逐主體隔離） |
| `skills_union_limit` | 30 | 技能載入聯集上限（`0`=不設限） |
| `factory_concurrency` | 4 | 同時轉換的檔案數 |
| `event_ingress_port` ／ `event_ingress_secret` | 未設 | 外部事件進氣口（`POST /event`）——**預設關閉**；設了 port 沒設 secret 則不啟動 |
| `event_outbound_url` ／ `event_outbound_secret` | 未設 | 員工回覆外發端點（未設＝回覆僅留在 Operoid） |
| `obridge_config_path` ／ `obridge_executable` ／ `obridge_autostart` | 未設 | Obridge 三件套（見[第 14 章](14-email-bridge.md)） |
| `server_autostart` ／ `server_port` ／ `server_token` | `true` ／ 7340 ／ 自動產生 | 本地服務 |
| `llm_env` | `{}` | 服務用的 provider key 快照（服務帳戶看不到使用者環境變數） |
| `update_notify` ／ `auto_update` | `true` ／ `false` | 更新提示／自動更新 |
| `allow_public_egress` | `false` | public 第三方雲（mineru.net 等）egress 總閘 |
| `mineru_command` ／ `mineru_dir` ／ `mineru_tier` | 未設 | MinerU 取用階梯（支援 `%VAR%`／`${VAR}` 展開） |
| `claude_terminal` 等 | — | Claude Code 整合偏好 |

## operoid.toml（企業模式）

放 `<資料目錄>/operoid.toml`（**存在即企業模式**）；範本 `operoid.example.toml` 隨部署包附上：

| 區段 | 內容 |
|---|---|
| `[server]` | host、port（7340）、master token、frontends_dir——**非 loopback ⇒ `[tls]` 必填** |
| `[tls]` | cert ＋ key（PEM）；只配一半會被拒絕啟動（fail closed） |
| `[llm.env]` | provider API keys（服務行程看不到使用者環境） |
| `[gbrain]` | `exe_path` 覆寫（可省） |
| `[knowledge]` | MinerU 階梯＋egress 分層（可省） |
| `[ingress]` | obridge 投遞口 port／secret（可省） |
| `[factory]` | 轉換多工上限，預設 4；飽和時每位遠端使用者至多 1 線程（可省） |
| `[update]` | `check_enabled = false` 可關閉新版檢查（氣隔環境；可省） |

**優先序**：`OSERVER_*` 環境變數 > `operoid.toml` > `app-settings.json`。**熱生效**：`[llm] env`／`[gbrain]`／`[ingress]`／`[factory]`；**須重啟**：host／port／`[tls]`／frontends_dir。

## obridge.toml

```toml
[operoid]            # 進氣目的地：Operoid 的 /event 與共用密鑰
ingress_url = "http://127.0.0.1:17341/event"
ingress_secret = "<密碼A>"

[listen]             # obridge 的外發接收端（員工回覆從這裡出去）
port = 17401
secret = "<密碼B>"

[[channels.email_imap]]   # Email 通道（可多個）
# IMAP/SMTP 帳號、輪詢間隔
# routes：收件地址 → employee_id（員工綁定「對外 email」後自動同步）
# senders：employee_id → From 地址

[[channels.<plugin>]]     # WASM 外掛通道（見附錄 B）
```

`[[channels]]` 與外掛設定**熱重載**（每 2 秒檢查）；`[listen]`／`[operoid]` 變更需重啟。

## GBrain config.json 與 DB plane

每顆腦的模型設定：`chat_model`（`provider:model`）、`models` 分級、`provider_base_urls`、`embedding_model`。「設定 → 模型」的控件會同步寫入 file plane（config.json）與 DB plane（gbrain 資料庫）——**runtime 以 file plane 為優先**；直接手改 config.json 可能被 DB plane 覆寫遮蔽，改模型請走 GUI，「清除所有 DB 覆寫」可歸零。

## 環境變數

| 變數 | 作用 |
|---|---|
| `OSERVER_TOKEN` ／ `OSERVER_PORT` | 覆蓋 master token／監聽 port（優先於 operoid.toml） |
| `GBRAIN_HOME` | 腦的家目錄（預設 `~/.gbrain`）；GUI「腦」頁可逐腦覆寫 |
| `ZHIPUAI_API_KEY`、`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`、`DEEPSEEK_API_KEY`、`GROQ_API_KEY`、`TOGETHER_API_KEY`、`OPENROUTER_API_KEY`、`DASHSCOPE_API_KEY`、`ZEROENTROPY_API_KEY` | 各 provider 的 API key（ollama 不需 key） |

## 資料與日誌位置

| 內容 | 位置 |
|---|---|
| 腦（頁面、圖譜、向量） | `GBRAIN_HOME`（預設 `~/.gbrain`）下的 PGLite＋來源 git repo |
| 員工／模板／承諾／提案 DB（SQLite）＋ `skills/` 樹 | 資料目錄（Windows：本機 AppData `com.operoid.studio`） |
| obridge 日誌 | 資料目錄下的 `obridge.log` |
| 員工輸出 | `employee_output_path`（預設 `~/employee-output`） |
| 工廠轉換稽核 | 來源 repo 內的 `conversion-report.json` |

解除安裝應用程式不會動到以上任何資料。

---
[← 16 企業版三種角色介面](16-enterprise-consoles.md)｜[索引](README.md)｜[18 疑難排解 →](18-troubleshooting.md)
