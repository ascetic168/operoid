# 脑与模型设置

> 适用版本：v0.5.11。「脑」的概念与定位见[附录 A](appendix-a-gbrain.md)；本章讲操作。

## 「脑」页

一颗脑＝一个 GBrain 知识本体（磁盘上是一个 `.gbrain` 目录及其父目录）。员工的知识查询都发生在其模板对应的脑上。

### 注册与切换

- **新增** → 二选一：「登录既有」（指向现有 `.gbrain` 的父目录）或「建立新脑（gbrain init）」。
- 建立新脑时可指定：名称、路径、embedding model 与维度、chat model——留空则用默认（embedding 默认 `llama-server:embeddinggemma-2`，768 维；「设置 → 模型」的「新建脑默认 embedding 模型」可改，仅影响新建脑）。
- **设为活动**——工厂写入、知识库操作都以活动脑为对象。
- 默认脑不可移除。

### 来源（sources）

脑的知识来自一个或多个 **git repo** 来源（工厂写入的页面就在来源 repo 里）：

- 「新增来源」：填来源 id（短名，如 `notes`）与 git repo 路径。
- 清单字段：id、路径、页数、上次同步；每行可「同步」「设活动」。
- 「同步全部」：对所有来源执行 sync（commit + sync + embed + extract）。
- 「绑定目录」：将来源绑到本地路径。
- **移除来源**时二选一：**封存**（从搜索隐藏、保留 72 小时，可还原）或**永久删除**（立即移除全部页面／向量／文件，不可逆）。

## 「设置 → 模型」

GBrain 的 `config.json` 是权威来源；这页的控件会直接写入它（以及 DB plane）：

| 区块 | 内容 |
|---|---|
| **主模型** | 一个 `provider:model`（如 `zhipu:glm-5.3-flash`）同步套用到对话与四个 tier |
| **模型路由（tier）** | 依任务分级：Utility（摘要等轻任务）、Reasoning（思考／抽取）、Deep（深度分析）、Subagent（子代理循环）。可「全部 tier 用同一个模型」 |
| **Provider base URL** | 直写 config.json 的端点覆写（file-plane） |
| **新建脑默认 embedding 模型** | `provider:model`＋维度；仅影响新建脑 |
| **gbrain 传输** | `MCP`（默认——员工知识工具走 `gbrain serve`，失败自动退回 CLI）或 `CLI`（纯子进程） |
| **LLM 端点解析** | 显示 API key 状态（「API key 已设／缺 API key（环境变量）」） |

注意事项：

- **DB plane 覆写**：部分键可能由 DB plane 生效（页面会列出「由 DB plane fallback 生效」的键）——改模型请用本页控件，不要直接改 `config.json`；「清除所有 DB 覆写」可归零。
- **schema pack 提示**：若活动脑仍是 v1（6 工厂），页面会建议执行「unify-types」迁移到 v2（15 标准型）——见[第 09 章](09-factories.md)。
- Subagent tier 仅 Anthropic 模型支持 prompt caching；其他 provider 的子代理循环成本随对话长度线性增（页面会警告）。

## 「设置 → 服务」

- **知识管线能力**：能力矩阵（git／bun／gbrain／MinerU／嵌入端点／多模态／VLM）＋「重新检查」。MinerU 可在此填**安装目录**（`mineru_dir`，支持 `%USERPROFILE%`／`${HOME}` 展开，跨机器可携带），保存后即时重测。
- **本地服务（oserver）**：port（默认 7340）、运行中／未运行、**开机自动启动 Operoid 服务**的注册／解除——见[第 15 章](15-service-and-updates.md)。
- **Obridge**：设置文件路径、可执行文件、随 App 启动——见[第 14 章](14-email-bridge.md)。

## 其他设置分页

- **应用程序**：界面语言、路径（notes repo、gbrain.exe、GBRAIN_HOME 覆写）、LLM 温度／max_tokens、工厂自动 sync、Agent-OS 开关、Obridge、版本与更新——分散见[第 15 章](15-service-and-updates.md)与[第 17 章](17-config-reference.md)。
- **登记表**：动作类别登记表（委任界线）——见[第 13 章](13-governance.md)。
- **知识授权**：知识政策、principals、TTL grants——见[第 13 章](13-governance.md)。
- **高级**：`config.json` 原始编辑器。⚠️ model／tier 键可能被 DB plane 覆写——请改用「模型」分页的控件编辑。

---
[← 返回索引](README.md)｜[08 员工管理 →](08-employees.md)
