# Document Factories

> Applies to: v0.5.11. Factories turn raw documents into structured knowledge pages and write them into the active brain's source repo — the main entrance for knowledge intake.

## Basic flow

1. Pick the active brain and the **active source** (when there is no source, the page prompts you to add one on the "Brains" page first).
2. **Drag and drop** files onto the matching factory (or click the drop zone to pick files).
3. Conversion and writing happen automatically — "Converting and writing (LLM structuring may take a few seconds)…".
4. Click a processed file to **preview** the generated page, check it page by page, edit it, then "Overwrite this page".
5. Press **"Sync to brain"** (`sync --source <id>`, with embed/extract) — only now can the knowledge be retrieved.

"Auto commit + sync after factory write" in "Settings → Application" is **on by default** — step 5 completes automatically after the write; when it is off you must press "Sync to brain" manually. For brains with no git remote, keep "Add --no-pull to sync" enabled.

## Factory types

The built-in factories of schema pack **v1**:

| Factory | Accepts | Output |
|---|---|---|
| **Image** | jpg/png and other image files | `media/` + a mandatory annotation page (see below) |
| **people** | CSV (Google Contacts export) | `people/` |
| **companies** | txt / md / pdf | `companies/` |
| **meeting** | txt / md / pdf | `meetings/` |
| **projects** | txt / md / pdf | `projects/` |
| **concepts** | txt / md / pdf | `concepts/` |
| **inbox (quick note)** | txt / md | `inbox/` (gbrain capture; **not in the graph** — quick notes form no entity links) |
| **Auto** | csv / txt / md / pdf | auto-dispatched to the matching type based on content; when it cannot be determined, a confirmation pops up for you to choose |

**schema pack v2** (15 canonical types) can be reached via "Run unify-types" in "Settings → Models" (adding media, tweet, deal, email, analysis and other types); new brains are recommended to go straight to v2; before migrating an existing v1 brain, make sure no gbrain process is running and back up the brain database.

### The image factory

An image **must have an annotation** before it can be imported ("Every image needs an annotation"): each image is stored into `media/` and a same-name annotation page is generated, with your description as the page body. On sync, gbrain does multimodal embedding and `image_of` links — provided the embedding service has multimodal enabled (`--mmproj`, see [Chapter 02](02-requirements.md)); when it is not enabled, images are skipped on sync with a notice.

## How complex PDFs are handled

PDFs are routed automatically by signal (`pdf_convert_policy`: `auto` / `always_fast` / `always_mineru`):

- **Simple PDFs** (plain text, single column, no figures) — the fast path extracts them directly.
- **Complex PDFs** (two-column papers, scans, images) — full conversion through **MinerU**: section chunking + figure notes + vectors, with images becoming standalone retrievable items. The conversion attempts are recorded in `conversion-report.json` in the source repo, for auditing.

Without MinerU installed, complex PDFs degrade in quality (see the degradation table in [Chapter 02](02-requirements.md)); for the egress trust tiers (whether documents are sent to third-party cloud parsing) see [Chapter 13](13-governance.md).

## Adding pages directly

The "+" at each factory's top-right corner: author a page in the editor with a compliant template, no file needed (e.g. one person record). On save, the LLM completes the person/company names you wrote into `[[wikilinks]]` and uses the title as the filename; "Name & write" or "Save & Sync" does it in one step.

Enterprise mode (Enterprise C′) additionally lets you specify a **classification level** and a **circle (department/project)** when adding — writing into a higher level requires the author to hold the matching clearance.

## Output statistics

The summary after conversion lists: pages written, frozen/not overwritten (existing-page protection — human-edited content is never auto-overwritten), dropped invalid-date events, and LLM enrichment success/failure counts. The people factory also has "Rebuild companies": scan people for companies/organizations and rebuild the `companies/` pages and links (`--clean` can be added to wipe and rebuild).

---
[← 08 Managing Employees](08-employees.md) | [Index](README.md) | [10 Inbox and Events →](10-inbox-and-events.md)
