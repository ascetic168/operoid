# Chapter 06 — Artifact

Version: 0.1
Status: Draft

---

## 1. Purpose

An Artifact is a **durable output of work**.

When an Employee does something meaningful, the result becomes an Artifact: a report, a spreadsheet, a drawing, a piece of code, a database record, an email, an analysis. Artifacts are how work becomes a permanent organizational asset instead of a fleeting message.

Artifacts are **first-class citizens** of the workspace. They have identity, ownership, provenance, and history. They do not live inside conversations, and they do not vanish when a session ends.

If work produces no Artifact, the work is not yet complete.

---

## 2. What Counts as an Artifact

Not every output of work is an Artifact. The principle "Artifacts are first-class citizens" is strong only if the boundary is sharp. The rule is simple:

> **An Artifact is a deliverable that the Workspace owns, can version, and can address by identity.**

Three tests must all hold:

1. **Ownership** — the Workspace holds the content itself, not merely a pointer to something held elsewhere.
2. **Versionability** — it makes sense to revise it and keep prior versions.
3. **Addressability** — other work must be able to cite it ("based on Artifact X").

If all three hold, it is an Artifact. If any one fails, it is something else.

### External effects are not Artifacts

When an Employee uses a Tool to change the outside world — writing a row to an ERP, sending an email, posting to a queue — the Workspace usually does not own the result. These are **external effects**, recorded as **Events** ("an order was created in the ERP; reference R"), optionally with a **proxy reference** so the Workspace can point back to the external thing.

The Workspace does not pretend to own, version, or serve as the source of truth for an external effect. This split keeps the Artifact store honest: it holds only what the organization genuinely owns.

### Two qualifications to "no Artifact = incomplete"

The claim in section 1 — that work without an Artifact is incomplete — is deliberately strong, but it has two limits:

- **Trivial outputs need not be Artifacts.** A one-line acknowledgement is better recorded as an Event. Forcing every output into the Artifact store produces noise and degrades the concept. An Artifact should carry enough weight to be worth versioning and citing.
- **Vigilant work is exempt.** A Commitment that watches ("keep this audit ready") may run for many cycles without producing a discrete deliverable. Such work is complete when its watch condition holds, not when an Artifact is committed. The rule applies to *productive* work, not to *vigilant* work.

---

## 3. Responsibilities

An Artifact is accountable for:

- **Carrying its content** — the actual output, in whatever form it takes.
- **Recording provenance** — which Employee produced it, when, from what inputs, under which Commitment or Task.
- **Maintaining versions** — preserving its history as it is revised.
- **Being discoverable** — so that other Employees and humans can find and reuse it.

An Artifact does not act. It records and persists.

---

## 4. Owns

An Artifact owns:

- Its **content** — the substance of the output.
- Its **metadata** — type, title, tags, language, format.
- Its **provenance** — authoring Employee, source inputs, related Task or Commitment.
- Its **version history** — every revision, with who changed what and when.
- Its **access record** — who has read or used it.

Artifacts are owned by the **Workspace**, produced by **Employees**, and may be associated with a **Project** or **Commitment**. They survive the Employee that made them.

---

## 5. Doesn't Own

An Artifact does **not** own:

- **The process that created it** — that belongs to the Employee and the Runtime.
- **Decisions about future work** — an Artifact informs decisions; it does not make them.
- **Other Artifacts** — relationships between Artifacts are tracked by the workspace, not owned by the Artifact.

An Artifact is a record and an asset. It is inert unless an Employee acts on it.

---

## 6. Lifecycle

```
Draft → Committed → Revised (v2…) → Superseded → Archived
```

- **Draft** — being produced by an Employee; not yet official.
- **Committed** — formally saved to the workspace; now a first-class asset with provenance.
- **Revised** — updated; prior versions preserved.
- **Superseded** — replaced by a newer Artifact but retained for history.
- **Archived** — retained long-term as an organizational record.

The transition from **Draft** to **Committed** is the moment work becomes real. Until an Artifact is committed, the work is provisional.

---

## 7. Future Extension

The Artifact concept may grow to support:

- **Signing and attestation** — cryptographic proof of authorship and integrity.
- **Lineage graphs** — explicit tracing of which Artifacts were derived from which others.
- **Templates** — starting points that produce consistent Artifact structures.
- **Cross-workspace sharing** — controlled publication of an Artifact to a federated workspace.
- **Lifecycle policies** — automatic archival or review based on age or type.
