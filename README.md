# Operoid

**English** | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md)

> **AI employees your company can actually trust.**
>
> Operoid is an open-source **AI Agent Operating System**: AI agents —
> **Employees** — hold persistent responsibilities in a shared **Workspace**,
> working inside boundaries that are **drawn by humans, enforced by the
> server, and auditable by anyone.**

Built with **Rust** (a resident service + a Tauri v2 desktop shell) and **Vue 3 + TypeScript**, running on your own hardware.
**Author:** [朱國棟 (Charlie Chu)](#about-the-author) · **License:** [MIT](#license)

---

## Why Operoid?

Every serious agent product now keeps working after you close the window — scheduled triggers, persistent memory, always-on runtimes. Continuous execution has become **table stakes**.

The unsolved question is the one every manager actually asks: ***why would I let it?***

- What exactly is it allowed to do?
- What is it allowed to know?
- And who answers when it gets it wrong?

Today's AI behaves like a brilliant **consultant**: it gives advice and leaves.

Inside the company it has:

- no identity
- no authorized scope
- no accountability when a decision goes wrong

Which is why AI at work still lives in small, tightly supervised windows.

An **employee** is different: an employee joins the organization, owns outcomes, and stays accountable. For an AI to do the same, it needs what any new hire needs:

- an **identity** the organization issues — and can revoke;
- a **delegation** that says what it may do without asking;
- **clearance** that says what it may know;
- a **paper trail** for everything it touches.

Operoid ships all four — **as architecture, not as a pricing tier** — in an open-source core that runs where your data lives: on your machine, or on your company's own intranet.

## An operating system for AI agents

Operoid relates to agents the way an operating system relates to programs: it provides the environment; it doesn't do the programs' work.

| OS concept | In Operoid |
|---|---|
| **Processes** | **Employees** — scheduled, run, and suspended |
| **Files** | **Artifacts** — durable outputs owned by the workspace, not the chat |
| **Memory** | **Working memory & knowledge** — restored on demand, not held resident |
| **Devices** | **Tools** — external capabilities behind a controlled interface |
| **The kernel** | **The Runtime** — wakes an Employee, restores its context, lets it work, puts it back to sleep |

The Runtime manages **execution**. It never manages **reasoning** — what an Employee thinks is its own.
That is why Operoid is an operating system, not an application.

## Governance, built into the architecture

### A delegation boundary humans can draw

"Should AI decide?" is not one question but three:

- Can the system decide correctly (**capability**)
- Who answers when the decision is wrong (**accountability**)
- Is this a decision a human should be making at all (**legitimacy**)

Only the first answer changes with technology.

Operoid makes the boundary constitutional — humans draw it — and operational as the **Action Registry**:
authority is granted per **action category**, never to "the system"; **strict by default**, expiring and re-signed, **frozen automatically after an incident**.

### A knowledge base that knows who is asking

- Same brain, same query, different identity → **different results**.
- Authorization happens **before retrieval**, evaluated in deterministic server code — never by an LLM, never fetch-then-hide.
- A missing or corrupt policy means **deny** (fail closed).
- Every privileged retrieval leaves a **receipt**: who got what, for which task, under which policy version.

### An identity and a paper trail for every Employee

- Every Employee authenticates with its own token — identity is resolved from the token chain, never from what a caller claims.
- Every piece of work lands as a versioned **Artifact** owned by the workspace, not the chat.
- Long-running responsibilities become **Commitments**: they survive restarts, retry on failure, and are **handed back to a human after repeated failures** instead of failing silently.

![The manager inbox: pending proposals to approve, and employees needing attention](assets/manager-inbox.png)

## How Operoid compares

The agent market falls into four categories. They all claim continuous operation; organizational governance is another matter:

|  | Personal AI assistants | Developer agent tools | Cloud "AI employee" SaaS | **Operoid** |
|---|---|---|---|---|
| Built for | Individuals | Engineers | Business teams | **Companies & teams** |
| Runs on | Your machine | Your terminal | The vendor's cloud | **Your machine or your intranet** |
| Works unattended | ✓ | limited | ✓ | ✓ |
| Delegation boundary — what AI may do without asking | ✗ | ✗ | enterprise tier | **architecture — per action category, human-drawn** |
| Knowledge permissions enforced before retrieval | ✗ | ✗ | partial, cloud-only | **fail-closed, server-enforced** |
| Agent identity & audit trail | ✗ | ✗ | enterprise tier | **built in — every privileged act leaves a receipt** |
| Open source | ✓ | ✓ | ✗ | ✓ · MIT |

## What it looks like

An Employee wakes on a trigger:

- Trigger sources: a schedule, an incoming email, a message from a manager or another human employee
- Restores its context, works inside its own sandboxed workspace (read/edit files, run commands, query knowledge, all under a per-template allowlist *you* control)
- Then goes back to sleep

In the chat you watch it work: every tool call, its arguments, its duration — and a clean markdown answer. When a decision exceeds its authority — it doesn't guess: it proposes, and a human approves.

![A conversation where the Employee's tool calls are visible between the question and the markdown answer](assets/chat-with-process.png)

Documents become knowledge — figures included. Two-column papers, scanned reports, and figure-heavy decks are parsed into section-aware, retrievable notes; every figure is its own retrievable item.
A real IEEE paper as the benchmark:

**Twelve questions about specific figures and tables — 12/12 hit in the top 5 (MRR 0.90).**

## What Operoid does today

The Architecture Handbook's roadmap is implemented end-to-end through **Phase 7** (as of **v0.5.0**) — all five milestones shipped:

1. ✅ **One Employee that truly works** — wake on a trigger, restore context, invoke tools, commit artifacts, sleep.
2. ✅ **Persistence** — work survives a full shutdown and restart.
3. ✅ **Shared Brains & Knowledge** — upgrade one Brain, many Employees adopt it.
4. ✅ **Templates & Instances** — one template, many independent Employees.
5. ✅ **Collaboration** — teams of Employees completing a Project together.

Also completed: a chat you can watch (per-tool-call process visibility), email and IM access to the same Employees via [obridge](obridge/), a knowledge-graph foundation, the document pipeline above, and [Claude Code](https://claude.com/claude-code) as a first agent entry point.
The full picture — concepts, principles, roadmap — is the **[Architecture Handbook](handbook/README.md)**.

## Two editions, one codebase

| | Personal | Enterprise |
|---|---|---|
| Artifact | Desktop installer | `operoid-enterprise-*` archive |
| Where it runs | Your own computer | Your company intranet |
| Setup | Install and run | One guided command: `oserver configure` |
| Access | Desktop app | Browser: `/admin` `/manager` `/user` — password login + roles |

Optional capabilities (knowledge-graph engine, local embeddings, complex-PDF parsing) are auto-detected and surfaced as a capability matrix inside the app; install notes are in **[DEPLOYMENT.md](DEPLOYMENT.md)**.

## Get started

- **Personal** — grab the installer from the [**Releases** page](https://github.com/ascetic168/Operoid/releases) and run it. No build needed. In-app updates: Settings → App → "Version & updates" — update notices are on by default (a banner when a new release ships; installing always needs one click); fully automatic updates are off by default since releases may be backward-incompatible.
- **Enterprise** — download the enterprise archive from the same release and run `oserver configure`: a guided wizard that checks prerequisites, sets up TLS, creates the admin account, and optionally registers the boot-time service. Walkthrough in **[DEPLOYMENT.md](DEPLOYMENT.md)**. Upgrades: check the admin console's Version card for new releases, then follow the manual upgrade procedure in **[DEPLOYMENT.md §12](DEPLOYMENT.md)**.
- **From source** — Rust toolchain + [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/):

  ```bash
  git clone https://github.com/ascetic168/Operoid.git
  cd Operoid
  npm install
  npm run tauri dev    # run the app (hot reload)
  ```

  Development commands: `npm run build` (frontend typecheck + build), `cargo test` / `cargo check` (Rust workspace). When changing `oserver`/`obridge`, build them together with `cargo dev-build` — the service spawns `obridge` as a sibling binary and cross-checks build ids.

## Go deeper

- **[Architecture Handbook](handbook/README.md)** — the constitution: concepts, principles, and the reasoning behind them ([Chapter 21 — Roadmap](handbook/21-Roadmap.md)).
- **[DEPLOYMENT.md](DEPLOYMENT.md)** — prerequisites, degradation behavior, and deploying both editions.
- **[The Shape of the Boundary](journal/The_Shape_of_the_Boundary_Charlie_Chu.pdf)** — the essay behind the Action Registry, distilled from a real (de-identified) manufacturing case.
- Questions & feedback: open a [GitHub issue](https://github.com/ascetic168/Operoid/issues).

## About the author

- I'm **Charlie Chu (朱國棟)**, founder of Operoid.
- Thirty years in the optoelectronic semiconductor industry — from engineer to senior executive.
- Currently a consultant helping enterprises adopt AI.
- Operoid's governance machinery — including the Action Registry — is distilled from real manufacturing-floor cases, not from theory.

## Bundled skill catalog

The product ships a curated skill catalog: `ocore/default-skills/` is imported
from [anthropics/knowledge-work-plugins](https://github.com/anthropics/knowledge-work-plugins)
(commit `d9d8e13`, 2026-10-27) by `scripts/import_kwp_skills.py` and seeded on
startup into the **skill catalog** (`<data_dir>/skills/catalog/`, read-only
mount `catalog/…`) — a browse-and-adopt shelf that does **not** load into
anyone's prompt. Principals adopt skills into their personal area from the UI
(and remove them again); only adopted skills — plus the org library and
employee drafts, bounded by `skills_union_limit` (default 30) — appear in an
AI employee's prompt. 111 role skills across 11 plugins (productivity, sales,
customer-support, product-management, marketing, legal, finance, data,
enterprise-search, operations, human-resources).

Upstream is © Anthropic and its partners, licensed under the **Apache License 2.0**
(MIT for partner plugins); their LICENSE files are bundled per plugin. Provenance
lives in `ocore/default-skills/SOURCE.md` and `ocore/default-skills/<plugin>/SOURCE.md`,
the lint/import audit in `IMPORT-REPORT.md`. These skills are procedural guidance,
not extra capability — every action they suggest still runs inside the employee's
tool allowlist and the behavior rules.

## License

Released under the **[MIT License](LICENSE)**.
Copyright © 2026 朱國棟 (Charlie Chu). See [LICENSE](LICENSE) for the full text.
