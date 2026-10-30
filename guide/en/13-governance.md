# Governance: The Action Registry and Knowledge Authorization

> Applies to: v0.5.11. Operoid's core claim is to make governance an architectural property: the **delegation boundary** (what employees may do) is governed by the Action Registry of action categories, and the **knowledge boundary** (what employees may know) by knowledge authorization. For the theory behind it, see [The Shape of the Boundary](../../journal/The_Shape_of_the_Boundary_Charlie_Chu.pdf) and [Architecture Handbook Ch.20](../../handbook/20-Security.md).

## The Action Registry ("Settings → Registry")

The canvas of the power to draw the line: registering which **commitment categories** may activate automatically without case-by-case approval.

### Strict by default

**Unregistered categories always go to human approval.** Tightening is always allowed; widening must pay a price — named accountability, a future expiry, and widening evidence; if any of them is missing, saving is rejected. Authorization is granted per **action category**, never to "the system"; it auto-freezes after an incident.

### The three delegation tiers

| Tier | Who decides | Entry conditions |
|---|---|---|
| **Human-decided** | The employee proposes; a human approves and signs before it runs | Irreversible, impact reaching customer/spend/safety/rights, or no named accountable person |
| **Fenced-autonomous** | The employee executes directly inside a tool-allowlist fence; humans review by sampling | Reversible + internal impact + named accountability + future expiry + widening evidence |
| **Owned-autonomous** | The employee runs it routinely; humans retain accountability and the emergency stop | Fenced-tier conditions + highly routine + an emergency-stop mechanism |

### What each category registers

- **Category id and classification criterion** — what kind of commitment belongs to this category.
- **Three questions**: reversible? blast radius (internal / customer / spend / safety / rights)? accountable person (a named human owner)?
- **Expiry (re-sign date)** — authorizations expire; a category past expiry without re-signing no longer auto-activates (the interface shows a "Lapsed" warning).
- **Sampling rate** — 1.0 = all cases go to human sampling review; sampling verdicts (classified correctly / incorrectly) are marked in the employee monitoring panel.
- **Widening evidence** — on what grounds the widening rests (for example, a record from a live trial).
- **Fences**: a tool allowlist (**can only narrow**, never widen, the employee's own allowlist), No outbound, Template only.
- **Emergency stop** (required for the Owned-autonomous tier) — how to stop it immediately, e.g. "cut the Obridge channel".

### Global fuse and budget

| Setting | Default | Effect |
|---|---|---|
| **Fuse keywords** | (empty) | If a commitment's title or condition contains any keyword, it overrides the category tier and forces human approval; empty = fuse disabled |
| **Weekly proposal budget** | unlimited | Cap on how many proposals may auto-activate per week |
| **Divergence threshold** | 0.2 | Raise an alarm when the 30-day sampling divergence rate exceeds this (the Registry page shows the current divergence rate) |

Practical advice: let the system run on the strict default for one to two weeks, group **recurring, low-risk actions with a named owner** into categories, and only then register a widening; leave high-risk categories in the Human-decided tier.

## Knowledge Authorization ("Settings → Knowledge")

What employees may know is managed by four parts:

### Knowledge policy (rules)

Rules are first-match by priority: **a matching DENY rejects → a matching ALLOW grants → no match = deny (fail closed)**. Rule fields: principals, scopes, classification levels, departments (all accept multiple CSV values).

**The classification level is a hard ceiling** — scopes above a principal's clearance cannot be opened by any rule or grant. Policy authority lives in Operoid; GBrain only sees source partitions.

### Principals and API tokens

Each principal (a human, a service, an employee) can have an API token issued; **the plaintext is shown only once** — it cannot be retrieved after the dialog closes. Tokens can be rotated and revoked.

### Temporary grants (TTL grants)

When a short-term task needs elevated access, issue a **time-limited** grant (measured in hours) that expires automatically and can be revoked at any time.

### Knowledge scopes (scopes)

The scopes→sources mapping (read-only): how departments and projects map to the sources in the brain.

## The Retrieval Receipt

Every privileged retrieval leaves a receipt: **who, in which task, under which policy version, saw what**. Post-hoc auditing is not digging through logs and guessing — it is looking up receipts.

## Egress tiering for document outflow

Document conversion (complex PDFs → MinerU) is governed too: `allow_public_egress` defaults to **off** — sending to a public third-party cloud (such as mineru.net cloud parsing) requires turning it on explicitly; your self-hosted private endpoint (`mineru_remote_url`) is not gated by this switch (setting it is itself the consent). The Claude Code handoff channel can be retired entirely with `claude_code_handoff_enabled=false` (the recommendation for enterprise mode); see [Chapter 12](12-knowledge.md).

---
[← 12 Knowledge Base and Claude Code](12-knowledge.md)｜[Index](README.md)｜[14 Email Bridge (Obridge) →](14-email-bridge.md)
