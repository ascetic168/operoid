# Operoid

**English** | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md)

> Operoid is not a chat application. It is an **AI Agent Operating System** — an
> operating environment where AI agents, called **Employees**, continuously do
> meaningful work inside a shared, persistent **Workspace**.

Most AI products are conversation-centric: you ask, it answers, the window
closes and the work is gone. Real work isn't like that. A buyer tracks an order
for weeks; a QA engineer follows a nonconformance from report to corrective
action to closure. Those responsibilities need an environment that
**persists, remembers, and keeps working after the window closes.**

Operoid exists to be that environment.

Built with **Rust** (a resident service + a Tauri v2 desktop shell) and
**Vue 3 + TypeScript** frontends over a local HTTP API.
**Author:** 朱國棟 (Charlie Chu) · **License:** [MIT](#license) · **Status:** see [Current status](#current-status)

---

## Why Operoid?

Today's AI behaves more like a **consultant** than an **employee**. A consultant
gives advice and leaves. An employee joins the organization, owns outcomes, and
stays accountable. Operoid is built for the latter.

Today's AI systems generally lack:

- **persistent responsibilities** — work vanishes when the chat ends.
- **long-term commitments** — no notion of "track this until it's done."
- **shared workspaces** — nowhere for multiple agents and humans to collaborate on the same things.
- **organizational knowledge** — what the model knows isn't what the organization knows.
- **enterprise roles** — an agent has no identity, authority, or accountability.
- **continuous execution** — nothing wakes the agent when something relevant happens.

Operoid treats AI as **organizational members, not chatbots.**

## What is an "AI Agent Operating System"?

A conventional OS manages processes, memory, files, and devices so programs can
run — it provides the environment, it doesn't do the programs' work. Operoid
does the same for AI agents:

| OS concept | In Operoid |
|---|---|
| **Processes** | **Employees** — agents that are scheduled, run, and suspended |
| **Files** | **Artifacts** — durable outputs owned by the workspace, not the chat |
| **Memory** | **Working memory & knowledge** — restored on demand, not held resident |
| **Devices** | **Tools** — external capabilities invoked through a controlled interface |
| **The kernel** | **The Runtime** — wakes an Employee, restores its context, lets it execute, puts it back to sleep |

The Runtime manages **execution**. It never manages **reasoning** — what an
Employee thinks is its own. That is why Operoid is an operating system, not an
application.

## Core concepts

| Concept | One-line role |
|---|---|
| **Workspace** | The organization. Everything lives inside exactly one. |
| **Employee** | The worker. An AI agent that owns responsibilities. |
| **Brain** | The intelligence. Reusable, versioned knowledge and persona. |
| **Artifact** | The result. Output of work, owned by the workspace. |
| **Knowledge** | The organization's curated, durable memory. |
| **Tool** | External capability an Employee may invoke. It never decides. |
| **Project** | A bounded collaboration toward a goal. |
| **Task** | A unit of work. Short-lived, executable. |
| **Commitment** | A persistent responsibility that outlives tasks. |
| **Trigger** | What decides an Employee should wake. |
| **Runtime** | The engine that manages lifecycle, never reasoning. |
| **Event** | The immutable record of what happened. |
| **Memory** | An Employee's working context, restored each wake. |

The full definitions — purpose, responsibilities, what each owns, lifecycle, and
future extension — live in the **[Architecture Handbook](handbook/README.md)**,
which is the constitution of this operating system.

## Current status

The Architecture Handbook is at **v0.2 (Draft)**, and the roadmap's milestones
have been **built end-to-end through Phase 7** — the vision in the handbook is
now a running system, not just a draft.

**v0.3.0 — a resident service, many frontends.** The backend now runs as
**`oserver`**, a local service (HTTP API on 127.0.0.1) that owns the Runtime:
Employees keep working whether or not any window is open. The desktop app is
now *one frontend among many* — anything that speaks HTTP can drive the same
backend.

- A **resident service architecture**: `ocore` (pure-Rust core: Runtime,
  scheduler, event bus, GBrain capabilities) + `oserver` (axum HTTP API with
  token auth) + the **Tauri v2 desktop shell** as a frontend.
- **Service lifecycle, two modes**: install the boot-time service (Employees
  run from power-on; closing the app changes nothing) or run without it (the
  service starts and stops with the app). Windows is fully implemented and
  verified; Linux (systemd) and macOS (launchd) are implemented but not yet
  verified on real machines.
- A **knowledge-graph foundation** built on [GBrain](https://github.com/garrytan/gbrain) — turn everyday files (contacts CSVs, meeting PDFs, company write-ups) into linked, queryable notes; sync, ask, and reason over them through a GUI instead of the CLI.
- A full **Agent-OS runtime** (Phases 1–7): an Employee lifecycle engine driven
  by Triggers; durable **Artifacts** and **Commitments** persisted in SQLite;
  **Templates → Instances** (define once, deploy many); **shared Brains** so one
  upgrade reaches every Employee; **Teams, Projects, and Task handoff** for
  multi-Employee collaboration; and a **conversational layer** with human–agent
  chat, message-driven waking, and a live observation panel.
- **Email in/out** via [obridge](obridge/) (bundled): inbound mail wakes the
  matching Employee through the event ingress; Employees reply through the
  send tool. IM works through WASM plugins.
- A first **agent entry point**: launch and monitor [Claude Code](https://claude.com/claude-code) from inside the workspace.

## Tech stack

**Frontend:** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router · vue-i18n · lucide-vue-next
**Core & service:** Rust — `ocore` (domain core) · `oserver` (axum service) · `obridge` (mail/WASM bridge)
**Desktop shell:** Tauri v2 (window + desktop-specific capabilities; all logic lives in the service)

## Prerequisites

To use the current knowledge-graph features, the desktop app expects:

| Tool | Why | Install |
|---|---|---|
| **git** | the sync flow commits before updating the graph | <https://git-scm.com/downloads> |
| **bun** | `gbrain` is installed and run through bun | <https://bun.com/docs/installation#installation> |
| **gbrain** | the GBrain knowledge-graph engine | <https://github.com/garrytan/gbrain> |

Paths are auto-detected (e.g. `~/.bun/bin/gbrain.exe` on Windows) and can be
overridden on the **Config** page.

## Install & run

**For most users — grab the prebuilt installer.** Download the latest build for
your platform from the
[**Releases** page](https://github.com/ascetic168/Operoid/releases) and run it.
No need to `git clone` or build from source unless you intend to develop Operoid.

### Installing the boot-time service (Linux / macOS)

On Linux and macOS the boot-time service starts **before any user logs in**
(systemd system unit in `/etc/systemd/system`, or a launchd LaunchDaemon in
`/Library/LaunchDaemons`). Notes for installing it:

- Install it **from your own user account via `sudo`** (e.g. `sudo oserver
  install`). The privileged step only writes the unit file; the service itself
  runs as **the user who installed it** (`User=` / `UserName`), so the SQLite
  DB, `app-settings.json`, and gbrain/obridge files keep the same owner as the
  desktop app — no permission conflicts with the GUI.
- If the installer cannot determine the invoking user (e.g. run from a pure
  root shell), it refuses with an error — install via `sudo` from your account
  instead.
- `HOME` is set explicitly for the service (Linux unit), so bun/gbrain
  convention paths resolve to your home, not `/root`.
- Remove it with `sudo oserver uninstall`.
- Linux/macOS service paths are implemented but **not yet verified on real
  machines** (Windows is).

### For developers (build from source)

Building the desktop app needs the **Rust toolchain** and the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/ascetic168/Operoid.git
cd Operoid
npm install          # install dependencies
npm run tauri dev    # run the app (hot reload)
npm run tauri build  # build a distributable installer
```

Frontend only (in a browser at http://localhost:1420): `npm run dev`,
`npm run build`.

## Development

```bash
npm run tauri dev             # full app, hot reload
npm run build                 # frontend typecheck + build
cd src-tauri && cargo test    # Rust unit tests
cd src-tauri && cargo check   # fast backend typecheck
```

## Project structure

```
src/              Vue 3 frontend (views, Pinia stores, i18n, HTTP wrappers)
                  — Brains, Factories, Config, Employee templates/instances,
                    Employee chat, Operations (live console), Inbox
ocore/            Rust domain core (zero Tauri deps)
                    domain · runtime · scheduler · event_bus · agents state
                    gbrain capabilities (cli/brains/factories/converters) · llm
oserver/          The resident service — axum HTTP API (token auth)
                    agent-os read/write · GBrain domain · operations console
                    event ingress /event · service install (Win/Linux/macOS)
src-tauri/        Desktop shell (Tauri v2) — window, desktop-only features
                    (Claude Code, note preview), command thin-layer, service
                    supervision (start-with-app / stop-with-app)
obridge/          Email bridge + WASM plugin host (IMAP in / SMTP out)
ocontract/        Shared contract types (Operoid ↔ obridge)
handbook/         The Architecture Handbook — the constitution (EN + 中文)
```

## Roadmap

The roadmap is laid out in the handbook, ordered by dependence. All five
milestones have been **implemented through Phase 7**:

1. ✅ **One Employee that truly works** — wake on a trigger, restore context, invoke a tool, commit an artifact, sleep.
2. ✅ **Persistence & Commitments** — work survives a full shutdown and restart (SQLite).
3. ✅ **Shared Brains & Knowledge** — upgrade one Brain, watch many Employees adopt it.
4. ✅ **Templates & Instances** — one template, many independent employees.
5. ✅ **Collaboration** — teams of Employees completing a Project together (Teams + Projects + Task handoff).

Phase 7 added the **human-collaboration layer**: commitments handed off to a
human, the Message concept, conversational chat, and error resilience. See
[Chapter 21 — Roadmap](handbook/21-Roadmap.md) for the full picture and what
comes next.

## License

Released under the **[MIT License](LICENSE)**.
Copyright © 2026 朱國棟 (Charlie Chu). See [LICENSE](LICENSE) for the full text.
