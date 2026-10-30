# 知识库与 Claude Code

> 适用版本：v0.5.11。图表级检索自 v0.5.0 起。GBrain 的架构说明见[附录 A](appendix-a-gbrain.md)。

## 「知识库」页

这页把 gbrain 指令包装成按钮、串流输出——绕过员工、直接操作作用中脑：

### 查询

| 操作 | 做什么 |
|---|---|
| **ask** | 混合检索——对脑提问，取回相关页面 |
| **query** | 混合检索（ask 的新一代指令） |
| **think** | 多跳合成——可用 `anchor:slug` 换行后接问题，以指定页为锚点推理 |

检索不只回文字：命中含图页面时，回复会带上**检索到的图表**与**来源 PDF**（图表向量＋VLM 图注的成果，见[第 02 章](02-requirements.md)）。

### 维护与诊断

| 操作 | 做什么 |
|---|---|
| **sync** | commit + sync + embed + extract——写入后的完整同步 |
| **extract** | `extract --stale`——补抽落后的图谱 |
| **stat** | 脑统计 |
| **doctor** | 健康检查 |
| **orphans** | 找出无入边的孤立页 |
| **storage** | 层状态 |
| **graph-query** | 输入 slug，看某实体的图谱边 |
| **重建 companies** | 从 people 重建公司页（可加 `--clean`） |
| **unify-types** | schema pack v1→v2 迁移（见[第 09 章](09-factories.md)） |

## Claude Code 整合

左侧导航底部的「打开 Claude Code」按钮，会以**当前脑的 MCP 服务器**启动 [Claude Code](https://claude.com/claude-code)——让你用终端里的 coding agent 直接查写这颗脑：

- **脑**：选哪颗脑（think / search 涵盖此脑全部来源）。
- **终端**：Windows Terminal／cmd／PowerShell，或自定义命令模板（`{cwd}`／`{cmd}` 占位符）。
- **工作目录**：Claude Code 编辑文件的位置（不影响脑的知识）。
- 启动前会自动检查：`claude` CLI 是否在 PATH、gbrain 是否就绪。

⚠️ **重要披露**（对话框内原文）：「此通道可读写整颗脑（不受知识范围限制）；对话与检索到的知识内容会送往云端的 Claude 端点。」——Claude Code 对所选脑有**完整读写**（含删页等写入操作），误改可靠 gbrain 版本历史恢复。个人版这是合理默认；**企业场景不应依赖此通道**（可以 `claude_code_handoff_enabled=false` 整个退役），请改用受治理的员工能力。

## 员工怎么查知识

员工的知识工具（think／search）走同一颗脑，但**经过治理**：企业模式依知识政策在检索前授权（见[第 13 章](13-governance.md)）。传输方式由「设置 → 模型 → gbrain 传输」决定：MCP（默认，`gbrain serve` 子进程，失败自动退回 CLI）或纯 CLI。对话里的每次知识查询都会显示在工具调用轨迹中（见[第 08 章](08-employees.md)）。

企业模式的主控台检索操作可被 `ops_retrieval_enabled=false` 停用——检索一律走员工工具，管理面不绕过治理。

---
[← 返回索引](README.md)｜[13 治理 →](13-governance.md)
