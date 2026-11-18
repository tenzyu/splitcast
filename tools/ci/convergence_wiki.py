"""Incremental provenance scan and bounded, opt-in Wiki proposals."""
from collections import Counter
from pathlib import Path
import os
import re
import tempfile

from check_docs import check_links, check_okf, frontmatter
from convergence import (Blocked, ENV_KEYS, OID, canonical, digest, document, git, git_path,
                         locked, path_parts, private_dir, process, read_file, require,
                         save, snapshot, tree)

MANIFEST = "docs/memory/provenance.json"


def pin(repo, revision):
    require(isinstance(revision, str) and OID.fullmatch(revision), "expected a full Git commit ID")
    require(git(repo, "cat-file", "-t", revision) == b"commit\n", "expected a source commit")
    git(repo, "merge-base", "--is-ancestor", revision, "HEAD")
    return revision


def local_paths(value):
    require(isinstance(value, list) and value and all(isinstance(path, str) for path in value),
            "declare nonempty explicit source paths")
    require(len(set(value)) == len(value), "duplicate source path")
    for path in value:
        path_parts(path)
        require(not any(char in path for char in "*?["), "provenance uses exact paths")
    return value


def scan(repo, *, since=None, ci_sha=None):
    source, dirty, working = snapshot(repo)
    if ci_sha is not None:
        require(source["commit"] == ci_sha and not dirty, "CI requires a clean checkout of its exact source SHA")
    data, checksum = document(repo / MANIFEST)
    require(MANIFEST in working and set(data) == {"version", "watch_from", "pages"}
            and data["version"] == 1 and isinstance(data["pages"], dict), "invalid provenance manifest v1")
    baseline = pin(repo, since or data["watch_from"])
    baseline_files = tree(repo, baseline)
    pages = data["pages"]
    concepts = {"MEMORY.md"} | {path for path in working if path.startswith("docs/memory/")
                                and path.count("/") == 2 and path.endswith(".md")}
    require(concepts <= pages.keys(), f"missing Wiki coverage: {', '.join(sorted(concepts - pages.keys()))}")
    errors = []
    results = {}
    revisions = {}
    for page, entry in sorted(pages.items()):
        path_parts(page)
        require(isinstance(entry, dict) and set(entry) <= {"kind", "sources", "review"}
                and entry.get("kind") in {"current", "reference", "historical"}, f"{page}: invalid provenance entry")
        kind = entry["kind"]
        require(page.endswith(".md") and (page in concepts or kind == "historical"), f"{page}: unsupported Wiki page")
        require((page != "MEMORY.md" and page not in {"docs/memory/index.md", "docs/memory/log.md",
                                                     "docs/memory/evidence.md"}) or kind != "current",
                f"{page}: writable Dreaming targets must be domain concepts")
        if kind == "current" and page in working:
            header = frontmatter(read_file(repo, page)[1].decode("utf-8"))
            require(header is not None and not re.search(r"^status:[ \t]*deprecated", header, re.M),
                    f"{page}: writable concept must have current OKF metadata")
        sources = local_paths(entry.get("sources"))
        require(page not in sources, f"{page}: page cannot depend on itself")
        missing = sorted({path for path in [page, *sources] if path not in working})
        if missing:
            errors.extend(f"{page}: missing source or page {path}" for path in missing)
        changed = [path for path in sources if working.get(path) != baseline_files.get(path)]
        source_revision = digest(canonical({path: working.get(path) for path in sources}))
        reviewed = None
        verification = "UNBASELINED"
        if "review" in entry:
            review = entry["review"]
            require(kind != "historical" and isinstance(review, dict)
                    and set(review) == {"commit", "evidence_commit", "evidence_path"},
                    f"{page}: review needs source and evidence commit plus evidence path")
            for key in ("commit", "evidence_commit"):
                if review[key] not in revisions:
                    revisions[review[key]] = tree(repo, pin(repo, review[key]))
            path_parts(review["evidence_path"])
            receipt = revisions[review["evidence_commit"]].get(review["evidence_path"])
            require(receipt is not None, f"{page}: missing pinned review record")
            older = revisions[review["commit"]]
            require(all(path in older for path in [page, *sources]), f"{page}: review source coverage incomplete")
            changed_since_review = [path for path in sources if working.get(path) != older[path]]
            if since is None:
                changed = changed_since_review
            reviewed = {**review, "evidence_blob": receipt[1]}
            verification = ("REVIEW_RECORDED" if not missing and not changed_since_review
                            and working[page] == older[page] else "UNVERIFIED")
        status = ("MISSING" if missing else "HISTORICAL" if kind == "historical" else
                  "STALE_CANDIDATE" if changed else "UNAFFECTED")
        results[page] = {"status": status, "verification": "HISTORICAL" if kind == "historical" else verification,
                         "kind": kind, "changed_sources": changed, "sources": sources, "review": reviewed,
                         "key": digest(canonical([page, working.get(page), source_revision, entry])),
                         "source_revision": source_revision}
    docs = [repo / page for page in sorted(working) if page.endswith(".md") and (ci_sha or page in pages)]
    count, broken_links = check_links(docs, repo)
    metadata = check_okf(repo)
    require(snapshot(repo)[0] == source and document(repo / MANIFEST)[1] == checksum,
            "source or manifest changed during scan")
    return {"source": source, "watch_from": baseline, "manifest": checksum, "dirty": dirty,
            "pages": results, "local_links": count, "broken_links": broken_links,
            "errors": errors + metadata}


def gardener(repo, batch, result, args, store):
    source = result["source"]
    revision = source["commit"]
    pages = result["pages"]
    targets = {page: pages[page]["sources"] for page in batch}
    prompt = (
        "Propose SplitCast Wiki corrections for source commit " + revision + ".\n"
        "First read MEMORY.md and the Dreaming and update protocols under docs/memory. "
        "Inspect only the named pages, their relevant declared sources and linked evidence as needed. "
        "Edit only the named pages, preserving dated failures, evidence, role scopes and unknowns. "
        "Source changes are candidates. Leave accurate paragraphs and unaffected prose intact. "
        "Use concrete exact source and evidence references. Never change baselines, rewrite historical "
        "handoffs, delegate, publish, run application tests or perform network, host or Git write operations. "
        "An empty diff is valid.\n" + "\n".join(
            f"{page}\n  current sources: {', '.join(sources)}\n"
            f"  changed sources: {', '.join(pages[page]['changed_sources']) or '(first review)'}"
            for page, sources in targets.items()
        )
    )
    outcome = {"source": source, "targets": batch}
    # Separate refs/config/index from the operator checkout. Only local Git objects are shared.
    with tempfile.TemporaryDirectory(prefix="worktree-", dir=store) as temporary:
        wt = Path(temporary) / "checkout"
        git(repo, "clone", "--shared", "--no-checkout", "--template=", "--", str(repo), str(wt))
        git(wt, "remote", "remove", "origin")
        git(wt, "checkout", "--detach", revision)
        initial, dirty, _ = snapshot(wt, include_ignored=True)
        require(initial == source and not dirty, "gardener checkout identity mismatch")
        config = read_file(wt / ".git", "config")
        argv = ["codex", "--ask-for-approval", "never", "exec", "--ephemeral",
                "--ignore-user-config", "--ignore-rules", "--sandbox", "workspace-write",
                "-c", "sandbox_workspace_write.network_access=false",
                "-c", "sandbox_workspace_write.writable_roots=[]", "-"]
        env = {key: os.environ[key] for key in (*ENV_KEYS, "CODEX_HOME", "XDG_CONFIG_HOME") if key in os.environ}
        execution, output = process(wt, argv, args.timeout, env, prompt=prompt.encode())
        outcome.update(process=execution, output_bytes=len(output))
        if execution["exit_code"] != 0 or execution.get("error"):
            return {**outcome, "status": "BLOCKED", "reason": "Codex failed; no candidate saved"}
        try:
            require(read_file(wt / ".git", "config") == config
                    and read_file(wt / ".git", "info/attributes") is None,
                    "gardener changed Git configuration")
            observed, changed, work = snapshot(wt, revision, include_ignored=True)
            require(observed["commit"] == revision, "gardener changed Git HEAD")
            index_changed = git(wt, "diff", "--cached", "--name-only", "-z", revision, "--")
            require(not index_changed, "gardener changed the index")
            require(set(changed) <= targets.keys(), "gardener wrote outside the selected concept pages")
            require(all(page in work and work[page][0] == "100644" for page in changed),
                    "gardener removed a page or changed its mode")
            require(all(read_file(wt, page)[1].decode("utf-8") for page in changed), "empty Wiki page")
            _, broken = check_links([wt / page for page in changed], wt)
            require(not broken and not check_okf(wt), "proposal failed existing Wiki link/OKF validation")
            require(snapshot(repo)[0] == source and snapshot(wt, revision, include_ignored=True)[0] == observed,
                    "source or proposal changed during Dreaming")
            if not changed:
                return {**outcome, "status": "NO_SEMANTIC_EDIT"}
            git(wt, "add", "--", *changed)
            patch = git(wt, "diff", "--cached", "--binary", "--no-ext-diff", "--no-textconv", revision, "--")
            require(len(patch) <= 256 * 1024 and patch, "missing or oversized candidate diff")
            with tempfile.NamedTemporaryFile(prefix="candidate-", suffix=".patch", dir=store, delete=False) as f:
                f.write(patch)
                patch_path = Path(f.name)
            try:
                git(repo, "apply", "--check", "--binary", str(patch_path))
                require(snapshot(repo)[0] == source, "caller changed before proposal validation")
            except BaseException:
                patch_path.unlink()
                raise
            return {**outcome, "status": "PROPOSED_PATCH", "changed": changed,
                    "patch": str(patch_path), "sha256": digest(patch), "semantic_review": "UNVERIFIED"}
        except (Blocked, UnicodeError) as error:
            return {**outcome, "status": "BLOCKED", "reason": str(error)}


def dream(repo, result, args):
    require(not result["errors"] and not result["broken_links"], "repair Wiki validation errors before Dreaming")
    pending = {page: row for page, row in result["pages"].items() if row["kind"] == "current"
               and (row["status"] == "STALE_CANDIDATE" or
                    args.include_unbaselined and row["verification"] == "UNBASELINED")}
    if not pending:
        return {"status": "NO_WORK", "source": result["source"]}, 0
    common = git_path(repo, "--git-common-dir")
    store = common / "convergence-wiki"
    if not store.exists() and not args.execute:
        return {"status": "PLAN_ONLY", "targets": list(pending)[:args.limit], "queued": len(pending)}, 0
    private_dir(store)
    with locked(store):
        eligible, exhausted, previous = [], [], {}
        for page, row in pending.items():
            attempts = sorted(store.glob(f"{row['key']}-*.json"))
            last = document(attempts[-1])[0] if attempts else {}
            previous[page] = last
            if last.get("status") in {"PROPOSED_PATCH", "NO_SEMANTIC_EDIT"}:
                continue
            if last.get("attempts", 0) >= 3 or last.get("status") == "RUNNING":
                exhausted.append(page)
            else:
                eligible.append(page)
        batch = eligible[:args.limit]
        if not batch:
            return ({"status": "BLOCKED" if exhausted else "NO_WORK", "exhausted": exhausted,
                     "retained": {page: previous[page].get("patch") for page in pending if previous[page]}},
                    2 if exhausted else 0)
        if not args.execute:
            return {"status": "PLAN_ONLY", "targets": batch, "queued": len(eligible), "exhausted": exhausted}, 0
        require(not result["dirty"] and snapshot(repo)[0] == result["source"],
                "commit or remove local changes before invoking Codex")
        paths = {page: store / f"{pending[page]['key']}-{previous[page].get('attempts', 0) + 1}.json"
                 for page in batch}
        for page, file in paths.items():
            save(file, {"status": "RUNNING", "source": result["source"],
                        "attempts": previous[page].get("attempts", 0) + 1})
        try:
            proposal = gardener(repo, batch, result, args, store)
        except (Blocked, OSError, TimeoutError) as error:
            proposal = {"status": "BLOCKED", "reason": str(error), "source": result["source"], "targets": batch}
        for page, file in paths.items():
            save(file, {**proposal, "attempts": previous[page].get("attempts", 0) + 1})
        return {**proposal, "remaining": len(eligible) - len(batch), "exhausted": exhausted}, (
            2 if proposal["status"] == "BLOCKED" else 0
        )


def bounded(low, high):
    def parse(value):
        result = int(value)
        if not low <= result <= high:
            raise ValueError(f"expected {low}..{high}")
        return result
    return parse


def add_commands(commands):
    group = commands.add_parser("wiki").add_subparsers(dest="wiki_action", required=True)
    reader = group.add_parser("scan")
    reader.add_argument("--since")
    reader.add_argument("--ci-sha", help="require clean pinned source and check all project Markdown")
    reader.add_argument("--summary", action="store_true", help="show counts instead of page records")
    writer = group.add_parser("dream")
    writer.add_argument("--execute", action="store_true", help="opt in to Codex provider use")
    writer.add_argument("--include-unbaselined", action="store_true", help="also plan first semantic reviews")
    writer.add_argument("--limit", type=bounded(1, 3), default=2)
    writer.add_argument("--timeout", type=bounded(1, 1800), default=600)


def wiki(repo, args):
    result = scan(repo, since=getattr(args, "since", None), ci_sha=getattr(args, "ci_sha", None))
    if args.wiki_action == "scan":
        code = 2 if result["errors"] or result["broken_links"] else 0
        if args.summary:
            pages = result.pop("pages")
            result["counts"] = Counter(row["status"] for row in pages.values())
            result["verification"] = Counter(row["verification"] for row in pages.values())
        return result, code
    return dream(repo, result, args)
