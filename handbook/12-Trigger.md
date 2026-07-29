# Chapter 12 — Trigger

Version: 0.1
Status: Draft

---

## 1. Purpose

A Trigger is **what wakes an Employee**.

By default, Employees sleep. Something must decide that there is work to do and rouse the right Employee. That something is a Trigger.

Triggers decouple **when** work happens from **what** the work is. The Employee knows what to do; the Trigger knows when it should start. This separation is what lets the system react to the world without keeping every Employee permanently resident.

A Trigger never performs work. It only signals.

---

## 2. Responsibilities

A Trigger is accountable for:

- **Watching for a condition** — an event, a time, a message, a manual instruction.
- **Identifying the target** — which Employee (or Employees) should wake.
- **Carrying context** — the information the Employee needs to understand why it was woken.
- **Signaling the Runtime** — handing off, not executing.

---

## 3. Owns

A Trigger owns:

- Its **condition or rule** — the precise thing it watches for.
- Its **target Employee(s)** — who it wakes.
- Its **payload** — the context delivered on firing.
- Its **enabled state** — whether it is currently armed.

### Kinds of Triggers

```
Trigger
├── Event-driven     (something happened — an Event arrived)
├── Time-driven      (a schedule or deadline was reached)
├── Message-driven   (a message or request was received)
└── Manual           (a human or another Employee explicitly invoked it)
```

---

## 4. Doesn't Own

A Trigger does **not** own:

- **The work itself** — that is the Employee's.
- **The decision of what to do** — the Trigger says "wake up, here is why"; the Employee decides what to do about it.
- **Scheduling policy** — priorities and concurrency limits belong to the Runtime.

A Trigger is an alarm, not an actor.

---

## 5. Lifecycle

```
Defined → Armed → Fired → Reset → (Armed again …)
```

- **Defined** — its rule and target are configured.
- **Armed** — actively watching.
- **Fired** — the condition was met; it has signaled the Runtime with its payload.
- **Reset** — ready to fire again (for recurring Triggers), or retired (for one-shot Triggers).

A Trigger may fire many times over its life, or exactly once, depending on its design.

---

## 6. Future Extension

The Trigger concept may grow to support:

- **Composite conditions** — firing only when several conditions align.
- **Filtering and routing** — sophisticated rules for which Employee wakes for which situation.
- **Debouncing and batching** — avoiding redundant wakes when many events arrive at once.
- **Cross-workspace triggers** — an event in one Workspace waking an Employee in another, under controlled terms.
