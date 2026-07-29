# Emploid Architecture Handbook

**Version:** 0.1
**Status:** Draft

> Emploid is not an AI chat application.
> It is an operating environment where AI agents — called Employees — continuously perform meaningful work inside a shared workspace.

---

## What Emploid Is

Emploid is an **AI Agent Operating System**.

It is the master control platform (主控平台) in which AI agents are recruited, given roles and authority, woken when there is work, and put to sleep when there is not. Every agent lives inside exactly one Workspace. Every artifact an agent produces belongs to the organization, not to a conversation. Every responsibility persists across model changes, session boundaries, and tool replacements.

An **Agent** and an **Employee** are the same thing. We use the word *Employee* because the system is modeled on how organizations actually work: people hold roles, own responsibilities, use tools, produce results, and persist across the work they do. Emploid brings that model to AI.

The name follows from this: an AI employee — an employee-like being — is an **emploid**. The platform is Emploid; the workers that live inside it are emploids. The formal architectural term used throughout this handbook remains **Employee**.

This handbook is the **constitution** of that operating system.

---

## What This Handbook Is — and Is Not

This handbook defines the **core abstractions** of Emploid: the objects the system is built from, the relationships between them, and the design philosophy that governs them.

It is **not** an API reference. It is **not** a technology survey. It contains no mandates about which programming language, database, model, or tool protocol must be used.

The principle is simple:

> Architecture should outlive technology.

The model that provides the language model today may be replaced in three years. The storage engine used today may be replaced in five. But if *Employee*, *Brain*, *Workspace*, *Artifact*, and *Commitment* still hold as concepts, the architecture is a success. This handbook exists to make those concepts stable.

If a proposed feature cannot be expressed in the concepts defined here, the answer is not to change the code first — it is to revisit the architecture.

---

## How to Read

The handbook is organized in four parts.

**Part I — Vision** establishes *why* Emploid exists and the principles every decision is measured against.

**Part II — Core Concepts** defines each first-class object: what it is, what it owns, what it does not own, how it lives, and how it may evolve.

**Part III — Runtime** defines how work actually flows: tasks, commitments, triggers, the runtime engine, events, and memory.

**Part IV — Platform** defines the data model, the agent and tool interfaces, security, and the roadmap.

Every concept chapter follows the same disciplined structure:

1. **Purpose** — why this object exists.
2. **Responsibilities** — what it is accountable for.
3. **Owns** — what belongs to it.
4. **Doesn't Own** — what is explicitly out of scope.
5. **Lifecycle** — how it is born, changes, and ends.
6. **Future Extension** — how it may grow without breaking.

Read Part I first. Then read Part II in order, because later concepts build on earlier ones. Part III and Part IV assume the core concepts.

---

## Table of Contents

### Part I — Vision

- [01 — Vision](01-Vision.md)
- [02 — Design Philosophy](02-Design-Philosophy.md)

### Part II — Core Concepts

- [03 — Workspace](03-Workspace.md)
- [04 — Employee](04-Employee.md)
- [05 — Brain](05-Brain.md)
- [06 — Artifact](06-Artifact.md)
- [07 — Knowledge](07-Knowledge.md)
- [08 — Tool](08-Tool.md)
- [09 — Project](09-Project.md)

### Part III — Runtime

- [10 — Task](10-Task.md)
- [11 — Commitment](11-Commitment.md)
- [12 — Trigger](12-Trigger.md)
- [13 — Runtime](13-Runtime.md)
- [14 — Event](14-Event.md)
- [15 — Memory](15-Memory.md)

### Part IV — Platform

- [16 — Workspace Model](16-Workspace-Model.md)
- [17 — Agent SDK](17-Agent-SDK.md)
- [18 — Tool SDK](18-Tool-SDK.md)
- [19 — Security](19-Security.md)
- [20 — Roadmap](20-Roadmap.md)

---

## Concept Map

```
Workspace
│
├── Employee  (the autonomous worker; an AI agent)
│      ├── Identity
│      ├── Brain          ← referenced, shared
│      ├── Role
│      ├── Capability
│      ├── Resources
│      ├── State
│      ├── Inbox          ← Tasks arrive here
│      ├── Commitments    ← long-term responsibilities
│      ├── Memory         ← working memory
│      └── Metrics
│
├── Brain     (reusable intelligence: knowledge, persona, prompt, memory)
├── Knowledge (curated organizational knowledge)
├── Artifact  (outputs of work; first-class citizens)
├── Tool      (external capability; never decides)
├── Project   (a bounded initiative)
│
└── Runtime   (wakes, restores, executes, commits, sleeps)
        ├── Task        (one executable objective)
        ├── Commitment  (persistent responsibility)
        ├── Trigger     (what wakes an Employee)
        └── Event       (immutable record of what happened)
```

**One-line roles:**

| Concept | One-line role |
|---------|---------------|
| Workspace | The organization. Everything lives inside exactly one. |
| Employee | The worker. An AI agent that owns responsibilities. |
| Brain | The intelligence. Knowledge and persona, reusable and versioned. |
| Artifact | The result. Output of work, owned by the workspace. |
| Knowledge | The memory of the organization. Curated and durable. |
| Tool | The capability. External power an Employee may invoke. |
| Project | The initiative. A bounded collaboration toward a goal. |
| Task | The unit of work. Short-lived, executable. |
| Commitment | The long game. Persistent responsibility that outlives tasks. |
| Trigger | The alarm. What decides an Employee should wake. |
| Runtime | The engine. Manages lifecycle, never reasoning. |
| Event | The record. Immutable fact of what happened. |
| Memory | The scratchpad. An Employee's working context, restored each wake. |

---

## How This Handbook Evolves

This is a living document. Changes follow two rules:

1. **Adding a concept** requires architectural review. The set of first-class concepts is intentionally small and must stay small.
2. **Changing a principle** requires changing the handbook first, then the code — never the other way around.

The handbook version is recorded at the top of this file. When the concepts change, the version changes with them.
