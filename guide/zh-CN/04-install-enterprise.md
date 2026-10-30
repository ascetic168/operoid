# 安装企业版

> 适用版本：v0.5.11。本章是企业版部署的**总览与名词对照**；完整逐步流程以 [DEPLOYMENT.md](../../DEPLOYMENT.md) 为权威（该文件随部署包附上，内容同仓库）。

## 部署包内容

从 [GitHub Releases](https://github.com/ascetic168/Operoid/releases) 下载对应平台的部署包，解压到服务器主机即可——**服务器不需要 Node/pnpm**：

| 平台 | 文件 |
|---|---|
| Windows x64 | `operoid-enterprise-windows-x64.zip` |
| Linux x64 | `operoid-enterprise-linux-x64.tar.gz` |

包内容：`oserver` 可执行文件（HTTP 服务＋员工执行引擎）、`obridge` 可执行文件（Email／IM 桥接）、三个角色前端静态文件（`frontends/{admin,manager,user}`）、`DEPLOYMENT.md`、`operoid.example.toml`、`deploy/local/mm-bridge/`（多模态嵌入转译层，选配）。

## 服务器主机前置需求

| 项目 | 说明 |
|---|---|
| GBrain | 安装于服务器主机（`gbrain` CLI）；脑（GBRAIN_HOME）与 notes repo 同机 |
| 嵌入服务 | 本机 llama.cpp `llama-server`（port 8080，EmbeddingGemma 2，768 维）——GBrain 嵌入／查询**硬依赖**，须常驻 |
| TLS 证书 | 内网 CA 签发（首选）或自签；非 loopback 监听**必须**配置 TLS，半配置（只有 cert 或只有 key）会被拒绝启动（fail closed） |
| LLM API keys | 写在 `operoid.toml` 的 `[llm] env`——服务账户看不到用户环境变量 |
| 多模态（选配） | 图片嵌入需部署 `deploy/local/mm-bridge/` 转译层（bun 单文件，8090 → 8080） |

## 部署设置向导：`oserver configure`

```bash
./oserver configure --data-dir /opt/operoid/data        # Linux
.\oserver.exe configure --data-dir C:\operoid           # Windows
```

向导依序进行（Enter 采用默认值，`?` 显示说明，写入前有摘要确认关卡）：

1. **前置检查**——探测 git／bun／gbrain，缺项给安装指引。
2. **部署模式**——个人（本机 loopback）或企业（内网服务）。
3. **监听与前端**——bind host/port、frontends 目录、TLS cert/key。
4. **知识脑工作目录**——gbrain 路径与 `gbrain_home`；可现场 `gbrain init` 建新脑。
5. **master token**——现场生成 64 个字符（仅此一次完整显示）。
6. **管理员账号**——默认 `admin`／`admin1234`，首次登录**强制**改密码（≥8 位）。
7. **系统服务**——可选择当场注册开机自启。

⚠️ 注册服务前务必先完成向导——服务在无终端环境首次运行时不会自己弹出向导，会直接启动失败。

## 服务注册

```bash
./oserver install      # 注册开机服务（Windows 为 SCM 服务「Operoid」，需系统管理员）
./oserver status       # {"installed": true, "running": true}
./oserver uninstall    # 移除
```

Windows 为已实机验证路径；Linux（systemd）／macOS（launchd）已实现，安装注意事项见 [DEPLOYMENT.md §11.3](../../DEPLOYMENT.md)。防火墙需开放 7340 port（指令见 §5）。

## 首次登录

1. 浏览器打开 `https://<服务器IP>:7340/`——三个前端的入口页。
2. 用向导创建的管理员账号登录 `/admin`；首次登录系统强制设置新密码。
3. 在「账号管理」创建 manager／user 账号；各角色的日常操作见[第 16 章](16-enterprise-consoles.md)。

## 升级

企业版**不自动更新**：admin 界面「版本」卡片可检查新版（结果缓存 24 小时；气隙环境可以 `operoid.toml` 的 `[update] check_enabled = false` 关闭对外连接）。升级是 zip 覆盖安装：停服务 → **备份数据目录** → 解压新版覆盖 → 起服务 → 核对 `/healthz` 的 version。完整流程与注意事项见 [DEPLOYMENT.md §12](../../DEPLOYMENT.md)。

## DEPLOYMENT.md 章节对照

| 想做什么 | 查 |
|---|---|
| 架构总览 | §1 |
| 服务器前置、嵌入服务启动指令 | §2、§10.1 |
| `operoid.toml` 每个字段 | §3（另见本指南[第 17 章](17-config-reference.md)） |
| TLS 证书 | §4 |
| 服务注册、防火墙 | §5 |
| 账号开通（含不用向导的手动流程） | §6 |
| obridge 邮件桥接 | §7（本指南[第 14 章](14-email-bridge.md)） |
| 部署后验收清单 | §8 |
| 部署疑难解答 | §9（本指南[第 18 章](18-troubleshooting.md)） |
| PDF 知识管线、MinerU | §10 |
| 升级 | §12 |

---
[← 03 安装个人版](03-install-personal.md)｜[返回索引](README.md)｜[05 快速上手 →](05-quickstart.md)
