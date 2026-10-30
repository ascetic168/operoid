# Brains and Models

> Applies to: v0.5.11. For the concept and role of a Brain, see [Appendix A](appendix-a-gbrain.md); this chapter is about the operations.

## The "Brains" page

One brain = one GBrain knowledge base (on disk, a `.gbrain` directory together with its parent directory). Every employee knowledge query happens on the brain mapped by its template.

### Registration and switching

- **Add** → two choices: "Register existing" (point at the parent directory of an existing `.gbrain`) or "Create new brain (gbrain init)".
- When creating a new brain you can specify: name, path, embedding model and dimensions, chat model — leave blank to use the defaults (embedding default `llama-server:embeddinggemma-2`, 768 dims; changeable via "Default embedding model for new brains" in "Settings → Models", affects newly created brains only).
- **"Set active"** — factory writes and Knowledge Base operations all target the active brain.
- The default brain cannot be removed.

### Sources

A brain's knowledge comes from one or more **git repo** sources (the pages factories write to live in the source repo):

- "Add source (git repo)": fill in the source id (a short name, e.g. `notes`) and the git repo path.
- List columns: id, Path, Pages, Last sync; each row offers "Sync" and "Set active".
- "Sync all": runs sync against all sources (commit + sync + embed + extract).
- "Bind folder": bind the source to a local path.
- When **removing a source** you choose one of two: **"Archive (kept 72h, restorable)"** (hidden from search, kept for 72 hours, restorable) or **"Delete permanently (irreversible)"** (removes all pages/vectors/files immediately, irreversible).

## "Settings → Models"

GBrain's `config.json` is the source of truth; the controls on this page write to it directly (and to the DB plane):

| Section | Content |
|---|---|
| **Main model** | One `provider:model` (e.g. `zhipu:glm-5.3-flash`) applied in one go to chat and the four tiers |
| **Model routing (tiers)** | Graded by task: Utility (light tasks such as synopses), Reasoning (thinking/extraction), Deep (deep analysis), Subagent (subagent loop). "Use one model for all tiers" is available |
| **Provider base URLs** | Endpoint overrides written directly to config.json (file-plane) |
| **Default embedding model for new brains** | `provider:model` + dimensions; affects newly created brains only |
| **gbrain transport** | `MCP` (default — employee knowledge tools go through `gbrain serve`, auto-fallback to CLI on failure) or `CLI` (pure subprocess) |
| **LLM endpoint resolution** | Shows the API key status ("API key set" / "Missing API key (env var)") |

Notes:

- **DB plane overrides**: some keys may take effect via the DB plane (the page lists the keys for which "the DB plane is the effective fallback") — to change models, use the controls on this page; do not edit `config.json` directly; "Clear all DB overrides" resets them to zero.
- **Schema pack hint**: if the active brain is still on v1 (6 factories), the page suggests running the "unify-types" migration to v2 (15 canonical types) — see [Chapter 09](09-factories.md).
- Only Anthropic models support prompt caching on the Subagent tier; with other providers the subagent loop's cost grows linearly with conversation length (the page warns about this).

## "Settings → Services"

- **Knowledge pipeline capabilities**: the capability matrix (git/bun/gbrain/MinerU/embedding endpoint/multimodal/VLM) + "Recheck". MinerU's **install directory** (`mineru_dir`, supports `%USERPROFILE%`/`${HOME}` expansion, portable across machines) can be filled in here; saving re-runs detection immediately.
- **Local service (oserver)**: port (default 7340), running/stopped, registering and unregistering **"Start Operoid service at boot"** — see [Chapter 15](15-service-and-updates.md).
- **Obridge**: config path, executable, start with the app — see [Chapter 14](14-email-bridge.md).

## Other settings tabs

- **Application**: interface language, paths (notes repo, gbrain.exe, GBRAIN_HOME override), LLM temperature/max_tokens, factory auto-sync, the Agent-OS switch, Obridge, Version & updates — spread across [Chapter 15](15-service-and-updates.md) and [Chapter 17](17-config-reference.md).
- **Registry**: the Action Registry (the delegation boundary) — see [Chapter 13](13-governance.md).
- **Knowledge**: knowledge policy, principals, TTL grants — see [Chapter 13](13-governance.md).
- **Advanced**: raw `config.json` editor. ⚠️ model/tier keys may be overridden by the DB plane — use the controls on the "Models" tab to edit them instead.

---
[← 06 Desktop App Tour](06-desktop-tour.md) | [Index](README.md) | [08 Managing Employees →](08-employees.md)
