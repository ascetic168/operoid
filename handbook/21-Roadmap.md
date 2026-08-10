# Chapter 21 — Roadmap

Version: 0.1
Status: Draft

> This roadmap describes directions, not commitments.
> The order reflects dependence: earlier milestones must be solid before later ones are attempted.
> Every milestone is measured against the principles in Chapter 02. If a milestone conflicts with a principle, the milestone changes — not the principle.

---

## 1. Purpose

This chapter sketches how Operoid may evolve while staying true to its constitution. It exists so that growth is **deliberate**: each new capability is recognized as an extension of the core concepts, not an exception to them.

---

## 2. Milestone 1 — A Single Employee That Truly Works

Before anything else, one Employee must be able to:

- wake on a Trigger,
- have its context restored,
- read its Inbox,
- invoke a Tool under permission,
- commit an Artifact,
- and return to sleep.

If this cycle is not reliable, nothing else matters. This milestone validates the Runtime, the Tool boundary, and the principle that context is restored, not remembered.

**Measured by:** an Employee that survives restarts, model swaps, and idle time without losing its Commitments or its place.

---

## 3. Milestone 2 — Persistence and Commitments

With one Employee working, the system must make **persistence real**:

- Commitments that outlive every session.
- Artifacts that belong to the workspace, not the conversation.
- Memory that is restored flawlessly on every wake.

This milestone validates Principles 3, 8, and 9 — that artifacts are first-class, context is restored, and commitments outlive tasks.

**Measured by:** work that continues correctly after the system is entirely shut down and restarted.

---

## 4. Milestone 3 — Shared Brains and Knowledge

One Brain should serve many Employees, and the Knowledge base should be curated, versioned, and retrievable.

This milestone validates Principles 1 and 6 — that knowledge is not the worker, and that Brains can be shared.

**Measured by:** upgrading a Brain and watching many Employees adopt it without losing their identities or in-flight work.

---

## 5. Milestone 4 — Templates and Instances

Employee Templates should deploy as Instances across an organization, sharing Brain and Role while owning independent Inboxes and Commitments.

This milestone validates the Spec/Status and Template/Instance separations.

**Measured by:** one Template producing many independent Instances — "Steve at every plant" — each tracking its own reality.

---

## 6. Milestone 5 — Collaboration

Multiple Employees should cooperate inside Projects: handing off Tasks, sharing context, and producing shared Artifacts — without any Employee owning another.

This milestone validates Principles 2 and 4 — that Employees own responsibilities, and that everything happens inside a Workspace — extended to many Employees at once.

**Measured by:** a Project completed by a team of Employees working in parallel and in sequence.

---

## 7. Horizon — Beyond v1

Looking further out, the architecture is designed to accommodate:

- **Skill learning** — Employees and Brains accumulating expertise from experience, so the organization gets smarter over time.
- **Cloning and parallelism** — temporary Employee copies for parallel work, with structured merging of results.
- **Marketplace** — Employee Templates, Brains, and Tools published and imported across Workspaces.
- **Federation** — controlled sharing of Knowledge, Artifacts, and Events across organizational boundaries.
- **Distributed Runtime** — execution across many nodes, with state still durably restored on every wake.
- **Human-agent teams** — humans and Employees as true peers inside Projects, with clear authority and accountability on both sides.

Most of these require no new core concepts — each is an extension of Employee, Brain, Workspace, Artifact, Tool, Commitment, and Runtime. The one exception is **human-agent teams**: bidirectional, reviewable conversational interaction between human and agent needs a dedicated interaction-layer concept, namely Message (Ch.16, added in v0.2). It does not replace work outputs (still Artifact / Commitment); it carries the exchange itself. **That is the test of a sound architecture: the future fits inside the concepts, instead of breaking them.**

---

## 8. The Standing Question

For every milestone, and for every feature ever proposed after this handbook is written, the question is the same:

> Does it honor the ten principles?

If yes, build it. If no, do not change the code to force it — change the proposal, or amend the handbook first.

The architecture is meant to outlive every technology, every model, and every contributor who passes through it. This handbook is how it does so.
