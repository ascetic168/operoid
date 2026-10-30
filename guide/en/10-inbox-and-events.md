# Inbox and Events

> Applies to: v0.5.11. The "Inbox" is the decision interface between you and your employees; "Events" is the system's activity log. Concept reading: [Architecture Handbook Ch.11 Commitment](../../handbook/11-Commitment.md).

## The Inbox

Entered from "Inbox" in the top bar. This is where things that need your decision are collected **across all employees**:

### Pending proposals

When an action an employee is about to take exceeds its authorization, it is not executed — instead the employee **proposes** and waits for your ruling. Each proposal can be "Approve"d or "Reject"ed, and can be expanded to show **"Why this needs approval"** — four possible reasons:

| Reason | Meaning |
|---|---|
| No authorized category matched (conservative resolution) | Strict by default: the Registry has no such category, so it goes to a human |
| Hit fuse keyword "…" | The title/condition hit a fuse keyword you set; this overrides the category tier |
| Category "…" has lapsed without re-signing | The authorization expired; it must be re-signed before it auto-activates again |
| Category "…" sits in human adjudication | You decided at registration time that this kind must be adjudicated by a human |

Approved actions accumulate into the "sampling divergence" statistics (see [Chapter 13](13-governance.md)).

Skill proposals are also approved here — skill drafts written by employees arrive in your inbox after a linter pass and a subagent pre-review (see [Chapter 11](11-skills.md)).

### Flagged employees

Employees whose state needs attention (e.g. repeated failures, needing intervention); press "View" to jump straight to that employee's watch panel.

### Other actions

- **"Delegate…"**: create a new commitment directly (title + completion condition) — same as delegating in [Chapter 08](08-employees.md).
- **"Archive"**, **"Cancel task"**, revising artifacts: manage work in progress.

## Events (the lifecycle event feed)

Entered from "Events" in the top bar. This is the live record of the **event bus**: what triggered an employee awake, when it finished its work, commitment state changes, factory-triggered reviews… When something goes wrong, this is the first stop for reconstructing the timeline of "what the system just did".

Events are a **read-only** activity stream — to take action, go back to the "Inbox" or the instance's right-click menu.

## What to do when proposals pile up

- Lots of "No authorized category matched (conservative resolution)" proposals = you have not registered categories yet: go to "Settings → Registry" and register the **recurring, low-risk** action categories as fenced-autonomous (see [Chapter 13](13-governance.md)), so routine work stops bothering you case by case.
- Let occasional high-risk proposals stay manual — that is exactly the point of strict-by-default.

---
[← 09 Document Factories](09-factories.md) | [Index](README.md) | [11 Skill Library →](11-skills.md)
