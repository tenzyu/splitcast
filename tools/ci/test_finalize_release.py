"""Offline metadata gate tests; no token, network, or real release required."""
import hashlib
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
NATIVE = "/nix/store/one"
OBS = "/nix/store/two"
WEB = "/nix/store/three"
DOCUMENT = {
    "version": 1, "revision": {"sha": SHA, "system": "x86_64-linux"},
    "roots": {"native": NATIVE},
    "closure": {NATIVE: {}, OBS: {}},
    "custom": [NATIVE, OBS],
}
MANIFEST = json.dumps(DOCUMENT).encode()
REF = {"ref": "refs/tags/" + TAG, "object": {"sha": SHA}}
PUBLISHER_BODY = (
    f"Experimental SplitCast Nix cache for commit {SHA}. "
    "Not a production or physical-media acceptance. Verify signatures, exact store outputs, "
    "source provenance and redistributability before use: "
    f"https://github.com/tenzyu/splitcast/blob/{SHA}/docs/binary-cache.md"
)
LEGACY_BODY = "# Experimental Nix cache, not deployment acceptance\n\nOriginal notes.\n"


def release_for(manifest):
    return {
        "id": 77, "tag_name": TAG, "target_commitish": SHA,
        "draft": False, "prerelease": True,
        "name": "Unverified experimental Nix cache " + SHA[:12],
        "body": PUBLISHER_BODY,
        "assets": [{
            "name": "manifest.json",
            "digest": "sha256:" + hashlib.sha256(manifest).hexdigest(),
        }],
    }


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
        release = release_for(MANIFEST)
        with self.assertRaisesRegex(ValueError, "asset digest"):
            verify.validate_release(release, REF, MANIFEST + b"\n", RUN, SHA, 1)
        with self.assertRaisesRegex(ValueError, "identity"):
            verify.validate_release(
                release, REF | {"object": {"sha": "b" * 40}}, MANIFEST, RUN, SHA, 1,
            )
        for modified in (
            release | {"draft": True},
            release | {"prerelease": False},
            release | {"target_commitish": "b" * 40},
            release | {"assets": []},
        ):
            with self.subTest(modified=modified), self.assertRaises(ValueError):
                verify.validate_release(modified, REF, MANIFEST, RUN, SHA, 1)

    def test_current_and_legacy_cache_layouts(self):
        legacy = json.dumps(DOCUMENT | {
            "roots": {"native": NATIVE, "web": WEB},
            "closure": DOCUMENT["closure"] | {WEB: {}},
            "custom": [NATIVE, OBS, WEB],
        }).encode()
        for manifest in (MANIFEST, legacy):
            with self.subTest(manifest=manifest):
                self.assertEqual(
                    verify.validate_release(release_for(manifest), REF, manifest, RUN, SHA, 1),
                    (TAG, hashlib.sha256(manifest).hexdigest()),
                )

    def test_invalid_manifest_with_matching_digest_is_rejected(self):
        for modified in (
            {"roots": {"native": NATIVE, "other": WEB}},
            {"roots": {"native": NATIVE, "web": NATIVE}},
            {"closure": {OBS: {}}},
            {"custom": [OBS, WEB]},
            {"custom": [NATIVE, WEB]},
            {"custom": [NATIVE, NATIVE]},
            {"custom": [NATIVE, OBS, WEB]},
            {"revision": DOCUMENT["revision"] | {"system": "aarch64-linux"}},
            {"revision": DOCUMENT["revision"] | {"sha": "b" * 40}},
        ):
            manifest = json.dumps(DOCUMENT | modified).encode()
            with self.subTest(modified=modified), self.assertRaisesRegex(ValueError, "tested roots"):
                verify.validate_release(release_for(manifest), REF, manifest, RUN, SHA, 1)

    def run_updater(self, release):
        reads = {
            "actions/runs/123": RUN,
            "actions/runs/123/attempts/1/jobs?per_page=100": JOBS,
            f"releases/tags/{TAG}": release,
            f"git/ref/tags/{TAG}": REF,
        }
        writes = []

        def fetch(path, payload=None):
            if payload is None:
                return reads[path]
            writes.append((path, payload))
            return release | payload

        with patch.object(verify, "api", side_effect=fetch), \
             patch.object(verify, "urlopen") as download:
            download.return_value.__enter__.return_value.read.return_value = MANIFEST
            verify.update_verified(123)
        return writes

    def test_only_expected_release_is_patched_and_rerun_is_noop(self):
        title = "Verified Nix delivery candidate " + SHA[:12]
        for body in (PUBLISHER_BODY, LEGACY_BODY):
            with self.subTest(body=body):
                release = release_for(MANIFEST) | {"body": body}
                [(path, payload)] = self.run_updater(release)
                self.assertEqual(path, "releases/77")
                self.assertEqual(set(payload), {"name", "body"})
                self.assertEqual(payload["name"], title)
                self.assertTrue(payload["body"].startswith(body))
                self.assertIn("<!-- splitcast-cache-verified:123:1 -->", payload["body"])
                self.assertEqual(self.run_updater(release | payload), [])

    def test_manual_title_and_missing_notice_are_rejected(self):
        release = release_for(MANIFEST)
        for modified, error in (
            ({"name": "Manual edit"}, "nonstandard release title"),
            ({"body": "No safety notice"}, "missing original release safety notice"),
            ({"body": PUBLISHER_BODY.replace(SHA, "b" * 40)}, "missing original release safety notice"),
        ):
            with self.subTest(modified=modified), self.assertRaisesRegex(ValueError, error):
                self.run_updater(release | modified)


if __name__ == "__main__":
    unittest.main()
