# Chapter 04 — Employee

Version: 0.1
Status: Draft

> An Employee and an AI Agent are the same object in Emploid.
> We call it an Employee because the system is modeled on how real organizations work.

---

## 1. Purpose

The Employee is the **only object in Emploid that actually works**.

Brains do not work. Tools do not work. Workspaces do not work. Every piece of meaningful work — every decision, every tool invocation, every artifact produced, every commitment honored — is performed by an Employee.

The Employee is therefore a **first-class object**: the central unit the Runtime manages, the unit that owns responsibility, and the unit an organization recruits, assigns, and holds accountable.

The Employee **references** a Brain. It is not a Brain. The Brain is *what it knows*; the Employee is *what does the knowing, deciding, and acting*.

---

## 2. Responsibilities

An Employee is accountable for:

- **Receiving work** — accepting Tasks into its Inbox.
- **Making decisions** — choosing what to do, in what order, with what tools.
- **Invoking Tools** — acting on the world through capabilities it is authorized to use.
- **Producing Artifacts** — turning work into durable, committed outputs.
- **Honoring Commitments** — carrying persistent responsibilities across many wakes.
- **Maintaining its working Memory** — keeping enough context to resume work correctly.

When something is done inside Emploid, an Employee did it. When something goes wrong, an Employee owned it.

---

## 3. Owns — The Employee Model

An Employee owns ten things. Together they form the complete Employee.

```
Employee
├── 1. Identity
├── 2. Brain
├── 3. Role
├── 4. Capability
├── 5. Resources
├── 6. State
├── 7. Inbox
├── 8. Commitments
├── 9. Memory
└── 10. Metrics
```

**1. Identity** — who the Employee is: ID, name, avatar, department, title, description, and owner. The public face of the agent within the organization.

**2. Brain** — a *reference*, not a copy. The Employee points at a Brain that holds its knowledge, persona, prompt, and long-term memory. Many Employees may share one Brain.

**3. Role** — the Employee's responsibility definition: mission, responsibilities, authority, KPIs, SOPs, and policies. Role defines *what the Employee is responsible for*, not what it knows.

**4. Capability** — what the Employee *can do*: send mail, query a database, browse, run code, operate CAD, call external services. Capability is the *ability*, not the instrument.

**5. Resources** — the concrete tools and systems the Employee is actually provisioned to use: the specific database, mail system, code runtime, design tool. Capability is abstract; Resources are the wired-up reality.

**6. State** — the runtime state of the Employee right now: Idle, Working, Waiting, Sleeping, Paused, or Error.

**7. Inbox** — the Employee's work queue. Each time the Employee wakes, it processes its Inbox. This is the front door for all incoming work.

**8. Commitments** — the Employee's long-term responsibilities: track this order to delivery, monitor this complaint, keep this audit ready. Commitments may persist for weeks or months and generate many Tasks.

**9. Memory** — the Employee's *working* memory, not its knowledge: who was contacted today, what a supplier promised, what a manager prioritized. It is the scratchpad of the current cycle of work.

**10. Metrics** — performance information: tasks completed, average response time, tool usage, cost, success rate. How the Employee is measured.

---

## 4. Doesn't Own

An Employee does **not** own:

- **The Scheduler** — when to wake is the Runtime's decision, driven by Triggers.
- **The Workspace** — the Employee lives inside a Workspace; it does not control it.
- **Projects** — an Employee participates in Projects; it does not own the Project concept.
- **The Knowledge Base** — knowledge belongs to the Brain; the Employee only references it.
- **Other Employees** — an Employee is a peer, not a supervisor of the system's internals. (Coordination between Employees is collaboration, not ownership.)

An Employee that tries to schedule itself, reconfigure the Workspace, or rewrite shared Knowledge has overstepped its boundary.

---

## 5. Lifecycle

### 5.1 Runtime State

At any moment an Employee is in exactly one state:

```
        Created
           │
           ▼
   ┌─── Idle ◄────────────┐
   │       │              │
   │       ▼              │
   │    Working           │
   │       │              │
   │       ▼              │
   │    Waiting ──────────┘   (resume when unblocked)
   │
   ├──► Sleeping   (default rest state; context persisted)
   ├──► Paused     (held by an operator; will not auto-wake)
   └──► Error      (failed; needs attention)
```

- **Idle** — awake, no current work; will act on the next Inbox item.
- **Working** — actively executing.
- **Waiting** — blocked on something external (a reply, a tool result, a dependency).
- **Sleeping** — the **default** state. The Employee is at rest, its context persisted, not resident in memory.
- **Paused** — deliberately held; Triggers will not wake it.
- **Error** — something failed; the Employee requires intervention before it can continue.

**Sleep is the default.** An Employee is woken only when a Trigger fires and there is work in its Inbox.

### 5.2 Existential Lifecycle

Beyond daily state, an Employee has a lifetime:

```
Created → Assigned → Active → … → Retired → Archived
```

An Employee may be created, given a Role and a Brain, work for months or years, be reassigned to a new Role, and eventually be retired. Its Artifacts and history persist after retirement.

---

## 6. Spec vs Status

To support versioning, snapshotting, and deployment, an Employee is split into two layers.

**Employee Spec** — relatively fixed:

- Identity
- Brain (reference)
- Role
- Capability
- Permission / Authority
- Resources

**Employee Status** — continuously changing:

- State
- Inbox
- Current Task
- Commitments
- Working Memory
- Metrics

The Spec answers *"what kind of employee is this?"* The Status answers *"what is this employee doing right now?"* Separating them makes it possible to version an Employee, snapshot it, and deploy the same Spec into many instances.

---

## 7. Template vs Instance

For enterprise-scale deployment, Employees come in two layers:

```
Employee Template
       │
       ▼
Employee Instance
```

A **Template** defines a kind of employee — for example, "Procurement Assistant Steve." **Instances** are concrete deployments of that Template:

- Steve @ Taiwan plant
- Steve @ Nanjing plant
- Steve @ Vietnam plant

All three instances **share** the Brain, Role, and Capability from the Template. Each instance **independently owns** its own Inbox, Commitments, Memory, and KPIs — because each plant has its own orders, suppliers, and priorities.

This is how one well-designed employee scales across an entire organization without becoming one overloaded agent.

---

## 8. Future Extension

The Employee may grow to support:

- **Multi-agent collaboration** — Employees handing Tasks to one another, sharing context, and forming teams inside a Project.
- **Cloning** — producing a temporary copy of an Employee for parallel work, then merging results.
- **Skill learning** — an Employee improving its own SOPs and capabilities from experience, fed back into its Brain.
- **Delegation and supervision** — structured authority between Employees (a lead Employee directing others), without violating the rule that no Employee owns another.
- **Marketplace profiles** — Employee Templates published and imported across Workspaces.

Whatever extensions arrive, the rule holds: **the Employee is the worker, the Brain is the knowledge, and the two never collapse into one.**
