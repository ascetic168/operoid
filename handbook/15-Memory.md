# Chapter 15 — Memory

Version: 0.1
Status: Draft

> Context is restored, not remembered. (Principle 8.)

---

## 1. Purpose

Memory is an Employee's **working context** — the scratchpad of what is happening *right now* in its cycle of work.

It is deliberately distinct from two other things it is often confused with:

- **Knowledge** is the durable, curated expertise of the organization (Chapter 07).
- **Brain long-term memory** is accumulated experience that travels with a Brain (Chapter 05).

Memory is neither of those. Memory is short-term, operational, and personal to one Employee: who was contacted today, what a supplier promised, what a manager asked to prioritize, what is still pending. It exists so that an Employee can resume work mid-stream after a sleep, without losing its place.

---

## 2. Responsibilities

Memory is accountable for:

- **Holding the current cycle's context** — the facts an Employee needs to keep working.
- **Being persisted on sleep** — so nothing in flight is lost.
- **Being restored on wake** — so the Employee picks up exactly where it left off.
- **Staying bounded** — working memory is a scratchpad, not an archive.

---

## 3. Owns

An Employee's Memory owns:

- **Recent interactions** — messages sent and received in the current work cycle.
- **Pending follow-ups** — things awaiting a reply or a result.
- **Transient notes** — observations and intentions not yet committed elsewhere.
- **Pointers** — references to the Commitments, Tasks, and Artifacts currently in play.

Memory belongs to a single Employee. It is not shared, because no two Employees share the same working context.

---

## 4. Doesn't Own

Memory does **not** own:

- **Durable knowledge** — that belongs to the Knowledge base and the Brain.
- **Long-term experience** — that belongs to the Brain.
- **Committed results** — those belong to Artifacts.
- **Other Employees' context** — each Employee's Memory is its own.

Working memory that grows without bound is a design error. Memory that is forgotten when it should persist is a bug. The boundary is: **if it must survive beyond this cycle of work, promote it to an Artifact, a Commitment, or Knowledge.**

---

## 5. Lifecycle

```
(on Wake) Restored ──► Accumulates during Execute ──► Persisted (on Sleep) ──► Restored (next Wake)
                                                            │
                                                      Pruned / promoted
```

- **Restored** — at wake, the Runtime rebuilds the Employee's Memory from what was persisted.
- **Accumulates** — during execution, the Employee records new interactions and notes.
- **Persisted** — at sleep, the current Memory is saved so the next wake is seamless.
- **Pruned / Promoted** — items that no longer matter are dropped; items that matter long-term are promoted into Artifacts, Commitments, or Knowledge, where they belong.

Memory is meant to turn over. A healthy Employee's working memory is small, current, and focused on the work at hand.

---

## 6. Future Extension

The Memory concept may grow to support:

- **Smart promotion** — automatically suggesting when a working note should become Knowledge or an Artifact.
- **Summarization** — compressing a long working session into a compact restored context.
- **Selective recall** — restoring only the slice of Memory relevant to the incoming Task.
- **Memory sharing on delegation** — handing a relevant slice of one Employee's Memory to another when work is delegated.
