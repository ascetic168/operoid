# 技能库

> 适用版本：技能系统自 v0.5.10 起、技能图书馆（catalog）自 v0.5.11 起。技能是程序性知识，**不是授权**——技能建议的任何动作仍在员工工具允许清单与行为准则内执行。

## 四区模型

「技能库」页由四个区组成，权限各自不同：

| 区 | 位置 | 谁能写 | 会进员工提示词吗 |
|---|---|---|---|
| **员工草稿区**（drafts） | 工作区 `skills/` | 员工自己 | 会 |
| **个人区**（personal） | 个人版默认 `~/.agents/skills`（与 Claude Code／ZCode 等生态共用同一目录） | 只有你（员工只读） | 会 |
| **公司共用库**（library） | `<data_dir>/skills/library/` | （企业版）管理员晋升后组织共用 | 会 |
| **技能图书馆**（catalog） | `<data_dir>/skills/catalog/` | 只读——出厂内置书架 | **不会**——采用前不占任何提示词 |

个人区设为空字符串（`skills_personal_dir = ""`）＝企业模式：个人区改为 `<data_dir>/skills/users/<principal>/`，逐主体隔离。

## 技能图书馆：浏览与采用

出厂即附策展技能目录：**11 个插件、111 个职能技能**（productivity、sales、customer-support、product-management、marketing、legal、finance、data、enterprise-search、operations、human-resources），内容导入自 [anthropics/knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins)。oserver 启动时自动种子进图书馆；你删掉的插件不会被复活。

操作：

- 页面按**插件分组**（可折叠）列出：drafts → 个人区 → 公司共用库 → 技能图书馆（自 v0.5.11 的排序）。
- 点技能卡可**直接查看 SKILL.md 内容**——先看内容再决定。
- **「加入」**：把技能从图书馆复制进你的个人区——只有「加入」后，AI 员工才会载入它。
- **「移除」**：从个人区移除（两次点击确认）。

## union 上限：`skills_union_limit`

单一员工实际载入的技能数（草稿＋个人区＋共用库合计）不得超过上限，**默认 30**（`0`＝不设限，不建议）。这是提示词面积的硬闸：到达上限时，任何「新增技能」动作——员工建草稿、你采用、管理员晋升、提案核可——都会被拒绝，直到先移除其他技能。图书馆书架**不计入**（未采用的技能不占提示词）。

## 技能是怎么诞生的：提案流程

只有人类会移动技能文件——员工永远不能直接发布：

1. **员工写草稿**（在工作区 `skills/<名称>/SKILL.md`）。
2. **员工提案**（`propose_skill` 工具）：系统先跑**确定性 linter**（格式检查），再由**独立子代理预审**。
3. **主体（你）核可**：提案出现在「待办」，核可后服务器把技能复制到你的个人区。
4. （企业版）**管理员晋升**：把个人区技能晋升到公司共用库，全组织可用。

技能格式：一个目录＋`SKILL.md`（YAML front matter 的 `name`／`description`＋正文＋选附脚本）。它也可以直接放进 `~/.agents/skills` 与其他工具共用——同一份技能，Claude Code／ZCode 与 Operoid 的员工都能读。

---
[← 返回索引](README.md)｜[12 知识库与 Claude Code →](12-knowledge.md)
