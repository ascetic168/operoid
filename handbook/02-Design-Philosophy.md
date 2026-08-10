# Chapter 02 — Design Philosophy

Version: 0.1
Status: Draft

---

## How to Use These Principles

These ten principles are the constitution of Operoid. Every architectural decision should be measurable against them.

When a proposed design conflicts with a principle, there are two acceptable outcomes:

1. The design changes to respect the principle.
2. The principle is revised — by amending this chapter first, then the code.

Changing the code to quietly violate a principle is never acceptable.

The principles are ordered intentionally. Earlier principles are more foundational; later ones depend on them.

---

## Principle 1 — Knowledge is not the worker.

What an agent *knows* and what an agent *is* are different things.

Knowledge lives in a **Brain**. Work is done by an **Employee**. An Employee references a Brain; it is not identical to one.

This separation lets one Brain serve many Employees, and lets an Employee upgrade its Brain without losing its identity, history, or responsibilities.

---

## Principle 2 — Employees own responsibilities.

An Employee is defined by what it is **accountable for**, not by what it knows.

Knowledge can be shared, copied, and replaced. Responsibility cannot — it belongs to a specific Employee with a specific role and authority.

When something goes wrong, the question is never "which Brain knew about this?" It is "which Employee owned this?"

---

## Principle 3 — Artifacts are first-class citizens.

The output of work — a report, a drawing, a record, a piece of code — is a durable object owned by the workspace.

Artifacts are never buried inside a conversation. They have identity, provenance, versions, and ownership. They survive the Employee that produced them and the session that created them.

Work that does not produce or update an artifact is, by definition, incomplete.

---

## Principle 4 — Everything happens inside a Workspace.

There is no work outside a Workspace. Every Employee, Brain, Artifact, Tool, and Commitment belongs to exactly one Workspace.

The Workspace is the boundary of trust, tenancy, and knowledge sharing. It is the organization.

This means the system never has free-floating state. If something exists, it has a home.

---

## Principle 5 — Tools never make decisions.

A Tool provides capability. It does not choose when to act.

The Employee decides. The Tool executes. This boundary is what keeps Employees accountable: the decision-maker is always the agent, never the instrument.

A Tool that begins making decisions has stopped being a Tool and must be reconceived.

---

## Principle 6 — Brains can be shared.

A Brain is reusable intelligence. Many Employees may reference the same Brain, just as many people may share the same training and expertise.

Sharing a Brain does not mean sharing a mind. Each Employee retains its own role, authority, inbox, and memory. They share what they *know*, not what they *are doing*.

---

## Principle 7 — Employees sleep by default.

An Employee is not a always-running process. It is a worker that is **woken when there is work** and **put to sleep when there is not**.

Sleep is the default state. Residence in memory is the exception.

This is what makes the system scalable: a workspace may contain thousands of Employees, but only the ones with work are ever awake.

---

## Principle 8 — Context is restored, not remembered.

An Employee does not keep its working context resident between wakes. Its context is **persisted** when it sleeps and **restored** when it wakes.

This is the difference between an Employee and a long-running chat session. The Employee's state is durable and reconstructable; it does not depend on a process staying alive.

What is forgotten is a bug. What is held in memory forever is a design error.

---

## Principle 9 — Commitments outlive Tasks.

A **Task** is a single executable objective: short, bounded, done.

A **Commitment** is a persistent responsibility: track this order to delivery, monitor this complaint to closure, keep this audit ready.

Commitments generate many Tasks over their lifetime, but they are not themselves Tasks. When the last Task of a Commitment completes, the Commitment may still continue.

Work that matters is structured as Commitments. Work that is merely busy is structured as Tasks.

---

## Principle 10 — The Runtime manages execution, not reasoning.

The **Runtime** decides *when* an Employee runs, *how long* it runs, and *what context* it runs with.

It never decides *what the Employee should think*.

Reasoning is the Employee's own, drawn from its Brain. Execution discipline — lifecycle, scheduling, restoration, commitment of artifacts — is the Runtime's. Mixing the two corrupts both.

---

## A Note on Technology

These principles say nothing about programming languages, databases, models, or tool protocols. That is deliberate.

Today's choices may be replaced. The principles must not be. When a new technology is considered, the only question that matters is whether it honors these ten principles. If it does, it fits. If it does not, no amount of convenience justifies it.
