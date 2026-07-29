# Chapter 16 — Workspace Model

Version: 0.1
Status: Draft

> This chapter defines entities and their relationships.
> It is not a database schema. It prescribes no storage technology, no tables, no query language.
> The model must hold regardless of how it is eventually stored.

---

## 1. Purpose

The Workspace Model describes **what exists** in a Workspace and **how those things relate**.

It is the reference everyone — engineers, future contributors, and AI agents assisting development — consults to understand the shape of the system. Each entity has a purpose, an owner, and a set of relationships. That is all this chapter defines.

---

## 2. Entity Relationship Overview

```
Workspace
│
├── owns ──► Employee ──references──► Brain
│              │                         │
│              ├── has ──► Inbox ──► Task
│              ├── has ──► Commitment ──► Task
│              ├── has ──► Memory
│              └── has ──► Metrics
│
├── owns ──► Brain ──references──► Knowledge
│
├── owns ──► Knowledge
│
├── owns ──► Artifact ◄──produced by── Employee
│              │
│              └── belongs to ──► Project / Commitment
│
├── owns ──► Tool
│
└── owns ──► Project ──participates── Employee
                                  │
                                  └──► Artifact, Commitment, Task

Runtime
├── drives ──► Employee (lifecycle)
├── reads ──► Trigger
├── records ──► Event
└── moves ──► Task, Commitment, Memory, Artifact
```

---

## 3. Entities

For each entity: **Purpose**, **Owner**, **Relationships**.

### Workspace
- **Purpose:** The organization; the outermost container.
- **Owner:** Itself (top-level).
- **Relationships:** Owns every other entity. Nothing exists outside a Workspace.

### Employee
- **Purpose:** The autonomous worker (an AI agent).
- **Owner:** Workspace.
- **Relationships:** References one Brain. Owns an Inbox, Commitments, Memory, Metrics. Participates in zero or more Projects. Produces Artifacts.

### Brain
- **Purpose:** Reusable intelligence.
- **Owner:** Workspace.
- **Relationships:** Referenced by one or more Employees. References slices of Knowledge. Has versions.

### Knowledge
- **Purpose:** The curated organizational knowledge base.
- **Owner:** Workspace.
- **Relationships:** Referenced by Brains. Source of embeddings. Versioned.

### Artifact
- **Purpose:** Durable output of work.
- **Owner:** Workspace.
- **Relationships:** Produced by an Employee. May belong to a Project or Commitment. Has versions.

### Tool
- **Purpose:** External capability, exposed via a Tool Spec.
- **Owner:** Workspace.
- **Relationships:** Invoked by Employees (subject to permission). Independent of Brains and Knowledge.

### Project
- **Purpose:** A bounded initiative.
- **Owner:** Workspace.
- **Relationships:** Has participant Employees. Owns its scoped Artifacts, Commitments, and Tasks.

### Task
- **Purpose:** One executable objective.
- **Owner:** An Employee (via its Inbox); may belong to a Commitment or Project.
- **Relationships:** Produced by a Commitment. Produces an Artifact on completion.

### Commitment
- **Purpose:** Persistent responsibility.
- **Owner:** An Employee.
- **Relationships:** Spawns Tasks. May belong to a Project. May produce Artifacts over its life.

### Trigger
- **Purpose:** What wakes an Employee.
- **Owner:** Workspace.
- **Relationships:** Targets one or more Employees. Often fires in response to an Event.

### Event
- **Purpose:** Immutable record of something that happened.
- **Owner:** Workspace.
- **Relationships:** Produced by Employees, Tools, Triggers, or the Runtime. Consumed by Triggers, audits, and metrics.

### Memory
- **Purpose:** An Employee's working context.
- **Owner:** A single Employee.
- **Relationships:** Restored and persisted by the Runtime. Distinct from Knowledge and Brain long-term memory.

---

## 4. Modeling Rules

Three rules keep the model honest:

1. **Everything has one home.** Every entity belongs to exactly one Workspace. There is no free-floating state.
2. **Ownership is not identity.** An Employee *references* a Brain; it does not own it. Many Employees may share one Brain. Confusing reference with ownership is the most common modeling error.
3. **Spec and Status are separate.** For Employees (and wherever it applies), the relatively-fixed definition is distinct from the continuously-changing runtime state. This is what makes versioning, snapshotting, and templating possible.

---

## 5. Future Extension

The model may grow to support:

- **New entity types** — only after architectural review (see Chapter 02).
- **Richer relationships** — lineage between Artifacts, delegation graphs between Employees.
- **Cross-workspace references** — controlled links for federation, without violating the one-home rule.
- **Temporal modeling** — explicit tracking of how entities and relationships change over time.
