# Desktop App Tour

> Applies to: v0.5.11. This chapter is the map of the whole desktop app — the chapters that follow then walk through each page one by one.

## Overall layout

- **Top bar**: the notification bell (with a pending-count badge), the "Inbox" entry, and the "Events" entry.
- **Left navigation** (top to bottom): Factories, Templates, Instances, Skill Library, Knowledge Base, Brains, Settings.
- **Bottom of the left navigation**: the "Open Claude Code" button (see [Chapter 12](12-knowledge.md)).
- **Above the main window**: where the capability warning banner and the update banner appear.

The interface language supports Traditional Chinese / Simplified Chinese / English; it auto-detects the system language by default and can be pinned in "Settings → Application → Interface language".

## What each page does

| Page | What it does | Details |
|---|---|---|
| **Factories** | Drop documents → convert → write into the Knowledge Base (images, contacts, meetings, projects, documents…) | [Chapter 09](09-factories.md) |
| **Templates** | Define reusable employee types: mapped brain, job description, tool allowlist | [Chapter 08](08-employees.md) |
| **Instances** | Deploy and manage the employees doing actual work; chat, watch, delegate | [Chapter 08](08-employees.md) |
| **Skill Library** | Browse the built-in skill catalog, manage your personal area and drafts, skill proposals | [Chapter 11](11-skills.md) |
| **Knowledge Base** | Operate GBrain directly: query (ask/query/think), sync, health checks | [Chapter 12](12-knowledge.md) |
| **Brains** | Register/switch brains, manage sources (git repo) and sync | [Chapter 07](07-brains-and-models.md) |
| **Settings** | Six tabs: Application, Models, Services, Registry, Knowledge, Advanced | [Chapter 07](07-brains-and-models.md), [Chapter 13](13-governance.md), [Chapter 15](15-service-and-updates.md), [Chapter 17](17-config-reference.md) |
| **Inbox** (top bar) | Approve employee proposals, keep an eye on flagged employees, delegate commitments | [Chapter 10](10-inbox-and-events.md) |
| **Events** (top bar) | The lifecycle event stream: triggers, wake-ups, completions | [Chapter 10](10-inbox-and-events.md) |

## The notification bell

The bell in the top bar collects **things that need your decision**:

- **"Pending proposals"** — when an employee exceeds its authorization it does not act on its own; it proposes and waits for you to approve (commitments, skill proposals, etc.);
- **"Flagged employees"** — employees whose state needs attention (e.g. repeated failures).

The bell shows a pending-count badge; open the popover for a quick glance, and press "Inbox →" to enter the full list. After launch, this list keeps updating automatically.

## The capability warning banner

When a knowledge-pipeline gap is detected (MinerU not installed, the embedding endpoint down, llama-server missing flags, etc.), a yellow banner appears above the main window explaining the impact and what to do about it:

- **"Recheck"** — press it to re-probe after installing the component;
- **"Dismiss (hidden for this session)"** — skip it for now; if the gap is still there at the next launch, the banner reappears.

For the full description of each gap and the degradation behavior, see [Chapter 02](02-requirements.md).

## The update banner

When a new version exists, the banner advances by state: "New version vX.Y.Z available" → "Update now" → "Downloading vX.Y.Z… (progress)" → "Updated to vX.Y.Z — relaunch to apply". The behavior is controlled by two switches in "Settings → Application → Version & updates"; see [Chapter 15](15-service-and-updates.md).

---
[← 05 Quick Start](05-quickstart.md) | [Index](README.md) | [07 Brains and Models →](07-brains-and-models.md)
