"""Integration tests with real temporary Git repositories and a fake Codex CLI."""
import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from convergence import AXES, digest, git, locked, main, snapshot, tree
from convergence_wiki import MANIFEST, scan


CONCEPTS = ("decisions", "evidence", "poc-mission", "open-risks",
            "session-state", "dreaming", "update-protocol")
TARGET = "docs/memory/decisions.md"
HISTORY = "docs/history/original.md"


class Repository(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.tmp = Path(temp.name)
        self.repo = self.tmp / "source"
        self.repo.mkdir()
        self.env = self.tmp / "bin"
        self.env.mkdir()
        self.contract = self.tmp / "contract.json"
        self.marker = self.tmp / "invocations"
        git(self.repo, "init", "-b", "main")
        git(self.repo, "config", "user.email", "test@example.invalid")
        git(self.repo, "config", "user.name", "Harness test")
        self.write(".gitignore", "ignored/\n")
        self.write("src.bin", b"\x00\x01")
        self.write("other.txt", "unrelated\n")
        self.write("MEMORY.md", "Capsule\n")
        self.write("docs/memory/index.md", '---\nokf_version: "0.2"\n---\n'
                   + "\n".join(f"[{name}]({name}.md)" for name in CONCEPTS))
        for name in CONCEPTS:
            self.write(f"docs/memory/{name}.md", "---\ntype: Concept\n---\nParagraph\n")
        self.write(HISTORY, "Original failed run\n")
        self.base = self.commit()
        self.manifest = {
            "version": 1, "watch_from": self.base,
            "pages": {
                **{f"docs/memory/{name}.md": {"kind": "reference", "sources": ["other.txt"]} for name in CONCEPTS},
                "MEMORY.md": {"kind": "reference", "sources": ["other.txt"]},
                "docs/memory/index.md": {"kind": "reference", "sources": ["other.txt"]},
                TARGET: {"kind": "current", "sources": ["src.bin"]},
                HISTORY: {"kind": "historical", "sources": ["src.bin"]},
            },
        }
        self.write(MANIFEST, json.dumps(self.manifest))
        self.commit()
        self.contract_data = {
            "version": 1, "id": "test-work", "base": self.base, "objective": "Inspect one source",
            "scope": ["src.bin", "draft/**", MANIFEST], "invariants": ["preserve observed results"],
            "checks": [{"id": "focused", "argv": [sys.executable, "-c", "print('executed')"], "timeout_seconds": 5}],
            "manual_gates": {key: "separately verify" for key in AXES[1:]},
            "forbidden_effects": ["deployment"], "exit_condition": "all declared software checks pass",
        }

    def write(self, path, data):
        path = self.repo / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data.encode() if isinstance(data, str) else data)
        return path

    def commit(self):
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "fixture")
        return git(self.repo, "rev-parse", "HEAD").decode().strip()

    def call(self, *args, cwd=None):
        prev = Path.cwd()
        stdout = io.StringIO()
        try:
            os.chdir(cwd or self.repo)
            with contextlib.redirect_stdout(stdout):
                result = main(args)
        finally:
            os.chdir(prev)
        return result, json.loads(stdout.getvalue())

    def enroll(self, code=None, timeout=5):
        if code is not None:
            self.contract_data["checks"][0].update(argv=[sys.executable, "-c", code], timeout_seconds=timeout)
        self.contract.write_text(json.dumps(self.contract_data))
        sha = digest(self.contract.read_bytes())
        self.assertEqual(self.call("init", str(self.contract), "--accept", sha)[0], 0)
        return sha

    def work(self, action, *args, cwd=None):
        return self.call(action, str(self.contract), *args, cwd=cwd)

    def fake_codex(self, code):
        program = self.env / "codex"
        program.write_text(f"#!{sys.executable}\nfrom pathlib import Path\n"
                           f"with Path({str(self.marker)!r}).open('a') as f: f.write('called\\n')\n" + code)
        program.chmod(0o700)

    def dream(self, *args, cwd=None):
        with patch.dict(os.environ, PATH=f"{self.env}:{os.environ['PATH']}"):
            return self.call("wiki", "dream", *args, cwd=cwd)

    def stale(self):
        self.write("src.bin", b"changed")
        return self.commit()


class WorkTests(Repository):
    def test_scope_checks_each_layer_even_when_working_copy_restores_baseline(self):
        self.enroll()
        self.write("other.txt", "staged out of scope")
        git(self.repo, "add", "other.txt")
        self.write("other.txt", "unrelated\n")
        self.write("outsider.bin", b"\xff")
        self.assertEqual(self.work("check")[1]["violations"], ["other.txt", "outsider.bin"])
        git(self.repo, "add", "other.txt")
        (self.repo / "outsider.bin").unlink()
        git(self.repo, "mv", "other.txt", "src-renamed")
        self.commit()
        self.assertEqual(self.work("check")[1]["violations"], ["other.txt", "src-renamed"])

    def test_raw_working_bytes_and_index_bound_to_success(self):
        self.enroll()
        p = self.write("draft/input.bin", b"\0\xff")
        status, receipt = self.work("run", "focused")
        self.assertEqual(status, 0)
        self.assertEqual(receipt["after"], receipt["identity"]["source"])
        self.assertEqual(self.work("run", "focused")[1]["reused"], True)
        times = p.stat()
        p.write_bytes(b"\0\xfe")
        os.utime(p, ns=(times.st_atime_ns, times.st_mtime_ns))
        self.assertEqual(self.work("status")[1]["checks"]["focused"]["status"], "UNVERIFIED")
        self.assertEqual(self.work("run", "focused")[0], 0)
        git(self.repo, "add", "draft/input.bin")
        self.assertEqual(self.work("status")[0], 3)
        self.commit()
        self.assertEqual(self.work("status")[1]["checks"]["focused"]["attempts"], 0)

    def test_contract_duplicate_changed_corrupt_missing_and_traversal(self):
        sha = self.enroll()
        self.assertIn("duplicate", self.work("init", "--accept", sha)[1]["reason"])
        self.contract_data["scope"].append("other.txt")
        self.contract.write_text(json.dumps(self.contract_data))
        self.assertIn("contract changed", self.work("check")[1]["reason"])
        self.contract.write_text('{"scope": [], "scope": ["**"]}')
        self.assertIn("invalid JSON", self.work("check")[1]["reason"])
        self.contract.unlink()
        self.assertIn("missing JSON", self.work("check")[1]["reason"])
        self.contract_data["scope"] = ["../escape"]
        self.contract.write_text(json.dumps(self.contract_data))
        self.assertIn("unsafe", self.work("check")[1]["reason"])

    def test_symlink_and_large_file_fail_closed(self):
        self.enroll()
        target = self.repo / "draft" / "input"
        target.parent.mkdir()
        target.symlink_to(self.contract)
        self.assertIn("cannot read safely", self.work("check")[1]["reason"])
        target.unlink()
        with target.open("wb") as file:
            file.truncate(8 * 1024 * 1024 + 1)
        self.assertIn("exceeds", self.work("check")[1]["reason"])

    def test_three_failures_at_fixed_identity_and_evidence_based_triage(self):
        self.enroll("raise SystemExit('substitution failed: HTTP error 416')")
        for attempt in range(1, 4):
            code, row = self.work("run", "focused")
            self.assertEqual((code, row["status"], row["id"]), (2, "FAIL", f"attempt-{attempt:06}"))
        self.assertIn("three attempts", self.work("run", "focused")[1]["reason"])
        _, triaged = self.work("inspect", "attempt-000003", "--tail", "1",
                               "--classify", "INFRASTRUCTURE", "--line", "1")
        self.assertEqual(triaged["tail"], ["substitution failed: HTTP error 416"])
        self.assertEqual(self.work("status")[1]["checks"]["focused"]["triage"]["line"], 1)
        self.write("src.bin", b"repaired")
        self.assertEqual(self.work("status")[0], 3)

    def test_clean_exit_after_source_edit_keeps_distinct_before_and_after_ids(self):
        self.enroll("from pathlib import Path; Path('src.bin').write_bytes(bytes([1, 2]))")
        code, row = self.work("run", "focused")
        self.assertEqual((code, row["process"]["exit_code"], row["status"]), (2, 0, "FAIL"))
        self.assertNotEqual(row["after"]["snapshot"], row["identity"]["source"]["snapshot"])
        self.assertEqual(self.work("status")[0], 3)

    def test_runner_changed_or_unsafe_post_run_source_cannot_pass(self):
        self.enroll("from pathlib import Path; Path('src.bin').unlink(); Path('src.bin').symlink_to('other.txt')")
        code, row = self.work("run", "focused")
        self.assertEqual((code, row["process"]["exit_code"], row["status"]), (2, 0, "FAIL"))
        self.assertIsNone(row["after"])
        self.assertIn("after_error", row)
        (self.repo / "src.bin").unlink()
        self.write("src.bin", b"\0\1")
        self.assertEqual(self.work("status")[1]["receipts"], 1)

    def test_runner_timeout_kills_group_and_reports_limits(self):
        code = ("import subprocess, sys; "
                "child=subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
                "print(child.pid, flush=True)")
        self.enroll(code, timeout=1)
        _, row = self.work("run", "focused")
        observed = row["process"]
        self.assertTrue(observed["timed_out"] and observed["group_kill_sent"] and observed["leader_reaped"])
        _, inspected = self.work("inspect", row["id"], "--tail", "1")
        child = Path("/proc") / inspected["tail"][0] / "stat"
        if child.exists():
            try:
                self.assertEqual(child.read_text().rsplit(")", 1)[1].split()[0], "Z")
            except FileNotFoundError:
                pass

    def test_log_private_redacted_and_tampering_rejected(self):
        self.enroll("print('token=very-private'); print('check executed')")
        _, row = self.work("run", "focused")
        folder = self.repo / ".git/convergence/test-work"
        log = folder / row["log"]["name"]
        self.assertEqual(log.stat().st_mode & 0o777, 0o600)
        self.assertNotIn(b"very-private", log.read_bytes())
        self.assertEqual(self.work("inspect", row["id"], "--tail", "2")[1]["tail"],
                         ["[redacted]", "check executed"])
        log.write_text("edited")
        self.assertIn("log missing or changed", self.work("status")[1]["reason"])

    def test_worktree_state_is_local_under_concurrent_lock(self):
        sha = self.enroll()
        other = self.tmp / "linked"
        git(self.repo, "worktree", "add", "--detach", str(other))
        with locked(self.repo / ".git/convergence/test-work"):
            self.assertIn("another convergence", self.work("check")[1]["reason"])
            self.assertEqual(self.work("init", "--accept", sha, cwd=other)[0], 0)
            self.assertEqual(self.work("run", "focused", cwd=other)[0], 0)
        self.assertEqual(self.work("status")[0], 3)

    def test_findings_deduplicate_and_require_resolution_for_blockers(self):
        self.enroll()
        self.assertEqual(self.work("run", "focused")[0], 0)
        common = ["--axis", "code", "--invariant", "recovery", "--behavior", "same failure",
                  "--owner", "maintainer", "--evidence", "review-item-1"]
        self.work("finding", *common, "--classification", "FOLLOW_UP")
        self.assertEqual(self.work("status")[0], 0)
        self.work("finding", *common, "--classification", "BLOCKER")
        self.work("finding", *common, "--classification", "BLOCKER")
        self.assertEqual(len(self.work("status")[1]["findings"]), 1)
        self.assertEqual(self.work("finding", *common, "--classification", "BLOCKER",
                                   "--disposition", "DEFERRED")[0], 2)
        self.assertEqual(self.work("status")[0], 2)
        self.work("finding", *common, "--classification", "BLOCKER", "--disposition", "RESOLVED",
                  "--evidence", "review-item-2")
        result, report = self.work("status")
        self.assertEqual(result, 0)
        self.assertEqual(report["approvals"]["merge"], "UNVERIFIED")
        self.assertTrue(all(gate["status"] == "UNVERIFIED" for gate in report["manual_gates"].values()))
        self.work("finding", *common, "--classification", "BLOCKER", "--axis", "game-performance")
        result, report = self.work("status")
        self.assertEqual((result, report["code"], report["status"]), (2, "PASS", "BLOCKED"))
        self.assertEqual(report["manual_gates"]["game-performance"]["status"], "BLOCKED")


class WikiTests(Repository):
    def test_stale_and_unrelated_controls_and_renamed_source(self):
        d = scan(self.repo)
        self.assertEqual(d["pages"][TARGET]["status"], "UNAFFECTED")
        self.assertEqual(d["pages"][TARGET]["verification"], "UNBASELINED")
        self.write("unrelated.md", "Any paragraph")
        self.commit()
        self.assertEqual(scan(self.repo)["pages"][TARGET]["status"], "UNAFFECTED")
        self.stale()
        d = scan(self.repo)
        self.assertEqual(d["pages"][TARGET]["status"], "STALE_CANDIDATE")
        self.assertEqual(d["pages"][HISTORY]["status"], "HISTORICAL")
        git(self.repo, "mv", "src.bin", "moved.bin")
        self.commit()
        code, d = self.call("wiki", "scan")
        self.assertEqual(code, 2)
        self.assertEqual(d["pages"][TARGET]["status"], "MISSING")

    def test_existing_checker_detects_invalid_links_and_missing_coverage(self):
        self.write(TARGET, "---\ntype: Concept\n---\n[broken](absent.md)")
        status, report = self.call("wiki", "scan")
        self.assertEqual(status, 2)
        self.assertIn("absent.md", report["broken_links"][0])
        self.write("docs/memory/not-indexed.md", "---\ntype: Concept\n---\n")
        self.assertIn("missing Wiki coverage", self.call("wiki", "scan")[1]["reason"])

    def test_ci_must_pin_clean_sha_and_checks_project_links(self):
        revision = git(self.repo, "rev-parse", "HEAD").decode().strip()
        self.assertEqual(self.call("wiki", "scan", "--ci-sha", revision)[0], 0)
        self.assertEqual(self.call("wiki", "scan", "--ci-sha", self.base)[0], 2)
        self.write("outside-wiki.md", "[broken](absent.md)")
        self.assertEqual(self.call("wiki", "scan", "--ci-sha", revision)[0], 2)
        revision = self.commit()
        self.assertIn("outside-wiki.md", self.call("wiki", "scan", "--ci-sha", revision)[1]["broken_links"][0])

    def test_semantic_review_receipt_pins_page_source_and_evidence_blob(self):
        revision = self.stale()
        self.write("review.md", "Observed review")
        evidence_commit = self.commit()
        self.manifest["pages"][TARGET]["review"] = {
            "commit": revision, "evidence_commit": evidence_commit, "evidence_path": "review.md",
        }
        self.write(MANIFEST, json.dumps(self.manifest))
        state = scan(self.repo)["pages"][TARGET]
        self.assertEqual(state["verification"], "REVIEW_RECORDED")
        self.assertEqual(state["review"]["evidence_blob"], tree(self.repo, evidence_commit)["review.md"][1])
        self.write(TARGET, "---\ntype: Concept\n---\nnew page bytes")
        self.assertEqual(scan(self.repo)["pages"][TARGET]["verification"], "UNVERIFIED")

    def test_empty_queue_dry_run_and_dirty_source_never_invoke_codex(self):
        self.fake_codex("raise SystemExit(19)")
        self.assertEqual(self.dream("--execute")[1]["status"], "NO_WORK")
        self.assertEqual(self.dream("--include-unbaselined")[1]["status"], "PLAN_ONLY")
        self.assertFalse(self.marker.exists())
        self.write("src.bin", b"uncommitted")
        self.assertIn("local changes", self.dream("--execute")[1]["reason"])
        self.assertFalse(self.marker.exists())

    def test_proposal_retained_unapplied_and_deduplicated_across_worktrees(self):
        revision = self.stale()
        before = snapshot(self.repo)
        self.fake_codex(f"with Path({TARGET!r}).open('a') as f: f.write('Proposed clarification\\n')\n")
        status, report = self.dream("--execute")
        self.assertEqual((status, report.get("status")), (0, "PROPOSED_PATCH"), report)
        self.assertEqual(report["changed"], [TARGET])
        self.assertEqual(snapshot(self.repo), before)
        artifact = Path(report["patch"])
        self.assertIn(b"+Proposed clarification", artifact.read_bytes())
        self.assertEqual(artifact.stat().st_mode & 0o777, 0o600)
        other = self.tmp / "parallel"
        git(self.repo, "worktree", "add", "--detach", str(other), revision)
        self.assertEqual(self.dream("--execute", cwd=other)[1]["status"], "NO_WORK")
        self.write("other-unrelated.txt", "Unrelated addition")
        self.commit()
        self.assertEqual(self.dream("--execute")[1]["status"], "NO_WORK")
        self.assertEqual(self.marker.read_text(), "called\n")

    def test_codex_nonzero_timeout_out_of_scope_and_retry_ceiling(self):
        self.stale()
        before = snapshot(self.repo)
        self.fake_codex("raise SystemExit(20)")
        status, report = self.dream("--execute")
        self.assertEqual((status, report["process"]["exit_code"]), (2, 20))
        self.fake_codex("import time\ntime.sleep(60)\n")
        status, report = self.dream("--execute", "--timeout", "1")
        self.assertEqual(status, 2)
        self.assertTrue(report["process"]["timed_out"])
        self.fake_codex(f"Path({HISTORY!r}).write_text('rewritten')\n")
        status, report = self.dream("--execute")
        self.assertEqual(status, 2)
        self.assertIn("outside", report["reason"])
        self.assertEqual(self.dream("--execute")[1]["exhausted"], [TARGET])
        self.assertEqual(self.marker.read_text().count("called"), 3)
        self.assertEqual(snapshot(self.repo), before)
        self.assertFalse(list((self.repo / ".git/convergence-wiki").glob("*.patch")))

    def test_deprecated_frontmatter_cannot_be_enrolled_as_writable(self):
        self.write(TARGET, "---\ntype: Concept\nstatus: deprecated\n---\nOriginal evidence")
        self.commit()
        self.fake_codex("raise SystemExit(8)")
        code, report = self.dream("--execute", "--include-unbaselined")
        self.assertEqual(code, 2)
        self.assertIn("current OKF metadata", report["reason"])
        self.assertFalse(self.marker.exists())

    def test_gardener_rejects_ignored_writes(self):
        self.stale()
        self.fake_codex("Path('ignored').mkdir(); Path('ignored/file').write_text('data')\n")
        status, report = self.dream("--execute")
        self.assertEqual(status, 2)
        self.assertIn("outside", report["reason"])


if __name__ == "__main__":
    unittest.main()
