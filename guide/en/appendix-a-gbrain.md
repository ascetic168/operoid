# Appendix A: Inside GBrain

> Applies to: v0.5.11. For readers meeting GBrain for the first time: **why Operoid needs a Brain**, and how it differs from the "RAG" and "wiki search" you may have heard of. If you would rather skip the architecture and just get hands-on, jump straight to [Chapter 07](07-brains-and-models.md) and come back to this appendix later.

## Start with a Question

Ask an Employee a real company question:

> "Which supplier had a problem with our etching process last year, and how was it resolved?"

The answer is scattered across your company's documents: one meeting record mentions the anomaly, the supplier page holds this company's profile, the project page covers the follow-up. **No single page puts the three together** — a human Employee answers by digging through the material and connecting it; for an AI Employee to do the same, it needs a knowledge base that can be browsed and connected. That is the Brain.

## Three Roads: Wiki Search, RAG, and GBrain

### Road 1: Wiki Search (Keyword Full-Text Search)

Your company may already have Notion / Confluence / an internal wiki. Search it with keywords directly — where does that fall short?

- **It only finds words that were written** — searching "etching supplier" will not surface a meeting record that only says "Etch chamber anomaly"; slightly different wording and you miss it.
- **No structure between pages** — you find ten documents containing "supplier", but the system has no idea which supplier, or which meeting each one relates to.
- **Cross-page questions are unsolvable** — "which supplier + which meeting + how it was resolved" requires chaining three pages together; keyword search only hands you three disconnected results.

### Road 2: Plain Standalone RAG (chunk → embed → top-k)

RAG (Retrieval-Augmented Generation) chops documents into chunks, computes vectors, and feeds the model the top-k most similar chunks. That solves the "different wording" problem — semantic closeness is enough to hit. But wiring RAG directly into an AI Employee still leaves three things missing:

1. **Fragments, not knowledge** — what comes back is anonymous chunks of text. Which document, who wrote it, which entities it relates to — all unknown; multi-hop reasoning (supplier → meeting → resolution) becomes a gamble.
2. **Figures usually vanish outright** — chunking pipelines are helpless against images and tables; and engineering knowledge lives disproportionately inside figures.
3. **No governance** — traditional RAG has no notion of "who is asking". For the same question, the intern and the CFO retrieve the same things — in an organization that is not a feature, it is an incident.

### Road 3: GBrain — a Structured Knowledge Brain

GBrain is the open-source knowledge graph engine Operoid uses (installation and official documentation at [github.com/garrytan/gbrain](https://github.com/garrytan/gbrain#cli-standalone-no-agent)). It maintains three mutually reinforcing layers at once:

| Layer | What it is | Where it comes from |
|---|---|---|
| **Pages (wiki)** | Human-readable, typed knowledge pages — people, companies, meetings, projects, concepts… each page carries YAML front matter (`type` / `title` / `tags`) | [Document Factories](09-factories.md) convert PDFs / CSVs / images into pages; you can also write them by hand |
| **Graph** | Entity links between pages: who works at which company, which meeting mentioned which project, which figure belongs to which document (`image_of`) | `[[wikilinks]]` written in a page become edges automatically; LLM enrichment fills in links for you |
| **Vectors (embeddings)** | A semantic index of every page (and every figure), computed by a local model (EmbeddingGemma 2) | Embedded automatically at `sync` |

Queries fuse the three roads: keywords and semantics together (`ask` / `query`), hops along the graph (`think`, multi-hop synthesis), and figures brought back as standalone retrieval items when needed. The opening question's journey becomes: hit the anomaly meeting page → the graph surfaces the supplier entity → follow the timeline to the resolution report — **three pages chained into one sourced answer**.

### The Three Roads Compared

| | Wiki search | Standalone RAG | **GBrain (Operoid's Brain)** |
|---|---|---|---|
| Data shape | Whole documents | Anonymous fragments | Typed pages + entity graph |
| Finds different wording | ✗ | ✓ | ✓ |
| Cross-page multi-hop questions | ✗ | A gamble | ✓ (`think` synthesizes along the graph) |
| Figures are retrievable | ✗ | Usually ✗ | ✓ (figure vectors + captions; complex PDFs via MinerU) |
| Knows "who is asking" | ✗ | ✗ | ✓ (knowledge authorization before retrieval, see below) |
| Still human-readable and editable | ✓ | ✗ (fragments) | ✓ (pages are markdown in a git repo) |
| Does editing one page require recomputing everything? | — | Usually | No — `sync` embeds incrementally |

## Why Operoid Uses "a Brain" Rather Than a Built-In RAG Module

1. **Execution and knowledge are separated.** Operoid's Runtime manages Employee execution — waking, restoring context, sleeping; **it never manages thinking**. Knowledge is the raw material an Employee reasons with, delegated to an independent Brain: upgrade one Brain and multiple Employees benefit at once.
2. **Knowledge must have provenance and boundaries.** When an Employee answers "company matters", every sentence should be traceable to specific pages; every privileged retrieval should leave a receipt (who, which task, under which policy version, what was seen — see [Chapter 13](13-governance.md)). **Same Brain, same query, different identity → different results**; a missing policy means denial (fail closed). These are architecture, not bolted-on retriever features.
3. **The pages are yours.** Knowledge lives as markdown pages in **your own** git repo — humans can read, edit, and roll back versions; GBrain only maintains the graph and vectors on top. No black-box database locks you in.

## Where You Encounter It in Operoid

| Scenario | Chapter |
|---|---|
| Registering / creating Brains, managing sources (git repos), syncing | [Chapter 07](07-brains-and-models.md) |
| Turning documents into pages (Factories) | [Chapter 09](09-factories.md) |
| Querying the Brain directly (ask / query / think / graph-query) | [Chapter 12](12-knowledge.md) |
| Employees verifying as they answer (think / search tools, via MCP) | [Chapter 08](08-employees.md), [Chapter 12](12-knowledge.md) |
| Brain model and embedding settings | [Chapter 07](07-brains-and-models.md) |
| Claude Code connected to a whole Brain directly (full read/write, Personal Edition) | [Chapter 12](12-knowledge.md) |

## Keeping a Brain Healthy

- **sync discipline**: sync after writing (Personal Edition Factories do it automatically by default); sync = commit + sync + embed + extract.
- **doctor**: health check; **orphans** finds isolated pages (no inbound edges at all — meaning they are not wired into the knowledge network).
- **schema**: new Brains should use schema pack v2 (15 standard types); v1 Brains can migrate via "unify-types" ([Chapter 09](09-factories.md)).
- **Embedding service**: the local llama-server must stay running (see [Chapter 02](02-requirements.md)) — without it, retrieval degrades to keywords only.
- **Version history**: pages live in git; mistaken agent edits can be rolled back (`revert_version`).

## Common Misconceptions

**"Isn't GBrain just a vector database?"** No. Vectors are only one of the three layers; the graph and the typed pages are what let it answer cross-page questions and underpin governance.

**"I already have Notion / Confluence — do I still need this?"** Those are **storage**; GBrain turns stored content **into knowledge that can be reasoned over** — differently worded content still gets found, pages connect to each other, figures are searchable. And the content remains grounded in markdown / git — no platform lock-in.

**"Isn't RAG enough?"** For a personal toy, perhaps. The three hard requirements of an organizational setting — multi-hop reasoning, figure retrieval, knowledge authorization before retrieval — are exactly the three pieces standalone RAG is missing.

**"Could an Employee retrieve confidential material?"** That is exactly what "knowledge authorization before retrieval" governs: a query passes through policy first (DENY → ALLOW → fail closed), and content beyond one's authority never enters the Employee's context at all; every privileged retrieval leaves a receipt ([Chapter 13](13-governance.md)).

**"Where does my data get uploaded?"** Brain data is files on your machine (PGLite + git repo), and embeddings run through the local llama-server. The only thing that ever leaves the machine is model inference itself (the LLM endpoint you configure) — see [DEPLOYMENT.md §11.4](../../DEPLOYMENT.md) for the full list.

## Further Reading

- [GBrain (GitHub)](https://github.com/garrytan/gbrain#cli-standalone-no-agent) — the engine itself and installation (the "CLI standalone" section).
- [Architecture Handbook Ch.05 Brain](../../handbook/05-Brain.md) and [Ch.07 Knowledge](../../handbook/07-Knowledge.md) — the design reasoning.
- [CHANGELOG v0.5.0](../../CHANGELOG.md) — the complex PDF / figure retrieval pipeline and its benchmark (top-5 hit 12/12 on twelve figure questions, MRR 0.90).

---
[← Ch.18 Troubleshooting](18-troubleshooting.md) | [Index](README.md) | [Appendix B →](appendix-b-development.md)
