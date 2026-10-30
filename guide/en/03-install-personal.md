# Installing the Personal Edition

> Applies to: v0.5.11. Personal Edition = the desktop installer, installed on your own computer — nothing to build.

## Download the Installer

Go to the [GitHub Releases](https://github.com/ascetic168/Operoid/releases) page and find the file for your platform under **Assets** of the latest release:

| Platform | File | Notes |
|---|---|---|
| Windows | `Operoid_*_x64-setup.exe` | **Recommended** — NSIS setup wizard; in-app auto-update installs in this format |
| Windows | `Operoid_*_x64_en-US.msi` | MSI package, suitable for enterprise software deployment |
| macOS (Apple Silicon) | `Operoid_*_aarch64.dmg` | M-series chips |
| macOS (Intel) | `Operoid_*_x64.dmg` | Intel chips |
| Linux | `Operoid_*_amd64.deb`, `Operoid-*_x86_64.rpm`, `Operoid_*_amd64.AppImage` | Pick one for your distribution |

The remaining assets need not be downloaded: `.sig` is the signature file for in-app auto-update, and `latest.json` is the update manifest.

## What Gets Installed

The installer contains the complete set of components — nothing else to prepare:

- The **Operoid desktop app** (Tauri v2);
- The **oserver** local service (the Employee execution engine);
- **obridge**, the bridge to external channels (email / IM).

## Installation Steps per Platform

### Windows

1. Run the `.exe` and follow the setup wizard to completion.
2. If you previously registered "Start Operoid service at boot", the wizard automatically stops/restores that service during install and uninstall (no manual action needed).

### macOS

1. Open the `.dmg` and drag **Operoid** into "Applications".
2. If a macOS security prompt appears on first run, confirm with "Open".

### Linux

```bash
# Debian / Ubuntu
sudo dpkg -i Operoid_*_amd64.deb
# RPM 系
sudo rpm -i Operoid-*_x86_64.rpm
# AppImage
chmod +x Operoid_*_amd64.AppImage && ./Operoid_*_amd64.AppImage
```

On Linux you must first install the Tauri v2 system dependencies such as `libwebkit2gtk-4.1`; the list is in [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## First Launch

1. Open Operoid — it automatically brings up the local service (`127.0.0.1:7340`, token generated automatically; service status is under "Settings → Services").
2. If [prerequisites](02-requirements.md) are missing, a "Missing prerequisites" dialog appears: it lists what is missing among git / bun / gbrain, what each is for, and install links. Install them and reopen Operoid.
3. **Capability warning banners** may appear at the top of the main window (MinerU not installed, embedding service not running, and so on) — missing items do not block use, but they cap knowledge quality; check the tables in [Chapter 02](02-requirements.md) to decide whether to act.

## Updates

The **Version & updates** section under "Settings → Application":

| Toggle | Default | Behavior |
|---|---|---|
| **Update notices** | On | Checks for a new version automatically at startup (and every 24 hours); shows a banner when one exists, and installing requires your confirmation |
| **Automatic updates** | Off | Downloads and installs a new version as soon as it is detected, then prompts you to relaunch |

⚠️ This project **does not guarantee forward compatibility between versions** — evaluate before enabling "Automatic updates"; for major changes between versions see [CHANGELOG](../../CHANGELOG.md).

Update flow: the banner shows "New version vX.Y.Z available" → "Update now" → download (progress shown) → "Updated to vX.Y.Z — relaunch to apply" → press "Relaunch". You can also skip the app and download the installer directly from the [Releases page](https://github.com/ascetic168/Operoid/releases) to install over the existing one (the NSIS wizard handles the service automatically).

## Uninstalling

- **Windows**: uninstall via "Settings → Apps → Installed apps"; uninstalling also removes the registered service.
- **macOS / Linux**: just delete the app (Linux: remove via the package manager).

Settings and data (Brains, Employees, conversation history, and so on) live in the user data directory and are not removed by uninstalling; see [Chapter 17](17-config-reference.md) for the full list of locations.

## Common Installation Problems

App won't open after installing, service not started, components not detected — see [Chapter 18: Troubleshooting](18-troubleshooting.md).

---
[← 02 System Requirements](02-requirements.md) | [Index](README.md) | [04 Installing the Enterprise Edition →](04-install-enterprise.md)
