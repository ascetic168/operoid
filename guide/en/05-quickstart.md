# Quick Start

> Applies to: v0.5.11. Prerequisites: the Personal Edition is installed per [Chapter 03](03-install-personal.md) and the [prerequisites](02-requirements.md) (git / bun / gbrain) are in place. The whole run takes about 15 minutes.

## 1. Launch Operoid

Open the desktop app. It automatically brings up the local service (oserver, `127.0.0.1:7340`) — if the "Missing prerequisites" dialog appears, close the gaps per [Chapter 02](02-requirements.md) first, then come back.

## 2. Turn On Agent-OS

Employee features are currently behind a feature flag: go to "Settings → Application" and tick **"Enable Agent-OS (employee templates/instances)"**. Without it, the "Templates" and "Instances" pages will tell you Agent-OS is not enabled.

## 3. Configure the Model and API Key

Go to "Settings → Models" (GBrain's `config.json` is the source of truth; the controls here write directly into it):

1. **Main model**: fill in `provider:model` format, e.g. `zhipu:glm-5.3-flash` (the provider prefix decides which API is used). It is applied to chat and the four internal tiered tasks alike.
2. **API key**: apply for one from the corresponding provider and set it as an environment variable (`ZHIPUAI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc. — full list in [Chapter 17](17-config-reference.md)), then **restart Operoid after setting it**. The Models page shows "API key set" or "Missing API key (env var)".
3. If you want knowledge retrieval, make sure the embedding service (llama-server) is resident at `127.0.0.1:8080` — the launch command is in [Chapter 02](02-requirements.md).

## 4. Prepare a Brain

Go to the "Brains" page:

- First time: press "Add" → "Create new brain (gbrain init)" and give it a name (the embedding model of a new brain defaults to `llama-server:embeddinggemma-2`).
- Press "Set active"; then under "Sources", press "Add source" to register a git repo as the knowledge source (the target of factory writes and sync).

What a Brain is and how it differs from RAG — see [Appendix A](appendix-a-gbrain.md); for the interface details see [Chapter 07](07-brains-and-models.md).

## 5. Create an Employee Template

Go to the "Templates" page and press "Add Template":

- **Name**: for example, "Procurement Assistant".
- **Brain**: pick the Brain you just created — every Employee verifies its answers through it.
- **Job description** (optional): copied to every instance, e.g. "Handles weekly report digest and vendor inquiries".
- **Tool allowlist**: tools you do not tick are invisible to the Employee. The default set includes think (deep thinking) / search (retrieval) / send (communication); whether it may also read and write workspace files or run commands is up to you — **this is the most basic delegation boundary**.

## 6. Deploy an Instance

Go to the "Instances" page and press "Deploy Instance": pick the source template and give the instance a name (e.g. `Steve@TW`). The template is the job definition; the instance is the actual "Employee" — one template can deploy multiple instances that never interfere with each other.

## 7. Your First Conversation

Right-click the instance card → **"Chat…"**, and send a message. You will see it work: between your question and the reply, every tool call is listed — what was called, with what parameters, and how long it took.

## 8. Delegate Your First Commitment

Right-click → **"Delegate…"**: give the Commitment a title (e.g. "Summarize this week's vendor quote changes") and a completion condition (e.g. "A report with a comparison table is produced"). The Employee wakes up and works toward the goal.

Under the strict-by-default rule, actions beyond its authority are not executed directly — the Employee **proposes** them to the "Inbox" page and waits for your approval. Take a look at the "Inbox": approve or reject, and you can follow its lifecycle activity on the "Events" page.

## Next Steps

- Drop company documents into [Document Factories](09-factories.md) so your Employees have knowledge to consult.
- Go to the [Skill Library](11-skills.md) and "Adopt" a few job skills for your Employees.
- Let Employees send and receive email: [Chapter 14](14-email-bridge.md).
- Stuck on a step: [Chapter 18: Troubleshooting](18-troubleshooting.md).

---
[← 04 Installing the Enterprise Edition](04-install-enterprise.md) | [Index](README.md) | [06 Desktop App Tour →](06-desktop-tour.md)
