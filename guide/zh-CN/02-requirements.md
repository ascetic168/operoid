# 系统需求

> 适用版本：v0.5.11。本章说明个人版（桌面）的平台、前置组件与能力降级；企业版服务器的前置需求见 [DEPLOYMENT.md §2](../../DEPLOYMENT.md)。

## 支持平台

| 平台 | 安装包 | 备注 |
|---|---|---|
| Windows | 安装向导（.exe）、.msi | x64；自动更新走 .exe 安装格式 |
| macOS | .dmg | Apple Silicon 与 Intel 各有安装包 |
| Linux | .AppImage、.deb、.rpm | x86_64；需 `libwebkit2gtk-4.1` 等系统库（见 [Tauri v2 前置需求](https://v2.tauri.app/start/prerequisites/)） |

硬件方面没有官方最低规格——任何能顺畅运行现代桌面应用的电脑即可。知识检索依赖的本机嵌入服务（见下节）是一个小型模型（EmbeddingGemma 2，768 维），常驻内存占用不大；若加装多模态投影器，BF16 版约多占 0.9 GB、Q8_0 版约 0.5 GB。LLM 推理本身走你设定的云端或本机端点，不在这台机器上跑。

## 必要前置组件

知识功能依赖三个外部组件，**全部自动检测**，缺了会在界面上明说：

| 组件 | 用途 | 没有它 | 安装 |
|---|---|---|---|
| **git** | 同步流程会在更新图谱前先 commit | 工厂写文件后无法 sync | <https://git-scm.com/downloads> |
| **bun** | JS 运行环境；`gbrain` 通过 bun 安装与运行 | gbrain 无法运行 | <https://bun.com/docs/installation#installation> |
| **gbrain** | GBrain 知识图谱引擎 CLI | 无知识库可查 | <https://github.com/garrytan/gbrain#cli-standalone-no-agent> |

路径会自动检测（Windows 为 `~/.bun/bin/gbrain.exe`），必要时可在「设置」页覆盖（见[第 07 章](07-brains-and-models.md)）。

首次启动时，若检测到缺项，会出现「缺少前置程序」对话框，列出缺的组件、用途说明与安装链接——装好后重开 Operoid 即可。GBrain 是什么、为什么需要它，见[附录 A](appendix-a-gbrain.md)。

## 知识管线选用组件

这些组件决定知识功能的**质量上限**，缺项不会阻断流程（内置降级），但建议装齐：

| 组件 | 用途 | 没有它 |
|---|---|---|
| **llama-server**（llama.cpp，跑 EmbeddingGemma 2） | 本机嵌入后端——检索的硬依赖，须常驻（默认 `127.0.0.1:8080`） | 检索退为仅关键字比对 |
| **mmproj 投影器**（选配） | 图片的「caption＋原图」联合向量 | 图片仅以 caption 文字索引 |
| **MinerU**（选配，**强烈建议**；版本须 > 4.0） | 复杂 PDF 解析（双栏／扫描／含图）——`uv tool install "mineru>4.0"` | 仅能以快速模式处理非常简单的 PDF（纯文字、单栏、无图表）；文档内图片无法进入知识库 |
| **VLM chat 端点**（取决于你设定的 chat 模型） | 生成端直接读原图作答 | 降级为「定位者」模式：答案指出哪张图，像素内容由人类开图确认 |

### 能力矩阵与警告横幅

「设置 → 服务」页有一张**能力矩阵**（git／bun／gbrain／MinerU／嵌入端点／多模态／VLM），安装或调整设置后按「重新检查」更新。缺项同时会以警告横幅出现在主窗口上方，例如：

- 「未检测到 MinerU——PDF 知识管线目前仅能以快速模式处理非常简单的 PDF…」
- 「嵌入模型未启用多模态——图片仅以文字说明（caption）索引…」
- 「llama-server 缺 -b/-ub 8192 旗标——长 chunk 会被拒收…」

横幅可按「关闭（本次执行不再显示）」暂时收起；根治方式依横幅说明安装或调整启动旗标。嵌入服务的完整启动指令见 [DEPLOYMENT.md §10.1](../../DEPLOYMENT.md) 与[第 18 章](18-troubleshooting.md)。

## 数据流向透明度

个人版的程序与数据都在本机：脑数据是本机文件、嵌入走本机 `127.0.0.1`、无第三方遥测。**会离开这台机器的只有模型推理**，且端点皆由你自行设定（内置对话／文档转换／Claude Code 三个通道各自送往哪里，详见 [DEPLOYMENT.md §11.4](../../DEPLOYMENT.md)；治理面的说明见[第 13 章](13-governance.md)）。

---
[← 01 认识 Operoid](01-introduction.md)｜[返回索引](README.md)｜[03 安装个人版 →](03-install-personal.md)
