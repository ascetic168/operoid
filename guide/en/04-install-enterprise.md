# Installing the Enterprise Edition

> Applies to: v0.5.11. This chapter is an **overview and glossary** of Enterprise Edition deployment; the authoritative step-by-step procedure is [DEPLOYMENT.md](../../DEPLOYMENT.md) (it ships with the deployment package and is identical to the copy in the repository).

## Deployment Package Contents

Download the deployment package for your platform from [GitHub Releases](https://github.com/ascetic168/Operoid/releases) and extract it on the server machine — **the server does not need Node/pnpm**:

| Platform | File |
|---|---|
| Windows x64 | `operoid-enterprise-windows-x64.zip` |
| Linux x64 | `operoid-enterprise-linux-x64.tar.gz` |

Package contents: the `oserver` executable (HTTP service + Employee execution engine), the `obridge` executable (email / IM bridging), the static files for the three role frontends (`frontends/{admin,manager,user}`), `DEPLOYMENT.md`, `operoid.example.toml`, and `deploy/local/mm-bridge/` (the multimodal embedding translation layer, optional).

## Server Prerequisites

| Item | Description |
|---|---|
| GBrain | Installed on the server machine (`gbrain` CLI); Brains (GBRAIN_HOME) and the notes repo live on the same machine |
| Embedding service | A local llama.cpp `llama-server` (port 8080, EmbeddingGemma 2, 768 dimensions) — a **hard dependency** of GBrain embedding/query, must stay resident |
| TLS certificate | Issued by the internal CA (preferred) or self-signed; non-loopback listeners **must** configure TLS, and a half configuration (cert only or key only) is refused at startup (fail closed) |
| LLM API keys | Written to `[llm] env` in `operoid.toml` — the service account cannot see users' environment variables |
| Multimodal (optional) | Image embedding requires deploying the `deploy/local/mm-bridge/` translation layer (a single bun file, 8090 → 8080) |

## The Deployment Wizard: `oserver configure`

```bash
./oserver configure --data-dir /opt/operoid/data        # Linux
.\oserver.exe configure --data-dir C:\operoid           # Windows
```

The wizard proceeds in order (Enter accepts the default, `?` shows help, and a summary confirmation gate runs before anything is written):

1. **Prerequisite check** — probes git / bun / gbrain and gives install guidance for anything missing.
2. **Deployment mode** — personal (local loopback) or enterprise (intranet service).
3. **Listen and frontends** — bind host/port, the frontends directory, TLS cert/key.
4. **Knowledge Brain working directory** — the gbrain path and `gbrain_home`; you can create a new Brain on the spot with `gbrain init`.
5. **Master token** — a 64-character value generated on the spot (shown in full only this once).
6. **Administrator account** — defaults to `admin` / `admin1234`; a password change (at least 8 characters) is **forced** on first login.
7. **System service** — optionally register boot autostart right away.

⚠️ Always finish the wizard before registering the service — on its first run in an environment without a terminal, the service will not pop up the wizard by itself; it will fail to start outright.

## Service Registration

```bash
./oserver install      # 註冊開機服務（Windows 為 SCM 服務「Operoid」，需系統管理員）
./oserver status       # {"installed": true, "running": true}
./oserver uninstall    # 移除
```

Windows is the field-verified path; Linux (systemd) / macOS (launchd) are implemented — for installation caveats see [DEPLOYMENT.md §11.3](../../DEPLOYMENT.md). The firewall must allow port 7340 (commands in §5).

## First Login

1. Open `https://<伺服器IP>:7340/` in a browser — the entry page for the three frontends.
2. Log in to `/admin` with the administrator account created by the wizard; on first login the system forces you to set a new password.
3. Create manager / user accounts under "Accounts"; day-to-day operation of each role is covered in [Chapter 16](16-enterprise-consoles.md).

## Upgrading

The Enterprise Edition **does not auto-update**: the "Version" card in the admin console can check for a new version (results are cached for 24 hours; air-gapped environments can turn off the outbound connection with `[update] check_enabled = false` in `operoid.toml`). Upgrading is a zip over-install: stop the service → **back up the data directory** → extract the new version over it → start the service → verify the version at `/healthz`. For the full procedure and caveats see [DEPLOYMENT.md §12](../../DEPLOYMENT.md).

## DEPLOYMENT.md Section Map

| What you want to do | See |
|---|---|
| Architecture overview | §1 |
| Server prerequisites, embedding service launch command | §2, §10.1 |
| Every field of `operoid.toml` | §3 (see also this guide's [Chapter 17](17-config-reference.md)) |
| TLS certificates | §4 |
| Service registration, firewall | §5 |
| Account provisioning (including the manual flow without the wizard) | §6 |
| obridge email bridging | §7 (this guide's [Chapter 14](14-email-bridge.md)) |
| Post-deployment acceptance checklist | §8 |
| Deployment troubleshooting | §9 (this guide's [Chapter 18](18-troubleshooting.md)) |
| PDF knowledge pipeline, MinerU | §10 |
| Upgrading | §12 |

---
[← 03 Installing the Personal Edition](03-install-personal.md) | [Index](README.md) | [05 Quick Start →](05-quickstart.md)
