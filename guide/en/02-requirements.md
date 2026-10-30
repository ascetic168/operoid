# System Requirements

> Applies to: v0.5.11. This chapter covers platforms, prerequisites, and capability degradation for the Personal Edition (desktop); for Enterprise Edition server prerequisites see [DEPLOYMENT.md §2](../../DEPLOYMENT.md).

## Supported Platforms

| Platform | Installer | Notes |
|---|---|---|
| Windows | Setup wizard (.exe), .msi | x64; auto-update uses the .exe installer format |
| macOS | .dmg | Separate installers for Apple Silicon and Intel |
| Linux | .AppImage, .deb, .rpm | x86_64; requires system libraries such as `libwebkit2gtk-4.1` (see [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)) |

There is no official minimum hardware specification — any computer that runs modern desktop applications smoothly is enough. The local embedding service that knowledge retrieval depends on (see the next section) is a small model (EmbeddingGemma 2, 768 dimensions) with a modest resident memory footprint; if you add the multimodal projector, the BF16 build takes roughly 0.9 GB more and the Q8_0 build roughly 0.5 GB. LLM inference itself runs on the cloud or local endpoint you configure, not on this machine.

## Required Prerequisites

Knowledge features depend on three external components, **all auto-detected**; when one is missing the interface says so plainly:

| Component | Purpose | Without it | Install |
|---|---|---|---|
| **git** | The sync flow commits before updating the graph | Factories cannot sync after writing files | <https://git-scm.com/downloads> |
| **bun** | JS runtime; `gbrain` is installed and run via bun | gbrain cannot run | <https://bun.com/docs/installation#installation> |
| **gbrain** | The GBrain knowledge-graph engine CLI | No knowledge base to query | <https://github.com/garrytan/gbrain#cli-standalone-no-agent> |

Paths are auto-detected (`~/.bun/bin/gbrain.exe` on Windows) and can be overridden on the "Settings" page if needed (see [Chapter 07](07-brains-and-models.md)).

On first launch, if anything is detected missing, a "Missing prerequisites" dialog appears listing the missing components, what they are for, and install links — install them and reopen Operoid. For what GBrain is and why it is needed, see [Appendix A](appendix-a-gbrain.md).

## Optional Knowledge Pipeline Components

These components determine the **quality ceiling** of knowledge features. A missing item never blocks the pipeline (built-in degradation), but installing all of them is recommended:

| Component | Purpose | Without it |
|---|---|---|
| **llama-server** (llama.cpp, running EmbeddingGemma 2) | Local embedding backend — a hard dependency of retrieval, must stay resident (default `127.0.0.1:8080`) | Retrieval degrades to keyword-only matching |
| **mmproj projector** (optional) | Joint caption + original-image vectors for images | Images are indexed by caption text only |
| **MinerU** (optional, **strongly recommended**; version must be > 4.0) | Complex PDF parsing (two-column / scanned / with figures) — `uv tool install "mineru>4.0"` | Only very simple PDFs (plain text, single column, no figures) can be handled in fast mode; images inside documents cannot enter the knowledge base |
| **VLM chat endpoint** (depends on the chat model you configure) | The generator reads the original image directly to answer | Degrades to "locator" mode: the answer points at the figure, and a human opens the image to confirm the pixel content |

### Capability Matrix and Warning Banners

The "Settings → Services" page has a **capability matrix** (git / bun / gbrain / MinerU / embedding endpoint / multimodal / VLM); after installing something or changing settings, press "Recheck" to refresh. Missing items also appear as warning banners at the top of the main window, for example:

- "MinerU not detected - the PDF knowledge pipeline can only handle very simple PDFs (plain text, single column, no figures) in fast mode…"
- "Embedding model has no multimodal support - images are indexed by caption text only…"
- "llama-server is missing the -b/-ub 8192 flags - long chunks will be rejected…"

Banners can be temporarily collapsed with "Dismiss (hidden for this session)"; to fix them for good, install the component or adjust the launch flags as the banner describes. The full launch command for the embedding service is in [DEPLOYMENT.md §10.1](../../DEPLOYMENT.md) and [Chapter 18](18-troubleshooting.md).

## Data Flow Transparency

In the Personal Edition, programs and data all stay local: Brain data is local files, embedding traffic goes to local `127.0.0.1`, and there is no third-party telemetry. **The only thing that leaves this machine is model inference**, and every endpoint is configured by you (where each of the three built-in channels — conversation / document conversion / Claude Code — sends its data is detailed in [DEPLOYMENT.md §11.4](../../DEPLOYMENT.md); the governance perspective is covered in [Chapter 13](13-governance.md)).

---
[← 01 Introducing Operoid](01-introduction.md) | [Index](README.md) | [03 Installing the Personal Edition →](03-install-personal.md)
