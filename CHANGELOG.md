# Changelog

本檔記錄 Operoid 各版本的變動。**發布流程**：

1. 發布前，在此加該版本段落（格式見下方；新版本在上方）。
2. 同步版本號（`package.json`／`Cargo.toml`／`tauri.conf.json`／`Cargo.lock`）。
3. `git tag -a vX.Y.Z` + push → GitHub Actions（`.github/workflows/release.yml`）自動抓取
   本檔中對應 tag 的段落作為 Release 說明，build 各平台安裝包後建立 **draft release**。
4. 到 GitHub Releases 頁審閱 draft，確認無誤後 publish。

> 抓取規則：`release.yml` 找本檔中 `## [vX.Y.Z]` 開頭的段落（到下一個 `## ` 為止）作為該
> 版本的 release body。故**版本段落標題必須含 tag 名**（如 `## [v0.2.5]`）。

---

## [Unreleased]

### Capability status reaches the enterprise user tier (upload surface)

- New `GET /api/knowledge/caps` (Req::User in the RBAC matrix): a
  sanitized, cheap capability probe for non-admin roles - MinerU
  resolved (file-existence only, no spawn) and embedding endpoint
  reachable/vision (`/v1/models` only, no long-input embed). No internal
  paths or endpoint URLs leak to user roles.
- User frontend (factory upload view): warning strips before upload when
  MinerU is missing ("only very simple PDFs at full quality - ask your
  administrator") or images are caption-only indexed. Silent when the
  status is unknown (server unreachable).
- Admin frontend (knowledge admin view): capability matrix card
  (MinerU tier/program/version, embedding vision + batch-flag
  discipline, chat VLM) from the existing admin-only health endpoint,
  with unlock instructions.
- ocore: `doctor::quick_status` + degraded semantics tests (offline
  embedding is "unknown", never reported as "missing vision").

### Capability awareness UI - MinerU / multimodal embedding not detected

- The K1 fallback chain (MinerU missing -> fast path, no multimodal
  embedding -> text-only index, no VLM -> locator mode) is invisible to
  users if it only lives in conversion-report.json. The desktop GUI now
  surfaces it:
  - **App-level warning banner** (prominent, dismissible per session,
    with a Recheck button): shown when MinerU is not detected ("only very
    simple PDFs can be processed"), when the embedding endpoint is up
    without vision, or when the llama-server batch-flag regression is
    detected. Unknown states (server down / probe failed) stay silent.
  - **Capability matrix card** in Settings > Services: MinerU (tier +
    program + version), embedding endpoint (model / dims / vision /
    long-input discipline), chat VLM (read-image capability) - each row
    with status icon, impact explanation and unlock instructions
    (localized zh-TW / zh-CN / en).
- oserver matrix inventory: `/api/knowledge/health` registered
  (admin-only, fail-closed) + in-process oneshot test asserting the
  response shape consumed by the GUI.

### K7 - End-to-end retrieval benchmark (P0 4/4) - P0 acceptance met

- New `#[ignore]` real test `converters::mineru_real_tests::real_k7_end_to_end_mueller2016`
  (run: `cargo test -p ocore real_k7 -- --ignored --nocapture`): the full
  pipeline on the mueller2016 corpus - K1 convert() end-to-end (S3 signal
  escalation, ladder tier-1 MinerU spawn), 27 text chunks + 9 figure notes
  with figure 1-9 / table I-V coverage assertions, throwaway gbrain brain
  (git init -> sources add -> sync --no-extract -> embed), then the 12
  experiment queries ported verbatim with the K2 prefix and --no-expand,
  scored by slug-attribute ground truth.
- **P0 gate passed: hit@5 12/12, MRR 0.903** (acceptance line 12/12 and
  >= 0.90; experiment baseline 0.917). Three-document attribution port is
  deferred to P1 with the sidecar landing.
- Run time ~280 s (MinerU CPU ~10 s/page dominates); corpus paths are
  asserted with actionable skip messages when missing.

### K6 — Pipeline health checks + deployment docs (P0 3/4)

- New `ocore::knowledge::doctor`: embedding-endpoint probe (`/v1/models`
  input_modalities / vision flag / n_ctx, plus a ~2,200-token live embedding
  probe that catches the llama-server `-b/-ub 8192` flag regression), chat
  endpoint VLM capability probe (1x1 PNG试探, cached per endpoint - decides
  K5 read-image vs locator mode), and a MinerU ladder probe (resolves the
  ladder and spawns `--version` for local tiers; installs hints when
  missing).
- oserver: `GET /api/knowledge/health` (admin-only via unlisted-route
  fail-closed default) exposes all three probes; optional
  `chat_base`+`chat_model` query params enable the VLM probe.
- DEPLOYMENT.md: new section 10 "PDF knowledge pipeline" - embedding
  service flag discipline (long-input probe), MinerU's four install forms
  (config override / PATH / remote / degrade), egress trust-tier table,
  conversion output layout, and the gbrain version discipline
  (0.60.105.0 verified; rerun the K7 benchmark after upgrades).

### K2 — Retrieval query task prefix (P0 2/4)

- Employee knowledge retrieval (`KnowledgeService.execute_source`, both MCP and
  CLI transports) and the Operations console `query` op now prepend the
  EmbeddingGemma 2 model-card query format `task: search result | query: `
  before calling `gbrain query` - the experiment interception proved gbrain
  passes query strings through verbatim, and the prefix is worth ~0.2 MRR
  (hybrid-RRF pipeline 0.794 -> 0.917 on the 12-query benchmark).
- Query expansion disabled by default (`--no-expand` on CLI, `expand: false`
  on MCP): the measured gain is zero while expansion costs an extra chat
  LLM call and sends the query text to the endpoint.

### K1 — MinerU converter for complex PDFs (multimodal retrieval upgrade, P0 1/4)

- **New converter** `ocore::converters::mineru`: complex PDFs are converted into
  retrieval-ready chunk notes (hybrid V3-t layout: section-bounded text chunks with
  inline figure captions + one standalone note per figure), a `figures.sqlite`
  sidecar seed table (doc/page/image path/caption/section/content md5; vector
  column filled by the upcoming K3), and an auditable `conversion-report.json`.
- **Complexity routing** (default `auto`): millisecond pdf-extract signals S1-S4
  (no text layer / glyph fragments / figure-table reference density / line
  fragmentation) decide between the fast path and MinerU; hard extraction errors
  always escalate to MinerU; `always_fast` / `always_mineru` policies supported.
  Signals and verdict are recorded in the conversion report.
- **MinerU resolution ladder**: config override (`mineru_command`, full path or
  command template - covers venv launchers without activation, `conda run`, `uv run`)
  -> PATH lookup of `mineru-kit` -> remote parsing -> degrade to pdf-extract with
  a quality warning.
- **Egress trust tiers**: remote parsing is governed per endpoint trust level -
  `local` always allowed, `private` (self-hosted api-server URL) consented by
  configuration, `public` third-party cloud (mineru.net) blocked unless
  `allow_public_egress = true` (default false). Every conversion report records
  the egress tier and endpoint. K5/K8 VLM calls will follow the same tiers.
- **MinerU v4 parsing**: `structured_content.json` blocks with section path
  reconstruction (regex-classified headings), atomic markdown pipe tables,
  LaTeX equations kept inline, REFERENCES/ACKNOWLEDGMENT skipped, figure captions
  recovered from neighbouring table blocks (real-corpus quirk), charts with
  numbered captions treated as figures.
- **Config surface**: six new AppConfig fields + `operoid.toml [knowledge]`
  section (`mineru_command`, `mineru_tier`, `mineru_remote_url`,
  `mineru_api_key`, `allow_public_egress`, `pdf_convert_policy`).
- Validated on the mueller2016 corpus (full-document MinerU v4 zip):
  27 text chunks + 9 figure notes with figures 1-9, tables I-V and all section
  levels covered; gbrain pipeline pre-check scored hit@5 12/12, MRR 0.958
  (12-query benchmark from the retrieval experiments).

## [v0.4.5] - 2026-10-08

### Embedding model switched to EmbeddingGemma 2 via local llama-server

- **Default embedding model upgrade**: brains now default to
  `llama-server:embeddinggemma-2` — Google EmbeddingGemma 2 (Apache 2.0,
  768-dim, 256K context) served by a local llama.cpp `llama-server` on
  port 8080 — replacing `ollama:embeddinggemma`. Updated the new-brain
  defaults (`ocore/src/brains.rs`), the desktop add-brain dialog
  (`src/views/BrainsView.vue`), and the real-machine knowledge tests;
  added `llama-server` to the provider tables with base URL
  `http://127.0.0.1:8080/v1` (`ocore/src/gbrain_config.rs`).
- **Existing brains migrated in place**: both live brains (host
  `~/.gbrain`, 3002 chunks; Operoid default brain, 44 chunks) were
  re-embedded via `gbrain migrate embeddings` — same-width 768→768
  guarded transition, query cache purged, self-retrieval smoke checks
  passed, post-migration retrieval verified on Traditional Chinese
  corpora.
- **Deployment docs**: `DEPLOYMENT.md` prerequisites now include the
  embedding service with the required launch flags (`-c 32768 -b 8192
  -ub 8192`; smaller physical batch sizes reject chunks over 512 tokens);
  security model §7.3 and the enterprise-C master plan reflect the new
  model and provider.
- **Configurable default**: the Models tab gains a "default embedding model
  for new brains" setting (model + dimensions, stored in app config;
  precedence: app setting > GBrain config > built-in constant), and the
  add-brain dialog prefill now reads from it instead of a hardcoded string.
- **Enterprise admin UI**: new `GET/PUT /api/brains/defaults` (admin by
  fail-closed RBAC) exposes the same default to the admin web — the brains
  page gains a defaults editor (three-language i18n); DEPLOYMENT.md
  documents the equivalent `app-settings.json` path for servers.
- **Rollback path**: brains can be migrated back with the same command,
  e.g. `gbrain migrate embeddings --to ollama:embeddinggemma --dim 768`.

## [v0.4.4] - 2026-10-07

### Enterprise user-tier factory with scoped conversion targets

- **Factory page for the enterprise user app**: ports the personal-edition
  factory (drop files → convert → preview → edit → overwrite) to the web UI
  user tier — schema-pack type chips, multi-file result list, per-page
  preview/edit/overwrite, auto-classify with a low-confidence confirm
  dialog, and the C13c authoring dialog. Three-language i18n included.
- **File upload staging**: new `POST /api/factories/upload` (multipart) and
  `POST /api/factories/upload/cleanup` — browser frontends have no local
  paths to hand to `run`/`classify`, so files land in an OS-temp staging
  area (per-principal directory + CSPRNG batch id, extension whitelist,
  25 MB/file, 50 files/batch, 64 MB body cap, 24-hour sweeper).
- **RBAC**: factory endpoints (types, upload, upload/cleanup, run,
  write-pages, save-authored, classify) open from Admin/Manager to
  `Req::User`; `extract-companies` stays admin-only (maintenance batch).
  Permission-matrix inventory updated.
- **Write ceiling on every factory write path (C13c, D-C13i)**: `run`,
  `write-pages`, and `save-authored` now enforce
  `enforce_write_ceiling` even when no explicit source is selected
  (defaulting to the configured notes repo).
- **Scoped conversion targets**: `run` and `write-pages` accept an optional
  `target` (circle × level, same shape as `save-authored`). New
  `run_to_scope_core` / `write_pages_to_scope_core` in ocore (provisioning
  and immediate per-source sync refactored out of `authored_to_scope_core`)
  auto-provision the matching scope/source and graph the pages right after
  conversion; the UI conversion-target bar applies to drop conversion and
  auto-classify, and overwrite pins to the target frozen at run time.
  Capture-pipeline types ignore the target.
- **Error mapping**: `knowledge.writeAboveClearance` now maps to HTTP 403
  (authorization verdict) instead of 500.

## [v0.4.3] - 2026-10-07

### 員工桌面工具（M1–M3）——員工從「建議」到「動手」

- **M1 agent loop 改造**：原生 function calling（使用 provider 原生工具呼叫協議；
  provider 不支援時自動降級既有文字 JSON 協議）、多訊息歷史（員工身分卡＋近端對話
  In/Out 上下文）、通道情境注入（email/IM 來源與回覆通道進 prompt）、Stalled
  單次抖動自動重試。
- **M2 員工桌面工具**：ToolContract 工具層＋工作區沙箱——`read_file`／`write_file`／
  `edit_file`／`run_command` 四個桌面工具，Employee 可在自己的沙箱工作區裡讀寫檔案、
  執行指令（read 後才能 edit；路徑限制在工作區內）。
- **M2.8 模板工具授權 UI**：員工模板的允許工具清單進入 UI——write-note＋四個桌面
  工具核取方塊（三語 i18n），部署時隨模板生效。
- **M3 todo 規劃工具**：`update_todos` 落庫（進度清單跨回合存活）、對話回合每 10 步
  週期提醒注入、自主循環 PLAN/EVAL 恆帶進度清單（對抗長任務的上下文漂移）。

### 對話表現力——員工的工作過程從黑盒變可見

- **tool_call 過程事件**：對話回合的每個工具呼叫落一筆事件（工具名／參數摘要／狀態／
  耗時／結果摘要，超長截斷），自動流進 watch API、事件流與 SSE；watch payload 的
  events 上限 20→60 筆（足夠還原完整回合）。
- **聊天頁表現力**（桌面＋user 網頁）：員工回覆以 Markdown 渲染（marked→DOMPurify
  消毒，表格／程式碼／清單）；工具過程摺疊列插在最後一則提問之後（逐步顯示、點開看
  參數，送出訊息自動展開）；working 狀態的「處理中…」指示；訊息內的 artifact 從只顯示
  id 改為可展開內容卡；黏底偵測（上捲閱讀歷史不被輪詢拖回底部）。
- **經理人對話頁**：manager 前端新增 `/chat/:id`（Inbox「聊天」連結原為死連結）——
  經理人可看員工對話與工作過程、也可直接傳訊；user 網頁訊息排序修正（最新在前未反轉）。
- **知識查詢 Markdown 化**：共用 `MarkdownText` 元件（@front/ui／桌面各一）——
  gbrain 的 Markdown 輸出與 `[[wikilink]]`／`[dir/slug]` 引用標記同時正確渲染；
  user/manager 聊天頁改用共用元件（marked/dompurify 依賴集中 @front/ui）。
- **三語 i18n** 同步（chat 區新增鍵）。

### Artifact API 與事件保留（M-A／M-B）

- **`GET /api/artifacts/{id}`**：單一 artifact 讀取端點（RBAC Authenticated＋「限自身」
  以 produced_by → 員工 owner_principal 判定，matrix 測試加列＋權限 e2e）；三端聊天頁
  的 artifact 卡對 watch 近 10 筆之外的舊產出 fallback 載入（桌面走同一 HTTP 端點）。
- **events 保留政策**：細粒度事件（`tool_call`/`llm`/`retrieval`/`plan`/`eval`）保留
  30 天、每日由 scheduler 日界臂自動清理（rowid 窗漸進消化）；里程碑事件
  （reply/turn_error/artifact/outbound…）永久保留；清除 >0 筆時記 `events_pruned`。

## [v0.4.2] - 2026-10-05

### obridge 併入企業封裝＋admin 表單式郵件橋接設定

- **build id 防呆**：oserver／obridge 內嵌 git 短 hash（`build_common.rs` 經各自
  build.rs 注入，dirty 亦標記）；雙方啟動 log 帶 id、`oserver version`／
  `obridge --version` 可查；oserver 帶起 obridge 前以 `--version` 核對，不一致
  大聲警告，`/api/obridge/status` 新增 `exe_build_match` 供 admin 介面顯示。
  動機：sibling 解析只認路徑不認版本——開發期曾發生 target/debug 的 obridge 被
  換回月前舊 artifact（`--check` 被無視→API 卡死）。另加 `cargo dev-build` 別名
  （兩 crate 一起編）與 `scripts/dev-rebuild.ps1`（先停 dev 行程、避開 Windows
  鎖檔 os error 5，建完印 build id 供核對）。

- **企業包內建 obridge**：`enterprise.yml` 改編譯 `oserver`＋`obridge`，部署包新增
  obridge 執行檔（與 oserver 同層）與 `obridge.example.toml` 範本——個人版／企業版
  的郵件橋接能力對齊（先前僅桌面安裝檔打包 obridge）。
- **oserver 子行程代管 obridge**（**僅企業模式**）：啟動期依 `obridge_autostart`
  自動帶起（僅設定檔存在時；Windows 加 `CREATE_NO_WINDOW`），優雅關機收尾 kill，
  stderr 導入 `<settings-dir>/obridge/obridge.log`；存檔後 `[listen]`/`[operoid]`
  變更才重啟（頻道走 obridge 自身 2 秒 mtime 熱載入）。個人模式 obridge 行程仍由
  桌面殼代管——oserver 若也 spawn 會變雙行程（同一份 autostart 旗標＋同一設定檔）。
- **obridge `--check`**：新旗標——只驗證設定檔可解析（缺檔不寫範本），供 oserver
  存檔前驗證（obridge 自身解析規則當單一真相）。
- **admin 前端「郵件橋接」頁**（`/obridge`，Admin 層級）：表單式設定取代 raw TOML
  ——ingress 模式（同機代管／遠端橋接）、listen port／密鑰、email-imap 頻道
  （IMAP/SMTP 帳號、路由地址→員工/腦下拉、寄件身分）、狀態卡（執行中 pid／
  autostart 開關／重啟鈕）；wasm 頻道原樣保留不進表單。三語 i18n。
- **新 API**（未列 RBAC 矩陣 → Admin fail-closed）：`GET /api/obridge/status`、
  `GET/PUT /api/obridge/config`、`POST /api/obridge/restart`。密碼／密鑰**遮蔽**
  （`null`＋`has_*` 旗標，留空＝保留現值）；managed 模式 ingress 密鑰（server
  token）僅伺服器端寫入設定檔，零回傳；存檔自動同步 `event_outbound_url`/`_secret`
  （回信閉環）與 `obridge_autostart`。設定寫入走「暫存檔 → `obridge --check` →
  原子改名」（check 逾時 10 秒即 kill、fail-closed 保留舊檔，API 不卡死），wasm
  頻道與未知頂層鍵 round-trip 保留。

## [v0.4.1] - 2026-10-04

### 部署精靈＋Web 三前端功能補齊＋密碼政策放寬

- **admin 前端「AI 員工模板」管理頁**：模板列表／建立（name／腦／role／tools allowlist）／
  inline 改名／刪除——補齊先前僅 API 與桌面殼層有的模板 CRUD（`POST/PATCH/DELETE
  /api/templates`）。
- **oserver `configure` 精靈**：首次執行（資料目錄無任何設定檔）且終端互動時自動彈出，
  或以 `oserver configure` 手動執行（可管線餵答案）。流程：前置檢查（git/bun/gbrain，
  缺項給安裝指引、可重檢）→ 部署模式 → bind/TLS/frontends → 知識腦工作目錄（重用
  `add_brain_core`，可現場 `gbrain init`）→ master token（CSPRNG 生成）→ 預設管理員
  帳號（`admin`/`admin1234`，首登強改）→ 可選註冊系統服務；寫入前有摘要確認閘。
  企業模式寫出 `operoid.toml`（工作區首個 toml writer）；headless 首次執行印指引後退出。
- **使用者級知識檢索**：新端點 `POST /api/knowledge/ask`（Req::User）——**僅** ask/query/
  think，同步回應，內部與 manager 面同一 KnowledgeService 路徑（C12a 身份過濾＋I1/I2
  fail-closed＋receipt＋`ops_retrieval_enabled` 總開關）；非檢索 op 一律 403。不進 op
  registry（id 為流水號，輪詢面維持 manager 專屬）。user 前端新增「知識查詢」頁
  （ask/think，結果渲染 gbrain 引用標註）。
- **manager 前端「營運主控台」**：對齊桌面 GUI OperationsView——stats/sync/extract/ask/
  query/think＋診斷 ops（doctor/orphans/storage/graph-query/unify-types）經既有
  `/api/operations`（Manager 層級）輪詢執行；ask/query/think 走 KnowledgeService 按
  登入者身份做知識範圍過濾（C12a）。輸出支援 gbrain `[[slug]]` 引用渲染（唯讀標註）。
- **manager 登入頁三語補齊**：login 區塊由僅 offline 一鍵補至完整 13 鍵（en/zh-TW/zh-CN）。
- **密碼下限 12→8 碼**（`MIN_PASSWORD_LEN`）：三前端 minlength 與文案、DEPLOYMENT.md 同步。

---

## [v0.4.0] - 2026-10-03

### 遠端化（E14）——企業模式伺服器端地基＋三角色前端（R1–R7）

> oserver 部署公司內網伺服器（HTTPS＋RBAC），使用者以瀏覽器經 `/admin`（系統管理者）、`/manager`（高階經理人）、`/user`（一般使用者）三個角色前端連入；個人模式（桌面 GUI）零變化。部署指引見 `DEPLOYMENT.md`。

- **R1 傳輸層**：`operoid.toml` 企業模式組態（存在即企業模式；`[server]`/`[tls]`/`[llm env]`/`[gbrain]`/`[ingress]`）、`--host` bind 解除寫死、**非 loopback 無 TLS 拒絕啟動**（DR-E5 fail-closed）、axum-server rustls HTTPS、CORS 收緊為允許清單、靜態三前端服務與入口頁。
- **R2 身份與 RBAC（C12b 最小版）**：帳號密碼登入（Argon2id、密碼政策 ≥12 碼、首登強改、停用即全 token 失效）；token 生命週期（**CSPRNG**、一 principal 多 token、TTL、last_used、逐 token 撤銷、refresh 輪替）；**端點級 RBAC 矩陣**（admin ⊃ manager ⊃ user、伺服器端 403 fail-closed、74 條路由 × 4 身份自動化斷言）；登入防暴（5 次失敗鎖 15 分）；員工歸屬（`owner_principal`）與「限自身」過濾（watch/inbox/events/員工操作/承諾核可）。
- **R3 API 補齊**：`POST /api/commitments/{id}/satisfy`；**SSE 事件推送**（`GET /api/stream`，一次性短票認證、user 過濾自身相關、心跳保活）；`GET /api/service/status`；`/healthz` 回報 version。
- **R4 前端 monorepo**（`frontends/`）：`@front/api-client`（token 記憶體持有、401/403/離線分類、SSE 短票流斷線自動換票重連）＋`@front/ui`（design tokens＋共用元件）＋三 app（Vue 3.5＋Vite 6＋vue-i18n 三語、角色守衛、登入頁含首登強改密）。
- **R5 一般使用者前端**：部署員工（歸屬自己）、員工聊天（watch 輪詢＋送訊息）、交辦（202）、員工提案核可/退回、收件匣與事件流（伺服器端過濾自身相關）。
- **R6 高階經理人前端**：營運儀表板（員工狀態總覽＋registry 分歧）、跨組織收件匣核可、**知識治理**（TTL 授權發放/撤銷＋scopes 總覽）。
- **R7 系統管理者前端**：帳號管理（建號/停用/重設密碼/角色指派）、知識授權管理（principal/token/scopes/policy 編輯器）、腦與前置檢查。
- **測試**：workspace 270+ 全綠、0 warning；真實企業模式 HTTPS 冒煙（建號→登入→強改→403 授權→停用失效→SSE 推送）與瀏覽器 e2e（三前端登入/首登強改/建號/部署/聊天/交辦）全過。
- **個人版／企業版 release 資產分離**：桌面安裝包＝個人版（零設定）；Release 另附 `operoid-enterprise-{windows-x64.zip, linux-x64.tar.gz}` 企業部署包（oserver＋三前端＋`DEPLOYMENT.md`＋`operoid.example.toml`，伺服器機不需 Node/pnpm）。
- **殘項**：M1 式真實環境驗收（真實 GBrain＋LLM 全閉環）；Linux systemd 實機驗證；obridge/工廠寫 UI 過渡期續用統一 GUI。

## [v0.3.7] - 2026-10-02

### Enterprise C′（Permission-aware Knowledge Fabric）

> 第一程式里程碑（M1）已兌現並以 real e2e 實機證明：**同一 GBrain＋同一查詢＋不同 AccessContext＝不同授權檢索結果**。

### 分級保密（C13a–c）

- **保密等級全序**：`Public < Internal < Confidential < Secret`（`SecurityLevel` 枚舉）；scope 的 classification 與規則的 classifications 條件升級為等級型別（既有 `internal` 資料零遷移）。
- **I9 保密天花板**：`scope.classification > principal.clearance` → 硬拒——先於一切規則，**grant 與 explicit deny 同級不可破**。未賦 clearance＝Internal（bootstrap 基準線）；operator＝Secret 基準線（既有列冪等升級）。
- **clearance 管理**：`PrincipalAttrs.clearance`（`set_principal_attrs` 既有入口賦權）；屬性富集讓 AccessContext 攜 clearance 到檢索邊界；receipt/事件記下判定當下的 clearance。
- 測試：天花板（超等級即拒／grant 不可破／operator 全級距／賦權後打開）＋既有 225 測試全綠。
- **政策管理面**：`POST /api/knowledge/policy`／`/scopes`／`principals/{id}/attrs`；設定頁「知識授權」分頁升級——政策規則編輯器（優先序/effect/白名單 CSV/成員制）、scope 建立表單（含保密等級）、principal clearance 賦權選單。
- **寫入端分級（Q12 兌現）**：`provision` 自動供給（圈子×等級→scope/source 慣例命名＋冪等建立＋owner 白名單合成）；`authored_to_scope_core`（撰寫器文章寫入指定圈子×等級，`sync --source` 立即入圖）；**寫入端天花板**（所選來源等級 > 作者 clearance → 403，與 I9 對稱）。
- **撰寫器寫入目標選擇**：對話框新增保密等級＋圈子選擇（company+internal＝預設公司層）；批次路徑（write-pages）同接寫入端天花板——工廠所有寫入路徑皆受分級管制。
---

### 新增（ocore/src/knowledge/ 模組）

- **身份基礎建設**：`principals` 表＋operator bootstrap（冪等）；AI 員工 principal 由 Employee 讀時推導；`ToolCtx.access: AccessContext`——身份到達檢索邊界（此前檢索完全無身份）；`oserver` `AccountProvider`/`SingleOperatorProvider` 插座。
- **知識授權**：`KnowledgeScope`（scope→GBrain source 映射，department/project/classification）＋`KnowledgePolicy`（優先序規則：principal/types/scopes/departments/projects/classifications/**成員制旗標**）——純函式評估、DENY 優先、**fail closed**（缺/壞 policy＝全 DENY＋事件）。
- **KnowledgeService（唯一檢索邊界）**：員工 think/search 與 Operations 主控台 ask/query/think 一律經 policy→授權 source 集→**逐 source 呼叫**（GBrain `pageReadFilter` SQL 級強制，零 GBrain 修改）；任務聚焦（QueryPlanner：綁專案任務自動收窄候選集，只縮不擴）。
- **Retrieval Receipt**：每筆檢索結構化落表＋`retrieval` 事件（Who→Employee→Task→Policy→Scope 鏈可重構；跨域標記；90 天保留＋每日 prune）。
- **臨時授權 TTL grants**：principal×scope×TTL；**三段式**——explicit deny 不可被 grant 架空、default deny 可由 valid grant 破；屆期查詢時即時判定＋scheduler 冪等掃描；create/revoke/expire 全稽核。
- **身份/政策稽核**：`principal_created`（冪等）／`principal_attrs_changed`／`knowledge_policy_changed`（version+1）／`receipts_pruned` 事件。
- **Bootstrap**：啟動時冪等把既有 sources 全數映射 `co-common` scope＋明示 allow-all policy——**單人版行為不變**（舊資料不搬頁，Q3）。

### 變更

- `ToolCtx` 擴欄 `access`＋`knowledge`（全部建構點同步；`agent_run_team` 繞道變體一併消除）。
- `build_tool_ctx` 簽名增 `db_path`；scheduler 增 grants 屆期/receipts prune 日界臂。
- EventsView 事件配色：`retrieval`＝sky、`knowledge_policy_changed`＝amber、`knowledge_policy_invalid`＝destructive。
- `gbrain_cli` op `stats` 對應 gbrain 0.60 的 `status` 指令（上游漂移修正，M0 實測）。

### 安全測試（提示詞 §16 十情境）

- T1 同查詢不同身分／T2 跨部門成員制／T3 受限文件永不入 context／T4 臨時授權屆期／T5 撤銷即時／T6 chunk 繼承鏈／T7 prompt injection（policy 為權威）／T8 冒名防護／T9 檢索稽核／T10 confused deputy——**全數解鎖並通過**（`cargo test -p ocore`＝224 過；real_* e2e `#[ignore]` 手動跑）。


> 設定頁分頁化＋登記表從原始 JSON 改為**結構化列表編輯器**——使用者不必再記 JSON 鍵名、也不會打錯字。**後端與 API 不變**（`GET/POST /api/registry`），預設行為不變（未登記類別一律送人類核可）。

### 設定頁分頁化

- `ConfigView.vue` 拆為 `config/ConfigLayout.vue` ＋五分頁：應用程式（AppTab）、模型（ModelsTab）、服務（ServicesTab）、登記表（RegistryTab）、進階（AdvancedTab）；router 改巢狀路由（`/config/{app,models,services,registry,advanced}`），i18n 新增 `configView.tabs.*` 三語。

### 登記表結構化編輯器（RegistryTab 重寫）

- **三層委任解說**（`registry/TierGuide.vue`）：人裁決層／圍欄自動層／有主自動層的分工與入場條件，附從嚴預設說明（未登記類別一律視為人裁決層）。
- **全域設定**：保險絲關鍵字 chips（可增刪；清空＝停用）、每週提案預算（留空＝不設限）、抽審分歧門檻（0–1）。
- **類別手風琴卡**（`registry/CategoryCard.vue`）：摘要列（id／說明／層級彩色徽章／屆期警示），點開就地展開完整表單；**依層級條件顯示**——fenced/owned 才出現屆期（datetime-local ↔ RFC3339 自動轉換）、抽審比例、放寬證據、圍欄（工具白名單／禁外發／僅模板），owned 另顯緊急停止機制。新增類別預設人裁決層（從嚴）。
- **存檔前客戶端鏡射 V1–V5 驗證**：id 非空且唯一、抽審比例 0–1、放寬層需問責人＋證據＋未來屆期、owned 需緊急停止；未通過的卡片**自動展開**並逐條行內提示，修正前不送出（伺服器仍是最終把關）。
- **載入正規化**：缺欄位補 serde 預設值——手改壞的檔案也能載入修復；屆期與抽審分歧警示保留。
- **進階收合區**：原始 JSON 由表單狀態生成，可手動編輯後「套用到表單」（parse→正規化），存檔一律以表單為準。
- i18n：`configView.registry*` 三語各 +75 鍵。
- 驗證：vue-tsc + vite build 0 error；瀏覽器煙霧測試（mock registry API）走完載入重試、展開、層級切換、驗證擋存、存檔版本 +1、新增／刪除類別、原始 JSON 套用全流程。

### docs

- README 三語：Current status 補「委任界線」（v0.3.4–v0.3.5）條目；Development 指令改為 workspace 根目錄 `cargo test`／`cargo check`（原 `cd src-tauri` 會漏掉 ocore/oserver 測試）；修正 zh-TW 重複破折號與 zh-CN 殘留繁體字。

## [v0.3.5] - 2026-09-29

> **動作類別登記表（委任界線）**——治理理論見 `journal/界線的形狀_朱國棟.pdf`；計畫 `docs/Operoid-計畫-動作類別登記表.md`（R0–R6）。配套**憲章修正 v1**（Handbook 中英：Ch.02 增補原則 11「界線由人類畫定」、Ch.20 §5 重寫為「委任界線」、Ch.11 §5 自動啟用路徑、Ch.20 §3 人類邊界含畫線）。**預設行為不變**：未建立登記表時，一切員工提案照舊送人類核可（從嚴預設）。

### 畫線器具：動作類別登記表（R1–R2）

- 新檔 `ocore/src/registry.rs`：ActionRegistry／ActionCategory 三層委任模型（人裁決層／圍欄自動層／有主自動層）。**存檔驗證 V1–V5＝不對稱修正的程式碼化**：放寬（fenced/owned）須具名課責＋未來屆期＋放寬證據，缺一拒存；收緊永遠允許。原子寫；缺檔／壞檔 fails closed（全部走人類核可，並記 `registry_invalid` 事件）。
- 員工提案分類決策樹（`classify_proposal`）：命中登記的自動層類別 → 跳過 Proposed 直接 Active（`auto_activated` 事件、watch「自動啟用」區段全數可見）；**關鍵詞保險絲**（對外/客戶/報價/金額/交期/合約/刪除）凌駕類別自評；系統 prompt 只列有效類別——員工看不到的類別就聲稱不了（封閉白名單＝保守解析＋新穎性上送）。
- `Commitment` 新增 `category_id`／`gate_reason`／`review_pending`（JSON blob 零遷移）。

### 時間性與圍欄（R3–R4）

- **屆期重簽**：逾期自動層類別記 `category_lapsed`（冪等）；自動啟用效力查表即時判斷——過期即失效，無狀態可漂移。
- **事故自動收縮**：承諾重試耗盡 → 所屬類別**連坐凍結**（`category_frozen`，降回人裁決層待重簽）——一次事故，收縮的是線，不是只停一筆。
- **圍欄執行**：登記類別 `no_outbound`（預設 true）→ 自主循環禁外發，兩層防護——PLAN 選項抑制（看不到就不會選）＋send 分支硬閘（`send_blocked` 事件）；對話回合的回覆外發不受限。

### 審核端治理（R5–R6）

- API：`GET /api/registry`（含 30 天分歧率）、`POST /api/registry`、`POST /api/commitments/{id}/review`、`POST /api/registry/drill`。
- 前端（i18n 三語）：設定頁「動作類別登記表」原始編輯器（version／逾期警告條／分歧率顯示）；watch 彈窗「自動啟用」區段＋抽審「歸類正確/錯誤」判定鈕；核可卡「為何待核可」原因行（M2 呈現設計——讓賭注可見）；Events 治理事件配色。
- **盲抽校準**：抽審判定（`sample_verdict`）＋分歧率超門檻警報（`divergence_alarm`，樣本 ≥5、每週冪等）；植入演練 v1（`drill_result`——直接呼叫分類函式，不經 LLM，只測確定性閘門）；流量預算（`budget_exceeded`＝白名單過窄警報／`gate_bypass_warning`＝零核可但大量自動啟用）；月度回饋（`monthly_feedback` 放對/放錯/攔截啟發式）。

### 其他

- fix(config)：登記表載入失敗時補重試按鈕。
- docs：JOURNEY 補 R0–R6 紀錄與 v0.3.4 release 狀態；設計／計畫／憲章修正案三份文件入 `docs/`。
- 驗證基準：ocore 184 測試（+32）、oserver 4、workspace 0 warning、npm build 0 error。

## [v0.3.4] - 2026-09-18

### 預設模型升回 glm-5.3-flash（解 E15）

- 上游 garrytan/gbrain#4727 已於 **gbrain 0.50.x** 為 zhipu recipe 補上 `thinking_by_default`——gbrain 不再把 GLM 5.x 誤判為非 thinking 模型。實測 think/chat 的 model-aware output cap 恢復 **16000/32000**（`output_tokens` 精確停在 16000 驗證），v0.3.1 回退 glm-4-flash 時的長回應截斷（`LLM_OUTPUT_TRUNCATED`）風險解除。
- `DEFAULT_CHAT_MODEL` 由 `zhipu:glm-4-flash` 升回 `zhipu:glm-5.3-flash`（`ocore/gbrain_config.rs`）；常數文檔與相關測試記錄完整升降級脈絡。
- 前端同步：三語系「主模型」placeholder 與 BrainsView「新增腦」預設 chat model。

## [v0.3.3] - 2026-09-07

> Harness 補強（W0–W3）——計畫 `docs/Operoid-計畫-Harness補強.md`。含兩個**行為變更**（見「⚠️」）。

### 員工可攔截：合作式停止（W1）

- 執行中的員工可被人工停止：`POST /api/employees/{id}/stop` 設旗標，runner 於步驟邊界（PLAN/ACT/EVAL 每輪、對話每步、任務間、承諾間）優雅中止；進行中的單次 LLM 呼叫會跑完（最長約 2 分鐘）。
- 停止語意：承諾維持 `Active`（人可再觸發）、記 `stopped` 事件、員工轉 **`Paused`**——不再被排程器自動喚醒；**人類傳訊息／交辦／核可＝明示恢復**（Paused→Sleeping）。
- 前端：監看 modal「停止」鈕（working 時可用）＋右鍵「停止」；`agent_os.employeeNotRunning`（409）等新錯誤碼三語。

### 員工封存與硬刪除（W1，E13）

- ⚠️ **`DELETE /api/employees/{id}` 語意變更**：預設＝**封存**（軟刪除）——`archived=true`、不再被喚醒／不收新訊息與事件，歷史（對話／事件／產出）完整保留可追溯；`POST /api/employees/{id}/unarchive` 解封。
- `DELETE ...?hard=true`＝**串聯硬刪除**（tasks／messages／events／commitments／artifacts／memory 全清）——僅供開發／測試，無 UI 入口。
- 封存時若員工執行中，順帶合作式停止。前端：右鍵「封存…」取代「刪除」、主列表過濾、「已封存」檢索區＋右鍵解封。

### 恢復力：LLM 重試＋承諾自動重排（W2，T2）

- LLM 層：**5xx 納入可重試**（原僅網路錯誤／429）；指數退避 5s→10s→20s（3 次上限）；4xx 立即失敗。token 用量（usage）best-effort 記入事件（kind `llm`，含 model＋tokens）。
- ⚠️ **承諾自動重試**：`run_autonomous` `Errored` 的承諾自動排程重跑——退避 10min·2^n（上限 4h）、最多 3 次，耗盡後記 `retry_exhausted` 事件待人類；`Satisfied`／人工再觸發歸零。排程器每 tick 只喚醒「退避到期」者——健康承諾不重跑、不出錯不排程（backpressure）。
- 事件匯流排：`dispatch_event` 路由命中時 fire-and-forget `brain_sync`（E8——員工查詢前圖譜已含剛寫入的完整長文；race 由 content 全文兜底）。

### 第一個行動工具：write-note（W3）

- 員工可把完整產出寫成 markdown 筆記：新工具 `write-note`（`ocore/write_note.rs`）——寫到 `employee_output_path`（新設定，預設 `{home}/employee-output`，**在 notes repo 之外、不入圖譜**）；人工 review 後移入 notes repo 走既有 sync 晉升（Handbook Ch.07 §6「衍生知識：產出→驗證→晉升」的最小落地）；收回＝刪檔。
- 防護：僅接受單一 `.md` 檔名（拒路徑分隔／絕對路徑）、拒覆寫、canonicalize 包含檢查；frontmatter 帶 `origin: operoid-employee`＋produced_by 可批次識別。
- 權限閘門 v1（建構期 allowlist）：`Employee.tools`／`EmployeeTemplate.tools`（模板部署繼承；建立模板 API 加 `tools` 參數、表單加核取）；對話迴圈 `write` 動作（寫檔＋Committed artifact）與自主循環 PLAN `tool=write` 分支（Draft、Satisfied 才晉升）；無權限回報員工不中斷。
- 前端：員工右鍵「開啟產出目錄」（新殼層指令 `employee_open_output_dir`）。

**品質**：ocore 152 tests passed（+19）、oserver 4；`cargo check --all-targets --workspace` 0 warning；前端 build 綠燈。

## [v0.3.2] - 2026-09-01（重發布）

### 工廠頁支援 gbrain schema v2（15 類型）＋動態 schema pack

- **單一類型資料表**：新增 `ocore/src/factory_types.rs`，內建 `gbrain-base-v2`（15 型：person/company/media/tweet/social-digest/analysis/atom/concept/source/deal/email/slack/writing/project/note）與 legacy `gbrain-base`（原 6 工廠）兩份定義（目錄、frontmatter 型別/標籤、管線、分類同義詞）。原本散落 5 處的 Rust hardcoded match（`target_dir_of`/`run_core`/`run_textual`/`save_authored_core`/`factory_open_dir`）與 `text_to_md::render` 的型別對照全部改為查表；未知類型明確報錯，移除「誤歸 concepts」的靜默 fallback。
- **動態 schema pack**：pack 名取自 gbrain config file plane 的 `schema_pack`（未設定 → v2、未知 pack → v2 表）。新增 `factory_types` Tauri command 與 `oserver` `GET /api/factories/types` 路由，回傳 pack 資訊＋類型清單＋v1 升級提示。
- **工廠頁主從式改版**：自動分類 hero 卡保留；15+ 類型改為緊湊 chip 清單（左側，動態載入），右側為選中類型的單一拖放/預覽工作區——頁面高度固定不捲頁，未來 pack 增減類型自動跟隨。非 v2 pack 顯示橫幅提示（連到設定頁的 unify-types 按鈕）。
- **分類器 pack 感知**：LLM prompt 的類型枚舉、同義詞正規化（people↔person、meetings↔meeting 等單複數/中文）、heuristic 對應全部改為依作用中 pack 查表；信心分級（高/中自動跑、低交確認）維持不變。v2 無 meeting 型——會議特徵在 v2 pack 交 LLM 判讀。
- **v2 對齊細節**：mentioned_names wikilink 改用該 pack 的 person/company 目錄（v2=`[[person/slug]]`）；wikilink 解析候選目錄（`note_view.rs`）涵蓋 v2+legacy 目錄，跨 pack 舊連結仍可開啟；v2 的 company 頁不再寫 `subtype`（type 已是 company）。
- i18n：三語系補齊 15 類型標題/輸出說明與範本（未知/自訂 pack 類型 fallback 顯示 id、通用範本）。
- 測試：`factory_types`（pack 對應/同義詞/kind 查表）、`classifier`（v2/legacy 雙 pack）、`text_to_md`（v2 render/wikilink 目錄）擴充，全數通過。

### 視窗啟動最大化

- 主視窗（`tauri.conf.json`）加 `"maximized": true`：啟動即最大化，避開 Windows DPI 縮放下 `center: true` 置中計算不準的問題；還原時仍退回 1280×840 置中。

### dev／安裝版的 oserver 舊碼殘留防護

- **dev**：`npm run tauri dev` 的 `beforeDevCommand` 改為新的 `npm run dev:all`（`cargo build -p oserver && npm run dev`）——每次 dev 先確保 `target/debug/oserver.exe` 為最新。此前 `tauri dev` 只編 GUI app crate，不會編 oserver，改過 oserver/ocore 後跑 dev 會出現「GUI 新、服務舊」的路由 404（已遇過一次：背景殘留的舊分離行程 + 未重編的 exe）。
- **升級安裝**：NSIS hooks 新增 PREINSTALL／POSTINSTALL——服務在跑時（以 `sc stop` 回傳碼偵測）先停服務釋放 `oserver.exe` 檔案鎖，裝完自動 `sc start` 重啟；原本已停止或未安裝服務者不受影響。修復「服務模式 + 升級」時舊服務續用記憶體中舊碼／覆寫 exe 失敗的縫隙。反安裝行為不變。

### 操作頁 think：輸出末列出全部引註（可點擊連結）

- 「操作」頁的 think 改以 `gbrain think --json` 執行（`ocore/src/gbrain_cli.rs` 新增 `run_think_json`），取得結構化 citations 後重排為人類可讀格式：問題標題、answer、Gaps、`Model/Pages/Takes/Graph/Citations` footer，最後新增「## 引註（Citations）」清單。
- 引註行採雙括號 wikilink 形式（`[[people/林家豪]]`、`[[林家豪]]（take #3）`）——前端 `OperationsView.linkSegments` 無條件匹配雙括號，不受單括號引註「slug 須含 `/`」規則限制，模型給含前綴或裸標題的 page_slug 皆可點擊開啟筆記（`openNote` 支援裸標題掃描 people/ 等已知目錄）。
- 隱去 `CITATIONS_STRUCTURED_NOT_INLINE`／`CITATIONS_INLINE_NOT_IN_STRUCTURED` 引註比對警告：gbrain 行內標記 regex 僅接受 ASCII slug（中文頁面恆無法判定為行內），且 glm-4-flash 常在本文留下 `[slug#N]` 佔位字面值——兩方向的比對警告對本應用恆為雜訊，引註已由清單完整呈現。其他警告照常顯示。
- 顯示前自 answer 剔除 `[slug#N]` 佔位標記。
- 寬容退路：非零退出碼或 JSON 解析失敗（舊版 gbrain）時逐行原樣輸出 stdout，操作不失敗；stderr（升級提示等）結束後補推不吞訊息。
- 已知取捨：`--json` 模式下 gbrain 整段輸出，answer 不再逐行串流（期間顯示「think：合成中…」step 提示）。
- 測試：新增 `citation_line_matches_link_segment_format`（引註行格式鎖定）。

## [v0.3.1] - 2026-08-31

### gbrain 整合升級——對齊 gbrain v0.46＋模型層重整

**對齊 gbrain v0.46（三項）**

- **MCP provider**：think／query 優先走 `gbrain serve` stdio MCP（rmcp 3.x），失敗自動 fallback CLI 子行程；管理操作仍走 CLI。新設定 `gbrain_transport`（mcp 預設／cli），設定頁可切換。
- **search／think 分離**：新增 `GbrainSearchTool`（`gbrain query` 混合檢索，無 LLM 合成、省 token）與 `GbrainToolset` 複合分派器；操作頁新增 query 按鈕。**注意**：gbrain v0.46 起 `ask` 已是 `query` 的別名（純檢索），帶引用連結的合成回答改由 `think` 提供。
- **schema pack v2 對齊**：factory 寫出的 frontmatter 加 `origin: operoid-factory`；legacy v1 腦顯示遷移橫幅＋unify-types 按鈕。

**模型層重整**

- 預設模型**回退為 `zhipu:glm-4-flash`**：gbrain（≤0.47.x）對 GLM 未視為 thinking-by-default（zhipu recipe 缺 `thinking_by_default`，上游 issue garrytan/gbrain#4727），glm-5.x 的 think／chat 分別被壓在 4000／4096 output tokens，長回應易截斷；glm-4-flash 為非推理模型不受影響。上游修復後可再升回。
- **think（多跳合成）顯式 model 改取 `models.think`**（缺時 fallback `chat_model`）：實測 thinking 模型的推理 token 計入合成 max_tokens 額度，長篇合成穩定 `LLM_OUTPUT_TRUNCATED`→空輸出；think 可獨立指到非推理模型（如 glm-4-flash），chat 維持強模型。手動操作、agent CLI、MCP 三條路徑一致。
- 修復 unify-types 按鈕：補 `--params {target_pack, apply}` 與 `--follow`（缺參數會 permanent fail；PGLite 腦無背景 worker 須 inline 執行）。

**config plane 事實修正（gbrain 0.47.6 實測）**

- gbrain runtime 對 model/tier 鍵採 **file plane（config.json）優先**，DB-plane 值被 shadow（`gbrain config get` 明示）——專案原本「DB plane 權威」的假設與事實相反。
- `set_model`／`set_models_all`（設定頁主模型／tier 編輯）與新腦的 `sync_new_brain_models` 改為**兩 plane 同步寫入**（file 直寫 + `gbrain config set`），不再只寫 DB 而被檔案舊值蓋掉。
- 設定頁 tier 顯示改用 `gbrain config get` 的有效值與來源（file／db 徽章如實呈現）；`db_overrides` 語義改為「檔案無值、由 DB fallback 的鍵」，警告橫幅與提示文案同步修正。

### 對話回合修復

- 對話回合 user prompt 與自主循環 PLAN prompt 注入現在時間（日期感知）。
- 同通道單一回覆保證：同目標一回合僅一則成功送出，抑制 send／finish 重複寫出 Out Message。新增對應測試。

**品質**：ocore 123 tests passed；oserver 4 passed；前端 build 綠燈。

## [v0.3.0] - 2026-08-19

### 前後端分離完成——Operoid 成為常駐服務＋多前端架構

P1–P5 五階段（計畫 `docs/Operoid-計畫-前後端分離.md`）全部落地。

#### 主要變動

**`ocore` 核心 crate（P1）**

- 全部領域邏輯（domain／runtime／scheduler／event bus／GBrain 能力域）脫離 Tauri——**零 Tauri 依賴**
- `Channel<CliLine>` 串流改 `LineSink` 回呼

**`oserver` 服務 binary（P2–P4）**

- axum HTTP API：agent-os 讀寫面＋GBrain 全域＋ring buffer 主控台輪詢
- `AuthProvider` trait（token 首個實作——企業版帳號插座）
- bind-first＋healthz；SQLite 全走 `spawn_blocking`
- per-brain 序列化待寫入面後續

**桌面前端切換 HTTP（P3–P4）**

- agent-os 與 GBrain 全頁面經 `127.0.0.1:7340`（Bearer token；CORS；wrappers 簽名不變、stores 零改動）
- GUI 正式成為「諸多前端之一」

**服務註冊（P5）**

- **Windows**：SCM（UAC 自我提權；實機驗證）
- **Linux**：systemd system unit（`/etc/systemd/system`——`sudo` 提權安裝，服務以安裝使用者身分執行）
- **macOS**：launchd LaunchDaemon（`/Library/LaunchDaemons`——同上，以安裝使用者身分執行）
- 後兩者 CI 編譯覆蓋、未實機驗證；設定頁「開機自動啟動」開關

**生命週期雙語意**

- 服務已裝 → 開機自啟、GUI 關閉不影響（A1）
- 未裝 → GUI 帶起帶走（A2）

**ingress 併入 oserver（P5）**

- `POST /event`（token＋去重）——GUI 不開，obridge 投件不再掉
- `ingress_server`／`event_bus` 殼層退役

**`src-tauri` 瘦身為殼**

- 視窗＋桌面專屬能力（claude_code／note_view／open dir）＋指令薄層

#### 已知邊界

- E13 員工封存語意、E8 fire-and-forget sync、SSE 串流——列於待處理清單
- Linux/macOS 服務註冊未實機驗證（程式在、CI 編譯覆蓋）

---

## [v0.2.7] - 2026-08-18

### 前後端分離首步（ocore）＋ think 修復＋預設模型改正

#### 主要變動

- **P1a 抽取 `ocore` 核心 crate**（前後端分離計畫首片，`docs/Operoid-計畫-前後端分離.md`）
  - 新 workspace member `ocore`：`domain/`、`agent_state`、`llm`、`outbound`、`i18n`＋`slug`／`gbrain_config`——**零 Tauri 依賴**，桌面殼與未來服務 binary（`oserver`）共用
  - `src-tauri` 以 re-export 保持既有程式碼路徑零改動；介面微調三處（行為零變）
  - 驗證：workspace 測試 137 全綠、0 warning、前端 build 0 error

- **E9 補遺——OperationsView 手動 think 顯式傳 `--model`**
  - E9 原修復只蓋 agent 路徑；手動 think 在 DB-plane models 未設時同樣 fallback 到 anthropic opus → synthesis skipped
  - 比照修復：以作用中腦的 `chat_model` 顯式指定

- **預設 chat model 改 `zhipu:glm-4-flash`**
  - glm-5.x 全系為推理模型（回應含 `reasoning_content`），gbrain think synthesis 解析不相容（`LLM_OUTPUT_NOT_JSON` → 隨機空輸出；coding 端點還會把 5.2 映射成 5.3）
  - glm-4-flash 非推理模型，標準與 coding 端點皆實證可用——新使用者開箱即用

- **obridge 打包**：安裝檔內建 obridge（externalBin＋beforeBundleCommand 自動建置）＋ CI 修復

## [v0.2.6] - 2026-08-17

### Obridge（Email bridge）落地

#### 主要變動

- **E7 步驟 3——Obridge（Operoid Bridge）**：Email 雙向通道（IMAP 收＋SMTP 寄）＋WASM 外掛體系（wasmtime＋component model；HTTP 類通道如 Slack/Teams 未來走外掛）
- **outbound v2（E12）**：完整 send Tool（tool-choice 編排）——外發統一為員工行動；對話回合 tool-loop（think/send/propose/finish）＋自主循環可主動通知
- **obridge 生命週期**：設定檔熱重載（watch toml mtime）＋Operoid 子進程代管（opt-in autostart）；外掛設定傳遞（WIT `init(config)`）
- **零設定**：路徑預設值＋執行檔自動偵測；設定檔為空時自動複製預設範本

---

## [v0.2.5] - 2026-08-13

### 自主循環真實環境打通

v0.2.4（Event 匯流排）以來，聚焦讓承諾驅動自主循環在真實 LLM 環境下真正能完成。

#### 主要變動

- **E2 自主循環可診斷性 + 可收斂性**
  - PLAN／EVAL 診斷軌跡：`record_event` 每輪記 `plan`/`eval` 事件（含 query/done/rationale）——Stalled 時也能看見 LLM 每輪判斷
  - PLAN prompt 強化：看見近期 artifact 內容（消除「PLAN 瞎子」）+ done 判據引導
  - 鬆綁重複偵測 `MAX_REPEAT=2`

- **E9 gbrain think synthesis 缺 LLM 修復**（E2 軌跡揭露的真根因）
  - 根因：think 子行程讀 DB-plane `models.*`（fallback → anthropic opus），不讀 `chat_model` → synthesis 找 `ANTHROPIC_API_KEY` 失敗 → "no LLM available; synthesis skipped"
  - 修法：`GbrainThinkTool` 顯式 `--model <chat_model>`，跳過 fallback 鏈（零 DB-plane 副作用）
  - **承諾驅動自主循環真實環境首度 Satisfied**（`real_run_autonomous`：Stalled 9-10 cycles → 1 cycle Satisfied）

- 收尾 sprint：T1 真實整合測試、文件債清理

#### 測試
`cargo test` — 103 passed; 7 ignored

---

## [v0.2.4] - 2026-08-12

### Event 匯流排架構

Handbook Ch.12 第四種 Trigger（Event-driven）落地：

- factory 寫入 → `InboundEvent` → 腦 → 員工 1:N 路由 → review/提案（Workstream A–E＋G）
- LLM 全域 Semaphore 並發節流（預設 4）
- `run_autonomous` 每輪先清 inbox（修復 doc/impl 不一致）

Webhook 進氣口（F）為 Phase 2 待辦。
