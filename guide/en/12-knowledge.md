# Knowledge Base and Claude Code

> Applies to: v0.5.11. Figure-level retrieval since v0.5.0. For GBrain's architecture, see [Appendix A](appendix-a-gbrain.md).

## The "Knowledge Base" page

This page wraps gbrain commands into buttons with streamed output — bypassing employees to operate the active brain directly:

### Queries

| Operation | What it does |
|---|---|
| **ask** | hybrid retrieval — ask the brain a question and get relevant pages back |
| **query** | hybrid retrieval (the successor to ask) |
| **think** | multi-hop synthesis — put `anchor:slug` on a new line followed by the question to reason with that page as the anchor |

Retrieval does not return text only: when a hit lands on a page with figures, the reply carries the **retrieved figures** and their **source PDFs** (the payoff of figure vectors + VLM captions, see [Chapter 02](02-requirements.md)).

### Maintenance and diagnostics

| Operation | What it does |
|---|---|
| **sync** | commit + sync + embed + extract — the full post-write sync |
| **extract** | `extract --stale` — catch up on lagging graph extraction |
| **stat** | brain statistics |
| **doctor** | health check |
| **orphans** | find orphan pages with no inbound edges |
| **storage** | layer status |
| **graph-query** | enter a slug to view an entity's graph edges |
| **Rebuild companies** | rebuild company pages from people (optionally add `--clean`) |
| **unify-types** | schema pack v1→v2 migration (see [Chapter 09](09-factories.md)) |

## Claude Code integration

The "Open Claude Code" button at the bottom of the left navigation launches [Claude Code](https://claude.com/claude-code) with **the current brain's MCP server** — letting you query and write this brain directly from a terminal coding agent:

- **Brain**: which brain to pick (think / search covers all sources of this brain).
- **Terminal**: Windows Terminal / cmd / PowerShell, or a custom command template (`{cwd}` / `{cmd}` placeholders).
- **Working directory**: where Claude Code edits files (does not affect the brain's knowledge).
- Before launch it automatically checks whether the `claude` CLI is on PATH and whether gbrain is ready.

⚠️ **Important disclosure** (verbatim from the dialog): "This channel reads and writes the whole brain (knowledge scopes do not apply); conversation and retrieved knowledge content are sent to the Claude endpoint in the cloud." — Claude Code has **full read/write** on the selected brain (including write operations such as deleting pages); mistaken edits can be restored via gbrain's version history. For the personal edition this is a reasonable default; **enterprise scenarios should not rely on this channel** (it can be retired entirely with `claude_code_handoff_enabled=false`) — use governed employee capabilities instead.

## How employees query knowledge

Employees' knowledge tools (think / search) go through the same brain, but **under governance**: in enterprise mode, retrieval is authorized against knowledge policies before it runs (see [Chapter 13](13-governance.md)). The transport is decided by "Settings → Models → gbrain transport": MCP (the default; a `gbrain serve` subprocess that automatically falls back to CLI on failure) or CLI only. Every knowledge query in a conversation shows up in the tool-call trace (see [Chapter 08](08-employees.md)).

In enterprise mode, the console's retrieval operations can be disabled with `ops_retrieval_enabled=false` — all retrieval goes through employee tools, and the admin console does not bypass governance.

---
[← 11 Skill Library](11-skills.md)｜[Index](README.md)｜[13 Governance →](13-governance.md)
