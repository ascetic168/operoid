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
- **Human** oversight applies where the cost of an error is high: approvals, audits, boundary-drawing (creating and revising the Action Registry), and the ability to pause or retire an Employee.

---

## 4. The Spec / Status Principle, Applied to Security

Security benefits from the same separation used for Employees:

- **Authority Spec** — the relatively fixed grants: an Employee's Role, its permitted Capabilities, the Tools it may invoke, the operations each Tool may perform. These change slowly and are reviewed deliberately.
- **Authority Status** — what is currently in effect: whether a Tool is enabled, whether an Employee is paused, what a human has most recently approved.

Keeping these separate means authority can be versioned, audited, and rolled back — just like an Employee Spec.

---

## 5. The Delegation Boundary

### 5.1 Three Tiers of Delegation

Every category of Employee action is assessed by three questions and assigned to one of three tiers. **The unit of authorization is the action category** — not the individual situation, and not the system; the granularity of authorization determines the granularity of accountability.

- **Reversibility** — can an error be recovered before the damage spreads?
- **Blast radius** — how far does an error reach: safety, customer commitments, money, employee rights?
- **Named accountability** — when it goes wrong, does a named, sign-bearing human exist to answer for it?

| Tier | Entry conditions (all required) | Division of labor |
|---|---|---|
| Human adjudication | Irreversible; or blast radius reaches safety, customer commitments, money, or employee rights; or accountability cannot be named | The Employee proposes; the human decides and signs |
| Fenced autonomy | Reversible; limited blast radius; accountability named | The Employee executes directly within the registered whitelist and its fences; humans sample and review, watch alerts |
| Owned autonomy | Reversible; highly routine; named accountability and an emergency stop in place | The Employee acts routinely; a named human bears accountability and retains the stop switch |

**Strict by default**: any question that cannot be answered in writing — above all, accountability that cannot be named — places the category in human adjudication. Strictness is the default, not a position; widening is the decision that requires separate justification.

The following categories are **presumed** to sit in human adjudication; moving them out follows the procedure in 5.2–5.3: spending above a threshold; sending externally to customers or partners; deleting or archiving organizational assets; changing another Employee's authority; an Employee modifying its own Role, allowlist, or the registry (**the self-authorization ban**).

For human-adjudication actions, an Employee proposes; a human approves; the action executes only after approval. The approval is itself an Event, so the decision is as accountable as the action. This propose-approve pattern is the **human-adjudication path for commitment creation** (Ch.11 §5): commitment proposals falling under an autonomous-tier category of the registry may enter Active without case-by-case approval, but every such activation is recorded as an Event and is visible to the human.

### 5.2 The Power to Draw the Line, and the Registry

The authority to create, modify, extend, or revoke the delegation boundary (**the power to draw the line**) belongs to humans forever and is never delegated — an Employee may not take part in deciding its own scope of authority.

The boundary exists as an **Action Registry**: a human-signed document that lists, category by category, the description, the three-question answers, the tier, the fences, the expiry, and the evidence for widening. Verbal authorization does not exist in the audit sense.

**The boundary-sovereignty test (Criterion R)**: remove every judgment the system has made; if the boundary's current position can be reconstructed from human documents alone — the registry, the signing and event records — the boundary belongs to humans; if the system's internal traces must be inspected to determine it, the boundary has effectively changed hands. This test is a standing audit item.

### 5.3 The Boundary in Time

Boundaries rarely fail by being overthrown; they fail by being forgotten. Three mechanisms keep the boundary alive:

- **Expiry and re-signing** — every autonomous-tier category in the registry carries an expiry; if it lapses without re-signing, it falls back to human adjudication. An authorization that never needs re-signing is, in institutional terms, unowned.
- **Asymmetric revision** — tightening requires only that anyone so propose; widening requires new evidence, a named accountable bearer, and a written record. The system (the registry's save validation) should enforce this asymmetry, so it does not depend on self-restraint.
- **Automatic contraction on incident** — when a safety incident or repeated failure (such as exhausted retries) occurs, the affected category freezes automatically and requires re-signing.

### 5.4 Legitimacy Conditions for Run-Time Application

Employees and the Runtime will classify concrete situations into categories at run time. That classification is inevitable; its legitimacy rests on four conditions:

1. **Humans write the test** — the classification criteria (the registry) are a human document; the Employee answers the test, it does not write it.
2. **Conservative resolution** — doubtful situations always go to the human channel; the whitelist is a closed set: what is not enumerated is not authorized.
3. **Blind-sample calibration** — automatic classifications are subject to human sampling; when divergence exceeds a threshold, the whitelist contracts automatically.
4. **Novelty escalates** — situations that depart from precedent may not classify themselves as routine.

### 5.5 The Metrics Ban

Metrics whose substance is the movement of the boundary — "automation rate", "approval-free ratio" — must never serve as performance indicators. The movement of a boundary is not an achievement; it is an event that requires justification.

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
