# Introducing Operoid

> Applies to: v0.5.11. This chapter introduces the product positioning and the core concepts — terms that every later chapter will use.

## What Operoid Is

**Operoid is an open-source operating system for AI Employees**: AI agents — **Employees** — take on lasting responsibilities in a shared workspace, and always work within boundaries that are drawn by humans, enforced by the server, and auditable after the fact. It is built with Rust (a resident service plus a Tauri v2 desktop shell) and Vue 3, and **runs on your own hardware** — your own computer, or your company's own intranet.

Most Agent products on the market already keep working after you close the window — "always running" has become the baseline. Operoid answers a more fundamental question: ***on what grounds should I let it act?***

- What is it actually allowed to do?
- What is it allowed to know?
- When it gets things wrong, who is accountable?

Today's AI is like a brilliant **consultant**: it gives its advice and leaves — no identity, no scope of authority, no one to hold accountable. An **Employee** is different: an Employee joins the organization, owns the outcomes, and stays responsible over time. To make AI do the same, it needs what every new hire needs:

| A new hire needs | What Operoid provides |
|---|---|
| An **identity** issued by the organization and revocable at any time | Each Employee authenticates with its own token; identity comes from the token chain, not from the caller's self-declaration |
| A **delegation** stating what it may do without asking | The Action Registry: authorization by action category — strict-by-default, expiring, and requiring re-signing |
| **Permissions** stating what it may know | Knowledge authorization is enforced by server code **before retrieval**; a missing policy means deny (fail closed) |
| A **record** of everything it handles | Artifacts are versioned, Commitments are trackable; every privileged retrieval leaves a receipt |

In Operoid, these four things are **architecture**, not add-ons.

## One Codebase, Two Editions

| | Personal Edition | Enterprise Edition |
|---|---|---|
| Distribution | Desktop installer | `operoid-enterprise-*` deployment package |
| Runs on | The user's own computer | A company intranet server |
| Setup | Ready to use right after installing | One guided command: `oserver configure` |
| Access | Desktop app | Browser: `/admin`, `/manager`, `/user` — password login with role-based permissions |

For Personal Edition installation see [Chapter 03](03-install-personal.md); for the Enterprise Edition see [Chapter 04](04-install-enterprise.md). This guide focuses on the Personal Edition; day-to-day operation of the Enterprise Edition is covered together in [Chapter 16](16-enterprise-consoles.md).

## The Employee Work Cycle

An Employee wakes on a **Trigger**: a schedule, incoming email, or a message from a manager or another human colleague. After waking it restores its context, works inside its own sandboxed workspace (reading files, editing files, running commands, querying knowledge — all under the template-level authorizations you configured), and then goes back to sleep.

You can watch it work in the conversation: every tool call, its parameters, how long it took — and then a clean Markdown reply. When a decision exceeds its authority, it does not guess — it **proposes**, and a human approves.

## Core Concepts at a Glance

These terms run throughout this guide and the product interface:

| Concept | In one sentence | Read more |
|---|---|---|
| **Employee** | An AI agent with an identity and responsibilities that keeps working; a template deploys multiple independent instances | [Architecture Handbook Ch.04](../../handbook/04-Employee.md) |
| **Brain** | The knowledge body managed by the GBrain knowledge engine; Employees verify their answers against it | This guide's [Appendix A](appendix-a-gbrain.md), [Handbook Ch.05](../../handbook/05-Brain.md) |
| **Artifact** | The output of an Employee's work; versioned, owned by the workspace, not by the conversation | [Ch.06](../../handbook/06-Artifact.md) |
| **Commitment** | Something an Employee has promised to complete: it has a completion condition, survives restarts, and returns to humans automatically after repeated failures | [Ch.11](../../handbook/11-Commitment.md) |
| **Trigger** | The event source that wakes an Employee: schedules, email, messages | [Ch.12](../../handbook/12-Trigger.md) |
| **Action Registry** | The delegation boundary drawn by humans: which categories of actions may run automatically without case-by-case approval | This guide's [Chapter 13](13-governance.md), [Handbook Ch.20](../../handbook/20-Security.md) |
| **Skill** | Procedural knowledge (how a certain kind of work is done), **not** authorization; actions remain bounded by the tool allowlist | This guide's [Chapter 11](11-skills.md) |
| **Runtime (execution engine)** | Wakes the Employee, restores context, lets it work, then puts it back to sleep — manages execution, never manages thinking | [Ch.13](../../handbook/13-Runtime.md) |

## How to Use This Guide

- First contact: continue with [Chapter 02: System Requirements](02-requirements.md) → installation → [Chapter 05: Quick Start](05-quickstart.md).
- Already installed: use [Chapter 06: Desktop App Tour](06-desktop-tour.md) as a map — look up the chapter for whichever page you are on.
- Wondering "why a Brain at all": jump straight to [Appendix A](appendix-a-gbrain.md).
- Rolling out in your organization or building on top of it: [Appendix B](appendix-b-development.md).

## Further Reading

- [Architecture Handbook](../../handbook/README.md) — the constitution of this system: concepts, principles, and the reasoning behind them (Traditional Chinese).
- [DEPLOYMENT.md](../../DEPLOYMENT.md) — prerequisites, degradation behavior, and deploying both editions.
- [The Shape of Boundaries](../../journal/界線的形狀_朱國棟.pdf) — the paper behind the Action Registry, distilled from real manufacturing cases.
- [CHANGELOG](../../CHANGELOG.md) — the complete changes in every version.

---
[← Index](README.md) | [02 System Requirements →](02-requirements.md)
