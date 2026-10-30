# The Three Enterprise Consoles

> Applies to: v0.5.11. Once deployment is complete (see [Chapter 04](04-install-enterprise.md)), opening `https://<server IP>:7340/` in a browser takes you to the entry page of the three frontends. Interface languages: Traditional Chinese / Simplified Chinese / English.

## Roles

| Role | Positioning | Entry |
|---|---|---|
| **admin** | System administrator: accounts, templates, skills, knowledge, mail bridge, version | `/admin` |
| **manager** | Managers: review proposals, supervise operations, knowledge governance | `/manager` |
| **user** | Regular users: own their own employees and use them day to day | `/user` |

Permissions **cascade downward**: admin ⊃ manager ⊃ user (endpoint-level RBAC). The master token is structurally an admin, intended only for initial setup and service-to-service integration.

Login is password-based: the first login **forces setting a new password** (≥8 characters); 5 consecutive failures lock the account for 15 minutes.

## The `/admin` console

| Page | What it does |
|---|---|
| **Overview** | Current system status; version card (check for updates; air-gapped environments can disable the check — see [Chapter 15](15-service-and-updates.md)) |
| **Accounts** | Create/disable/enable accounts, assign roles, reset passwords (the temporary password is shown only once) |
| **Knowledge authz** | Knowledge policy, principals and tokens, TTL grants, knowledge scopes — same as [Chapter 13](13-governance.md) |
| **Employee templates** | Organization-wide CRUD for employee templates |
| **Brains & prereq** | Register brains, configure models; prerequisite component status |
| **Skill library** | Manage the company shared library; **promote** personal-area skills to organization-wide |
| **Mail bridge** | Settings form for the email channel (mirrors `obridge.toml` — see [Chapter 14](14-email-bridge.md)) |

## The `/manager` console

| Page | What it does |
|---|---|
| **Dashboard** | Team employees and operational status |
| **Inbox** | Proposals awaiting approval (commitments, skill proposals), abnormal employees — same as [Chapter 10](10-inbox-and-events.md) |
| **Skill library** | Review skill proposals; adopt skills |
| **Knowledge governance** | Issue/revoke TTL grants (time-limited elevated knowledge authorization) |
| **Operations console** | Knowledge retrieval operations (can be disabled with `ops_retrieval_enabled=false` — retrieval should always go through employee tools) |
| **Events** | Lifecycle events; click an employee to open a conversation |

## The `/user` console

| Page | What it does |
|---|---|
| **My employees** | Recruit from templates, deploy instances, converse — the web version of the desktop's "employee templates / employee instances" |
| **Knowledge Q&A** | Ask the brain directly, in a governed way |
| **Inbox** | Approve proposals, abnormal employees |
| **Skill library** | Browse the skill library and "Adopt" skills into your personal area; submit skill proposals |
| **Factories** | Drag and drop documents into the knowledge base — the web version of the desktop Factories |
| **Events** | Lifecycle events of your own employees |

Since v0.5.9, the user console can also converse with **colleagues'** employees (access is still constrained by knowledge policy).

## Feature comparison with the Personal Edition

| Feature | Personal Edition (desktop) | Enterprise Edition (web) |
|---|---|---|
| Employee templates / instances | Two pages in the same section | admin manages templates; user deploys instances |
| Skill proposal flow | Approved in the Inbox → enters the personal area | user proposes → manager/admin approve → **admin promotes** to the shared library |
| Knowledge authorization | "Settings → Knowledge" | admin "Knowledge authz" + manager "Knowledge governance" |
| Factories | Desktop | user "Factories" |
| Claude Code handoff | Built in (can read and write the whole brain) | Retired by default (`claude_code_handoff_enabled=false`) — enterprise scenarios should not depend on this channel |
| Updates | Built into the app | Version card notice + manual upgrade |

---
[← 15 Service and Updates](15-service-and-updates.md) | [Index](README.md) | [17 Configuration Reference →](17-config-reference.md)
