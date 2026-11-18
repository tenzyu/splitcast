#!/usr/bin/env python3
"""Read-only Markdown link and OKF v0.2 structure checks for the active tree."""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
LINK = re.compile(r"(?<!!)\]\(([^)\n]+)\)")
IGNORED_DIRS = {".git", "artifacts", "result", ".direnv", "target", ".venv", "node_modules"}


def markdown_files(root=ROOT):
    return sorted(
        p for p in root.rglob("*.md")
        if not any(part in IGNORED_DIRS for part in p.relative_to(root).parts)
    )


def check_links(paths, root=ROOT):
    bundle = root / "docs" / "memory"
    errors = []
    n = 0
    for doc in paths:
        if doc.is_symlink() or not doc.resolve().is_relative_to(root):
            errors.append(f"{doc.relative_to(root)}: unsafe Markdown path")
            continue
        for href in LINK.findall(doc.read_text(encoding="utf-8")):
            href = href.strip().split(" \"", 1)[0]
            if not href or href.startswith("#") or re.match(r"^[a-z][a-z0-9+.-]*:", href, re.I):
                continue
            try:
                local = unquote(urlsplit(href).path)
            except ValueError:
                errors.append(f"{doc.relative_to(root)}: malformed link {href!r}")
                continue
            if not local:
                continue
            target = (bundle if local.startswith("/") else doc.parent) / local.lstrip("/")
            if not target.resolve().is_relative_to(root) or not target.exists():
                errors.append(f"{doc.relative_to(root)} -> {href}: missing or outside repository")
            n += 1
    return n, errors


def frontmatter(text):
    parts = text.split("---\n", 2)
    return parts[1] if text.startswith("---\n") and len(parts) == 3 else None


def check_okf(root=ROOT):
    bundle = root / "docs" / "memory"
    errors = []
    index = bundle / "index.md"
    index_text = index.read_text(encoding="utf-8") if index.is_file() else ""
    if not index_text.startswith(
        '---\nokf_version: "0.2"\n---\n'
    ):
        errors.append("docs/memory/index.md: expected root OKF v0.2 declaration")
    names = set()
    for doc in bundle.glob("*.md"):
        if doc.name in {"index.md", "log.md"}:
            continue
        header = frontmatter(doc.read_text(encoding="utf-8"))
        if header is None:
            errors.append(f"{doc.relative_to(root)}: missing YAML frontmatter")
            continue
        if not re.search(r"^type:\s*\S+", header, flags=re.M):
            errors.append(f"{doc.relative_to(root)}: missing OKF type")
        names.add(doc.name)
    for required in {"decisions.md", "evidence.md", "poc-mission.md", "open-risks.md",
                     "session-state.md", "dreaming.md", "update-protocol.md"}:
        if required not in names or f"({required})" not in index_text:
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
