# 文档工厂

> 适用版本：v0.5.11。工厂把原始文件变成结构化知识页，写进活动脑的来源 repo——知识入库的主要入口。

## 基本流程

1. 选好活动脑与**活动来源**（没有来源时页面会提示先到「脑」页新增）。
2. 把文件**拖放**到对应工厂（或点拖放区选文件）。
3. 自动转换并写入——「转换并写入中（LLM 结构化可能需数秒）…」。
4. 点选已处理文件可**预览**生成的页面，逐页检查、编辑后「覆盖此页」。
5. 按**「Sync 到脑」**（`sync --source <id>`，含 embed/extract）——至此知识才可被检索。

「设置 → 应用程序」的「工厂写档后自动 commit + sync」**默认开启**——写入后自动完成第 5 步；关闭时需手动按「Sync 到脑」。无 git remote 的脑，建议维持「sync 加 --no-pull」开启。

## 工厂类型

Schema pack **v1** 的内置工厂：

| 工厂 | 接受 | 写入 |
|---|---|---|
| **图片** | jpg/png 等图片文件 | `media/`＋强制注解页（见下） |
| **people** | CSV（Google Contacts 导出） | `people/` |
| **companies** | txt / md / pdf | `companies/` |
| **meeting** | txt / md / pdf | `meetings/` |
| **projects** | txt / md / pdf | `projects/` |
| **concepts** | txt / md / pdf | `concepts/` |
| **inbox（速记）** | txt / md | `inbox/`（gbrain capture；**不进图谱**——速记不做实体连结） |
| **自动** | csv / txt / md / pdf | 依内容判断分派到各类型；无法判定时弹出确认让你选 |

**schema pack v2**（15 个标准类型）可用「设置 → 模型」的「执行 unify-types」迁移（新增 media、tweet、deal、email、analysis 等类型）；新脑建议直接用 v2，既有 v1 脑迁移前先确认没有 gbrain 进程在跑并备份脑数据库。

### 图片工厂

图片**必须填注解**才会入库（「每张图都要填注解」）：每张图存进 `media/` 并生成同名注解页，你的描述就是页面内容。sync 时 gbrain 会做多模态嵌入与 `image_of` 连结——前提是嵌入服务启用了多模态（`--mmproj`，见[第 02 章](02-requirements.md)）；未启用时图片在 sync 会被跳过并提示。

## 复杂 PDF 怎么被处理

PDF 依信号自动分流（`pdf_convert_policy`：`auto`／`always_fast`／`always_mineru`）：

- **简单 PDF**（纯文字、单栏、无图表）——快速路径直接抽取。
- **复杂 PDF**（双栏论文、扫描、含图）——经 **MinerU** 完整转换：章节切块＋图片笔记＋向量，图片成为独立检索项。转换尝试过程记录在来源 repo 的 `conversion-report.json`，可审计。

没装 MinerU 时复杂 PDF 质量下降（见[第 02 章](02-requirements.md)的降级表）；egress 信任分层（文档会不会送去第三方云解析）见[第 13 章](13-governance.md)。

## 直接新增页面

每个工厂右上角「+」：不用文件，直接在编辑器以合规 template 新增页面（例如一笔人物资料）。存档时 LLM 会把你写的人名／公司名补成 `[[wikilink]]`、以 title 作为档名；「取名写入」或「保存并 Sync」一步完成。

企业模式（Enterprise C′）新增时还可指定**保密等级**与**圈子（部门／项目）**——写入更高的等级需要作者具备对应 clearance。

## 产出统计

转换完成的摘要会列出：写入页数、冻结未覆写（既有页保护——不自动覆盖人工编辑过的内容）、丢弃非法日期事件、LLM 富集成功／失败数。people 工厂另有「重建 companies」：从 people 扫出公司／组织，重建 `companies/` 页与连结（可加 `--clean` 清除重建）。

---
[← 返回索引](README.md)｜[10 待办与事件 →](10-inbox-and-events.md)
