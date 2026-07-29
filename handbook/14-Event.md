# Chapter 14 — Event

Version: 0.1
Status: Draft

---

## 1. Purpose

An Event is an **immutable record that something happened**.

An Employee committed an Artifact. A Tool was invoked. A Commitment was satisfied. A Trigger fired. Each of these is an Event — a fact, stamped in time, that the system remembers.

Events are the backbone that lets Emploid's parts stay decoupled. Triggers watch for Events; the Runtime records them; audits and metrics are built from them. Because Events are immutable, they form a trustworthy history of everything the workspace has done.

---

## 2. Responsibilities

An Event is accountable for:

- **Stating a fact** — what happened, unambiguously.
- **Carrying its context** — who was involved, what was affected, and relevant details.
- **Being stamped in time** — when it occurred.
- **Being immutable** — once recorded, never changed.

An Event describes the past. It does not command the future.

---

## 3. Owns

An Event owns:

- Its **type** — the category of thing that happened.
- Its **payload** — the details specific to this occurrence.
- Its **timestamp** — when it occurred.
- Its **source** — which Employee, Tool, Trigger, or system produced it.
- Its **immutability** — it cannot be edited after emission.

### Kinds of Events

```
Event
├── Lifecycle      (an Employee woke, slept, errored)
├── Domain         (an order arrived, a complaint closed, a deadline passed)
├── Tool           (a Tool was invoked, succeeded, or failed)
├── Artifact       (an Artifact was committed, revised, superseded)
└── System         (the workspace started, suspended, archived)
```

---

## 4. Doesn't Own

An Event does **not** own:

- **The action it describes** — the action already happened; the Event only records it.
- **The reaction to it** — reactions are the job of Triggers and Employees.
- **Decisions** — an Event is a fact, not a verdict.

An Event is history, not a directive.

---

## 5. Lifecycle

```
Emitted → Stored → Consumed (by Triggers, audits, metrics) → Archived
```

- **Emitted** — produced by some part of the system at the moment something happened.
- **Stored** — appended to the workspace's immutable record.
- **Consumed** — read by Triggers (which may wake Employees), by audit trails, and by metrics.
- **Archived** — retained long-term as the workspace's memory of itself.

Events are append-only. Correction is done by emitting a new Event, never by editing an old one.

---

## 6. Future Extension

The Event concept may grow to support:

- **Event sourcing** — reconstructing the state of any object by replaying its Events.
- **Streaming** — real-time consumption for live monitoring and dashboards.
- **Cross-workspace events** — controlled publication of Events to federated workspaces.
- **Retention policies** — rules for how long different Event types are kept.
