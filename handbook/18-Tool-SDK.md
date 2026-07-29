# Chapter 18 — Tool SDK

Version: 0.1
Status: Draft

> This chapter defines the **interface** every Tool must honor.
> No programming language. No specific protocol. The contract is what matters.

---

## 1. Purpose

The Tool SDK is the **contract between an Employee and a Tool**.

It defines what a Tool must expose to be usable inside Emploid, and what guarantees the Runtime makes around a Tool invocation. By fixing this contract, the system lets Tools be written once and used by any authorized Employee — and lets new kinds of Tools (a new driver, a new protocol) be added without changing the Employees that use them.

A capability that does not honor this contract is not a Tool. It is an ungoverned risk.

---

## 2. The Tool Interface

Every Tool must expose:

```
Tool
│
├── Spec          — a complete description (see Chapter 08)
├── Execute()     — run the operation, given permitted inputs
└── (implicit)    — return a result or a structured error
```

- **Spec** is mandatory and complete: purpose, permission, input, output, timeout, retry, logging. A Tool without a full Spec is rejected at registration.
- **Execute** is the single operation. It accepts inputs that conform to the Spec, performs the operation, and returns either a result or a structured error.

A Tool has exactly one operation surface: Execute. If a capability needs many operations, it is many Tools, each with its own Spec.

---

## 3. The Contract a Tool Must Honor

To be trusted, every Tool upholds:

- **Permission is checked before execution.** The Runtime verifies that the invoking Employee is authorized; the Tool never bypasses this.
- **Inputs are validated against the Spec.** Anything outside the declared contract is refused.
- **Timeout is enforced.** An Execute that runs too long is abandoned, not allowed to run forever.
- **Transient failure is retried — then reported.** The retry policy is part of the Spec; when retries are exhausted, a structured error is returned.
- **Every invocation is logged.** What was called, by whom, with what inputs, and what resulted — recorded as an Event.

---

## 4. What a Tool Must Not Do

A Tool must not:

- **Decide whether to act** based on its own judgment. It executes on valid request; it refuses on invalid or unauthorized request. It does not choose. (Principle 5.)
- **Exceed its declared Spec.** No hidden inputs, no undocumented side effects.
- **Hold the Employee's authority.** The Employee carries authority; the Tool carries permission to be invoked.
- **Hide failures.** Errors must be returned, not swallowed.

A Tool that violates these is not a misbehaving Tool — it is a misclassified Employee, and a security problem.

---

## 5. Tool Drivers

Tools reach the outside world through **drivers**, each of which honors the same Tool interface:

```
Tool driver
├── Local       — built into the platform
├── Protocol    — an external tool-protocol server
├── Service     — a remote HTTP-style API
├── Database    — a queryable store
├── Code        — a runtime that executes a script or program
└── Native      — direct platform integration
```

The driver is an implementation detail. To the Employee and the Runtime, every Tool looks the same: a Spec and an Execute.

---

## 6. Future Extension

The Tool SDK may grow to support:

- **Streaming execution** — for Tools that produce results progressively.
- **Composition** — declaring a Tool that orchestrates other Tools, while remaining itself a simple, decidable Tool.
- **Dynamic Spec discovery** — a Tool advertising its own Spec at connection time.
- **Budgets and quotas** — per-Employee limits on costly Tools, enforced through the SDK.
