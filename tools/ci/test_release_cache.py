import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError


spec = importlib.util.spec_from_file_location("release_cache", Path(__file__).with_name("release_cache.py"))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class MetadataTests(unittest.TestCase):
    def test_uploaded_assets_must_match_all_digests_sizes_and_unique_names(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            asset = directory / "object.nar.xz"
            asset.write_text("fixture bytes")
            record = {"sha256": cache.digest(asset), "size": asset.stat().st_size}
            manifest = directory / "manifest.json"
            manifest.write_text(json.dumps({"assets": {asset.name: record}}))
            uploaded = [{"name": p.name, "digest": "sha256:" + cache.digest(p), "size": p.stat().st_size} for p in (asset, manifest)]
            self.assertEqual(cache.verify_uploaded_assets(directory, [uploaded]), 2)
            for bad in (uploaded[:-1], uploaded + uploaded[:1],
                        [uploaded[0] | {"digest": "sha256:wrong"}, uploaded[1]],
                        [uploaded[0] | {"size": 0}, uploaded[1]]):
                with self.assertRaises(ValueError):
                    cache.verify_uploaded_assets(directory, [bad])

    def test_source_asset_filenames_do_not_prove_archive_contents(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in cache.SOURCE_ASSETS:
                (directory / name).write_text("not a source archive")
            with self.assertRaises(tarfile.ReadError):
                cache.verify_sources(directory)

    def test_only_upstream_404_is_a_cache_miss(self):
        path = "/nix/store/" + "a" * 32 + "-fixture"
        for status in (403, 404, 500):
            with patch.object(cache, "read_url", side_effect=HTTPError("fixture", status, "fixture", {}, None)):
                if status == 404:
                    self.assertEqual(cache.upstream_info((path, {})), (path, None))
                else:
                    with self.assertRaises(HTTPError):
                        cache.upstream_info((path, {}))

    def test_forwarding_requires_exact_identity_and_official_nar_origin(self):
        path = "/nix/store/" + "a" * 32 + "-fixture"
        expected = {"narHash": "expected", "narSize": 42, "references": [path]}
        original = f"StorePath: {path}\nNarHash: hash\nNarSize: 42\nReferences: {Path(path).name}\nURL: nar/object.nar.xz\n"
        with patch.object(cache, "run", return_value="expected"):
            with patch.object(cache, "read_url", return_value=original.encode()):
                self.assertIn("URL: https://cache.nixos.org/nar/object.nar.xz", cache.upstream_info((path, expected))[1])
            for text in (original.replace("NarSize: 42", "NarSize: 43"),
                         original.replace("nar/object.nar.xz", "https://unapproved.invalid/object")):
                with patch.object(cache, "read_url", return_value=text.encode()):
                    with self.assertRaises(ValueError):
                        cache.upstream_info((path, expected))

    def test_only_url_changes_and_signatures_survive(self):
        original = "StorePath: path\nURL: nar/archive.nar.xz\nSig: upstream:first\nSig: project:second\n"
        rewritten = cache.rewrite_url(original, "archive.nar.xz")
        self.assertEqual(rewritten.replace("URL: archive", "URL: nar/archive"), original)
        for text in ("StorePath: path\n", "URL: one\nURL: two\n", "invalid"):
            with self.assertRaises(ValueError):
                cache.rewrite_url(text, "archive.nar.xz")

    def test_inventory_refuses_missing_sources_symlinks_and_nested_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with self.assertRaises(ValueError):
                cache.inventory(directory)
            for name in cache.SOURCE_ASSETS:
                (directory / name).write_text("fixture source")
            self.assertEqual(set(cache.inventory(directory)), cache.SOURCE_ASSETS)
            (directory / "link").symlink_to(directory / "THIRD_PARTY.md")
            with self.assertRaises(ValueError):
                cache.inventory(directory)
            (directory / "link").unlink()
            (directory / "nested").mkdir()
            with self.assertRaises(ValueError):
                cache.inventory(directory)

    def test_normalization_never_discards_nonempty_nix_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "log").mkdir()
            preserved = directory / "log" / "keep.txt"
            preserved.write_text("source evidence")
            with self.assertRaises(OSError):
                cache.prune_empty_cache_directories(directory)
            self.assertEqual(preserved.read_text(), "source evidence")

    def test_empty_bookkeeping_directory_names_are_not_a_nix_version_contract(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "build-trace-v2").mkdir()
            (directory / "future-empty-directory").mkdir()
            cache.prune_empty_cache_directories(directory)
            self.assertEqual(sorted(path.name for path in directory.iterdir()), [])

    def test_rejects_foreign_paths_and_incomplete_closures(self):
        source = {"sha": "fixture"}
        path = "/nix/store/" + "a" * 32 + "-splitcast-rs-core-0.1.0"
        selected = {"native": path}
        manifest = {"version": 1, "revision": source, "roots": selected,
                    "closure": {path: {"references": []}}, "custom": [path]}
        with patch.object(cache, "revision", return_value=source), patch.object(cache, "roots", return_value=selected), patch.object(cache, "run", return_value=path):
            cache.validate_manifest(manifest)
            manifest["closure"][path]["references"] = ["/missing"]
            with self.assertRaises(ValueError):
                cache.validate_manifest(manifest)
            manifest["closure"][path]["references"] = []
            manifest["revision"] = {"sha": "wrong"}
            with self.assertRaises(ValueError):
                cache.validate_manifest(manifest)
        for path in ("/etc/passwd", "/nix/store/../escape", "/nix/store/" + "a" * 32 + "-x/y"):
            with self.assertRaises(ValueError):
                cache.store_hash(path)


@unittest.skipUnless(os.environ.get("SPLITCAST_CACHE_PROTOCOL_TEST") == "1", "explicit Nix protocol fixture")
class NixProtocolTests(unittest.TestCase):
    def test_flat_cache_signatures_and_tamper_rejection(self):
        with tempfile.TemporaryDirectory(prefix="splitcast-cache-protocol-", dir=os.environ.get("TMPDIR")) as temporary:
            directory = Path(temporary)
            private = directory / "private"
            public = directory / "public"
            subprocess.run(["nix-store", "--generate-binary-cache-key", "fixture-1", str(private), str(public)], check=True)
            root = cache.run("nix", "build", "--no-link", "--print-out-paths", "--expr",
                             'let f = builtins.getFlake (toString ./.); p = f.inputs.nixpkgs.legacyPackages.x86_64-linux; in p.runCommand "splitcast-cache-fixture" {} "echo fixture > $out"',
                             "--impure")
            exported = directory / "cache"
            cache.run("nix", "copy", "--no-recursive", "--to", exported.as_uri(), root)
            metadata = exported / (cache.store_hash(root) + ".narinfo")
            nar = exported / cache.fields(metadata.read_text())["URL"]
            cache.flatten_export(exported, [root])
            for name in cache.SOURCE_ASSETS:
                (exported / name).write_text("fixture source")
            self.assertEqual(sorted(p.name for p in exported.iterdir() if not p.is_file()), [])
            self.assertIn(nar.name, cache.inventory(exported))
            verify = ["nix", "store", "verify", "--store", exported.as_uri(), "--no-contents", "--sigs-needed", "1",
                      "--option", "trusted-public-keys", public.read_text().strip(), "--option", "substituters", "", root]
            self.assertNotEqual(subprocess.run(verify).returncode, 0, "unsigned fixture was accepted")
            cache.flat_cache_command(exported, "store", "sign", "--key-file", str(private), root)
            self.assertEqual(sorted(p.name for p in exported.iterdir() if not p.is_file()), [])
            subprocess.run(verify, check=True)
            consumer = "local?root=" + str(directory / "consumer")
            cache.run("nix", "copy", "--from", exported.as_uri(), "--to", consumer,
                      "--option", "trusted-public-keys", public.read_text().strip(), root)
            copied = directory / "consumer" / root.lstrip("/")
            self.assertEqual(copied.read_text(), "fixture\n")
            original = metadata.read_text()
            metadata.write_text(original.replace("NarSize: ", "NarSize: 1"))
            self.assertNotEqual(subprocess.run(verify).returncode, 0, "tampered metadata was accepted")
            metadata.write_text(original)
            with (exported / nar.name).open("r+b") as output:
                output.write(b"tampered")
            result = subprocess.run(["nix", "copy", "--from", exported.as_uri(), "--to", "local?root=" + str(directory / "corrupt-consumer"),
                                     "--option", "trusted-public-keys", public.read_text().strip(), root])
            self.assertNotEqual(result.returncode, 0, "tampered NAR was accepted")


if __name__ == "__main__":
    unittest.main()
