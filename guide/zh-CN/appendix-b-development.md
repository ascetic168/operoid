# 附录 B：Operoid 开发说明

> 适用版本：v0.5.11。给要**在 Operoid 上开发自家接口**或 **fork 修改**的企业团队——假设你们以 Vibe Coding（Claude Code 等 coding agent）开发。核心信息：**先让 agent 读契约与示例，再动手**；本附录给你信息地图与可直接复制的示范提示词。

## 先选路线：插件扩充 vs fork 修改

| | 路线一：Obridge 插件 | 路线二：fork 修改 |
|---|---|---|
| 适合 | 对接企业 IM／内部系统——加一个通道、一个进出接口 | 改核心行为——新 API、新工具、新治理规则 |
| 成本 | 低——沙箱内写一个 WASM 组件，**不碰核心代码** | 高——维护 fork、承担上游同步 |
| 升级 | Operoid 升级照常，插件原样可用 | 每次升级要合并上游 |

**原则：能用插件就不 fork；要 fork，把改动集中、把契约层留给上游。**

## 信息地图：该读哪里

Coding agent 最怕的不是不会写，而是**读错地方**。以下是每类任务的权威信息源（路径相对于仓库根）：

| 要做什么 | 读这里 | 说明 |
|---|---|---|
| 通道插件接口 | `obridge/wit/channel.wit` | **契约**：`channel`（source／init／poll／send）与 `host`（make-request／kv／clock-now）；刻意不提供 TCP/TLS |
| 插件怎么写 | `obridge-plugin-example/`（echo 示例） | wit_bindgen 用法、kv 持久化、配置传递、构建命令 |
| 进出接口契约 | `ocontract/src/lib.rs` | `InboundEvent`／`SendPayload` 字段——薄外壳，两端都映射它 |
| obridge 操作 | `obridge/README.md` | 构建运行、**全链 e2e 验证步骤**、WASM 插件开发、热重载、托管 |
| 配置模板 | `obridge/config.example.toml` | `[[channels]]` 怎么写 |
| 事件进出口 | 本指南[第 14 章](14-email-bridge.md)、[第 17 章](17-config-reference.md) | `event_ingress_port/secret`、`event_outbound_url/secret` |
| 员工执行引擎 | `ocore/src/runtime.rs` | 员工唤醒、工具循环、承诺生命周期 |
| HTTP API 与权限 | `oserver/src/routes.rs`、`oserver/src/rbac.rs` | 端点清单与 RBAC 矩阵（74 路由 × 4 身份） |
| 配置字段 | `ocore/src/app_config.rs` | 每个字段都有注释说明设计理由 |
| 桌面 IPC | `src-tauri/src/lib.rs` | 桌面 → 服务的全部命令 |
| 概念与术语 | `handbook/Chinese/`（Ch.14 事件、Ch.16 讯息、Ch.18/19 SDK） | 为什么这样设计 |
| 历史与脉络 | `CHANGELOG.md`、`JOURNEY.md` | 每个决策的来龙去脉 |
| 企业部署 | `DEPLOYMENT.md` | 服务器端配置与服务 |

## Fork 修改指南

### Workspace 结构

| 目录 | 角色 |
|---|---|
| `ocore/` | 核心域逻辑（Runtime／Store／Tools／GBrain 客户端）——零 Tauri 依赖的函数库 |
| `oserver/` | HTTP 服务（企业模式；个人版也由桌面托管） |
| `obridge/` | 外部通道桥接（Email 内置＋WASM 插件） |
| `ocontract/` | 事件与桥接契约类型——**两端共用的语言，改它要最慎重** |
| `src-tauri/` ＋ `src/` | 桌面壳（Tauri v2）＋ Vue 3 前端 |
| `frontends/` | 企业版三角色网页（pnpm workspace） |
| `ocore/default-skills/` | 内置技能图书馆（11 插件 111 技能） |

### 构建与开发循环

```bash
git clone <你的 fork> && cd Operoid
npm install
npm run tauri dev        # 桌面應用（熱重載）
```

⚠️ **改 `oserver` 或 `obridge` 时，两个要一起编**：`cargo dev-build`。服务以 sibling 方式带起 obridge，并互相核对 build id——只编一边会在启动时报不匹配（这是设计，不是 bug；对应排解见[第 18 章](18-troubleshooting.md)）。

- 前端类型检查＋构建：`npm run build`；Rust 全局检查：`cargo check`／`cargo test`。
- CI 的 `check.yml` 会在三平台跑 `cargo check`——推分支前本地先过一遍。

### 上游同步策略

- `main` 跟上游、开发在 `feat/<主題>` 分支——升级时 rebase 你的分支。
- **不修 `ocontract`**：绝大多数接口需求用不着改契约层；真需要（例如新的通道类型）→ 这是该向上游提 PR 的信号，而不是在 fork 里私有扩充。
- 修 bug、通用的治理改进——也建议 PR 回上游，降低长期维护成本。

## 完整示例：接通企业 IM

场景：公司用某企业 IM（Slack／Teams／钉钉／飞书等，**都有 HTTP API**——这正是插件体系的甜蜜点），要让员工收发 IM 消息。

### 架构决定：poll 插件还是 webhook 直投？

| 方案 | 适合 | 做法 |
|---|---|---|
| **A：poll 插件**（推荐起点） | IM 有「列消息」API | 写 WASM 插件，host 每 N 秒调用你的 `poll()`，你用 `make-request` 拉 API |
| **B：webhook 直投** | IM 只推 webhook、或要低延迟 | 写一个小接收器（bun 单文件即可），验签后 `POST /event` 到 Operoid 的进气口 |

两方案员工回复都走同一条路：插件的 `send()` 或你的接收器把 `SendPayload` 送到 IM API。

### 步骤（方案 A）

1. **盘点 IM 的 API**：消息列表端点、发送端点、鉴权方式（bot token？）、能不能用 `page/cursor` 做增量。
2. **创建插件项目**：复制 `obridge-plugin-example/` 为起点，改 `Cargo.toml` 的 crate 名。
3. **实现四个函数**：`source()`（注册键，如 `"acme-im"`）、`init()`（读 `[[channels.acme-im]]` 的 JSON 配置——token、默认群组）、`poll()`（拉新消息→`InboundEvent`，kv 记游标去重）、`send()`（调用发送 API）。
4. **构建与部署**：`rustup target add wasm32-wasip2` → `cargo build --target wasm32-wasip2` → 复制 `.wasm` 到 **obridge 配置文件同目录的 `plugins/`**，文件名惯例 `<name>-<poll_secs>.wasm`。
5. **接线**：`obridge.toml` 加 `[[channels.acme-im]]` 配置块（2 秒内热重载）；员工路由同 email（`brain_id`／`employee_id` 字段或 routes 映射）。
6. **验证**：照 `obridge/README.md` §「全鏈 e2e 手動驗證步驟」——IM 发消息 → `obridge.log` 见命中 → 员工对话出现 In 消息 → 员工回复 → IM 收到。

### 示范提示词一：开发通道插件（复制给 Claude Code）

```text
你是我們的 Rust 開發代理。請為 Operoid 的 obridge 開發一個企業 IM 通道外掛（WASM component）。
工作目錄：Operoid 倉庫根。請嚴格照以下順序進行。

## 第一步：先讀契約，不要憑空想像
依序讀完再寫碼：
1. obridge/wit/channel.wit        —— 通道介面：channel(source/init/poll/send) 與
                                     host(make-request/kv-get/kv-set/clock-now)。
                                     注意：沙箱刻意不提供 TCP/TLS，一切外部存取走 host 的
                                     make-request（HTTP、10 秒 timeout）。
2. obridge-plugin-example/src/lib.rs —— 官方 echo 範例：wit_bindgen::generate! 寫法、
                                     export! 用法、kv 持久化、init 收 JSON 設定的慣例。
3. obridge/README.md              —— §WASM 外掛開發（建置/部署/檔名慣例）、
                                     §設定熱重載、§全鏈 e2e 手動驗證步驟。
4. obridge/config.example.toml    —— [[channels]] 設定寫法。
5. ocontract/src/lib.rs           —— InboundEvent 欄位（source/brain_id/employee_id/
                                     title/content/external_ref/occurred_at/reply_to）。

## 需求
對接 <IM 名稱> 的 HTTP API（API 文件：<連結>，bot token：<如何取得>）：
1. poll()：用 host 的 make-request 呼叫 <訊息列表端點>，把「上次之後的新訊息」轉成
   inbound-event 清單——source="<im>"、title=頻道/群組名、content=訊息本文、
   external_ref=IM 訊息 id、reply_to=可回覆的錨點（頻道+訊息 id）。
   以 host kv 存去重游標（最後看過的訊息 id），確保 obridge 重啟不重發。
2. send(to, employee_id, text)：用 make-request 呼叫 <發送端點>，把 text 送到 to 指定的
   頻道/群組；任何失敗回 Err(含原因的字串)，不得 panic。
3. init(config)：解析 [[channels.<im>]] 區塊（JSON 字串，至少含 token；未設定收到 "{}"）
   ，寫入 host kv；必須冪等（每次實例化都會被呼叫）。
4. 在 obridge/config.example.toml 加上 <im> 的設定範例（含註解）。

## 硬性限制
- 只能使用 host 能力（make-request/kv/clock-now）；沒有檔案、沒有 TCP/TLS。
- 不得修改 obridge/wit/channel.wit 與 ocontract/——介面以現契約為準。
- 專案結構與 wit_bindgen 用法比照 obridge-plugin-example；新 crate 名 <obridge-plugin-<im>>。

## 建置與測試
- rustup target add wasm32-wasip2
- cargo build -p <crate> --target wasm32-wasip2
- 先跑通官方往返測試：cargo test -p obridge example_plugin -- --ignored --nocapture
- 為新外掛寫單元測試：設定解析（缺 token 應報可讀錯誤）、事件欄位映射、游標去重、
  send 的失敗路徑。

## 驗收條件（全部達成才算完成）
1. .wasm 建置成功，複製到 plugins/<im>-<poll_secs>.wasm 後，obridge 啟動 log 認得此通道。
2. 從 IM 發訊息 → obridge.log 見 poll 命中 → Operoid 對應員工對話出現 In 訊息。
3. 員工回覆 → IM 群組收到訊息。
4. 重啟 obridge 兩次，不重發舊訊息。
5. cargo test 全綠。
```

### 示范提示词二：webhook 直投接收器（方案 B）

```text
你是我們的開發代理。請寫一個「IM webhook → Operoid 進氣口」的小型接收服務。

先讀：
1. obridge/README.md §全鏈 e2e 手動驗證步驟（event_ingress / event_outbound 契約）。
2. ocontract/src/lib.rs 的 InboundEvent（POST /event 的 body 欄位以此為準，snake_case）。
3. 本專案指南 guide/14-email-bridge.md 的「讓員工收發 email：四步」。

需求：
1. bun 單檔（比照 deploy/local/mm-bridge/ 的風格），監聽 http://127.0.0.1:18080/webhook。
2. 驗證 <IM> 的 webhook 簽章（<驗簽方式>），把每則訊息轉成一筆事件：
   POST http://127.0.0.1:<event_ingress_port>/event
   Authorization: Bearer <event_ingress_secret>
   body：{ "kind": ..., "source": "<im>", "title": ..., "content": ...,
           "external_ref": ..., "reply_to": ... }   （欄位名照 InboundEvent）
3. Operoid 不可達時暫存並重試（指數退避，上限 <N> 次）；提供 /healthz。
4. 列出我需要在 app-settings.json 的 app_config 設定的鍵
   （event_ingress_port、event_ingress_secret）與員工回覆外發需要的
   event_outbound_url/secret 對應關係。
驗收：IM 發訊息 → 員工被喚醒（對話出現 In 訊息）；停掉 Operoid 30 秒再恢復，期間的
訊息補投成功、順序正確。
```

### 示范提示词三：fork 后加一个 API 端点

```text
你是我們的 Rust 開發代理。請在 Operoid 企業服務新增一個內部報表 API 端點：
GET /api/admin/reports/employee-weekly（僅 admin）。

先讀：
1. oserver/src/routes.rs —— 路由怎麼註冊、回應格式慣例。
2. oserver/src/rbac.rs —— 端點級 RBAC 矩陣怎麼登記（admin 角色）。
3. oserver/src/writes.rs 或相近模組 —— 現有 handler 的錯誤處理慣例。

需求：
1. 回傳近 7 天每位員工：喚醒次數、完成承諾數、待核可提案數（資料來源：SQLite，
   先讀 oserver/src 的 DB 存取慣例）。
2. 遵守現有錯誤/認證慣例；加整合測試（含無 token 401、非 admin 403）。
3. cargo check -p oserver 與 cargo test -p oserver 全綠。
注意：這是 fork 內的私有端點——不要動 ocontract/ 與前端合約；若未來想回饋上游，
先開 issue 討論。
```

## 安全注意

- **沙箱即边界**：插件拿不到 TCP/TLS/文件——只能经 host 的三个能力。这是设计：接口需求应推着你用 IM 的 HTTP API，而不是绕过沙箱。
- **凭证落点**：IM token 写 `obridge.toml`（插件经 init 收到）；Operoid 进出密钥写 `app-settings.json`／`operoid.toml`。皆为本地文件——把配置文件纳入公司的备份与权限管理，不要提交进 git。
- **对外通道先画线**：IM／Email 是员工触及外部世界的通道。上线前用[动作类别登记表](13-governance.md)决定「员工自动回复外部」属哪一层——拿不准就留在人裁决层。
- **日志**：开发期盯 `obridge.log`；企业部署加看服务日志（`oserver` 的 stderr／平台日志）。

---
[← 附录 A](appendix-a-gbrain.md)｜[返回索引](README.md)
