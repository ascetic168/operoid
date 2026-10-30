# 技能庫

> 本章撰寫中。背景：CHANGELOG v0.5.10／v0.5.11。

## 預定內容

- 技能是什麼：程序性知識（SKILL.md），不是授權——行動仍受工具允許清單約束
- 四區模型：「員工草稿區」（drafts）／「個人區」（personal，預設 `~/.agents/skills`，與 Claude Code／ZCode 共享）／「公司共用庫」（library）／「技能圖書館」（catalog，唯讀書架）
- 「技能圖書館」：內建 11 個外掛、111 個職能技能；「加入」到個人區才會載入；「移除」
- union 上限：`skills_union_limit`（預設 30）——草稿＋個人區＋共用庫合計；超限時的行為
- 技能提案流程：員工草稿→提案（linter＋子代理預審）→主體核可→（企業版）管理員晉升到共用庫

---
[← 10 待辦與事件](10-inbox-and-events.md)｜[索引](README.md)｜[12 知識庫與 Claude Code →](12-knowledge.md)
