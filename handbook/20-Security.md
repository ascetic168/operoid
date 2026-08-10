# Chapter 20 — Security

Version: 0.1
Status: Draft

---

## 1. Purpose

Security in Operoid is not a feature added later. It is a property of the architecture itself.

Because Employees invoke Tools that act on the real world — sending messages, writing records, operating systems — the system must answer three questions at all times:

1. **Identity** — *who* is acting?
2. **Authority** — *what* are they permitted to do?
3. **Accountability** — *can we prove* what they did?

This chapter defines the concepts that answer those questions. It prescribes no specific cryptography or protocol; it defines the boundaries those mechanisms must enforce.

---

## 2. Core Concepts

### Identity

Every actor has an identity: an Employee, a human user, a connected system. Action is never anonymous. If something happened, the system knows *who* did it.

### Authority

Authority defines what an identity may do. It is granted deliberately, scoped narrowly, and withdrawn when no longer needed.

The guiding rule is **least privilege**: an Employee, a Tool, a human — each receives only the authority required for its role, and nothing more.

### Permission

Permission is the *enforcement* of authority at the point of action. When an Employee invokes a Tool, the Runtime checks permission before execution (Chapter 19). Permission is checked, never assumed.

### Accountability

Every meaningful action produces an **Event** (Chapter 14) — an immutable record of who did what, when, with what inputs. Accountability is the property that the history is complete, truthful, and tamper-evident.

---

## 3. Boundaries

Security is enforced at four boundaries:

```
1. Workspace boundary     — isolation between organizations
2. Employee boundary      — one Employee cannot exceed its Role's authority
3. Tool boundary          — a Tool cannot act beyond its Spec and permission
4. Human boundary         — human oversight over high-impact actions
```

- **Workspace** is the outermost trust boundary. Contents never leak across Workspaces except through controlled federation.
- **Employee** authority is bounded by its Role. An Employee may not reconfigure the Workspace, rewrite shared Knowledge, schedule itself, or control another Employee.
- **Tool** authority is bounded by its Spec. A Tool never decides, never exceeds its contract, never bypasses permission checks.
- **Human** oversight applies where the cost of an error is high: approvals, audits, and the ability to pause or retire an Employee.

---

## 4. The Spec / Status Principle, Applied to Security

Security benefits from the same separation used for Employees:

- **Authority Spec** — the relatively fixed grants: an Employee's Role, its permitted Capabilities, the Tools it may invoke, the operations each Tool may perform. These change slowly and are reviewed deliberately.
- **Authority Status** — what is currently in effect: whether a Tool is enabled, whether an Employee is paused, what a human has most recently approved.

Keeping these separate means authority can be versioned, audited, and rolled back — just like an Employee Spec.

---

## 5. High-Impact Actions

Some actions carry enough consequence that Employees must not perform them unilaterally. For these, Operoid requires **human-in-the-loop**:

- spending above a threshold.
- sending externally to customers or partners.
- deleting or archiving organizational assets.
- changing another Employee's authority.

For such actions, an Employee proposes; a human approves; the action executes only after approval. The approval is itself an Event, so the decision is as accountable as the action.

This propose-approve pattern also applies to **commitment creation**: when an Employee identifies something worth long-term tracking during a conversation, it may propose a Commitment (`Proposed` status, with a completion condition) that enters `Active` only after human approval (Ch.11 §5). The human may also reject it (`Rejected`).

---

## 6. Failure Modes

Security must degrade safely:

- **If permission cannot be determined, the action is denied.** The system fails closed, never open.
- **If a Tool misbehaves, it is disabled,** not trusted to self-correct.
- **If an Employee errors repeatedly, it is paused,** not allowed to continue unchecked.
- **If audit is unavailable, sensitive actions are blocked** until accountability is restored.

---

## 7. Future Extension

The Security model may grow to support:

- **Fine-grained, attribute-based authority** — permissions computed from properties rather than hardcoded.
- **Delegation chains** — structured, revocable grants of authority between Employees.
- **Cryptographic attestation** — signed proof of identity, authority, and action.
- **Threat detection** — pattern-based detection of anomalous Employee or Tool behavior.
- **Compliance reporting** — automatic generation of audit views for specific regulations or standards.
