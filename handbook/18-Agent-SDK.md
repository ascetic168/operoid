# Chapter 18 — Agent SDK

Version: 0.1
Status: Draft

> This chapter defines **interfaces**, not implementations.
> There is no programming language here. The point is to name the operations an Employee exposes, so that any future runtime can honor them.

---

## 1. Purpose

The Agent SDK is the **contract between the Runtime and an Employee**.

It defines what an Employee can be asked to do — start, pause, resume, cancel — and what the workspace can be asked to do on the Employee's behalf. By fixing this contract at the concept level, Operoid ensures that Employees and the Runtime can evolve independently: a new Brain, a new model, a new tool protocol, none of them requires breaking this interface.

---

## 2. Employee Lifecycle Interface

These are the operations an Employee exposes to the Runtime:

```
Employee
│
├── Run()        — begin working: process the Inbox, reason, act
├── Pause()      — suspend execution, holding state for resume
├── Resume()     — continue from where Pause stopped
└── Cancel()     — stop permanently, recording the reason
```

- **Run** is the entry point. A Trigger fires, the Runtime restores context, and Run is called.
- **Pause** and **Resume** let the Runtime manage contention and waiting without losing the Employee's place.
- **Cancel** ends the current execution cleanly, with a recorded reason.

These four are the lifecycle. Everything else the Employee does — reasoning, calling Tools, committing Artifacts — happens *inside* Run.

---

## 3. Workspace Interface

These are the operations the workspace exposes *to* an Employee, so that an Employee can change the world rather than merely think about it:

```
Workspace (as seen by an Employee)
│
├── OpenProject()     — enter the context of an initiative
├── CommitArtifact()  — persist an output as a first-class Artifact
└── PublishEvent()    — announce that something happened
```

- **OpenProject** scopes the Employee's work to a particular initiative, giving it the right context.
- **CommitArtifact** is the act that turns provisional work into a durable organizational asset. Until an Employee commits, nothing is real.
- **PublishEvent** lets the Employee signal the rest of the workspace — which may, through Triggers, wake other Employees.

An Employee that cannot commit Artifacts or publish Events can only talk. It cannot work.

---

## 4. Tool Invocation Interface

Through the workspace, an Employee may invoke Tools:

```
Tool
│
└── Execute()         — run a defined operation, subject to permission
```

- **Execute** carries the Employee's intent to a Tool. The Tool acts; the Employee owns the outcome. The Runtime enforces permission and limits around the call.

---

## 5. Design Rules

The Agent SDK follows three rules:

1. **The interface is minimal.** Run, Pause, Resume, Cancel, OpenProject, CommitArtifact, PublishEvent, Execute. If an operation is not on this list, the Employee should not be able to do it directly. Human↔employee conversational interaction (Message, Ch.16) is not on this list — an Employee's Out Message is produced by the **Runtime on its behalf** (within a conversational turn), not by a direct Employee SDK operation, so the interface stays closed.
2. **The interface is stable.** These names should outlive every implementation. Adding an operation requires architectural review.
3. **The interface respects the boundaries.** No operation lets an Employee reconfigure the Workspace, rewrite shared Knowledge, schedule itself, or control another Employee. Those powers do not belong to an Employee.

---

## 6. Future Extension

The Agent SDK may grow to support:

- **Delegation** — an Employee asking another Employee to take on a Task, through a structured operation.
- **Streaming** — progressive output during Run, for long-running work.
- **Introspection** — operations for an Employee to inspect its own state and Commitments.
- **Negotiation** — structured handshakes between collaborating Employees.

Whatever is added, the rule holds: **the SDK exposes lifecycle and workspace operations; it never exposes the ability to escape the Employee's boundary.**
