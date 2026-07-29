# Chapter 03 — Workspace

Version: 0.1
Status: Draft

---

## 1. Purpose

The Workspace is the **organization**.

It is the outermost container in Emploid. Everything that exists — every Employee, Brain, Artifact, Knowledge base, Tool, Project, Task, and Commitment — lives inside exactly one Workspace.

The Workspace exists to enforce the most important boundary in the system: the boundary of **what belongs together**. Work, knowledge, authority, and results that belong to one organization never leak into another.

A Workspace does not perform work. It is the place where work happens.

---

## 2. Responsibilities

The Workspace is accountable for:

- **Tenancy** — owning every object inside it and enforcing that nothing exists without a home.
- **Isolation** — keeping its contents separate from every other Workspace.
- **Hosting** — providing the shared environment in which Employees, Tools, and Knowledge coexist.
- **Lifecycle of the organization** — being created, suspended, and retired as a whole.
- **Identity of the whole** — carrying the organization's name, configuration, and policies.

---

## 3. Owns

A Workspace owns:

- **Employees** — every worker belongs to it.
- **Brains** — the intelligence available to its employees.
- **Knowledge** — the curated knowledge base of the organization.
- **Artifacts** — every durable output produced inside it.
- **Tools** — the capabilities configured for use inside it.
- **Projects** — the initiatives underway.
- **Commitments and Tasks** — the work, both persistent and immediate.

All of these are subordinate to the Workspace. None of them can outlive their Workspace; if the Workspace is retired, its contents are archived with it.

---

## 4. Doesn't Own

A Workspace does **not** own:

- **The act of working** — that belongs to Employees.
- **Reasoning** — that belongs to Brains and Employees.
- **Tool execution logic** — that belongs to Tools.
- **Scheduling decisions** — that belongs to the Runtime.

The Workspace is a container and a boundary, not an actor. A Workspace that begins making decisions has overstepped its role.

---

## 5. Lifecycle

```
Created
  │
  ▼
Active  ◄────────┐
  │              │
  ▼              │
Suspended ───────┘   (resume)
  │
  ▼
Archived
```

- **Created** — the Workspace is initialized; its first objects may be added.
- **Active** — normal operation; work flows, Employees run, Artifacts accumulate.
- **Suspended** — paused; no Employees wake, no Triggers fire; contents are preserved.
- **Archived** — read-only retirement; the Workspace and everything in it are preserved as history but no longer active.

A Workspace is long-lived. Its lifecycle is measured in years, not sessions.

---

## 6. Future Extension

The Workspace may grow to support:

- **Federation** — controlled sharing of Knowledge or Artifacts across Workspaces without merging them.
- **Sub-workspaces** — divisions or teams within a larger organization, with their own boundaries.
- **Templates** — pre-configured Workspaces (a set of Employees, Brains, and Tools) that can be cloned for a new organization.
- **Migration** — moving a Workspace and its contents between deployments.
