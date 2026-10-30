# Service and Updates

> Applies to: v0.5.11. This chapter covers the lifecycle of the Personal Edition's local service (oserver) and version updates; for enterprise service deployment see [Chapter 04](04-install-enterprise.md).

## How the local service runs

The Operoid desktop app is only the shell; what actually runs employees is the **local service oserver** (listening on `127.0.0.1:7340`, with a token generated and saved automatically). There are two run modes, decided by the **"Start Operoid service at boot"** toggle in "Settings → Services":

| Mode | Behavior |
|---|---|
| **Not registered** (default) | The service starts when the app opens and stops together with it — nothing stays resident in the background |
| **Registered** (boot service) | Starts at boot and stays resident; closing the app does not affect it — **employees keep working** (scheduled tasks and tasks arriving by email are still processed) |

You can check the status anytime under "Settings → Services → Local service (oserver)" (running / not running, port).

Command-line equivalents (advanced):

```bash
oserver install     # register the boot service (requires administrator on Windows; goes through a UAC elevation prompt)
oserver status      # {"installed": true, "running": true}
oserver uninstall   # remove
oserver version     # version + build hash + build time
```

Platform details: on Windows it is the SCM service "Operoid" (verified on a real machine); on Linux systemd, on macOS launchd (install with `sudo oserver install`; the service runs as the installing user — see [DEPLOYMENT.md §11.3](../../DEPLOYMENT.md)). On upgrade/uninstall the install wizard stops/restores the service automatically.

## API keys under service mode

The boot service runs as the system service account and **cannot see your account's environment variables**. When the service is started/registered, Operoid snapshots the provider API keys present in your environment at that moment into the settings file — so: **after changing or adding an API key, restart the app (or re-register the service)** for it to take effect. In the Enterprise Edition, keys are written directly to `[llm] env` in `operoid.toml`.

## Version & updates

"Settings → Application → Version & updates":

| Toggle | Default | Behavior |
|---|---|---|
| **Update notices** | On | On startup (and every 24 hours) checks [GitHub Releases](https://github.com/ascetic168/Operoid/releases); a banner appears when a new version is available, but **installation always requires your confirmation** |
| **Automatic updates** | Off | As soon as a new version is detected it is downloaded and installed in the background (minisign signature verified); you are prompted to relaunch when done |

⚠️ This project **does not guarantee backward compatibility between versions** — that is exactly why "Automatic updates" is off by default; evaluate before turning it on, and get in the habit of reading the breaking entries in the [CHANGELOG](../../CHANGELOG.md).

When an update is not ready yet (for example the release pipeline for the new version is not in place), the page tells you to try again later or to download manually from the Releases page.

### Enterprise differences

The Enterprise Edition **never auto-updates**: the "Version" card in `/admin` can check for new versions (results are cached for 24 hours; air-gapped environments can set `[update] check_enabled = false` in `operoid.toml` to disable outbound connections). Upgrades are manual overwrite installs (back up the data directory first); for the full procedure see [DEPLOYMENT.md §12](../../DEPLOYMENT.md).

---
[← 14 Email Bridge](14-email-bridge.md) | [Index](README.md) | [16 Enterprise Consoles →](16-enterprise-consoles.md)
