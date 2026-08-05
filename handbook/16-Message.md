# Chapter 16 — Message

Version: 0.1
Status: Draft

> A Message is the interface of interaction, not the home of work. The durable outputs of work remain Artifacts; long-term responsibilities remain Commitments.

---

## 1. Purpose

A Message is **a record of one conversational turn between a human and an Employee** — a piece of text with a direction (human→employee, or employee→human).

Emploid is deliberately not a chat application: work must not be buried in conversation (Principle 3). But human–Employee collaboration (Goal 4) needs an interaction layer — a human gives direction, an Employee replies or asks back. A Message carries **the interaction itself**, while the durable results that the interaction triggers or produces still land separately as Tasks (work items), Artifacts (outputs), and Commitments (responsibilities).

A Message solves two things: (a) an Employee needs to be able to **speak** to a human — reply, ask, propose; (b) these exchanges need to be retained as reviewable interaction records, not just vanish once they become an Inbox Task.

---

## 2. Responsibilities

A Message is accountable for:

- **Carrying the text of a turn** — what was said this exchange.
- **Marking direction** — human→employee, or employee→human.
- **Timestamping** — when it happened.
- **Optionally relating** — to the Commitment it pertains to, or an Artifact it carries.

---

## 3. Owns

A Message owns:

- Its **direction** — In (human→employee) or Out (employee→human).
- Its **text content**.
- Its **timestamp**.
- Its **relations** — (optional) a Commitment, (optional) a carried Artifact.

---

## 4. Doesn't Own

A Message does **not** own:

- **The work itself** — that belongs to Task / Artifact / Commitment. A Message is the interface of trigger and review, not a container for work.
- **Durability** — a Message is an interaction record; it may be organized, summarized, or pruned. It is not a permanent artifact like an Artifact. The conclusion of an important exchange should be promoted to an Artifact or Commitment.
- **Decisions** — a Message states an interaction; it does not judge.

A Message is the interface of interaction, not the home of work.

---

## 5. Lifecycle

```
Sent → Seen (read by the recipient) → (optional) promoted to Artifact / Commitment → Pruned
```

- **Sent** — produced by a human (In) or by the Runtime on behalf of an Employee (Out).
- **Seen** — read by the recipient (a human sees it in the UI; an Employee reads it on its next wake).
- **Promotion** — if the conclusion of an exchange is durable, it should be promoted to an Artifact (output) or Commitment (responsibility). The Message itself is not kept as a permanent record.
- **Pruned** — interaction records may be summarized or cleaned up to stay readable.

---

## 6. Relationship to existing concepts

- **Inbox / Task**: a human Message (In) becomes a Task in the Employee's Inbox (a Message-driven Trigger, Ch.12) and wakes the Employee. The Message preserves the "interaction record" side; the Task is the "work item" side — two views of the same turn.
- **Event**: an Employee may also use an Event (Ch.14) to record "something happened"; but an Event is immutable history that expects no reply. A Message is **an interaction that expects a turn** — an Employee's Out Message may be a question awaiting a human reply.
- **Artifact**: if an interaction produces a durable output, that is an Artifact (Ch.06); the Message is merely the interaction that triggered it. A one-line brief reply can stay a Message and need not be promoted.

---

## 7. Future Extension

- **Group chat** — shared conversations among multiple humans and/or Employees.
- **Structured messages** — typed payloads (forms, choices) beyond plain text.
- **Retention policy** — rules for how long different Message kinds are kept and how they are summarized.
