# Troubleshooting

> Applies to: v0.5.11. Principle: **read the banners and status first, then check the logs**. For enterprise deployment problems also see [DEPLOYMENT.md §9](../../DEPLOYMENT.md).

## First diagnostic steps

1. What does the **warning banner** at the top of the main window say? (Missing capabilities are spelled out there.)
2. The **capability matrix** under "Settings → Services" — which item is red? Press "Recheck".
3. Local service status: on the same page, is "Local service (oserver)" "running"?
4. Relevant logs: `obridge.log` (email problems), `conversion-report.json` in the source repo (PDF conversion problems), `oserver status` / `oserver version` (service problems).

## Common Personal Edition symptoms

| Symptom | Cause and remedy |
|---|---|
| "Missing prerequisites" at startup | git / bun / gbrain missing — install via the link in the dialog, then **reopen the app**; if a path is wrong, fix the `gbrain.exe` path under "Settings → Application" |
| "Operoid local service is not running" | Reopen the app (it brings the service up automatically); or run `oserver` manually. If the port is taken, use `OSERVER_PORT` to switch ports |
| Banner "MinerU not detected" | Complex PDFs degrade to fast mode — install with `uv tool install "mineru>4.0"`; or fill in the MinerU install directory (`mineru_dir`) under "Settings → Services" |
| Banner "Embedding model has no multimodal support" | Images are indexed by caption text only — add `--mmproj` to the llama-server launch flags (for downloading the projector GGUF see [DEPLOYMENT.md §2](../../DEPLOYMENT.md)) |
| Banner "llama-server is missing the -b/-ub 8192 flags" | Long chunks are rejected (default 512) and knowledge retrieval quality degrades — add `-b 8192 -ub 8192` to the launch command and retest. On Enterprise you can verify via `embedding.long_input_ok` from `GET /api/knowledge/health` |
| PDF conversion warning "downgraded to pdf_extract" | Every rung of the MinerU access ladder missed — `ladder.attempts` in `conversion-report.json` records why each rung missed (path / permission / version) |
| Employee does not respond when woken | Check the three prerequisites: git / bun / gbrain; whether the model API key is set (endpoint resolution under "Settings → Models"); check the "Events" page to see whether it was triggered at all |
| Employee does not receive email | See the troubleshooting section of [Chapter 14](14-email-bridge.md) (routes matching, IMAP credentials, `obridge.log`) |
| LLM suddenly stops working in service mode | The service account cannot see user environment variables — **after changing an API key you must restart the app / re-register the service** so the snapshot refreshes |
| Settings/data missing after an update | It won't happen — data lives in the settings/data directories ([Chapter 17](17-config-reference.md)); updates only replace the program. If it genuinely misbehaves, check whether you are logged into the system with a different user account |

## Common Enterprise Edition symptoms

| Symptom | Cause and remedy |
|---|---|
| Startup fails with "non-loopback requires TLS" | Complete `[tls]` in `operoid.toml` (cert + key **both present**), or bind 127.0.0.1 instead |
| Pages load but all data is empty | Check the browser console (CSP / CORS / certificates); compare the `version` from `/healthz` — the usual cause is **an old service still running**; kill the old process and restart |
| Login returns 429 | 5 consecutive wrong passwords lock the account for 15 minutes (restarting the service clears it); an admin can use "Reset password" |
| obridge behaves like an old version | Old and new binaries mixed — `exe_build_match=false` in `/api/obridge/status` is the giveaway; oserver and obridge must come from the same build (same deployment package / `cargo dev-build`) |
| Image embedding fails | Multimodal needs the mm-bridge shim (8090→8080) — see the "multimodal" section of [DEPLOYMENT.md §2](../../DEPLOYMENT.md) |

## FAQ

**Q: Forgot the administrator password?** Another admin can reset it; or rebuild the account through the API with the master token (manual procedure in [DEPLOYMENT.md §6](../../DEPLOYMENT.md)).

**Q: Can two computers share one brain?** A brain is a local directory; to go cross-machine use the Enterprise Edition (one server, everyone connects to it) — do not share the folder directly.

**Q: How do I start over completely?** Delete the brain (removing the source on the "Brains" page offers optional permanent deletion) + delete the settings/data directories (location table in [Chapter 17](17-config-reference.md)). Back up first.

**Q: Where do I report problems?** [GitHub Issues](https://github.com/ascetic168/Operoid/issues) — include the output of `oserver version` and excerpts of the relevant banners / logs.

---
[← 17 Configuration Reference](17-config-reference.md) | [Index](README.md) | [Appendix A →](appendix-a-gbrain.md)
