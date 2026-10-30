# Configuration Reference

> Applies to: v0.5.11. This chapter is a quick reference for every settings file and environment variable; for step-by-step enterprise server configuration, [DEPLOYMENT.md §3](../../DEPLOYMENT.md) is authoritative.

## Settings files at a glance

| File | Location | Format | Who uses it |
|---|---|---|---|
| `app-settings.json` | Settings directory (see below) | JSON | Base settings shared by Personal + Enterprise (`{"app_config": {...}}`) |
| `operoid.toml` | Same settings directory | TOML | **Presence = enterprise mode** (personal mode has no such file and its behavior is unchanged) |
| `obridge.toml` | Wherever you specify (`obridge_config_path`) | TOML | obridge channel settings; the GUI can edit it directly like an editor |
| `config.json` | Next to each brain's `.gbrain` | JSON | GBrain's model settings — the authority behind "Settings → Models" |

**Settings directory**: Windows `%APPDATA%\com.operoid.studio`; macOS `~/Library/Application Support/com.operoid.studio`; Linux `~/.config/com.operoid.studio`.

## Common app-settings.json fields

The following fields in the `app_config` block are the ones relevant to daily use (full structure in `ocore/src/app_config.rs`):

| Field | Default | Description |
|---|---|---|
| `agent_os_enabled` | `false` | Master switch for the employee subsystem (employee templates / instances) |
| `brains` + `active_brain_id` + `active_source_id` | — | Brain registry and the active entries (managed on the GUI "Brains" page) |
| `gbrain_exe_path` | `~/.bun/bin/gbrain.exe` | Path to the gbrain executable |
| `gbrain_transport` | `"mcp"` | Transport for employee knowledge queries: `mcp` (falls back to the CLI automatically on failure) or `cli` |
| `default_embedding_model` + `_dimensions` | Built-in 768 dimensions | Initial embedding values for **newly created brains** |
| `employee_output_path` | `~/employee-output` | Sandbox root where employees "write notes" |
| `auto_sync` + `sync_no_pull` | `true` + `true` | Auto commit+sync after factory writes; sync runs with `--no-pull` |
| `llm_temperature` / `llm_max_tokens` | 0.2 / 4096 | LLM structured sampling |
| `turn_max_steps` | 40 | Fuse on the tool loop per conversation turn |
| `autonomy_max_cycles` / `autonomy_max_minutes` | 25 / 15 | Autonomous work budget |
| `subagent_max_steps` | 20 | Fuse on subagent steps |
| `skills_personal_dir` | `~/.agents/skills` | Personal skills area; **empty string = enterprise mode** (isolated per principal) |
| `skills_union_limit` | 30 | Upper limit on the skill-loading union (`0` = unlimited) |
| `factory_concurrency` | 4 | Number of files converted concurrently |
| `event_ingress_port` / `event_ingress_secret` | Unset | Ingress for external events (`POST /event`) — **off by default**; setting a port without a secret prevents startup |
| `event_outbound_url` / `event_outbound_secret` | Unset | Outbound endpoint for employee replies (unset = replies stay inside Operoid only) |
| `obridge_config_path` / `obridge_executable` / `obridge_autostart` | Unset | The Obridge trio (see [Chapter 14](14-email-bridge.md)) |
| `server_autostart` / `server_port` / `server_token` | `true` / 7340 / auto-generated | Local service |
| `llm_env` | `{}` | Snapshot of provider keys for the service (the service account cannot see user environment variables) |
| `update_notify` / `auto_update` | `true` / `false` | Update notices / automatic updates |
| `allow_public_egress` | `false` | Master gate for egress to public third-party clouds (mineru.net etc.) |
| `mineru_command` / `mineru_dir` / `mineru_tier` | Unset | The MinerU access ladder (supports `%VAR%` / `${VAR}` expansion) |
| `claude_terminal` etc. | — | Claude Code integration preferences |

## operoid.toml (enterprise mode)

Lives at `<data directory>/operoid.toml` (**its presence means enterprise mode**); the template `operoid.example.toml` ships with the deployment package:

| Section | Contents |
|---|---|
| `[server]` | host, port (7340), master token, frontends_dir — **non-loopback ⇒ `[tls]` required** |
| `[tls]` | cert + key (PEM); configuring only half is refused at startup (fail closed) |
| `[llm.env]` | provider API keys (the service process cannot see the user's environment) |
| `[gbrain]` | `exe_path` override (optional) |
| `[knowledge]` | MinerU ladder + egress tiering (optional) |
| `[ingress]` | obridge delivery port/secret (optional) |
| `[factory]` | conversion concurrency cap, default 4; when saturated, at most 1 slot per remote user (optional) |
| `[update]` | `check_enabled = false` disables new-version checks (air-gapped environments; optional) |

**Precedence**: `OSERVER_*` environment variables > `operoid.toml` > `app-settings.json`. **Hot-applied**: `[llm] env` / `[gbrain]` / `[ingress]` / `[factory]`; **restart required**: host / port / `[tls]` / frontends_dir.

## obridge.toml

```toml
[operoid]            # ingress destination: Operoid's /event and the shared secret
ingress_url = "http://127.0.0.1:17341/event"
ingress_secret = "<password A>"

[listen]             # obridge's outbound receiving endpoint (employee replies leave from here)
port = 17401
secret = "<password B>"

[[channels.email_imap]]   # Email channel (multiple allowed)
# IMAP/SMTP account, polling interval
# routes: recipient address → employee_id (synced automatically after an employee binds its "external email")
# senders: employee_id → From address

[[channels.<plugin>]]     # WASM plugin channel (see Appendix B)
```

`[[channels]]` and plugin settings **hot-reload** (checked every 2 seconds); `[listen]` / `[operoid]` changes require a restart.

## GBrain config.json and the DB plane

Per-brain model settings: `chat_model` (`provider:model`), `models` tiers, `provider_base_urls`, `embedding_model`. The controls under "Settings → Models" write to both the file plane (config.json) and the DB plane (the gbrain database) — **at runtime the file plane takes precedence**; hand-editing config.json directly can end up shadowed by DB-plane overrides, so change models through the GUI; "Clear all DB overrides" resets everything to zero.

## Environment variables

| Variable | Purpose |
|---|---|
| `OSERVER_TOKEN` / `OSERVER_PORT` | Override the master token / listening port (takes precedence over operoid.toml) |
| `GBRAIN_HOME` | Home directory of brains (default `~/.gbrain`); can be overridden per brain on the GUI "Brains" page |
| `ZHIPUAI_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `TOGETHER_API_KEY`, `OPENROUTER_API_KEY`, `DASHSCOPE_API_KEY`, `ZEROENTROPY_API_KEY` | API key for each provider (ollama needs no key) |

## Data and log locations

| Contents | Location |
|---|---|
| Brains (pages, graph, vectors) | PGLite + the source git repo under `GBRAIN_HOME` (default `~/.gbrain`) |
| Employee/template/commitment/proposal DB (SQLite) + the `skills/` tree | Data directory (Windows: local AppData `com.operoid.studio`) |
| obridge log | `obridge.log` under the data directory |
| Employee output | `employee_output_path` (default `~/employee-output`) |
| Factory conversion audit | `conversion-report.json` inside the source repo |

Uninstalling the app does not touch any of the data above.

---
[← 16 Enterprise Consoles](16-enterprise-consoles.md) | [Index](README.md) | [18 Troubleshooting →](18-troubleshooting.md)
