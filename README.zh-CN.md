# Emploid

[English](README.md) | [繁體中文](README.zh-TW.md) | **简体中文**

> Emploid 不是一个聊天应用程序。它是一套 **AI Agent 操作系统** —— 一个作业环境，
> 让 AI 代理（称为 **Employee 员工**）在一个共享、持久的工作空间（**Workspace**）里
> 持续完成有意义的工作。

多数 AI 产品以对话为中心：你问、它答，窗口一关工作就消失了。但真正的工作不是这样。
采购助理会追踪一张订单长达数周；质量工程师会从异常报告、到纠正措施、再到结案一路追踪。
这些职责需要一个能**持久、能记忆、并在窗口关闭后仍继续运行**的环境。

Emploid 的存在，就是为了成为那个环境。

以 **Tauri v2（Rust）** + **Vue 3 + TypeScript** 打造。
**作者：** 朱國棟 (Charlie Chu) · **授权：** [MIT](#授权) · **状态：** v0.2.0 —— 见[当前状态](#当前状态)

---

## 为什么需要 Emploid？

今天的 AI，行为更像一个**顾问**，而不是一个**员工**。顾问给完建议就离开；
员工加入组织、承担成果、并持续负责。Emploid 是为后者而打造的。

今天的 AI 系统普遍缺乏：

- **持久的职责** —— 对话结束，工作就消失。
- **长期的承诺** —— 没有"跟踪此事直到完成"的概念。
- **共享的工作空间** —— 没有一个地方让多个代理与人类就相同事物协作。
- **组织知识** —— 模型所知，并不等于组织所知。
- **企业角色** —— 代理没有身份、职权或问责。
- **持续的执行** —— 没有东西会在相关事件发生时把代理唤醒。

Emploid 把 AI 视为**组织成员，而不是聊天机器人。**

## 什么是"AI Agent 操作系统"？

传统操作系统管理进程、内存、文件与设备，让程序得以运行 —— 它提供环境，不做程序的工作。
Emploid 对 AI 代理做同样的事：

| 操作系统概念 | 在 Emploid 里 |
|---|---|
| **进程** | **Employee 员工** —— 被调度、运行与挂起的代理 |
| **文件** | **Artifact 成果物** —— 由工作空间拥有、而非由对话拥有的持久产出 |
| **内存** | **工作记忆与知识** —— 按需恢复，而非长期驻留 |
| **设备** | **Tool 工具** —— 通过受控接口调用的外部能力 |
| **内核（kernel）** | **Runtime 执行引擎** —— 唤醒 Employee、恢复其上下文、让它运行、再让它休眠 |

Runtime 管理**执行**，从不管理**思考** —— Employee 想什么，是它自己的事。
这就是为什么 Emploid 被称为操作系统，而不是应用程序。

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
| **Trigger 触发器** | 决定何时该唤醒 Employee。 |
| **Runtime 执行引擎** | 管理生命周期的引擎，从不管理思考。 |
| **Event 事件** | 已发生事实的不可变记录。 |
| **Memory 工作记忆** | Employee 的工作上下文，每次唤醒时重新恢复。 |

完整的定义 —— 目的、职责、各自拥有什么、生命周期与未来扩展 —— 见
**[架构手册](handbook/Chinese/README.md)**，它是这套操作系统的宪法。

## 当前状态

架构手册为 **v0.2（草稿）**，而路线图的里程碑已**一路实现至 Phase 7** ——
手册中的愿景现在是一个运行中的系统，而不只是草稿。

当前版本（v0.2.x）实现了完整的 Agent-OS 技术栈：

- 一个 **Tauri v2 桌面工作空间**（Vue 3 + TypeScript 前端、Rust 后端）。
- 一个以 [GBrain](https://github.com/garrytan/gbrain) 为基础的**知识图谱层** ——
  把日常文件（联系人 CSV、会议 PDF、公司介绍）变成互连、可查询的笔记；
  通过 GUI 而非命令行来同步、提问与推理。
- 完整的 **Agent-OS 执行引擎**（Phase 1–7）：由 Trigger 驱动的 Employee 生命周期引擎；
  持久化于 SQLite 的 **Artifact 成果物**与 **Commitment 承诺**；
  **模板 → 实例**（定义一次、部署多次）；**共享 Brain** 让一次升级惠及所有 Employee；
  **团队、项目与任务交接**实现多 Employee 协作；以及**对话层** —— 人机聊天、
  消息驱动唤醒、实时观察面板。
- 第一个**代理入口**：在工作空间内启动并监视 [Claude Code](https://claude.com/claude-code)。

> ℹ️ GBrain 知识图谱功能是当前版本的*知识*层。
> 手册中定义的 Employee / Runtime / Commitment 架构如今已实现并运行中。

## 技术栈

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**后端：** Tauri v2 · Rust

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
[**Releases** 页面](https://github.com/ascetic168/Emploid/releases)下载对应平台的最新版本并运行。
除非你要开发 Emploid，否则不需要 `git clone` 或从源码构建。

### 开发者（从源码构建）

构建桌面应用需要 **Rust 工具链**与 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/ascetic168/Emploid.git
cd Emploid
npm install          # 安装依赖
npm run tauri dev    # 运行应用（热重载）
npm run tauri build  # 构建分发用安装包
```

仅前端（于 http://localhost:1420 在浏览器运行）：`npm run dev`、`npm run build`。

## 开发

```bash
npm run tauri dev             # 完整应用，热重载
npm run build                 # 前端类型检查 + 构建
cd src-tauri && cargo test    # Rust 单元测试
cd src-tauri && cargo check   # 后端快速类型检查
```

## 项目结构

```
src/              Vue 3 前端（views、Pinia stores、i18n、带类型 IPC 包装）
                  —— Brains、Factories、Config、员工模板／实例、
                    员工对话、Operations（实时观察面板）
src-tauri/src/    Rust 后端
                    config · converters · factories · gbrain_cli · claude_code
                    brains · classifier · note_view · llm · prereq · i18n
                    runtime · agent_state · scheduler · note_server
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
