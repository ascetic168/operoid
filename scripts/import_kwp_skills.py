#!/usr/bin/env python3
"""Import curated plugins from Anthropic knowledge-work-plugins into ocore/default-skills/.

Pipeline (per plugin):
  skills/<name>/          -> dest/<plugin>/<name>/          (drop the inner `skills/` level)
  shared/                 -> dest/<plugin>/shared/          (cross-file reference targets)
  CONNECTORS.md README.md LICENSE* -> dest/<plugin>/
  SKILL.md bodies:        rewrite `../../X` -> `../X` for targets that resolve in the new
                          layout; prepend a one-line environment note (Claude Cowork
                          surfaces -> local file tools); run the deterministic linter
                          (mirrors ocore::skills::lint_skill_text) for the import report.
Provenance: <plugin>/SOURCE.md + top-level IMPORT-REPORT.md.

Content is kept otherwise pristine (Apache-2.0 / MIT upstream; see LICENSE files).
Usage:  python scripts/import_kwp_skills.py [--source <clone-path>] [--clean]
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DEST = REPO_ROOT / "ocore" / "default-skills"
UPSTREAM_URL = "https://github.com/anthropics/knowledge-work-plugins"

# Curation: generic corporate role plugins. Excluded on purpose:
#   partner-built/* (vendor developer docs), cowork-plugin-management +
#   pdf-viewer (Claude-specific meta tooling), bio-research (niche, script
#   deps), design / engineering / small-business (available later — add to
#   this list and rerun).
PLUGINS = [
    "customer-support",
    "data",
    "enterprise-search",
    "finance",
    "human-resources",
    "legal",
    "marketing",
    "operations",
    "product-management",
    "productivity",
    "sales",
]

ENV_NOTE = (
    "> 【環境註記】本文改編自 Anthropic《knowledge-work-plugins》開源庫"
    "（Apache-2.0，出處見本外掛目錄 SOURCE.md）。原文假設 Claude Cowork 環境："
    "Artifacts／Pages／Slides 等呈現面改以本環境可用的檔案工具產出；"
    "`/plugin:command` 斜線指令改為讀取同外掛對應技能目錄的 SKILL.md；"
    "CRM／日曆／email 等連接器不存在時，依技能內建的檔案 fallback 進行。"
)

# ── deterministic linter (mirror of ocore::skills::lint_skill_text) ──────────
OVERRIDE_PHRASES = [
    "ignore previous", "ignore all previous", "ignore the above", "ignore above",
    "disregard previous", "disregard the above", "disregard all",
    "new instructions override",
    "忽略先前", "忽略之前", "忽略以上", "忽略上述", "無視先前", "無視之前",
    "無視上述", "推翻上述", "覆寫上述",
]
SECRET_KEYWORDS = ["api_key", "apikey", "api key", "password", "passwd",
                   "secret", "密碼", "金鑰", "祕密"]
OUTBOUND_TOOLS = ["send_message", "run_command"]
MAX_SKILL_MD_BYTES = 128 * 1024


def parse_frontmatter(text: str) -> tuple[bool, str, str]:
    """Mirror of the loose line parser (post folded-scalar upgrade semantics):
    returns (has_frontmatter, name, description)."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return False, "", ""
    name = desc = ""
    i = 1
    while i < len(lines):
        line = lines[i].strip()
        i += 1
        if line == "---":
            break
        if not line or ":" not in line:
            continue
        key, value = line.split(":", 1)
        value = value.strip().strip('"').strip("'").strip()
        if key.strip() == "name" and not name:
            name = value
        elif key.strip() == "description" and not desc:
            desc = value
    return True, name, desc


def lint(text: str, size: int) -> tuple[str, list[tuple[str, str]]]:
    findings: list[tuple[str, str]] = []
    lower = text.lower()
    for p in OVERRIDE_PHRASES:
        if p.lower() in lower:
            findings.append(("high", f"override_phrase:{p}"))
    for kw in SECRET_KEYWORDS:
        if kw in lower:
            findings.append(("warn", f"secret_keyword:{kw}"))
    n = lower.count("http://") + lower.count("https://")
    if n:
        findings.append(("info", f"external_url:{n}"))
    for t in OUTBOUND_TOOLS:
        if t in lower:
            findings.append(("info", f"outbound_reference:{t}"))
    has_fm, name, desc = parse_frontmatter(text)
    if not has_fm:
        findings.append(("warn", "no_frontmatter"))
    else:
        if not desc:
            findings.append(("warn", "no_description"))
        if not name:
            findings.append(("info", "no_name"))
    if size > MAX_SKILL_MD_BYTES:
        findings.append(("warn", f"oversize:{size}"))
    if any(lv == "high" for lv, _ in findings):
        risk = "high"
    elif any(lv == "warn" for lv, _ in findings):
        risk = "medium"
    else:
        risk = "low"
    return risk, findings


def rewrite_refs(body: str, plugin_dest: Path, rel_md: str, warnings: list[str]) -> str:
    """New layout drops the inner `skills/` level, so `../../X` becomes `../X`.
    Only rewrite when the target resolves in the destination plugin dir;
    verify `../<sibling>/SKILL.md` targets too, warn on anything unresolvable."""
    def sub_two(m: re.Match) -> str:
        target = m.group(1)
        if (plugin_dest / target).exists():
            return "../" + target
        warnings.append(f"{rel_md}: 無法解析的相對引用 ../../{target}（保留原文）")
        return m.group(0)

    out = re.sub(r"\.\./\.\./([A-Za-z0-9_\-][A-Za-z0-9_\-./]*)", sub_two, body)
    # Claude 外掛根變數：導入配置拔掉中層 skills/ 後，資產與技能同在外掛根下一層。
    out = out.replace("${CLAUDE_PLUGIN_ROOT}/skills/", "../")

    for m in re.finditer(r"(?<![\w./])(?:\.\./)+([A-Za-z0-9_\-][A-Za-z0-9_\-./]*)", out):
        target = m.group(1)
        if not (plugin_dest / target).exists():
            warnings.append(f"{rel_md}: 相對引用 ../{target} 在新配置不存在（保留原文，斜線指令類請以環境註記解讀）")
    return out


def inject_note(text: str) -> str:
    """Prepend the environment note right after the frontmatter close (or at top)."""
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        for i in range(1, len(lines)):
            if lines[i].strip() == "---":
                return "\n".join(lines[: i + 1]) + "\n\n" + ENV_NOTE + "\n" + "\n".join(lines[i + 1:])
    return ENV_NOTE + "\n\n" + text


def import_plugin(src_plugin: Path, dest_plugin: Path, warnings: list[str]) -> dict:
    stats = {"skills": 0, "files": 0}
    dest_plugin.mkdir(parents=True, exist_ok=True)

    for skill_dir in sorted((src_plugin / "skills").iterdir()):
        if skill_dir.is_file():
            # 散置資產（如 productivity 的 dashboard.html，被技能以
            # ${CLAUDE_PLUGIN_ROOT}/skills/<file> 引用）——放到外掛根。
            shutil.copy2(skill_dir, dest_plugin / skill_dir.name)
            continue
        if not (skill_dir / "SKILL.md").is_file():
            warnings.append(f"{src_plugin.name}: skills/{skill_dir.name} 無 SKILL.md（更深層巢狀不支援），略過")
            continue
        target = dest_plugin / skill_dir.name
        shutil.copytree(skill_dir, target, dirs_exist_ok=True)
        stats["skills"] += 1
        stats["files"] += sum(1 for _ in target.rglob("*") if _.is_file())

    for extra in ("shared", "commands", "agents"):
        if (src_plugin / extra).is_dir():
            shutil.copytree(src_plugin / extra, dest_plugin / extra, dirs_exist_ok=True)
    for f in src_plugin.iterdir():
        if f.is_file() and (f.name in ("CONNECTORS.md", "README.md") or f.name.startswith("LICENSE")):
            shutil.copy2(f, dest_plugin / f.name)

    # Normalize SKILL.md bodies (refs rewrite + env note), keep everything else.
    for md in dest_plugin.rglob("SKILL.md"):
        rel = md.relative_to(dest_plugin).as_posix()
        text = md.read_text(encoding="utf-8")
        text = rewrite_refs(text, dest_plugin, rel, warnings)
        text = inject_note(text)
        md.write_text(text, encoding="utf-8", newline="\n")
    return stats


def source_md(plugin: str, sha: str, imported_at: str) -> str:
    return (
        f"# Source\n\n- Upstream: {UPSTREAM_URL} (`{plugin}`)\n"
        f"- Commit: `{sha}`\n- Imported: {imported_at} by `scripts/import_kwp_skills.py`\n"
        "- License: Apache-2.0 (repo root LICENSE; partner plugins carry their own MIT\n"
        "  licenses — see per-directory LICENSE files)\n"
        "- Layout change vs upstream: `skills/<name>/` flattened to `<name>/`;\n"
        "  `../../` references rewritten to `../`; environment note prepended to\n"
        "  each SKILL.md. Bodies otherwise pristine.\n"
        "- Re-seed rule: oserver copies this whole directory into the company\n"
        "  library only when it is missing — delete the plugin dir and restart to\n"
        "  re-seed an updated import.\n"
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source", help="path to an existing clone of knowledge-work-plugins")
    ap.add_argument("--dest", default=str(DEFAULT_DEST))
    ap.add_argument("--clean", action="store_true", help="wipe dest plugin dirs before import")
    args = ap.parse_args()

    if args.source:
        src_root = Path(args.source).resolve()
        if not src_root.is_dir():
            print(f"source 不存在：{src_root}", file=sys.stderr)
            return 2
    else:
        src_root = Path(tempfile.mkdtemp(prefix="kwp-")) / "kwp"
        subprocess.run(["git", "clone", "--depth", "1", UPSTREAM_URL, str(src_root)], check=True)
    sha = subprocess.run(["git", "-C", str(src_root), "rev-parse", "HEAD"],
                         capture_output=True, text=True, check=True).stdout.strip()
    imported_at = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    dest_root = Path(args.dest).resolve()
    dest_root.mkdir(parents=True, exist_ok=True)

    report_lines = [f"# Import report — {UPSTREAM_URL}", f"- Commit: `{sha}`",
                    f"- Imported: {imported_at}", ""]
    warnings: list[str] = []
    lint_counts = {"high": 0, "medium": 0, "low": 0}
    total_skills = 0

    shutil.copy2(src_root / "LICENSE", dest_root / "LICENSE")
    (dest_root / "SOURCE.md").write_text(source_md("(curated set)", sha, imported_at), encoding="utf-8")

    for plugin in PLUGINS:
        src_plugin = src_root / plugin
        if not (src_plugin / "skills").is_dir():
            warnings.append(f"找不到外掛或 skills/ 目錄：{plugin}")
            continue
        dest_plugin = dest_root / plugin
        if args.clean and dest_plugin.is_dir():
            shutil.rmtree(dest_plugin)
        stats = import_plugin(src_plugin, dest_plugin, warnings)
        (dest_plugin / "SOURCE.md").write_text(source_md(plugin, sha, imported_at), encoding="utf-8")

        risks = {"high": 0, "medium": 0, "low": 0}
        for md in dest_plugin.rglob("SKILL.md"):
            text = md.read_text(encoding="utf-8")
            risk, _ = lint(text, len(text.encode("utf-8")))
            risks[risk] += 1
        for k in lint_counts:
            lint_counts[k] += risks[k]
        total_skills += stats["skills"]
        report_lines.append(
            f"- **{plugin}**: {stats['skills']} skills, {stats['files']} files; "
            f"linter high={risks['high']} medium={risks['medium']} low={risks['low']}"
        )

    report_lines += ["", f"Total: {total_skills} skills; linter {lint_counts}", "", "## Warnings"]
    report_lines += [f"- {w}" for w in warnings] or ["- none"]
    (dest_root / "IMPORT-REPORT.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")

    print("\n".join(report_lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
