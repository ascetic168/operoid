# Managing Employees: Templates and Instances

> Applies to: v0.5.11. Employee features require turning on "Settings → Application → Enable Agent-OS (employee templates/instances)" first. Concept reading: [Architecture Handbook Ch.04 Employee](../../handbook/04-Employee.md).

## The two-layer model: template → instance

- **Employee template** — the definition of a job: which brain it maps to, the job description, which tools are allowed. Reusable.
- **Employee instance** — "that employee" deployed from a template: its own name, its own state and history. One template can deploy multiple instances that never interfere with each other.

## Employee templates (the "Templates" page)

Press "Add Template":

| Field | Description |
|---|---|
| Name | e.g. "Procurement Assistant" |
| Brain | the knowledge source this kind of employee checks against when answering (one template maps to one brain) |
| Role (optional) | a machine-readable role code, e.g. `procurement` |
| Job description (optional) | job description copied to every instance on deploy, e.g. "Weekly report digest and vendor inquiries" |
| Tool allowlist | **tools left unchecked are invisible to the employee** — the most basic delegation boundary |

### The tool allowlist

- **Default set**: think (deep reasoning) / search (retrieval) / send (communication) — the basic knowledge-and-communication bundle.
- **Allow write-note**: the employee can write its output to a dedicated output directory (default `~/employee-output`, not in the graph; after human review, move it into the source repo and promote it via sync).
- **Allow reading / writing / editing workspace files**: report drafts, CSVs and other content in the employee's own workspace; edit is exact string replacement (a read is required first).
- **Allow run-command in the workspace** (advanced): data processing, running company scripts — evaluate carefully before granting.

Templates can be renamed and deleted; "Template Detail" shows the creation time and the settings.

## Employee instances (the "Instances" page)

"Deploy Instance": pick the source template, fill in the instance name (e.g. `Steve@TW`), and optionally an external email (used for routing by the [Email Bridge](14-email-bridge.md)).

**Right-click** an instance card to manage it:

| Action | Description |
|---|---|
| "Chat…" | open a chat (see the next section) |
| "Message…" | send a message to this employee (it will be woken up to process it) |
| "Watch…" | open the watch panel (see the next section) |
| "Delegate…" | create a commitment: give a **commitment title** (e.g. "Track the etching-yield incident to closure") and a **completion condition** (e.g. "A report with an action timeline and financial impact is produced") — once created, the employee wakes immediately and works until it is done |
| "Open output directory" | open this employee's output folder |
| Rename / "Delete Instance" | — |
| "Archive…" | history (chat/events/artifacts) is kept and the employee simply stops being woken up; "Unarchive" anytime. Archived employees are grouped below the list and remain traceable |

## Chat

- You can watch it work: between your question and the Markdown reply, **every tool call** is listed — tool name, arguments, elapsed time; answers stream out in real time.
- "{n} tool call(s)" can be expanded to view the full trace; "Thinking" means it is reasoning.
- **"Clear conversation"** only clears the messages — work outputs (Artifacts) and commitments (Commitments) are kept.
- Pressing "Stop" while the employee is working is a **cooperative stop**: it stops gracefully at the next step boundary; send a message or delegate to resume.

## The watch panel

"Watch…" opens this employee's live dashboard:

- **"Pending proposals"** / **"Auto-activated (registered categories)"** — which actions are waiting for you, and which are auto-activated via the Registry;
- **Spot review** — mark auto-activated commitments as "Classified correctly" / "Misclassified", feeding the divergence-rate statistics;
- **Active commitments, current tasks, recent artifacts, lifecycle events, resolved commitments, completed tasks**.

## Subagents

Since v0.5.7, an employee can spawn **subagents (`spawn_subagent`)**: a one-off, read-only research context used to look up information and read documents, then bring the conclusions back to the parent task — it cannot write anything. The step fuse defaults to 20 steps (`subagent_max_steps`, see [Chapter 17](17-config-reference.md)).

## Communication between employees

Employees have internal mailboxes (they can message each other and look up the roster of colleagues), controlled by the `internal_mail_enabled` master switch in "Settings → Application" (on by default; off = the tool does not exist). In the enterprise edition, users can also talk to **colleagues'** employees from their own console (since v0.5.9).

---
[← 07 Brains and Models](07-brains-and-models.md) | [Index](README.md) | [09 Document Factories →](09-factories.md)
