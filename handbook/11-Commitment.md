# Chapter 11 — Commitment

Version: 0.1
Status: Draft

> Commitments outlive Tasks. (Principle 9.)

---

## 1. Purpose

A Commitment is a **persistent responsibility**.

Where a Task is a single executable objective that ends, a Commitment is an ongoing duty that continues until a completion condition is met. "Track this purchase order until goods are received." "Monitor this customer complaint until resolved." "Keep this audit ready until it closes."

Commitments are how Operoid represents the kind of work that *matters* in an organization: the responsibilities that span days, weeks, or months and that no single conversation could ever contain.

A Commitment is not a large Task. It is a different kind of object — one that *generates* Tasks over its lifetime.

---

## 2. Responsibilities

A Commitment is accountable for:

- **Declaring its completion condition** — the exact state in which it is satisfied.
- **Owning an Employee** — the one accountable for seeing it through.
- **Spawning Tasks** — breaking the long responsibility into executable work, again and again, as needed.
- **Persisting across wakes** — surviving every sleep, every session, every model change, until it is satisfied.
- **Recording its history** — what has been done toward it, and what remains.

---

## 3. Owns

A Commitment owns:

- Its **completion condition** — the definition of done.
- Its **owning Employee** — who is accountable.
- Its **generated Tasks** — the executable work it has produced.
- Its **state and history** — progress, milestones, events.
- Its **links** — to Artifacts, Projects, and the people or systems it concerns.

---

## 4. Doesn't Own

A Commitment does **not** own:

- **Moment-to-moment execution** — that is the Employee's, one Task at a time.
- **The Employee's full attention** — an Employee may hold many Commitments at once.
- **The Knowledge required** — that is drawn from the Brain and Knowledge base.

A Commitment defines *what must remain true over time*; it does not perform the work to keep it true.

---

## 5. Lifecycle

```
Proposed → Active ──────────────► Satisfied → Archived
   │            │                     ▲
   └─► Rejected ├─► Suspended ─────────┘   (resumed)
                │
                └─► spawns Tasks repeatedly throughout its life
```

- **Proposed** — an Employee identified something worth long-term tracking during a conversation and proactively proposed it (with a completion condition). Awaits human approval. Does not run until approved (no Tasks, no wake).
- **Active** — after human approval (or direct delegation), the Employee is working it, generating Tasks as needed.
- **Suspended** — deliberately paused; not forgotten.
- **Satisfied** — the completion condition is met.
- **Rejected** — the human declined the Employee's proposal; it ends without entering Active.
- **Archived** — retained with its full history.

A Commitment's life is measured against its **completion condition**, not against time. When the condition is met, the Commitment ends — regardless of how many Tasks it took to get there.

---

## 6. Future Extension

The Commitment concept may grow to support:

- **Hierarchies** — a Commitment delegating sub-responsibilities to other Employees.
- **Satisfaction proofs** — structured evidence that the completion condition was truly met.
- **Escalation** — automatic routing to a human or lead Employee when a Commitment stalls.
- **Patterns** — reusable Commitment templates for common organizational duties (order tracking, audit readiness, incident monitoring).
