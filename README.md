# GBrainStudio

**English** | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md)

**GBrainStudio** is a friendly desktop companion to
[GBrain](https://github.com/garrytan/gbrain). Drop in your everyday files and
get back a connected, queryable knowledge graph — without hand-writing links or
touching the command line.

Built with **Tauri v2 (Rust)** + **Vue 3 + TypeScript**.
**Author:** 朱國棟 (Charlie Chu) · **License:** [MIT](#license)

---

## Why GBrainStudio?

[GBrain](https://github.com/garrytan/gbrain) is a powerful knowledge-graph
engine — but it is CLI-first and expects **hand-authored notes**. Every
cross-reference has to be a correctly-formatted, correctly-named link, or it
silently fails to connect in the graph. Building a corpus that way is slow and
unforgiving, and daily work means memorizing commands and reading terminal
output.

**GBrainStudio takes the friction out:**

- 📥 **Drop a file, get a linked note.** Feed in a contacts CSV, a meeting-minutes
  PDF, or a company write-up — GBrainStudio turns it into a valid note with all
  cross-references generated for you, so links actually resolve in the graph.
- 🔗 **Never hand-write a link.** Mention a person or company and it is
  auto-connected to the graph; you write plain prose, the app writes the links.
- 🖱️ **A GUI for the whole workflow.** Run sync / ask / think with live streamed
  output; click any reference in an answer to open the source note in your
  browser.
- 🧠 **Multiple knowledge graphs, visualized.** Create, register, and sync
  isolated brains and their sources without ever opening a terminal.

In short: GBrainStudio turns GBrain from a power-user CLI into an approachable
tool for **building and exploring** your knowledge graph.

## What is GBrain?

[GBrain](https://github.com/garrytan/gbrain) is the underlying knowledge-graph
engine: your notes become a queryable graph of people, companies, meetings, and
concepts. Head to its repository for the engine itself.

> ℹ️ **GBrainStudio is a separate companion GUI — it is *not* GBrain.** It drives
> the `gbrain` CLI for you; the engine, graph storage, and retrieval all remain
> GBrain's.

## Features

### 🏭 Factories — turn source files into linked notes

Drag a file onto a card (or click to pick one) and it is converted and written
straight into your notes:

| Factory | Accepts | Becomes |
|---|---|---|
| **people** | CSV / TXT / MD | people notes (CSV = contacts; TXT/MD = one person, LLM-structured) |
| **companies** | TXT / PDF | company notes (LLM-structured) |
| **meeting** | TXT / MD / PDF | meeting notes (LLM-structured) |
| **inbox** | TXT / MD | a quick capture |

- **Batch drop:** drop several files at once; a results list shows each file's status — click any one to preview and edit it before syncing.
- **Source-aware:** notes land in the right place — the active brain's active
  source repo — so a one-click **Sync to brain** picks them up.
- **Authoring editor (`+`):** start from a template and write naturally; on save,
  the names you mention are linked into the graph for you.

### 🔧 Operations — run GBrain without the terminal

Wrap the `gbrain` CLI; output streams live into a console.

- **stats · sync · extract** — keep the corpus healthy.
- **ask · think** — ask questions and get multi-hop, cited answers (`think`
  accepts an optional `anchor:` to focus on a note).
- **Diagnostics** — `doctor`, `orphans`, `storage`, `graph-query`.
- **Rebuild companies** — regenerate company notes from your people notes.
- **Clickable references:** any `[[people/JLin]]` / `[people/JLin]` tag in an
  answer is highlighted; click it to read that note in your browser.

### 🧠 Brains — manage multiple knowledge graphs

`gbrain` has no "list all brains" command, so GBrainStudio gives you one:

- Register an existing brain or create a new one.
- Give each brain multiple **sources** (git repos) and sync per-source or all at
  once.
- Switch brains and everything — config, sources, the active target — follows.

### ⚙️ Config — one place for every setting

Edit GBrain's authoritative `config.json` (models, embeddings, providers) and
the app's own settings (paths, output folders, sync flags, language) side by
side.

### And more

- **Startup check** — verifies `git` / `bun` / `gbrain` and points you to
  installers for anything missing.
- **Three languages** — Traditional Chinese, Simplified Chinese, English, with
  automatic detection and a manual override.

## Prerequisites

| Tool | Why | Install |
|---|---|---|
| **git** | the sync flow commits before updating the graph | <https://git-scm.com/downloads> |
| **bun** | `gbrain` is installed and run through bun | <https://bun.com/docs/installation#installation> |
| **gbrain** | the GBrain engine itself | <https://github.com/garrytan/gbrain> |

Paths are auto-detected (e.g. `~/.bun/bin/gbrain.exe` on Windows); override them
on the **Config** page if needed.

## Install & run

> Building the desktop app needs the **Rust toolchain** and the
> [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install          # install dependencies
npm run tauri dev    # run the app (hot reload)
npm run tauri build  # build a distributable installer
```

Frontend only (in a browser at http://localhost:1420): `npm run dev`,
`npm run build`.

## Quick start

1. **Launch** GBrainStudio — a dialog lists anything still missing.
2. **Config** — confirm the notes folder and `gbrain` path.
3. **Brains** — pick or create a brain, add a **source** (a git repo for your
   notes), and set it active.
4. **Factories** — drag a file onto the matching card, then **Sync to brain**.
5. **Operations** — run `think` with a question; click any highlighted name to
   open that note.

## Configuration

Stored via `tauri-plugin-store` in app data:

| Field | Meaning |
|---|---|
| `notes_repo_path` | fallback notes folder; factories prefer the active source's folder |
| `gbrain_exe_path` | path to the `gbrain` executable |
| `factory_targets` | output sub-folders (`people` / `companies` / `meetings`) |
| `auto_sync` | commit + sync automatically after a factory write |
| `sync_no_pull` | pass `--no-pull` (recommended for brains with no remote) |
| `llm_temperature` / `llm_max_tokens` | sampling for factory LLM structuring |
| `locale` | UI language (`null` = auto-detect) |

> A note's file lives in the **active brain's active source repo**. Clicking a
> reference searches that source first, then the others, then the fallback
> folder — so you always open the right file.

## Project structure

```
src/              Vue 3 frontend (views, Pinia stores, i18n, typed IPC wrappers)
src-tauri/src/    Rust backend (config, converters, factories, gbrain_cli,
                  brains, note_view, llm, prereq, i18n)
```

## Tech stack

**Frontend:** Vue 3 · TypeScript · Vite · Tailwind CSS v4 · Pinia · Vue Router ·
vue-i18n · lucide-vue-next. **Backend:** Tauri v2 · Rust.

## Development

```bash
npm run tauri dev             # full app, hot reload
npm run build                 # frontend typecheck + build
cd src-tauri && cargo test    # Rust unit tests
cd src-tauri && cargo check   # fast backend typecheck
```

## Troubleshooting

- **`think` / `ask` say "(no LLM available)"?** `gbrain think` picks its model
  from `models.think → models.default → GBRAIN_MODEL → opus`. `gbrain init
  --chat-model X` sets `chat_model` but **not** `models.default`, so `think` can
  silently fall back to Anthropic Opus. Fix it on the brain:
  `gbrain config set models.default <provider:model>`
  (e.g. `groq:llama-3.3-70b-versatile`). (GBrainStudio's own factory structuring
  reads `chat_model` directly and is unaffected.)
- **Clicking a reference says "note not found"?** The note doesn't exist in any
  source of the active brain, or its name differs in case. GBrainStudio tries an
  exact match, then a case-insensitive scan — check the file really exists under
  the active source.

## License

Released under the **[MIT License](LICENSE)**.

Copyright © 2026 朱國棟 (Charlie Chu). See [LICENSE](LICENSE) for the full text.
