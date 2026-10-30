# 配置参考

> 适用版本：v0.5.11。本章是所有配置文件与环境变量的速查表；企业版服务器配置的逐步说明以 [DEPLOYMENT.md §3](../../DEPLOYMENT.md) 为权威。

## 配置文件总览

| 文件 | 位置 | 格式 | 谁用 |
|---|---|---|---|
| `app-settings.json` | 配置目录（见下） | JSON | 个人版＋企业版共用的基底设置（`{"app_config": {...}}`） |
| `operoid.toml` | 同配置目录 | TOML | **存在＝企业模式**（个人模式无此文件、行为零变化） |
| `obridge.toml` | 你指定（`obridge_config_path`） | TOML | obridge 通道配置；GUI 可当编辑器直接改 |
| `config.json` | 每颗脑的 `.gbrain` 旁 | JSON | GBrain 的模型设置——「设置 → 模型」的权威来源 |

**配置目录**：Windows `%APPDATA%\com.operoid.studio`；macOS `~/Library/Application Support/com.operoid.studio`；Linux `~/.config/com.operoid.studio`。

## app-settings.json 常用字段

以下为 `app_config` 区块中与日常使用相关的字段（完整结构见 `ocore/src/app_config.rs`）：

| 字段 | 默认 | 说明 |
|---|---|---|
| `agent_os_enabled` | `false` | 员工子系统总开关（员工模板／实体） |
| `brains` ＋ `active_brain_id` ＋ `active_source_id` | — | 脑注册与作用中对象（GUI「脑」页管理） |
| `gbrain_exe_path` | `~/.bun/bin/gbrain.exe` | gbrain 可执行文件路径 |
| `gbrain_transport` | `"mcp"` | 员工知识查询传输：`mcp`（失败自动退 CLI）或 `cli` |
| `default_embedding_model` ＋ `_dimensions` | 内置 768 维 | **新建脑**的 embedding 初始值 |
| `employee_output_path` | `~/employee-output` | 员工「写入笔记」的沙箱根目录 |
| `auto_sync` ＋ `sync_no_pull` | `true` ＋ `true` | 工厂写文件后自动 commit+sync；sync 加 `--no-pull` |
| `llm_temperature` ／ `llm_max_tokens` | 0.2 ／ 4096 | LLM 结构化采样 |
| `turn_max_steps` | 40 | 对话回合工具循环保险丝 |
| `autonomy_max_cycles` ／ `autonomy_max_minutes` | 25 ／ 15 | 自主工作预算 |
| `subagent_max_steps` | 20 | 子代理步数保险丝 |
| `skills_personal_dir` | `~/.agents/skills` | 技能个人区；**空字符串＝企业模式**（逐主体隔离） |
| `skills_union_limit` | 30 | 技能加载并集上限（`0`=不设限） |
| `factory_concurrency` | 4 | 同时转换的文件数 |
| `event_ingress_port` ／ `event_ingress_secret` | 未设 | 外部事件进气口（`POST /event`）——**默认关闭**；设了 port 没设 secret 则不启动 |
| `event_outbound_url` ／ `event_outbound_secret` | 未设 | 员工回复外发端点（未设＝回复仅留在 Operoid） |
| `obridge_config_path` ／ `obridge_executable` ／ `obridge_autostart` | 未设 | Obridge 三件套（见[第 14 章](14-email-bridge.md)） |
| `server_autostart` ／ `server_port` ／ `server_token` | `true` ／ 7340 ／ 自动生成 | 本地服务 |
| `llm_env` | `{}` | 服务用的 provider key 快照（服务账户看不到用户环境变量） |
| `update_notify` ／ `auto_update` | `true` ／ `false` | 更新提示／自动更新 |
| `allow_public_egress` | `false` | public 第三方云（mineru.net 等）egress 总闸 |
| `mineru_command` ／ `mineru_dir` ／ `mineru_tier` | 未设 | MinerU 取用阶梯（支持 `%VAR%`／`${VAR}` 展开） |
| `claude_terminal` 等 | — | Claude Code 集成偏好 |

## operoid.toml（企业模式）

放 `<数据目录>/operoid.toml`（**存在即企业模式**）；示例文件 `operoid.example.toml` 随部署包附上：

| 区段 | 内容 |
|---|---|
| `[server]` | host、port（7340）、master token、frontends_dir——**非 loopback ⇒ `[tls]` 必填** |
| `[tls]` | cert ＋ key（PEM）；只配一半会被拒绝启动（fail closed） |
| `[llm.env]` | provider API keys（服务进程看不到用户环境） |
| `[gbrain]` | `exe_path` 覆写（可省） |
| `[knowledge]` | MinerU 阶梯＋egress 分层（可省） |
| `[ingress]` | obridge 投递口 port／secret（可省） |
| `[factory]` | 转换并发上限，默认 4；饱和时每位远程用户至多 1 线程（可省） |
| `[update]` | `check_enabled = false` 可关闭新版检查（气隔环境；可省） |

**优先级**：`OSERVER_*` 环境变量 > `operoid.toml` > `app-settings.json`。**热生效**：`[llm] env`／`[gbrain]`／`[ingress]`／`[factory]`；**须重启**：host／port／`[tls]`／frontends_dir。

## obridge.toml

```toml
[operoid]            # 进气目的地：Operoid 的 /event 与共用密钥
ingress_url = "http://127.0.0.1:17341/event"
ingress_secret = "<密码A>"

[listen]             # obridge 的外发接收端（员工回复从这里出去）
port = 17401
secret = "<密码B>"

[[channels.email_imap]]   # Email 通道（可多个）
# IMAP/SMTP 账号、轮询间隔
# routes：收件地址 → employee_id（员工绑定「对外 email」后自动同步）
# senders：employee_id → From 地址

[[channels.<plugin>]]     # WASM 插件通道（见附录 B）
```

`[[channels]]` 与插件配置**热重载**（每 2 秒检查）；`[listen]`／`[operoid]` 变更需重启。

## GBrain config.json 与 DB plane

每颗脑的模型设置：`chat_model`（`provider:model`）、`models` 分级、`provider_base_urls`、`embedding_model`。「设置 → 模型」的控件会同步写入 file plane（config.json）与 DB plane（gbrain 数据库）——**runtime 以 file plane 为优先**；直接手改 config.json 可能被 DB plane 覆写遮蔽，改模型请走 GUI，「清除所有 DB 覆写」可归零。

## 环境变量

| 变量 | 作用 |
|---|---|
| `OSERVER_TOKEN` ／ `OSERVER_PORT` | 覆盖 master token／监听 port（优先于 operoid.toml） |
| `GBRAIN_HOME` | 脑的主目录（默认 `~/.gbrain`）；GUI「脑」页可逐脑覆写 |
| `ZHIPUAI_API_KEY`、`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`、`DEEPSEEK_API_KEY`、`GROQ_API_KEY`、`TOGETHER_API_KEY`、`OPENROUTER_API_KEY`、`DASHSCOPE_API_KEY`、`ZEROENTROPY_API_KEY` | 各 provider 的 API key（ollama 不需 key） |

## 数据与日志位置

| 内容 | 位置 |
|---|---|
| 脑（页面、图谱、向量） | `GBRAIN_HOME`（默认 `~/.gbrain`）下的 PGLite＋来源 git repo |
| 员工／模板／承诺／提案 DB（SQLite）＋ `skills/` 树 | 数据目录（Windows：本机 AppData `com.operoid.studio`） |
| obridge 日志 | 数据目录下的 `obridge.log` |
| 员工输出 | `employee_output_path`（默认 `~/employee-output`） |
| 工厂转换审计 | 来源 repo 内的 `conversion-report.json` |

卸载应用程序不会动到以上任何数据。

---
[← 16 企业版三种角色界面](16-enterprise-consoles.md)｜[索引](README.md)｜[18 疑难解答 →](18-troubleshooting.md)
