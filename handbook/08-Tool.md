# Chapter 08 — Tool

Version: 0.1
Status: Draft

> A Tool is capability. It is never a decision-maker.

---

## 1. Purpose

A Tool exposes an **external capability** to Employees.

An Employee can reason, but reasoning alone changes nothing. To act on the world — to send a message, query a database, run a calculation, operate a design tool, call an external service — the Employee invokes a Tool.

Tools are how Operoid reaches outside its own boundary. They are the hands and eyes of the Employee. But like hands, they do not choose what to grasp. The Employee chooses; the Tool grasps.

---

## 2. Responsibilities

A Tool is accountable for:

- **Executing a single, well-defined operation** when invoked.
- **Honoring its contract** — the inputs it accepts and the outputs it returns.
- **Respecting permissions** — refusing anything the invoking Employee is not authorized to do.
- **Behaving predictably** — timeout, retry, and logging are part of what makes a Tool trustworthy.

A Tool does not decide *whether* to act. It acts when a legitimate caller asks, and refuses when the caller lacks authority.

---

## 3. Owns

Every Tool owns a **Tool Spec**, which must contain:

- **Spec** — what the Tool does, in plain terms.
- **Permission** — what authority is required to invoke it.
- **Input** — the parameters it accepts, with types and constraints.
- **Output** — what it returns, including error cases.
- **Timeout** — how long a call may run before it is abandoned.
- **Retry** — what happens on transient failure.
- **Logging** — what is recorded for every invocation.

These seven are not optional. A capability without a complete Tool Spec is not yet a Tool — it is an ungoverned risk.

### Tool Drivers

Tools reach the outside world through different kinds of **drivers**:

```
Tool
├── Local Tool
├── Protocol Tool     (an external tool-protocol server)
├── REST / Service    (an HTTP-style API)
├── Database          (a queryable store)
├── Code Runtime      (executes a script or program)
└── Native            (built directly into the platform)
```

Any particular protocol or technology is **just one kind of driver**. The architecture does not depend on any of them; it depends only on the Tool Spec.

---

## 4. Doesn't Own

A Tool does **not** own:

- **Decisions** — it never chooses when or whether to act. (Principle 5.)
- **Responsibility for outcomes** — that belongs to the Employee that invoked it.
- **Authority** — a Tool carries permissions, but authority is granted to Employees by their Roles.
- **Stateful memory of its own** — a Tool may be stateless; any state belongs to the external system it wraps.

A Tool that begins deciding is no longer a Tool. It has become an unauthorized Employee.

---

## 5. Lifecycle

```
Registered → Enabled → Disabled → Deprecated → Retired
```

- **Registered** — its Spec is known to the workspace.
- **Enabled** — available for Employees to invoke, subject to permissions.
- **Disabled** — temporarily turned off; invocations refused.
- **Deprecated** — slated for removal; Employees should migrate away.
- **Retired** — removed; the Spec is retained for history.

---

## 6. Future Extension

The Tool concept may grow to support:

- **Dynamic discovery** — automatically detecting newly available Tools and registering their Specs.
- **Composition** — chaining Tools into higher-level capabilities while keeping each Tool simple and decidable.
- **Marketplace** — publishing Tool Specs for reuse across Workspaces.
- **Observability** — richer telemetry on how, how often, and how successfully each Tool is used.
- **Quotas and budgets** — limiting an Employee's use of costly Tools.
