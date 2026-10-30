# Skill Library

> Applies to: the skills system since v0.5.10, the Skill Catalog since v0.5.11. Skills are procedural knowledge, **not authorization** — any action a skill suggests still executes within the employee's tool allowlist and code of conduct.

## The four-zone model

The "Skill Library" page is made up of four zones, each with different permissions:

| Zone | Location | Who can write | Loaded into employee prompts? |
|---|---|---|---|
| **Employee Drafts** (drafts) | workspace `skills/` | The employee itself | Yes |
| **Personal Area** (personal) | personal edition default `~/.agents/skills` (the same directory shared with Claude Code / ZCode and the rest of that ecosystem) | Only you (employees read-only) | Yes |
| **Company Library** (library) | `<data_dir>/skills/library/` | (Enterprise edition) shared org-wide after an admin promotes it | Yes |
| **Skill Catalog** (catalog) | `<data_dir>/skills/catalog/` | Read-only — the built-in factory shelf | **No** — takes up no prompt space until adopted |

Setting the personal area to an empty string (`skills_personal_dir = ""`) = enterprise mode: the personal area becomes `<data_dir>/skills/users/<principal>/`, isolated per principal.

## The Skill Catalog: browsing and adopting

A curated skill catalog ships out of the box: **11 plugins, 111 job skills** (productivity, sales, customer-support, product-management, marketing, legal, finance, data, enterprise-search, operations, human-resources), with content imported from [anthropics/knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins). oserver seeds them into the catalog automatically at startup; plugins you delete are not resurrected.

Operations:

- The page lists skills **grouped by plugin** (collapsible): drafts → Personal Area → Company Library → Skill Catalog (the ordering since v0.5.11).
- Clicking a skill card **opens the SKILL.md content directly** — read the content before deciding.
- **"Adopt"**: copies the skill from the catalog into your Personal Area — only after "Adopt" will AI employees load it.
- **"Remove"**: removes it from the Personal Area (confirmed with a second click).

## The union cap: `skills_union_limit`

The number of skills a single employee actually loads (drafts + personal area + company library combined) must not exceed the cap, **default 30** (`0` = unlimited, not recommended). This is a hard gate on prompt surface: once the cap is reached, any action that adds a skill — an employee writing a draft, you adopting, an admin promoting, a proposal being approved — is rejected until other skills are removed first. The catalog shelf is **not counted** (unadopted skills take up no prompt space).

## How skills are born: the proposal flow

Only humans move skill files — employees can never publish directly:

1. **The employee writes a draft** (in the workspace `skills/<name>/SKILL.md`).
2. **The employee proposes** (the `propose_skill` tool): the system first runs a **deterministic linter** (format checks), then an **independent subagent pre-reviews** it.
3. **The principal (you) approves**: the proposal appears in the "Inbox"; once approved, the server copies the skill into your Personal Area.
4. (Enterprise edition) **The admin promotes**: promotes a Personal Area skill to the Company Library, making it available org-wide.

Skill format: a directory + `SKILL.md` (YAML front matter with `name` / `description` + the body + optional scripts). You can also drop it straight into `~/.agents/skills` to share it with other tools — the same skill is readable by Claude Code / ZCode and by Operoid employees alike.

---
[← 10 Inbox and Events](10-inbox-and-events.md)｜[Index](README.md)｜[12 Knowledge Base and Claude Code →](12-knowledge.md)
