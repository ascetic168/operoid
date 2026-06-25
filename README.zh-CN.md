# GBrainStudio

[English](README.md) | [繁體中文](README.zh-TW.md) | **简体中文**

**GBrainStudio** 是 [GBrain](https://github.com/garrytan/gbrain) 的友好桌面伙伴。
把日常文件丢进来，就能得到一张互相关联、可查询的知识图谱 —— 无需手写链接、无需碰命令行。

以 **Tauri v2（Rust）** + **Vue 3 + TypeScript** 打造。
**作者：** 朱國棟 (Charlie Chu) · **授权：** [MIT](#授权)

---

## 为什么需要 GBrainStudio？

[GBrain](https://github.com/garrytan/gbrain) 是一套强大的知识图谱引擎 —— 但它以 CLI 为主，
而且要求**手工撰写笔记**：每个交叉引用都必须是格式正确、命名正确的链接，否则在图谱里会
默默断开。这样积累语料既慢又容错率低，日常工作还得背命令、读终端输出。

**GBrainStudio 把这些摩擦去掉：**

- 📥 **丢个文件，得到一则已链接的笔记。** 丢进通讯录 CSV、会议纪要 PDF、或公司介绍 ——
  GBrainStudio 把它转成合规笔记，所有交叉引用都自动生成，链接确实能在图谱里解析。
- 🔗 **永远不必手写链接。** 提到某个人或某家公司，就自动接进图谱；你只管写白话文，链接交给程序。
- 🖱️ **整套流程都有 GUI。** 跑 sync / ask / think 带实时流式输出；点答案里的任一引用，
  就能在浏览器打开来源笔记。
- 🧠 **多个知识图谱，可视化管理。** 创建、登记、同步多个隔离的脑及其来源，全程不必开终端。

一句话：GBrainStudio 把 GBrain 从进阶玩家的 CLI，变成一套**好上手**的知识图谱
构建与探索工具。

## GBrain 是什么？

[GBrain](https://github.com/garrytan/gbrain) 是底层的知识图谱引擎：你的笔记会变成一张
可查询的图，涵盖人物、公司、会议与概念。引擎本身的细节请见它的仓库。

> ℹ️ **GBrainStudio 是独立的配套 GUI —— 它不是 GBrain。** 它帮你驱动 `gbrain` CLI；
> 引擎、图谱存储与检索都仍是 GBrain 的事。

## 功能

### 🏭 工厂 —— 把来源文件变成已链接的笔记

把文件拖到卡片上（或点击选文件），即转换并直接写入笔记：

| 工厂 | 接受 | 变成 |
|---|---|---|
| **people** | Google Contacts CSV / TXT / MD | 人物笔记（CSV 一档多人；TXT/MD 一人一档，LLM 结构化） |
| **companies** | TXT / PDF | 公司笔记（LLM 结构化） |
| **meeting** | TXT / MD / PDF | 会议笔记（LLM 结构化） |
| **inbox** | TXT / MD | 速记 capture |

- **批量拖放：** 一次丢多个文件；结果清单显示每个文件的状态，可点任一文件预览、编辑后再同步。
- **来源感知：** 笔记写对位置 —— 作用中脑的作用中来源 repo —— 所以一键 **Sync 到脑** 就接得上。
- **手写编辑器（`+`）：** 从 template 开始自然书写；保存时你提到的人名/公司名会自动连进图谱。

### 🔧 操作 —— 不必开终端就能用 GBrain

包装 `gbrain` CLI；输出实时流式到 console。

- **stats · sync · extract** —— 维持语料健康。
- **ask · think** —— 问问题，得到多跳、带引用的答案（`think` 可用 `anchor:` 聚焦某则笔记）。
- **诊断** —— `doctor`、`orphans`、`storage`、`graph-query`。
- **重建 companies** —— 从人物笔记重新生成公司笔记。
- **可点击引用：** 答案里的 `[[people/JLin]]` / `[people/JLin]` 标签会高亮；点下去就能在浏览器读该笔记。

### 🧠 脑 —— 管理多个知识图谱

`gbrain` 没有“列出所有脑”的指令，GBrainStudio 给你一个：

- 登记既有脑，或创建新脑。
- 每脑可有多个**来源**（git repo），可逐来源或全部同步。
- 切换脑时，config、来源、作用中目标全部跟着走。

### ⚙️ 设置 —— 所有设置集中一处

并排编辑 GBrain 权威的 `config.json`（model、embedding、provider）与本系统自有设置
（路径、输出文件夹、sync 标志、语言）。

### 还有

- **启动检查** —— 检查 `git` / `bun` / `gbrain`，缺失者直接给安装链接。
- **三种语言** —— 繁中、简中、英文，自动检测并可手动覆盖。

## 前置需求

| 工具 | 用途 | 安装 |
|---|---|---|
| **git** | sync 流程会在更新图谱前先 commit | <https://git-scm.com/downloads> |
| **bun** | `gbrain` 通过 bun 安装与运行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 引擎本体 | <https://github.com/garrytan/gbrain> |

路径会自动检测（Windows 为 `~/.bun/bin/gbrain.exe`）；必要时可在“设置”页覆盖。

## 安装与运行

> 构建桌面应用需要 **Rust 工具链**与 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install          # 安装依赖
npm run tauri dev    # 运行应用（热重载）
npm run tauri build  # 构建分发用安装包
```

仅前端（于 http://localhost:1420 在浏览器运行）：`npm run dev`、`npm run build`。

## 快速上手

1. **启动** GBrainStudio —— 缺失会以窗口列出。
2. **设置** —— 确认笔记文件夹与 `gbrain` 路径。
3. **脑** —— 选择或创建脑，加入一个**来源**（放笔记的 git repo），并设为作用中。
4. **工厂** —— 把文件拖到对应卡片，再按 **Sync 到脑**。
5. **操作** —— 用一个问题跑 `think`；点任一高亮的人名即可打开该笔记。

## 设置

经 `tauri-plugin-store` 存于 app data：

| 字段 | 含义 |
|---|---|
| `notes_repo_path` | 兜底的笔记文件夹；工厂优先用作用中来源的文件夹 |
| `gbrain_exe_path` | `gbrain` 可执行文件路径 |
| `factory_targets` | 输出子文件夹（`people` / `companies` / `meetings`） |
| `auto_sync` | 工厂写文件后自动 commit + sync |
| `sync_no_pull` | 加 `--no-pull`（无 remote 的脑建议开启） |
| `llm_temperature` / `llm_max_tokens` | 工厂 LLM 结构化的采样参数 |
| `locale` | 界面语言（`null` = 自动检测） |

> 一份笔记的文件位于“作用中脑的作用中来源 repo”。点引用时会先搜该来源、再其他来源、
> 最后才退到兜底文件夹 —— 永远开对文件。

## 项目结构

```
src/              Vue 3 前端（views、Pinia stores、i18n、带类型 IPC 包装）
src-tauri/src/    Rust 后端（config、converters、factories、gbrain_cli、
                  brains、note_view、llm、prereq、i18n）
```

## 技术栈

**前端：** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router ·
vue-i18n · lucide-vue-next。**后端：** Tauri v2 · Rust。

## 开发

```bash
npm run tauri dev             # 完整应用，热重载
npm run build                 # 前端类型检查 + 构建
cd src-tauri && cargo test    # Rust 单元测试
cd src-tauri && cargo check   # 后端快速类型检查
```

## 疑难排查

- **`think` / `ask` 显示“（no LLM available）”？** `gbrain think` 的 model 取用顺序为
  `models.think → models.default → GBRAIN_MODEL → opus`。`gbrain init --chat-model X` 只设了
  `chat_model` 而**未设** `models.default`，所以 `think` 可能默默退回 Anthropic Opus。修正：
  在该脑上 `gbrain config set models.default <provider:model>`
  （如 `groq:llama-3.3-70b-versatile`）。（GBrainStudio 自有的工厂结构化直接读 `chat_model`，不受影响。）
- **点引用显示“找不到笔记”？** 该笔记不在作用中脑的任一来源中，或文件名大小写不同。
  GBrainStudio 会先精确比对、再大小写宽容扫描 —— 请确认文件确实在作用中来源下。

## 授权

本项目以 **[MIT 授权](LICENSE)** 发布。

Copyright © 2026 朱國棟 (Charlie Chu)。完整条文见 [LICENSE](LICENSE)。
