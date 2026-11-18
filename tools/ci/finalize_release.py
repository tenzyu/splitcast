#!/usr/bin/env python3
"""Mark a SplitCast Nix release verified only after its independent consumer passed.

Privileged metadata updater: no untrusted build artifacts are executed, and no
release asset, tag, signature, source revision, or prerelease status is altered.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
from urllib.request import urlopen

REPO = "tenzyu/splitcast"
SHA = re.compile(r"[0-9a-f]{40}\Z")
REQUIRED_JOBS = {"build", "publish", "consume"}


def api(path, payload=None):
    command = ["gh", "api", f"repos/{REPO}/{path}"]
    if payload is not None:
        command += ["--method", "PATCH", "--input", "-"]
    completed = subprocess.run(
        command, input=json.dumps(payload) if payload is not None else None,
        text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    return json.loads(completed.stdout)


def latest_successful_run():
    data = api("actions/workflows/release-cache.yml/runs"
               "?branch=main&event=workflow_dispatch&status=success&per_page=20")
    for run in data.get("workflow_runs", []):
        if run.get("status") == "completed" and run.get("conclusion") == "success":
            return run["id"]
    return None


def validate_run(run, jobs):
    sha = run.get("head_sha", "")
    if (run.get("name") != "Release Nix cache"
            or run.get("event") != "workflow_dispatch"
            or run.get("head_branch") != "main"
            or run.get("status") != "completed"
            or run.get("conclusion") != "success"
            or not SHA.fullmatch(sha)
            or run.get("head_repository", {}).get("full_name") != REPO
            or run.get("path") != ".github/workflows/release-cache.yml"):
        raise ValueError("not a successful release-cache workflow from trusted main")
    attempt = run.get("run_attempt")
    if type(attempt) is not int or attempt < 1:
        raise ValueError("invalid workflow attempt")
    results = {}
    for job in jobs.get("jobs", []):
        name = job.get("name")
        if name in REQUIRED_JOBS:
            if name in results or job.get("run_attempt") != attempt:
                raise ValueError("duplicate or stale release job")
            results[name] = job.get("conclusion")
    if results != {name: "success" for name in REQUIRED_JOBS}:
        raise ValueError(f"build/publish/consume did not all pass: {results}")
    return sha, attempt


def validate_release(release, ref, manifest, run, sha, attempt):
    tag = f"nix-cache-{sha}-{run['id']}-{attempt}"
    if (release.get("tag_name") != tag
            or release.get("target_commitish") != sha
            or release.get("draft") is not False
            or release.get("prerelease") is not True
            or release.get("immutable") is True
            or ref.get("object", {}).get("sha") != sha
            or ref.get("ref") != f"refs/tags/{tag}"):
        raise ValueError("release/tag/source identity or publication flags differ")
    manifest_sha = hashlib.sha256(manifest).hexdigest()
    assets = [a for a in release.get("assets", []) if a.get("name") == "manifest.json"]
    if len(assets) != 1 or assets[0].get("digest") != "sha256:" + manifest_sha:
        raise ValueError("public manifest differs from GitHub's asset digest")
    document = json.loads(manifest)
    if (document.get("version") != 1
            or document.get("revision", {}).get("sha") != sha
            or document.get("revision", {}).get("system") != "x86_64-linux"
            or set(document.get("roots", {})) != {"native"}
            or len(document.get("custom", [])) != 3
            or not set(document["roots"].values()) <= set(document.get("closure", {}))):
        raise ValueError("published manifest does not describe tested roots")
    return tag, manifest_sha


def update_verified(run_id):
    run = api(f"actions/runs/{run_id}")
    attempt = run.get("run_attempt")
    jobs = api(f"actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100")
    sha, attempt = validate_run(run, jobs)
    tag = f"nix-cache-{sha}-{run_id}-{attempt}"
    release = api(f"releases/tags/{tag}")
    ref = api(f"git/ref/tags/{tag}")
    manifest_url = f"https://github.com/{REPO}/releases/download/{tag}/manifest.json"
    with urlopen(manifest_url, timeout=60) as response:
        manifest = response.read()
    tag, digest = validate_release(release, ref, manifest, run, sha, attempt)
    expected_title = f"Verified Nix delivery candidate {sha[:12]}"
    original_title = f"Unverified experimental Nix cache {sha[:12]}"
    marker = f"<!-- splitcast-cache-verified:{run_id}:{attempt} -->"
    body = release.get("body") or ""
    if marker in body:
        if release.get("name") != expected_title:
            raise ValueError("verification marker exists but release title differs")
        print(f"Already verified: {tag}")
        return
    if release.get("name") != original_title:
        raise ValueError("nonstandard release title; refuse to overwrite manual edits")
    if "# Experimental Nix cache, not deployment acceptance" not in body:
        raise ValueError("missing original release safety notice")
    verification = (
        "\n\n## Verified Nix cache delivery (automated)\n\n"
        f"- Source commit: `{sha}`\n"
        f"- Successful [workflow run](https://github.com/{REPO}/actions/runs/{run_id}/attempts/{attempt}): "
        "`build`, `publish`, and independent `consume` jobs all passed.\n"
        f"- Published manifest SHA-256: `{digest}`\n"
        "- Scope: signed Nix cache consumption on `x86_64-linux`, not production or runtime acceptance.\n\n"
        f"{marker}\n"
    )
    updated = api(f"releases/{release['id']}",
                  {"name": expected_title, "body": body + verification})
    if (updated.get("name") != expected_title
            or updated.get("tag_name") != tag
            or updated.get("draft") is not False
            or updated.get("prerelease") is not True):
        raise ValueError("unexpected GitHub release response")
    print(f"Verified: https://github.com/{REPO}/releases/tag/{tag}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--run-id", type=int, help="completed release workflow run")
    mode.add_argument("--latest", action="store_true", help="reconcile latest successful release")
    args = parser.parse_args()
    if os.environ.get("GITHUB_REPOSITORY") != REPO:
        raise ValueError("must run from approved GitHub repository")
    run_id = latest_successful_run() if args.latest else args.run_id
    if run_id is None:
        print("No completed successful cache run to verify")
        return
    update_verified(run_id)


if __name__ == "__main__":
    main()
