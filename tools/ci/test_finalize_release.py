"""Offline metadata gate tests; no token, network, or real release required."""
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "finalize_release", Path(__file__).with_name("finalize_release.py"))
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)

SHA = "a" * 40
RUN = {
    "id": 123, "head_sha": SHA, "name": "Release Nix cache",
    "path": ".github/workflows/release-cache.yml", "head_branch": "main",
    "status": "completed", "conclusion": "success", "event": "workflow_dispatch",
    "head_repository": {"full_name": "tenzyu/splitcast"}, "run_attempt": 1,
}
JOBS = {"jobs": [
    {"name": name, "conclusion": "success", "run_attempt": 1}
    for name in ("build", "publish", "consume")
]}
TAG = f"nix-cache-{SHA}-123-1"
MANIFEST = json.dumps({
    "version": 1, "revision": {"sha": SHA, "system": "x86_64-linux"},
    "roots": {"native": "/nix/store/one", "web": "/nix/store/two"},
    "closure": {"/nix/store/one": {}, "/nix/store/two": {}},
    "custom": ["a", "b", "c"],
}).encode()


class FinalizeReleaseTests(unittest.TestCase):
    def test_successful_exact_attempt_is_required(self):
        self.assertEqual(verify.validate_run(RUN, JOBS), (SHA, 1))
        invalid_runs = [
            RUN | {"conclusion": "failure"},
            RUN | {"event": "pull_request"},
            RUN | {"head_branch": "feature"},
            RUN | {"head_repository": {"full_name": "other/repo"}},
            RUN | {"run_attempt": 2},
            RUN | {"path": "other.yml"},
        ]
        for run in invalid_runs:
            with self.subTest(run=run), self.assertRaises(ValueError):
                verify.validate_run(run, JOBS)
        invalid_jobs = [
            {"jobs": JOBS["jobs"][:-1]},
            {"jobs": JOBS["jobs"] + [JOBS["jobs"][0]]},
            {"jobs": [
                {**job, "conclusion": "skipped"} if job["name"] == "consume" else job
                for job in JOBS["jobs"]
            ]},
        ]
        for jobs in invalid_jobs:
            with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                verify.validate_run(RUN, jobs)

    def test_release_manifest_and_tag_are_pinned(self):
        import hashlib
        release = {
            "tag_name": TAG, "target_commitish": SHA, "draft": False,
            "prerelease": True,
            "assets": [{
                "name": "manifest.json",
                "digest": "sha256:" + hashlib.sha256(MANIFEST).hexdigest(),
            }],
        }
        ref = {"ref": "refs/tags/" + TAG, "object": {"sha": SHA}}
        self.assertEqual(
            verify.validate_release(release, ref, MANIFEST, RUN, SHA, 1),
            (TAG, hashlib.sha256(MANIFEST).hexdigest()),
        )
        for modified in (
            release | {"draft": True},
            release | {"prerelease": False},
            release | {"target_commitish": "b" * 40},
            release | {"assets": []},
        ):
            with self.subTest(modified=modified), self.assertRaises(ValueError):
                verify.validate_release(modified, ref, MANIFEST, RUN, SHA, 1)
        with self.assertRaises(ValueError):
            verify.validate_release(
                release, ref, MANIFEST.replace(b"x86_64-linux", b"aarch64-linux"),
                RUN, SHA, 1)

    def test_only_expected_release_is_patched_and_rerun_is_noop(self):
        import hashlib
        digest = hashlib.sha256(MANIFEST).hexdigest()
        release = {
            "id": 77, "tag_name": TAG, "target_commitish": SHA,
            "draft": False, "prerelease": True,
            "name": "Unverified experimental Nix cache " + SHA[:12],
            "body": "# Experimental Nix cache, not deployment acceptance\n",
            "assets": [{"name": "manifest.json", "digest": "sha256:" + digest}],
        }
        ref = {"ref": "refs/tags/" + TAG, "object": {"sha": SHA}}
        def fetch(path, payload=None):
            if payload is not None:
                self.assertEqual(path, "releases/77")
                self.assertTrue(payload["name"].startswith("Verified Nix delivery candidate"))
                self.assertIn("<!-- splitcast-cache-verified:123:1 -->", payload["body"])
                return release | payload
            if path.startswith("actions/runs/123/attempts/"):
                return JOBS
            if path == "actions/runs/123":
                return RUN
            if path.startswith("git/ref/"):
                return ref
            return release
        class Bytes:
            def __enter__(self): return self
            def __exit__(self, *_): pass
            def read(self): return MANIFEST
        with patch.object(verify, "api", side_effect=fetch), \
             patch.object(verify, "urlopen", return_value=Bytes()):
            verify.update_verified(123)
        already = release | {
            "name": "Verified Nix delivery candidate " + SHA[:12],
            "body": release["body"] + "<!-- splitcast-cache-verified:123:1 -->",
        }
        def fetch_already(path, payload=None):
            if path.startswith("releases/tags/"):
                return already
            return fetch(path, payload)
        with patch.object(verify, "api", side_effect=fetch_already), \
             patch.object(verify, "urlopen", return_value=Bytes()):
            verify.update_verified(123)


if __name__ == "__main__":
    unittest.main()
