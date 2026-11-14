#!/usr/bin/env python3
"""Prepare, sign, and independently consume a flat GitHub Releases Nix cache."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
from urllib.error import HTTPError
from urllib.parse import urljoin, urlparse
from urllib.request import urlopen


UPSTREAM = "https://cache.nixos.org/"
NIXOS_KEY = "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
STORE_PATH = re.compile(r"/nix/store/([0-9a-df-np-sv-z]{32})-([^/\s]+)")
ASSET_NAME = re.compile(r"[a-zA-Z0-9][a-zA-Z0-9._+-]*")
SOURCE_ASSETS = {"project-source.tar.xz", "cargo-sources.tar.xz", "obs-source.tar.xz", "nixpkgs-source.tar.xz", "THIRD_PARTY.md"}
REQUIRED_SOURCE_MEMBERS = {
    "project-source.tar.xz": {"Cargo.lock", "flake.lock", "LICENSE", "LICENSES/GPL-2.0-or-later.txt",
                              "nix/obs-websocket-loopback.patch", "nix/obs-websocket-loopback.patch.license",
                              "packages/splitcast-rs/default.nix", "packages/splitcast-rs/src/agent.rs"},
    "obs-source.tar.xz": {"./COPYING", "./plugins/obs-websocket/LICENSE",
                         "./plugins/obs-websocket/src/websocketserver/WebSocketServer.cpp"},
    "nixpkgs-source.tar.xz": {"./COPYING", "./pkgs/applications/video/obs-studio/default.nix",
                             "./pkgs/applications/video/obs-studio/fix-nix-plugin-path.patch"},
}


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def store_hash(path):
    match = STORE_PATH.fullmatch(path)
    if not match:
        raise ValueError(f"invalid store path: {path}")
    return match[1]


def fields(text):
    result = {}
    for line in text.splitlines():
        key, separator, value = line.partition(": ")
        if not separator or key in result and key != "Sig":
            raise ValueError("malformed or duplicate narinfo field")
        result[key] = value
    return result


def rewrite_url(text, url):
    if "URL" not in fields(text):
        raise ValueError("missing NAR URL")
    return "\n".join(f"URL: {url}" if line.startswith("URL: ") else line for line in text.splitlines()) + "\n"


def validate_info(text, path, expected):
    info = fields(text)
    references = {f"/nix/store/{name}" for name in info.get("References", "").split()}
    nar_hash = run("nix", "hash", "convert", "--hash-algo", "sha256", "--to", "sri", info["NarHash"])
    if (info["StorePath"] != path or nar_hash != expected["narHash"]
            or int(info["NarSize"]) != expected["narSize"]
            or references != set(expected["references"])):
        raise ValueError(f"narinfo identity mismatch: {path}")
    return info


def read_url(url):
    with urlopen(url, timeout=60) as response:
        return response.read()


def upstream_info(item):
    path, expected = item
    try:
        text = read_url(f"{UPSTREAM}{store_hash(path)}.narinfo").decode()
    except HTTPError as error:
        if error.code == 404:
            return path, None
        raise
    info = validate_info(text, path, expected)
    url = urljoin(UPSTREAM, info["URL"])
    if not url.startswith(UPSTREAM + "nar/") or urlparse(url).query or urlparse(url).fragment:
        raise ValueError(f"unexpected upstream URL: {url}")
    return path, rewrite_url(text, url)


def revision():
    if run("git", "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("release source must be committed and clean")
    return {"sha": run("git", "rev-parse", "HEAD"),
            "locks": {name: digest(name) for name in ("Cargo.lock", "flake.lock")},
            "system": "x86_64-linux"}


def roots():
    return {name: run("nix", "eval", "--raw", f".#{attribute}.outPath")
            for name, attribute in (("native", "default"), ("web", "splitcast-web"))}


def validate_manifest(manifest):
    if manifest["version"] != 1 or manifest["revision"] != revision() or manifest["roots"] != roots():
        raise ValueError("release source, locks, system, or roots differ")
    closure = manifest["closure"]
    if not closure or not set(manifest["roots"].values()) <= closure.keys():
        raise ValueError("missing root in closure")
    for path, info in closure.items():
        store_hash(path)
        if not set(info["references"]) <= closure.keys():
            raise ValueError(f"incomplete closure: {path}")
    if not set(manifest["custom"]) <= closure.keys():
        raise ValueError("custom path outside closure")
    allowed = set(manifest["roots"].values()) | {run("nix", "eval", "--raw", ".#splitcast-obs.outPath")}
    for path in manifest["custom"]:
        if path not in allowed:
            raise ValueError(f"unreviewed custom dependency: {path}")


def inventory(directory):
    result = {}
    for path in sorted(directory.iterdir()):
        if path.name == "manifest.json":
            continue
        if path.is_symlink() or not path.is_file() or not ASSET_NAME.fullmatch(path.name):
            raise ValueError(f"unsafe release asset: {path.name}")
        if path.stat().st_size >= 2_000_000_000:
            raise ValueError(f"release asset too large: {path.name}")
        result[path.name] = {"sha256": digest(path), "size": path.stat().st_size}
    if len(result) + 1 > 1000 or not SOURCE_ASSETS <= result.keys():
        raise ValueError("missing source materials or too many release assets")
    return result


def write_manifest(directory, manifest):
    manifest["assets"] = inventory(directory)
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def verify_uploaded_assets(directory, pages):
    manifest_path = directory / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    expected = manifest["assets"] | {"manifest.json": {"sha256": digest(manifest_path), "size": manifest_path.stat().st_size}}
    assets = [asset for page in pages for asset in page]
    observed = {asset["name"]: {"sha256": asset["digest"].removeprefix("sha256:"), "size": asset["size"]} for asset in assets}
    if len(assets) != len(observed) or observed != expected:
        raise ValueError("Remote release assets differ. Leave this release draft.")
    return len(assets)


def verify_index(directory, manifest, signed):
    validate_manifest(manifest)
    if inventory(directory) != manifest["assets"]:
        raise ValueError("release asset inventory differs")
    expected_infos = {store_hash(path) + ".narinfo" for path in manifest["closure"]}
    if {path.name for path in directory.glob("*.narinfo")} != expected_infos:
        raise ValueError("narinfo index differs from closure")
    for path, expected in manifest["closure"].items():
        info = validate_info((directory / (store_hash(path) + ".narinfo")).read_text(), path, expected)
        if path in manifest["custom"]:
            if not ASSET_NAME.fullmatch(info["URL"]) or info["URL"] not in manifest["assets"]:
                raise ValueError("missing custom NAR asset")
        elif not info["URL"].startswith(UPSTREAM + "nar/"):
            raise ValueError("foreign upstream NAR URL")
    paths = list(manifest["closure"]) if signed else [p for p in manifest["closure"] if p not in manifest["custom"]]
    if paths:
        flat_cache_command(directory, "store", "verify", "--no-contents", "--sigs-needed", "1",
                           "--option", "substituters", "", "--option", "trusted-public-keys", trust_keys(), *paths)
    if manifest["custom"]:
        flat_cache_command(directory, "store", "verify", "--no-trust", *manifest["custom"])


def trust_keys():
    return Path("nix/cache-public-key").read_text().strip() + " " + NIXOS_KEY


def verify_sources(directory):
    for name, required in REQUIRED_SOURCE_MEMBERS.items():
        with tarfile.open(directory / name) as archive:
            if not required <= set(archive.getnames()):
                raise ValueError(f"incomplete corresponding source archive: {name}")
    with tarfile.open(directory / "cargo-sources.tar.xz") as archive:
        names = archive.getnames()
        if not any(name.endswith("/Cargo.toml") for name in names) or not any("license" in name.lower() for name in names):
            raise ValueError("Cargo source archive lacks crate sources or licenses")
    print("Source archives contain project build inputs, patches, licenses, Cargo sources, upstream OBS, and pinned Nixpkgs expressions.")


def prune_empty_cache_directories(directory):
    for path in directory.iterdir():
        if path.is_dir() and not path.is_symlink():
            path.rmdir()


def flat_cache_command(directory, *args):
    try:
        return run("nix", *args, "--store", directory.as_uri())
    finally:
        prune_empty_cache_directories(directory)


def flatten_export(destination, custom):
    for path in custom:
        metadata = destination / (store_hash(path) + ".narinfo")
        info = fields(metadata.read_text())
        nar = destination / info["URL"]
        if nar.parent != destination / "nar" or not ASSET_NAME.fullmatch(nar.name):
            raise ValueError("unexpected Nix export layout")
        nar.rename(destination / nar.name)
        metadata.write_text(rewrite_url(metadata.read_text(), nar.name))
    prune_empty_cache_directories(destination)


def prepare(destination, sources):
    source = revision()
    selected = roots()
    built = Path("artifacts/ci/outputs.txt").read_text().splitlines()
    if built != list(selected.values()):
        raise ValueError("captured package outputs differ from evaluated roots")
    closure = json.loads(run("nix", "path-info", "--recursive", "--json", "--json-format", "1", *built))
    closure = {path: {k: info[k] for k in ("narHash", "narSize", "references")} for path, info in closure.items()}
    manifest = {"version": 1, "revision": source, "roots": selected, "closure": closure, "custom": []}
    verify_sources(sources)
    destination.mkdir(parents=True, exist_ok=False)
    for name in SOURCE_ASSETS:
        shutil.copyfile(sources / name, destination / name)
    with ThreadPoolExecutor(max_workers=8) as pool:
        for path, text in pool.map(upstream_info, sorted(closure.items())):
            if text is None:
                manifest["custom"].append(path)
            else:
                (destination / (store_hash(path) + ".narinfo")).write_text(text)
    validate_manifest(manifest)
    run("nix", "copy", "--no-recursive", "--to", destination.as_uri(), *manifest["custom"])
    flatten_export(destination, manifest["custom"])
    write_manifest(destination, manifest)
    verify_index(destination, manifest, signed=False)
    print(json.dumps({"custom": manifest["custom"], "assets": len(manifest["assets"]) + 1,
                      "manifest_sha256": digest(destination / "manifest.json")}, indent=2))


def sign(directory, expected_hash):
    private = os.environ.pop("NIX_CACHE_SIGNING_KEY")
    if digest(directory / "manifest.json") != expected_hash:
        raise ValueError("bundle is not the trusted build's manifest")
    manifest = json.loads((directory / "manifest.json").read_text())
    verify_index(directory, manifest, signed=False)
    with tempfile.TemporaryDirectory(prefix="splitcast-sign-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
        key = Path(temporary) / "key"
        key.touch(mode=0o600)
        key.write_text(private)
        del private
        public = run("nix", "key", "convert-secret-to-public", input=key.read_text())
        if public != Path("nix/cache-public-key").read_text().strip():
            raise ValueError("signing secret does not match the committed public key")
        flat_cache_command(directory, "store", "sign", "--key-file", str(key), *manifest["custom"])
    write_manifest(directory, manifest)
    verify_index(directory, manifest, signed=True)
    print(digest(directory / "manifest.json"))


def consume(url, expected_hash):
    if not url.startswith("https://github.com/tenzyu/splitcast/releases/download/"):
        raise ValueError("unapproved release origin")
    data = read_url(url + "/manifest.json")
    if hashlib.sha256(data).hexdigest() != expected_hash:
        raise ValueError("published manifest differs from the publisher's manifest")
    manifest = json.loads(data)
    validate_manifest(manifest)
    evidence = Path("artifacts/ci")
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / "consumed-manifest.json").write_bytes(data)
    for path in manifest["roots"].values():
        warm = subprocess.run(["nix", "path-info", path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if warm.returncode == 0:
            raise ValueError(f"consumer root already present: {path}")
        validate_info(read_url(url + "/" + store_hash(path) + ".narinfo").decode(), path, manifest["closure"][path])
    run("nix", "store", "verify", "--store", url, "--recursive", "--no-contents", "--sigs-needed", "1",
        "--option", "substituters", "", "--option", "trusted-public-keys", trust_keys(), *manifest["roots"].values())
    output = run("nix", "build", ".#default", ".#splitcast-web", "--no-link", "--print-out-paths",
                 "--max-jobs", "0", "--builders", "", "--option", "substituters", url,
                 "--option", "trusted-public-keys", trust_keys(), "--option", "require-sigs", "true",
                 "--option", "accept-flake-config", "false")
    if output.splitlines() != list(manifest["roots"].values()):
        raise ValueError("consumed outputs differ")
    (evidence / "consumed-outputs.txt").write_text(output + "\n")
    native = manifest["roots"]["native"] + "/bin/splitcast"
    print(run(native, "selfcheck"))
    print(run(native, "validate", "--config", "packages/splitcast-rs/example.toml"))
    print("Fresh consumer obtained both exact roots with builds disabled and signatures required.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("directory", type=Path)
    prepare_parser.add_argument("sources", type=Path)
    sign_parser = commands.add_parser("sign")
    sign_parser.add_argument("directory", type=Path)
    sign_parser.add_argument("manifest_hash")
    consume_parser = commands.add_parser("consume")
    consume_parser.add_argument("url")
    consume_parser.add_argument("manifest_hash")
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.directory.resolve(), args.sources)
    elif args.command == "sign":
        sign(args.directory.resolve(), args.manifest_hash)
    else:
        consume(args.url, args.manifest_hash)
