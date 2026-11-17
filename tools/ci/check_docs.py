#!/usr/bin/env python3
"""Read-only Markdown link and OKF v0.2 structure checks for the active tree."""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
BUNDLE = ROOT / "docs" / "memory"
LINK = re.compile(r"(?<!!)\]\(([^)\n]+)\)")
IGNORED_DIRS = {".git", "artifacts", "result", ".direnv", "target", ".venv", "node_modules"}


def markdown_files():
    return sorted(
        p for p in ROOT.rglob("*.md")
        if not any(part in IGNORED_DIRS for part in p.relative_to(ROOT).parts)
    )


def check_links(paths):
    errors = []
    n = 0
    for doc in paths:
        for href in LINK.findall(doc.read_text(encoding="utf-8")):
            href = href.strip().split(" \"", 1)[0]
            if not href or href.startswith("#") or re.match(r"^[a-z][a-z0-9+.-]*:", href, re.I):
                continue
            try:
                local = unquote(urlsplit(href).path)
            except ValueError:
                errors.append(f"{doc.relative_to(ROOT)}: malformed link {href!r}")
                continue
            if not local:
                continue
            target = (BUNDLE if local.startswith("/") else doc.parent) / local.lstrip("/")
            if not target.exists():
                errors.append(f"{doc.relative_to(ROOT)} -> {href}: missing {target.relative_to(ROOT)}")
            n += 1
    return n, errors


def check_okf():
    errors = []
    index = BUNDLE / "index.md"
    if not index.is_file() or not index.read_text(encoding="utf-8").startswith(
        '---\nokf_version: "0.2"\n---\n'
    ):
        errors.append("docs/memory/index.md: expected root OKF v0.2 declaration")
    names = set()
    for doc in BUNDLE.glob("*.md"):
        if doc.name in {"index.md", "log.md"}:
            continue
        s = doc.read_text(encoding="utf-8")
        fm = s.split("---\n", 2)
        if not s.startswith("---\n") or len(fm) < 3:
            errors.append(f"{doc.relative_to(ROOT)}: missing YAML frontmatter")
            continue
        header = fm[1]
        if not re.search(r"^type:\s*\S+", header, flags=re.M):
            errors.append(f"{doc.relative_to(ROOT)}: missing OKF type")
        names.add(doc.name)
    for required in {"decisions.md", "evidence.md", "poc-mission.md", "open-risks.md",
                     "session-state.md", "dreaming.md", "update-protocol.md"}:
        if required not in names or f"({required})" not in index.read_text(encoding="utf-8"):
            errors.append(f"docs/memory/index.md: concept {required} unavailable or unindexed")
    return errors


def main():
    paths = markdown_files()
    n, broken = check_links(paths)
    errors = broken + check_okf()
    for problem in errors:
        print("ERROR:", problem)
    print(f"Markdown={len(paths)} local_links={n} broken={len(broken)} okf_issues={len(errors)-len(broken)}")
    raise SystemExit(bool(errors))


if __name__ == "__main__":
    main()
