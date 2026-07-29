# Chapter 10 — Task

Version: 0.1
Status: Draft

---

## 1. Purpose

A Task is **one executable objective**.

It is the smallest unit of work an Employee performs in a single stretch: "draft this email," "look up this order," "generate this report," "update this record." A Task has a clear owner, a clear input, and a clear output — and when the output is committed, the Task is done.

Tasks are intentionally **short-lived**. They live in an Employee's Inbox, get done, and end. Work that goes on for weeks is not a Task; it is a Commitment that *produces* Tasks.

---

## 2. Responsibilities

A Task is accountable for:

- **Naming its objective** — what, concretely, must be accomplished.
- **Naming its owner** — which Employee will perform it.
- **Carrying its input** — the information or Artifacts needed to do the work.
- **Producing its output** — the Artifact or result that marks completion.
- **Tracking its status** — where it stands right now.

---

## 3. Owns

A Task owns:

- Its **input** — parameters, source Artifacts, references.
- Its **output** — the produced Artifact or result.
- Its **status** — and the history of how it got there.
- Its **linkage** — the Commitment or Project it belongs to, if any.

A Task belongs to exactly one Employee (its owner) and lives in that Employee's Inbox until complete.

---

## 4. Doesn't Own

A Task does **not** own:

- **Persistence of purpose** — that belongs to the Commitment. A Task ends; the reason for it may continue.
- **The Employee** — a Task is assigned *to* an Employee; it does not control one.
- **Other Tasks** — sequencing between Tasks is managed by the Employee and the Commitment, not by individual Tasks.

A Task is an event of work, not an ongoing structure.

---

## 5. Lifecycle

```
Created → Assigned → In Progress → Waiting → Completed
                                   │
                                   └─► Failed / Cancelled
```

- **Created** — defined, awaiting an owner.
- **Assigned** — placed in an Employee's Inbox.
- **In Progress** — the Employee is actively working it.
- **Waiting** — blocked on an external dependency (a reply, a tool result).
- **Completed** — the output is committed; the Task is done.
- **Failed / Cancelled** — the Task could not or should not complete; the reason is recorded.

Most Tasks should move from Assigned to Completed quickly. A Task that lingers is a signal that it should have been a Commitment, or that it is blocked.

---

## 6. Future Extension

The Task concept may grow to support:

- **Decomposition** — a large Task splitting into sub-Tasks, optionally assigned to other Employees.
- **Dependencies** — declaring that one Task cannot start until another completes.
- **Templates** — reusable Task definitions for repeated work.
- **Estimation** — expected effort and cost, for planning and metrics.
