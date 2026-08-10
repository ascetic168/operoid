# Chapter 05 — Brain

Version: 0.1
Status: Draft

---

## 1. Purpose

The Brain is **reusable intelligence**.

It holds everything an Employee *knows* and *is disposed to do* — separate from who the Employee *is* and what the Employee *is responsible for*. By extracting intelligence into its own object, Operoid lets knowledge be shared, versioned, upgraded, and replaced independently of the workers that use it.

The Brain answers the question: *"Given this situation, how should someone with this expertise think and respond?"*

The Employee answers: *"I am the one in this situation, and I own the outcome."*

A Brain never executes. It is consulted.

---

## 2. Responsibilities

A Brain is accountable for providing:

- **Persona** — the character and voice the Employee speaks with.
- **Knowledge** — the curated expertise the Employee can draw on.
- **Long-term Memory** — accumulated experience that persists across sessions, distinct from an Employee's short-term working memory.
- **Prompt** — the instructions and dispositions that shape how the Employee reasons.
- **Preferred Models** — guidance on which language or embedding models suit this Brain best.
- **Embedding** — the vector representation that lets the Brain retrieve relevant knowledge.
- **Skills** — named, reusable capabilities the Brain teaches its Employees (how to read a purchase order, how to summarize an audit).

A Brain is also responsible for being **versioned** — its knowledge and prompt evolve over time in a trackable way.

---

## 3. Owns

A Brain owns:

- Its **Persona** and **Prompt**.
- Its **Knowledge references** — the slices of the Knowledge base it draws from.
- Its **Long-term Memory** — accumulated experience tied to this expertise.
- Its **Embeddings** — the vectors that make its knowledge retrievable.
- Its **Model preferences**.
- Its **Version history** — Brain v1, v2, v3, each a coherent snapshot.

A Brain may be referenced by **many Employees**. Sharing is the point: one Brain can serve a whole department of similarly-trained Employees.

---

## 4. Doesn't Own

A Brain does **not** own:

- **Execution** — it never runs, calls tools, or commits artifacts. That is the Employee's job.
- **Responsibility** — a Brain is not accountable for outcomes; the Employee that uses it is.
- **An Inbox or Commitments** — those belong to Employees.
- **Working memory of a specific task** — that belongs to the Employee.
- **Authority** — a Brain grants no permissions. The Employee's Role does.

A Brain is expertise, not an agent. A Brain that begins executing or owning commitments has been misapplied.

---

## 5. Lifecycle

```
Draft → Published (v1) → … → vN → Deprecated → Archived
                          ▲
                          │
                    (Employees upgrade)
```

- **Draft** — being authored; not yet usable.
- **Published** — a numbered version is live and may be referenced by Employees.
- **Upgrade** — a new version (v2, v3…) supersedes the old. Employees may upgrade to the new version, choosing when to adopt it.
- **Deprecated** — no new Employees may reference it; existing ones are encouraged to upgrade.
- **Archived** — preserved for history; no longer active.

Because Brains are versioned, an organization can improve its collective intelligence without losing the ability to understand what an Employee did under an older Brain.

---

## 6. Future Extension

The Brain may grow to support:

- **Skill learning** — accumulating new Skills from the experience of the Employees that use it, so expertise compounds over time.
- **Specialization** — branching a general Brain into specialized variants for specific domains.
- **Composition** — combining multiple Brains (or Skills from different Brains) into a richer expertise.
- **Marketplace** — publishing Brains for import across Workspaces.
- **Evaluation** — formal scoring of a Brain's quality and reliability before promotion.

Throughout all of this, the rule holds: **the Brain is knowledge, never the worker.**
