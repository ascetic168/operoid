# Operoid

[English](README.md) | [繁體中文](README.zh-TW.md) | **简体中文**

> Operoid 不是一个聊天应用程序。它是一套 **AI Agent 操作系统** —— 一个作业环境，
> 让 AI 代理（称为 **Employee 员工**）在一个共享、持久的工作空间（**Workspace**）里
> 持续完成有意义的工作。

多数 AI 产品以对话为中心：你问、它答，窗口一关工作就消失了。但真正的工作不是这样。
采购助理会追踪一张订单长达数周；质量工程师会从异常报告、到纠正措施、再到结案一路追踪。
这些职责需要一个能**持久、能记忆、并在窗口关闭后仍继续运行**的环境。

Operoid 的存在，就是为了成为那个环境。

以 **Tauri v2（Rust）** + **Vue 3 + TypeScript** 打造。
**作者：** 朱國棟 (Charlie Chu) · **授权：** [MIT](#授权) · **状态：** 见[当前状态](#当前状态)

---

## 为什么需要 Operoid？

今天的 AI，行为更像一个**顾问**，而不是一个**员工**。顾问给完建议就离开；
员工加入组织、承担成果、并持续负责。Operoid 是为后者而打造的。

今天的 AI 系统普遍缺乏：

- **持久的职责** —— 对话结束，工作就消失。
- **长期的承诺** —— 没有"跟踪此事直到完成"的概念。
- **共享的工作空间** —— 没有一个地方让多个代理与人类就相同事物协作。
- **组织知识** —— 模型所知，并不等于组织所知。
- **企业角色** —— 代理没有身份、职权或问责。
- **委任界线** —— 没有一条有原则的答案，能回答"机器不必请示就能做什么"。
- **持续的执行** —— 没有东西会在相关事件发生时把代理唤醒。

Operoid 把 AI 视为**组织成员，而不是聊天机器人。**

## 什么是"AI Agent 操作系统"？

传统操作系统管理进程、内存、文件与设备，让程序得以运行 —— 它提供环境，不做程序的工作。
Operoid 对 AI 代理做同样的事：

| 操作系统概念 | 在 Operoid 里 |
|---|---|
| **进程** | **Employee 员工** —— 被调度、运行与挂起的代理 |
| **文件** | **Artifact 成果物** —— 由工作空间拥有、而非由对话拥有的持久产出 |
| **内存** | **工作记忆与知识** —— 按需恢复，而非长期驻留 |
| **设备** | **Tool 工具** —— 通过受控接口调用的外部能力 |
| **内核（kernel）** | **Runtime 执行引擎** —— 唤醒 Employee、恢复其上下文、让它运行、再让它休眠 |

Runtime 管理**执行**，从不管理**思考** —— Employee 想什么，是它自己的事。
这就是为什么 Operoid 被称为操作系统，而不是应用程序。

## 人类画得出的委任界线

"该不该让 AI 决策？"其实不是一个问题，而是三个。**能力提问**——系统能否正确决策？
答案随每一次模型更新而变动。**问责提问**——决策失准时，责任落在谁身上？
**正当性提问**——这一类决策，是否本来就应由人类作出？只有第一问的答案会随技术移动。
一套委任政策要成立，必须三问全过——而模型越强，同样的规格残差造成的损害规模越大，
把界线留在人类手里的理由反而**更充分**，不是更少。

Operoid 把这件事当成宪法层级的原则——
**[手册原则 11"界线由人类画定"](handbook/Chinese/02-Design-Philosophy.md)**——并将它操作化：

- **委任的单位是动作类别**，不是"系统"。每一类动作依三问评定——可逆性、爆炸半径、
  具名问责——分派到三个层级：**人裁决层**、**围栏自动层**、**有主自动层**。
- **从严预设**。人类未登记的类别一律送人类裁决；放宽须提出证据并经签署，收紧永远廉价。
  员工不得参与决定自己的权限范围；奖励界线扩张的指标（如"自动处理率"）不得作为成效指标。
- **界线保持鲜活、可稽核**。授权会届期、须重签；事故自动把相关类别冻结回人类裁决；
  机器的归类由人类盲抽复核，分歧超标即收缩界线。主权归属的检验是**文件可重建性**：
  若仅凭人类自有文件即可重建界线的现行位置，界线属于人类；若必须检视系统自身的轨迹
  才知道界线在哪，界线已经易主。

这些机制不是从理论发明的，而是从一个真实（已去识别化）的制造业个案蒸馏而来：
一个自主代理自行判断"何者属常规"，把每一次界线漂移都记为成就。
完整分析见论文**[界线的形状](journal/界线的形状_朱国栋.pdf)**；其机制以**动作类别登记表**的形式
随产品交付——见[当前状态](#当前状态)。

## 服务器执行的知识边界

动作类别登记表画的是**动作**的边界——Employee 可以*做*什么；知识布料（Enterprise C′）
画的是**知识**的边界——Employee 可以*知道*什么。两条边界有同一种宪法形状：
由人类画定、由服务器确定性执行、事后可稽核。

- **授权发生在检索之前。** 政策以纯 Rust 确定性评估——LLM 可以协助理解查询，
  但永不参与授权。未授权的知识不会进入候选集，更不会进 LLM 的上下文；
  没有任何「先取回再遮蔽」。
- **一块布料，按范围分区。** 全公司／部门／项目／受限知识映射到 GBrain 的
  *source*；每次查询只对**授权 source 集合**发出，由 GBrain 在 SQL 层强制——
  零修改 GBrain 本体。个人脑维持物理隔离。
- **Fail closed。** 政策缺失或损坏＝拒绝并记录事件；明示 DENY 规则高于一切，
  连临时授权都无法架空。
- **身份由服务器端裁定。** 每个 principal（人类操作者或 AI 员工）以自己的
  API token 认证；身份出自 token 链，永不出自调用端的自称。
- **特权行为皆留收据。** 每次检索写下结构化纪录——谁、在哪个任务、依哪版政策、
  看到哪些范围。grant 的发放／撤销／届期、政策变更、身份变更都是稽核事件。
  临时授权依 TTL 到期，撤销即时失效。
- **保密等级硬天花板。** 范围带保密等级（`public < internal < confidential < secret`），
  principal 带 clearance。超过自己 clearance 的范围一律拒绝——任何规则、
  任何临时授权都打不开。同一个天花板也适用**写入端**：作者不能把文章发进
  比自己 clearance 更高的来源；工厂产出在写入当下就归入你选的范围。
- **同一脑＋同一查询＋不同身份＝不同结果。** 这句话已对真实 GBrain 做了
  端到端实证——它是知识布料真的在运作的最小证明。

一切在「**设置 → 知识授权**」管理：principals 与 API tokens、临时授权、
范围→source 映射。随附两个企业面开关（`ops_retrieval_enabled` 自控制台退役检索 op；
`claude_code_handoff_enabled` 退役 Claude Code handoff）。

## 核心概念

| 概念 | 一句话角色 |
|---|---|
| **Workspace 工作空间** | 组织。一切事物都隶属于恰好一个。 |
| **Employee 员工** | 工作者。承担责任的 AI 代理。 |
| **Brain 大脑** | 智能。可复用、可版本化的知识与人格。 |
| **Artifact 成果物** | 成果。工作的产出，归工作空间所有。 |
| **Knowledge 知识库** | 组织经策展且持久的记忆。 |
| **Tool 工具** | Employee 可调用的外部能力。它永远不做决策。 |
| **Project 项目** | 为某个目标而成立的有限度协作。 |
| **Task 任务** | 工作单位。短期、可执行。 |
| **Commitment 长期职责** | 比任务活得更久的持久职责。 |
| **Action Registry 动作类别登记表** | 委任界线。哪些动作类别可免请示而行——由人类画定、可稽核。 |
| **Knowledge Policy 知识政策** | 知识边界。谁可检索哪个范围——检索前评估、fail closed。 |
| **Trigger 触发器** | 决定何时该唤醒 Employee。 |
| **Runtime 执行引擎** | 管理生命周期的引擎，从不管理思考。 |
| **Event 事件** | 已发生事实的不可变记录。 |
| **Memory 工作记忆** | Employee 的工作上下文，每次唤醒时重新恢复。 |

完整的定义 —— 目的、职责、各自拥有什么、生命周期与未来扩展 —— 见
**[架构手册](handbook/Chinese/README.md)**，它是这套操作系统的宪法。

## 当前状态

架构手册为 **v0.2（草稿）**，而路线图的里程碑已**一路实现至 Phase 7** ——
手册中的愿景现在是一个运行中的系统，而不只是草稿。

**v0.3.6 —— 常驻服务、多前端。** 后端如今以 **`oserver`** 常驻服务的形态运行
（127.0.0.1 的 HTTP API），由它拥有 Runtime：**无论窗口开关，Employee 持续工作**。
桌面 app 成为「诸多前端之一」——任何能说 HTTP 的程序都能驱动同一个后端。

当前版本实现完整的 Agent-OS 技术栈：

- **常驻服务架构**：`ocore`（纯 Rust 核心：Runtime、调度器、事件总线、GBrain 能力域）
  ＋ `oserver`（axum HTTP API，token 认证）＋ **Tauri v2 桌面壳**（前端之一）。
- **服务生命周期双语意**：安装开机服务（Employee 从开机就运行；关闭 app 毫无影响）
  或不安装（服务随 app 启停）。Windows 已实现并实机验证；Linux（systemd）与
  macOS（launchd）已实现、尚未实机验证。
- 一个以 [GBrain](https://github.com/garrytan/gbrain) 为基础的**知识图谱层** ——
  把日常文件（联系人 CSV、会议 PDF、公司介绍）变成互连、可查询的笔记；
  通过 GUI 而非命令行来同步、提问与推理。
- 完整的 **Agent-OS 执行引擎**（Phase 1–7）：由 Trigger 驱动的 Employee 生命周期引擎；
  持久化于 SQLite 的 **Artifact 成果物**与 **Commitment 承诺**；
  **模板 → 实例**（定义一次、部署多次）；**共享 Brain** 让一次升级惠及所有 Employee；
  **团队、项目与任务交接**实现多 Employee 协作；以及**对话层** —— 人机聊天、
  消息驱动唤醒、实时观察面板。
- **可拦截的生命周期＋恢复力**：执行中的 Employee 可随时**合作式停止**与**封存**
  （历史完整保留、可追溯可解封）；出错的承诺以指数退避**自动重试**，
  连续失败达上限后停止并转人工处理。
- **人类画得出的委任界线**（v0.3.4–v0.3.5）：**动作类别登记表**——以动作类别为单位的
  三层委任、从严预设，附关键词保险丝、届期重签、事故连坐冻结、抽审分歧警报；
  在「设置 → 登记表」编辑（结构化表单；进阶可切原始 JSON）。
  界线的"为什么"与"形状"：[人类画得出的委任界线](#人类画得出的委任界线)。
- **服务器执行的知识边界**（v0.3.7）：知识图谱上的**授权检索**——范围分区、
  检索前授权、确定性 fail-closed 政策、per-principal token、临时授权（TTL）、
  检索收据、任务聚焦，以及「设置 → 知识授权」管理页。
  见[服务器执行的知识边界](#服务器执行的知识边界)。
- **write-note——员工的第一个行动工具**：Employee 把完整产出写成 markdown 笔记到
  专属产出目录（在知识库之外）；人工审阅后将满意的笔记移入 notes repo，
  经既有同步晋升为组织知识（产出 → 验证 → 晋升）。可依模板逐一开启权限。
- **Email 进出**：内置的 [obridge](obridge/) 桥接器（收信经事件进气口唤醒对应
  Employee；Employee 通过 send 工具回信）。IM 类通道走 WASM 插件。
- 第一个**代理入口**：在工作空间内启动并监视 [Claude Code](https://claude.com/claude-code)。

> ℹ️ GBrain 知识图谱功能是当前版本的*知识*层。
> 手册中定义的 Employee / Runtime / Commitment 架构如今已实现并运行中。

## 技术栈

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**核心与服务：** Rust —— `ocore`（领域核心）· `oserver`（axum 服务）· `obridge`（Email/WASM 桥接）
**桌面壳：** Tauri v2（窗口＋桌面专属能力；所有逻辑都在服务里）

## 前置需求

要使用当前的知识图谱功能，桌面应用需要：

| 工具 | 用途 | 安装 |
|---|---|---|
| **git** | sync 流程会在更新图谱前先 commit | <https://git-scm.com/downloads> |
| **bun** | `gbrain` 通过 bun 安装与运行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 知识图谱引擎 | <https://github.com/garrytan/gbrain> |

路径会自动检测（Windows 为 `~/.bun/bin/gbrain.exe`）；必要时可在"配置"页覆盖。

## 安装与运行

**一般用户 —— 直接下载预编译安装包即可。** 到
[**Releases** 页面](https://github.com/ascetic168/Operoid/releases)下载对应平台的最新版本并运行。
除非你要开发 Operoid，否则不需要 `git clone` 或从源码构建。

### 个人版与企业版（v0.4.0）

| | 个人版 | 企业版 |
|---|---|---|
| 资产 | 桌面安装文件 | `operoid-enterprise-*` 部署包 |
| 运行位置 | 用户自己的电脑 | 公司内网服务器 |
| 设置 | 无——安装即用 | `operoid.toml`（bind＋TLS）＋`oserver install` |
| 前端 | 桌面 app | 浏览器：`/admin` `/manager` `/user` |
| 认证 | 隐藏的本机 token | 密码登录＋RBAC（admin／manager／user） |
| 邮件桥接（obridge） | 内置，由桌面 app 托管 | 内置；由 oserver 托管——`/admin` 表单设置（或部署在另一台机器） |

桌面安装文件**就是**个人版。企业部署从同一个 Release 下载 `operoid-enterprise-*`
部署包，照 **[DEPLOYMENT.md](DEPLOYMENT.md)** 操作——服务器机不需要 Node/pnpm。
两个注意事项：**不要**在企业服务器上运行桌面 GUI（它使用的是个人模式的凭据）；
两个版本可在同一内网安全并存（个人版仅绑定 loopback）。

### 安装开机服务（Linux / macOS）

Linux 与 macOS 的开机服务在**任何用户登录前**就会启动
（Linux：`/etc/systemd/system` 的 systemd system unit；
macOS：`/Library/LaunchDaemons` 的 launchd LaunchDaemon）。安装注意事项：

- 请**从你自己的账号通过 `sudo` 安装**（例如 `sudo oserver install`）。
  提权只用于写入系统位置的 unit/plist 文件；服务本身以**安装者的身份**运行
  （`User=`／`UserName`），因此 SQLite DB、`app-settings.json`、
  gbrain／obridge 文件的所有者与桌面 app 一致——不会和 GUI 起权限冲突。
- 若安装程序无法判定安装用户（例如直接在 root shell 执行），会直接
  报错拒绝——请改从你的账号以 `sudo` 安装。
- 服务的 `HOME` 会明确指向你的 home（Linux unit 内设置），
  bun／gbrain 的惯例路径因此不会落到 `/root`。
- 移除服务：`sudo oserver uninstall`。
- Linux／macOS 的服务路径已实现但**尚未实机验证**（Windows 已验证）。

### 开发者（从源码构建）

构建桌面应用需要 **Rust 工具链**与 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/ascetic168/Operoid.git
cd Operoid
npm install          # 安装依赖
npm run tauri dev    # 运行应用（热重载）
npm run tauri build  # 构建分发用安装包
```

仅前端（于 http://localhost:1420 在浏览器运行）：`npm run dev`、`npm run build`。

直接开发 `oserver`／`obridge` 时，请用 `cargo dev-build` **两个一起编译**
（或 `scripts/dev-rebuild.ps1`——会先停掉 dev 进程再编译）：oserver 以 sibling
方式拉起同目录的 `obridge`，只编译单一 crate 就会跑到另一个的旧文件。两个可执行文件
都内嵌 build id（git hash）并在拉起时互相核对——不一致会在启动 log 大声警告，
并显示在 admin 界面（`/api/obridge/status` 的 `exe_build_match`）。

## 开发

```bash
npm run tauri dev             # 完整应用，热重载
npm run build                 # 前端类型检查 + 构建
cargo test                    # Rust 单元测试（整个 workspace：ocore、oserver…）
cargo check                   # 后端快速类型检查（整个 workspace）
```

## 项目结构

```
src/              Vue 3 前端（views、Pinia stores、i18n、HTTP 包装）
                  —— Brains、Factories、Config、员工模板／实例、
                    员工对话、Operations（实时控制台）、收件箱
ocore/            Rust 领域核心（零 Tauri 依赖）
                    domain · runtime · scheduler · event_bus · agent 状态
                    知识授权（policy/service/planner/grants/receipts/identity）
                    GBrain 能力域（cli/brains/factories/converters）· llm
oserver/          常驻服务 —— axum HTTP API（token 认证）
                    agent-os 读写面 · GBrain 全域 · 操作控制台（ring buffer 轮询）
                    知识授权管理（principals/tokens/grants）
                    事件进气口 /event · 服务注册（Windows/Linux/macOS）
src-tauri/        桌面壳（Tauri v2）—— 窗口＋桌面专属功能
                    （Claude Code、笔记预览）、指令薄层、服务托管
obridge/          Email 桥接器＋WASM 插件宿主（IMAP 收／SMTP 发）
ocontract/        共享契约类型（Operoid ↔ obridge）
handbook/         架构手册 —— 宪法（英文 + 中文）
```

## 路线图

路线图勾勒于手册中，按依赖关系排序。五个里程碑皆已**实现至 Phase 7**：

1. ✅ **一个真正能工作的 Employee** —— 因 Trigger 唤醒、恢复上下文、调用工具、提交成果物、休眠。
2. ✅ **持久化与 Commitment** —— 工作能扛过完全关机与重启（SQLite）。
3. ✅ **共享的 Brain 与知识** —— 升级一个 Brain，多个 Employee 同步采用。
4. ✅ **模板与实例** —— 一个模板，多个独立员工。
5. ✅ **协作** —— 一群 Employee 合作完成一个 Project（团队 + 项目 + 任务交接）。

Phase 7 新增了**人机协作层**：交办承诺给人类、Message 概念、聊天对话与错误韧性。
完整说明见[第二十一章 — 路线图](handbook/Chinese/21-Roadmap.md)。

## 授权

本项目以 **[MIT 授权](LICENSE)** 发布。
Copyright © 2026 朱國棟 (Charlie Chu)。完整条文见 [LICENSE](LICENSE)。
