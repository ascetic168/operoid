# Source

- Upstream: https://github.com/anthropics/knowledge-work-plugins (`sales`)
- Commit: `d9d8e1331e45e87e36792040e840da4fc6260b4b`
- Imported: 2026-10-27 21:29 UTC by `scripts/import_kwp_skills.py`
- License: Apache-2.0 (repo root LICENSE; partner plugins carry their own MIT
  licenses — see per-directory LICENSE files)
- Layout change vs upstream: `skills/<name>/` flattened to `<name>/`;
  `../../` references rewritten to `../`; environment note prepended to
  each SKILL.md. Bodies otherwise pristine.
- Re-seed rule: oserver copies this whole directory into the company
  library only when it is missing — delete the plugin dir and restart to
  re-seed an updated import.
