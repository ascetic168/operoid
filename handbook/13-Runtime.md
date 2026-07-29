# Chapter 13 — Runtime

Version: 0.1
Status: Draft

> The Runtime manages execution. It never manages reasoning. (Principle 10.)

---

## 1. Purpose

The Runtime is the **engine** of Emploid.

It is the counterpart to the operating-system kernel: it does not do the work, but it makes the work possible. The Runtime decides which Employee runs, when, for how long, and with what context — and it ensures that when work is done, the result is safely committed before the Employee returns to sleep.

Everything in Part II and the rest of Part III — Employees, Tasks, Commitments, Triggers, Events, Memory — is *material* the Runtime moves. The Runtime is what moves it.

This chapter defines only the **lifecycle** of a single unit of execution. It deliberately does not describe scheduling algorithms, concurrency models, or any implementation technology. Those change; the lifecycle does not.

---

## 2. Responsibilities

The Runtime is accountable for:

- **Waking** the right Employee when a Trigger fires.
- **Restoring context** — giving the Employee its Inbox, Commitments, Memory, and the relevant slice of its Brain, freshly reconstituted each time.
- **Executing** — letting the Employee reason, decide, and invoke Tools.
- **Enforcing discipline** — timeouts, permissions, resource limits, and isolation.
- **Committing artifacts** — ensuring outputs are persisted before the Employee sleeps.
- **Persisting state** — saving the Employee's Memory and status so the next wake is a clean restoration, not a loss.
- **Putting the Employee back to sleep** when there is no more work.

---

## 3. Owns

The Runtime owns:

- The **execution loop** — the wake → restore → execute → commit → sleep cycle.
- The **scheduler** — the policy for which Employee runs when, and how many run at once.
- The **lifecycle discipline** — enforcing that every execution passes through the same well-defined phases.
- The **isolation boundary** — keeping one Employee's execution from interfering with another's.

---

## 4. Doesn't Own

The Runtime does **not** own:

- **Reasoning** — what the Employee thinks is the Employee's, drawn from its Brain. The Runtime never tells an Employee what to conclude.
- **Business decisions** — whether to approve an order, how to respond to a complaint, what to write in a report: all the Employee's.
- **Knowledge or memory content** — those belong to the Brain and the Employee.
- **Responsibility for outcomes** — that belongs to the Employee.

This is the most important boundary in the system. The instant the Runtime begins influencing *what* an Employee should think, two things break at once: the Runtime becomes untestable, and the Employee becomes unaccountable.

---

## 5. Lifecycle — The Execution Cycle

This is the canonical cycle the Runtime enforces for every Employee, every time it works:

```
       ┌───────────────────────────────────────────────┐
       │                                               ▼
   Trigger ──► Wake ──► Restore Context ──► Execute ──► Commit ──► Sleep
   fires                                       │        Artifact
                                               │
                                          (invoke Tools,
                                           produce output)
```

**1. Wake.** A Trigger fires. The Runtime selects the target Employee, brings it out of its sleeping state, and prepares it to run.

**2. Restore Context.** The Runtime reconstructs the Employee's working context: its Inbox, its active Commitments, its working Memory, and the relevant portion of its Brain. Nothing is assumed to be "still in memory." Context is rebuilt deliberately every time. (Principle 8.)

**3. Execute.** The Employee reasons and acts — reading its Inbox, making decisions, invoking Tools, producing output. The Runtime watches, enforces limits, and records what happens as Events, but it does not intervene in the reasoning.

**4. Commit Artifact.** Before the Employee can rest, the Runtime ensures that every output meant to persist is safely committed as an Artifact, and that the Employee's Memory and status are saved. This is the boundary between provisional and real.

**5. Sleep.** With work committed and state persisted, the Employee returns to its default sleeping state. It holds nothing resident. It can be restored perfectly the next time it wakes.

---

## 6. Why Context Is Restored, Not Remembered

A long-running process keeps its state in memory and loses it if it crashes. An Employee is not a long-running process.

By restoring context on every wake and persisting it on every sleep, Emploid guarantees that an Employee's state is **durable and reconstructable**. A crash, a restart, a model swap, or a migration never destroys in-flight work — because the work was committed before the Employee slept, and the context was saved.

This is what makes it possible for a workspace to hold thousands of Employees while only the ones with work are ever awake.

---

## 7. Future Extension

The Runtime may grow to support:

- **Parallel and concurrent execution** — many Employees working at once, with safe isolation.
- **Preemption and backpressure** — managing load when more Employees want to run than resources allow.
- **Cooperative execution** — Employees yielding to one another within a Project.
- **Distributed runtime** — execution spread across multiple nodes, with state still durably restored.
- **Observability** — rich tracing of the execution cycle for debugging and metrics.

Whatever the Runtime gains, the core contract is fixed: **it manages the cycle of execution, and it leaves the reasoning to the Employee.**
