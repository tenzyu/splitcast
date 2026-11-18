#!/usr/bin/env python3
"""Worktree-local work contracts and source-bound software receipts (Linux/stdlib)."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import hmac
import io
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
FILE_LIMIT = 8 * 1024 * 1024
TOTAL_LIMIT = 128 * 1024 * 1024
SLUG = re.compile(r"[a-z][a-z0-9-]{0,63}")
OID = re.compile(r"(?:[a-f0-9]{40}|[a-f0-9]{64})")
AXES = ("code", "functional-poc", "game-performance", "operational-recovery")
ENV_KEYS = (
    "PATH", "HOME", "USER", "LANG", "LC_ALL", "TERM", "TMPDIR", "XDG_RUNTIME_DIR",
    "CARGO_HOME", "CARGO_TARGET_DIR", "RUSTUP_HOME", "RUSTC", "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS", "SSL_CERT_FILE", "NIX_SSL_CERT_FILE", "NIX_PATH",
    "NIX_PROFILES", "NIX_BUILD_CORES", "IN_NIX_SHELL", "DIRENV_DIR",
    "LD_LIBRARY_PATH", "LIBRARY_PATH", "PKG_CONFIG_PATH", "RUST_SRC_PATH",
    "AR", "CC", "LD", "PKG_CONFIG", "CARGO_BUILD_JOBS", "NIX_BINTOOLS", "NIX_CC",
    "NIX_CFLAGS_COMPILE", "NIX_ENFORCE_NO_NATIVE", "NIX_HARDENING_ENABLE",
    "NIX_LD", "NIX_LDFLAGS", "NIX_LD_LIBRARY_PATH", "NIX_STORE",
    "NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu",
    "NIX_CC_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu",
    "NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu",
)
SENSITIVE = re.compile(rb"(?i)token|secret|password|authorization|credential|private.key|api.?key|://[^ /]+@")


class Blocked(Exception):
    pass


def require(condition, message):
    if not condition:
        raise Blocked(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(data):
    return json.dumps(data, sort_keys=True, ensure_ascii=True, separators=(",", ":")).encode()


def timestamp():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def git(repo, *args):
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env.update(GIT_TERMINAL_PROMPT="0", GIT_OPTIONAL_LOCKS="0",
               GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
    cmd = ["git", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", *args]
    result = subprocess.run(cmd, cwd=repo, env=env, capture_output=True, timeout=45)
    require(result.returncode == 0, f"Git {args[0]} failed (exit {result.returncode})")
    return result.stdout


def git_path(repo, *args):
    return Path(os.fsdecode(git(repo, "rev-parse", "--path-format=absolute", *args).rstrip(b"\n")))


def path_parts(path):
    require(isinstance(path, str), "expected a repository-relative path")
    parts = path.split("/")
    require(all(part and part not in {".", "..", ".git"} for part in parts)
            and not any(ord(char) < 32 or ord(char) == 127 for char in path)
            and "\\" not in path, "unsafe repository path")
    return parts


def read_file(root, path):
    """Read a bounded regular file without following any path component."""
    parts = path_parts(path)
    flags = os.O_NOFOLLOW | os.O_CLOEXEC
    folder = os.open(root, flags | os.O_DIRECTORY)
    try:
        for part in parts[:-1]:
            child = os.open(part, flags | os.O_DIRECTORY, dir_fd=folder)
            os.close(folder)
            folder = child
        with os.fdopen(os.open(parts[-1], flags | os.O_NONBLOCK, dir_fd=folder), "rb") as file:
            before = os.fstat(file.fileno())
            require(stat.S_ISREG(before.st_mode), f"{path}: expected a regular file")
            require(before.st_size <= FILE_LIMIT, f"{path}: exceeds {FILE_LIMIT} bytes")
            data = file.read(FILE_LIMIT + 1)
            after = os.fstat(file.fileno())
            require((before.st_ctime_ns, before.st_size) == (after.st_ctime_ns, after.st_size)
                    and len(data) == before.st_size, f"{path}: changed while reading")
            return ("100755" if before.st_mode & stat.S_IXUSR else "100644"), data
    except FileNotFoundError:
        return None
    except OSError as error:
        raise Blocked(f"{path}: cannot read safely ({error.strerror})") from error
    finally:
        os.close(folder)


def entries(raw, *, index=False):
    result = {}
    for record in raw.split(b"\0"):
        if not record:
            continue
        meta, name = record.split(b"\t", 1)
        path = os.fsdecode(name)
        path_parts(path)
        mode, second, third = meta.decode().split()
        require(mode in {"100644", "100755"} and (not index or third == "0"),
                f"{path}: unsupported Git mode or unmerged index")
        result[path] = (mode, second if index else third)
    return result


def tree(repo, ref):
    return entries(git(repo, "ls-tree", "-r", "-z", ref))


def head(repo):
    return git(repo, "rev-parse", "--verify", "HEAD^{commit}").decode().strip()


def snapshot(repo, base=None, *, include_ignored=False):
    revision = head(repo)
    tree_id = git(repo, "rev-parse", f"{revision}^{{tree}}").decode().strip()
    committed = tree(repo, revision)
    index = entries(git(repo, "ls-files", "--stage", "-z"), index=True)
    args = [] if include_ignored else ["--exclude-standard"]
    def unmanaged():
        return {os.fsdecode(p) for p in git(repo, "ls-files", "-z", "--others", *args).split(b"\0") if p}

    untracked = unmanaged()
    working, work_bytes = {}, {}
    total = 0
    for path in sorted(index.keys() | untracked):
        item = read_file(repo, path)
        if item is None:
            continue
        mode, data = item
        total += len(data)
        require(total <= TOTAL_LIMIT, "repository snapshot exceeds read budget")
        algorithm = "sha256" if len(revision) == 64 else "sha1"
        blob = hashlib.new(algorithm, f"blob {len(data)}\0".encode() + data).hexdigest()
        working[path] = (mode, blob)
        work_bytes[path] = (mode, digest(data))
    layers = [tree(repo, base) if base and base != revision else committed, committed, index, working]
    changed = sorted({name for left, right in zip(layers, layers[1:])
                      for name in left.keys() | right.keys() if left.get(name) != right.get(name)})
    require(head(repo) == revision and untracked == unmanaged()
            and index == entries(git(repo, "ls-files", "--stage", "-z"), index=True),
            "Git identity or untracked paths changed during snapshot; rerun when edits stop")
    identity = {"commit": revision, "tree": tree_id,
                "snapshot": digest(canonical([revision, tree_id, index, work_bytes]))}
    return identity, changed, working


def private_dir(path):
    path.mkdir(mode=0o700, exist_ok=True)
    attrs = path.lstat()
    require(stat.S_ISDIR(attrs.st_mode) and attrs.st_uid == os.getuid()
            and not attrs.st_mode & 0o077, f"{path.name}: expected an owner-only directory")
    return path


@contextmanager
def locked(folder):
    fd = os.open(folder / "lock", os.O_CREAT | os.O_NOFOLLOW | os.O_RDWR, 0o600)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Blocked("another convergence operation owns this state; retry after it exits") from error
        yield
    finally:
        os.close(fd)


def save(path, value):
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as tmp:
        try:
            tmp.write(canonical(value) + b"\n")
            tmp.flush()
            os.fsync(tmp.fileno())
            os.replace(tmp.name, path)
        finally:
            Path(tmp.name).unlink(missing_ok=True)


def document(path):
    item = read_file(path.parent, path.name)
    require(item is not None, f"{path.name}: missing JSON file")
    raw = item[1]
    try:
        value = json.loads(raw, object_pairs_hook=unique_fields)
    except (ValueError, RecursionError) as error:
        raise Blocked(f"{path.name}: invalid JSON or duplicate key") from error
    require(isinstance(value, dict), f"{path.name}: expected a JSON object")
    return value, digest(raw)


def unique_fields(pairs):
    result = dict(pairs)
    if len(result) != len(pairs):
        raise ValueError("duplicate field")
    return result


def accepted_contract(repo, path):
    data, checksum = document(path)
    require(set(data) == {"version", "id", "base", "objective", "scope", "invariants",
                          "checks", "manual_gates", "forbidden_effects", "exit_condition"}
            and data["version"] == 1, "contract v1 is missing or has unexpected fields")
    require(isinstance(data["id"], str) and SLUG.fullmatch(data["id"]), "invalid work ID")
    base = data["base"]
    require(isinstance(base, str) and OID.fullmatch(base), "base must be a full commit ID")
    require(git(repo, "cat-file", "-t", base) == b"commit\n", "base must identify a commit")
    git(repo, "merge-base", "--is-ancestor", base, "HEAD")
    for key in ("objective", "exit_condition"):
        require(isinstance(data[key], str) and data[key].strip(), f"{key} must be nonempty text")
    for key in ("scope", "invariants", "forbidden_effects"):
        values = data[key]
        require(isinstance(values, list) and values
                and all(isinstance(value, str) and value.strip() for value in values),
                f"{key} must be a nonempty array of strings")
    for entry in data["scope"]:
        path_parts(entry.removesuffix("/**"))
        require(not any(char in entry.removesuffix("/**") for char in "*?["), "use explicit paths or dir/**")
    require(isinstance(data["manual_gates"], dict) and set(data["manual_gates"]) == set(AXES[1:])
            and all(isinstance(value, str) and value.strip() for value in data["manual_gates"].values()),
            "manual_gates must describe each independent physical/operational axis")
    checks = data["checks"]
    require(isinstance(checks, list) and checks, "declare at least one executable check")
    ids = set()
    for check in checks:
        require(isinstance(check, dict) and set(check) == {"id", "argv", "timeout_seconds"},
                "each check needs id, argv and timeout_seconds")
        name, argv, timeout = check["id"], check["argv"], check["timeout_seconds"]
        require(isinstance(name, str) and SLUG.fullmatch(name) and name not in ids, "invalid or duplicate check ID")
        ids.add(name)
        require(isinstance(argv, list) and 0 < len(argv) <= 64
                and all(isinstance(arg, str) and 0 < len(arg) <= 512 and arg.isprintable()
                        for arg in argv), "argv must be nonempty printable strings; never include secrets")
        require(type(timeout) is int and 1 <= timeout <= 3600, "timeout_seconds must be 1..3600")
    return data, checksum


def scope(repo, contract):
    identity, changed, _ = snapshot(repo, contract["base"])
    violations = [path for path in changed if not any(
        path == rule or rule.endswith("/**") and path.startswith(rule[:-2])
        for rule in contract["scope"]
    )]
    return {"source": identity, "changed": changed, "violations": violations}


def environment(folder):
    env = {key: os.environ[key] for key in ENV_KEYS if key in os.environ}
    keyfile = folder / "env-key"
    item = read_file(folder, keyfile.name)
    if item is None:
        key = secrets.token_bytes(32)
        with os.fdopen(os.open(keyfile, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "wb") as file:
            file.write(key)
    else:
        key = item[1]
        require(len(key) == 32, "invalid environment identity key")
    info = {"platform": sys.platform, "variables": sorted(env),
            "fingerprint": hmac.digest(key, canonical(env), "sha256").hex(),
            "nix_shell": bool(env.get("IN_NIX_SHELL")), "direnv": bool(env.get("DIRENV_DIR"))}
    return env, info


def process(repo, argv, timeout, env, *, prompt=None):
    """Capture bounded output in memory and reap the owned process-group leader."""
    started_at = timestamp()
    deadline = time.monotonic() + timeout
    result = {"started_at": started_at, "timeout_seconds": timeout, "exit_code": None,
              "timed_out": False, "group_kill_sent": False, "leader_reaped": False}
    output = bytearray()
    with tempfile.TemporaryFile() if prompt is not None else open(os.devnull, "rb") as stdin:
        if prompt is not None:
            stdin.write(prompt)
            stdin.seek(0)
        try:
            child = subprocess.Popen(argv, cwd=repo, env=env, stdin=stdin,
                                     stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                     start_new_session=True, umask=0o077)
        except OSError as error:
            return {**result, "error": f"launch: {error.strerror}", "finished_at": timestamp()}, b""
        try:
            with child.stdout as stream, selectors.DefaultSelector() as selector:
                selector.register(stream, selectors.EVENT_READ)
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0 or not selector.select(remaining):
                        raise subprocess.TimeoutExpired(argv[0], timeout)
                    chunk = os.read(stream.fileno(), 65536)
                    if not chunk:
                        break
                    if len(output) + len(chunk) > FILE_LIMIT:
                        raise Blocked("output budget exceeded")
                    output.extend(chunk)
                result["exit_code"] = child.wait(timeout=max(0, deadline - time.monotonic()))
                result["leader_reaped"] = True
        except (subprocess.TimeoutExpired, Blocked, KeyboardInterrupt, OSError) as error:
            result["timed_out"] = isinstance(error, subprocess.TimeoutExpired)
            result["descendant_cleanup"] = "UNVERIFIED"
            result["error"] = ("timeout" if result["timed_out"] else str(error)
                               if isinstance(error, Blocked) else type(error).__name__)
            try:
                os.killpg(child.pid, signal.SIGKILL)
                result["group_kill_sent"] = True
            except OSError as cleanup_error:
                result["group_kill_error"] = cleanup_error.strerror
            try:
                result["exit_code"] = child.wait(timeout=3)
                result["leader_reaped"] = True
            except subprocess.TimeoutExpired:
                pass
        return {**result, "finished_at": timestamp()}, output


def redacted(output):
    secrets_in_env = {os.fsencode(value) for key, value in os.environ.items()
                      if SENSITIVE.search(key.encode()) and len(value) >= 4}
    # This is a sanitized diagnostic log. Trusted commands must avoid printing secrets.
    log = bytearray()
    private = False
    truncated = b"[diagnostic log truncated]\n"
    safe = bytes(char if char == 9 or 32 <= char < 127 else 63 for char in range(256))
    for line in io.BytesIO(output):
        line = line.rstrip(b"\r\n")
        if b"-----BEGIN " in line:
            private = True
        if private or SENSITIVE.search(line) or len(line) > 4096 or any(
            value in line for value in secrets_in_env
        ):
            sanitized = b"[redacted]"
        else:
            sanitized = line.translate(safe)
        if b"-----END " in line:
            private = False
        if len(log) + len(sanitized) + 1 + len(truncated) > FILE_LIMIT:
            log.extend(truncated)
            break
        log.extend(sanitized + b"\n")
    return log


def save_log(path, data):
    with os.fdopen(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "wb") as file:
        file.write(data)
        file.flush()
        os.fsync(file.fileno())
    return {"name": path.name, "sha256": digest(data), "bytes": len(data), "lines": data.count(b"\n")}


def verified_log(folder, log):
    item = read_file(folder, log["name"])
    require(item is not None and digest(item[1]) == log["sha256"], "receipt log missing or changed")
    return item[1]


def receipt_identity(repo, spec, env, info, source, checksum):
    program = spec["argv"][0]
    search = os.pathsep.join(str(repo / directory) for directory in env.get("PATH", os.defpath).split(os.pathsep))
    executable = shutil.which(str(repo / program) if "/" in program else program, path=search)
    stat_id = None
    if executable:
        attrs = Path(executable).stat()
        stat_id = [attrs.st_dev, attrs.st_ino, attrs.st_size, attrs.st_mtime_ns, attrs.st_ctime_ns]
    return {"check": spec["id"], "argv": spec["argv"], "timeout_seconds": spec["timeout_seconds"],
            "environment": info, "executable": stat_id, "source": source, "contract": checksum}


def matching_receipts(history, identity):
    return [row for row in history if row["identity"] == identity]


def run_check(repo, folder, contract, checksum, history, args, report, env, info):
    require(not report["scope"]["violations"], "correct scope violations before running checks")
    spec = next((check for check in contract["checks"] if check["id"] == args.check_id), None)
    require(spec is not None, "unknown check ID")
    identity = receipt_identity(repo, spec, env, info, report["scope"]["source"], checksum)
    matches = matching_receipts(history, identity)
    if matches and matches[-1]["status"] == "PASS":
        verified_log(folder, matches[-1]["log"])
        return {**matches[-1], "reused": True}
    require(len(matches) < 3, "three attempts at unchanged source/command/environment; resolve the cause")
    if matches and (matches[-1]["status"] == "RUNNING" or matches[-1]["process"].get("cleanup_uncertain")):
        raise Blocked("previous runner cleanup is uncertain; inspect the reserved attempt before proceeding")
    name = f"attempt-{len(history) + 1:06d}"
    file = folder / f"{name}.json"
    row = {"id": name, "identity": identity, "status": "RUNNING", "reserved_at": timestamp()}
    save(file, row)
    result, stdout = process(repo, spec["argv"], spec["timeout_seconds"], env)
    log = save_log(folder / f"{name}.log", redacted(stdout))
    try:
        current_contract = document(args.contract)[1]
        after = scope(repo, contract)["source"]
        unchanged = receipt_identity(repo, spec, env, info, after, current_contract) == identity
        after_error = None
    except (Blocked, OSError, subprocess.TimeoutExpired) as error:
        after, unchanged, after_error = None, False, type(error).__name__
    if result.get("error") and not result["leader_reaped"] and not str(result["error"]).startswith("launch:"):
        result["cleanup_uncertain"] = True
    row.update(status="PASS" if result["exit_code"] == 0 and not result.get("error") and unchanged else "FAIL",
               process=result, after=after, after_error=after_error, unchanged=unchanged, log=log)
    save(file, row)
    return row


def findings(folder):
    return [document(path)[0] for path in sorted(folder.glob("finding-*.json"))]


def record_finding(folder, args, source):
    key = {"axis": args.axis, "invariant": args.invariant, "behavior": args.behavior}
    path = folder / f"finding-{digest(canonical(key))}.json"
    row = {**key, "classification": args.classification, "owner": args.owner,
           "evidence": args.evidence, "disposition": args.disposition, "source": source}
    require(args.classification != "BLOCKER" or args.disposition != "DEFERRED",
            "resolve a blocker with evidence instead of deferring it")
    if path.exists():
        previous = document(path)[0]
        require(previous["classification"] != "BLOCKER" or previous["disposition"] == "RESOLVED"
                or args.disposition == "RESOLVED" or args.classification == "BLOCKER",
                "an open blocker requires an explicit resolution")
        require(previous["disposition"] != "OPEN" or args.disposition != "RESOLVED"
                or previous["evidence"] != args.evidence, "resolution needs distinct evidence")
        if previous == row:
            return previous
    save(path, row)
    return row


def work_status(repo, folder, contract, checksum, history, scoped, env, info):
    notes = findings(folder)
    blockers = [row for row in notes if row["classification"] == "BLOCKER" and row["disposition"] == "OPEN"]
    checks = {}
    for spec in contract["checks"]:
        ident = receipt_identity(repo, spec, env, info, scoped["source"], checksum)
        rows = matching_receipts(history, ident)
        row = rows[-1] if rows else None
        if row and row["status"] == "PASS":
            verified_log(folder, row["log"])
        checks[spec["id"]] = {"status": row["status"] if row else "UNVERIFIED",
                              "attempts": len(rows), "latest": row["id"] if row else None,
                              "triage": row.get("triage") if row else None}
    code = ("BLOCKED" if scoped["violations"] or any(row["axis"] == "code" for row in blockers) or any(
        item["status"] in {"FAIL", "RUNNING"} for item in checks.values()
    ) else "PASS" if all(item["status"] == "PASS" for item in checks.values()) else "UNVERIFIED")
    return {"work": contract["id"], "objective": contract["objective"], "base": contract["base"],
            "contract": checksum, "scope": scoped, "code": code, "checks": checks,
            "status": "BLOCKED" if blockers else code,
            "exit_condition": contract["exit_condition"], "invariants": contract["invariants"],
            "forbidden_effects": contract["forbidden_effects"],
            "manual_gates": {axis: {"status": "BLOCKED" if any(row["axis"] == axis for row in blockers)
                                               else "UNVERIFIED", "requirement": text}
                             for axis, text in contract["manual_gates"].items()},
            "approvals": {"merge": "UNVERIFIED", "deployment": "UNVERIFIED"},
            "open_blockers": blockers, "findings": notes, "receipts": len(history)}


def dispatch(repo, args):
    contract, checksum = accepted_contract(repo, args.contract)
    admin = private_dir(git_path(repo, "--absolute-git-dir") / "convergence")
    folder = private_dir(admin / contract["id"])
    with locked(folder):
        registration = folder / "contract.json"
        if args.action == "init":
            require(checksum == args.accept, "digest differs from the reviewed contract")
            require(not registration.exists(), "duplicate work ID; use a new ID to re-evaluate scope")
            save(registration, {"checksum": checksum, "path": str(args.contract), "base": contract["base"]})
            return {"status": "REGISTERED", "id": contract["id"], "contract": checksum}, 0
        require(document(registration)[0] == {"checksum": checksum, "path": str(args.contract), "base": contract["base"]},
                "contract changed; review and enroll it under a new ID")
        history = [document(path)[0] for path in sorted(folder.glob("attempt-*.json"))]
        if args.action == "inspect":
            row = next((row for row in history if args.attempt == row["id"]), None)
            require(row is not None and "log" in row, "unknown attempt or missing log")
            if args.tail or args.classify:
                data = verified_log(folder, row["log"])
                if args.classify:
                    require(row["status"] == "FAIL" and args.line and 1 <= args.line <= row["log"]["lines"],
                            "classify a failure using an existing sanitized log line")
                    row["triage"] = {"classification": args.classify, "line": args.line}
                    save(folder / f"{row['id']}.json", row)
                if args.tail:
                    row["tail"] = [line[:300] for line in data.decode().splitlines()[-args.tail:]]
            return row, 0
        scoped = scope(repo, contract)
        env, info = environment(folder)
        report = work_status(repo, folder, contract, checksum, history, scoped, env, info)
        if args.action == "run":
            row = run_check(repo, folder, contract, checksum, history, args, report, env, info)
            return row, 0 if row["status"] == "PASS" else 2
        if args.action == "finding":
            return record_finding(folder, args, scoped["source"]), 0
        if args.action == "check":
            return scoped, 2 if scoped["violations"] else 0
        return report, {"PASS": 0, "BLOCKED": 2, "UNVERIFIED": 3}[report["status"]]


def render(report):
    lines = [f"# {report['work']}", "", report["objective"], "",
             f"Work **{report['status']}** · Software **{report['code']}** at `{report['scope']['source']['commit']}`",
             f"Snapshot `{report['scope']['source']['snapshot']}`", "",
             *(f"- {name} {item['status']} ({item['latest'] or 'no current receipt'})"
               for name, item in report["checks"].items()),
             *(f"- {axis} {gate['status']} — {gate['requirement']}" for axis, gate in report["manual_gates"].items()),
             *(f"- Scope violation `{path}`" for path in report["scope"]["violations"]),
             *(f"- Blocker on {row['axis']} · {row['behavior']} · {row['owner']} · {row['evidence']}"
               for row in report["open_blockers"]), "", report["exit_condition"], "",
             "Merge and deployment require separate operator decisions."]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    for name in ("init", "check", "run", "status", "report", "handoff", "finding", "inspect"):
        cmd = commands.add_parser(name)
        cmd.add_argument("contract", type=Path)
        if name == "init":
            cmd.add_argument("--accept", required=True, metavar="SHA256")
        if name == "run":
            cmd.add_argument("check_id")
        if name == "inspect":
            cmd.add_argument("attempt")
            cmd.add_argument("--tail", type=int, choices=range(1, 21), metavar="1..20")
            cmd.add_argument("--classify", choices=["INFRASTRUCTURE", "CODE", "UNKNOWN"])
            cmd.add_argument("--line", type=int)
        if name == "finding":
            cmd.add_argument("--axis", required=True, choices=AXES)
            cmd.add_argument("--classification", required=True, choices=["BLOCKER", "FOLLOW_UP", "QUESTION"])
            cmd.add_argument("--disposition", default="OPEN", choices=["OPEN", "RESOLVED", "DEFERRED"])
            for arg in ("invariant", "behavior", "owner", "evidence"):
                cmd.add_argument(f"--{arg}", required=True)
    from convergence_wiki import add_commands, wiki
    add_commands(commands)
    args = parser.parse_args(argv)
    try:
        repo = git_path(Path.cwd(), "--show-toplevel")
        if args.action == "wiki":
            report, code = wiki(repo, args)
        else:
            args.contract = args.contract.absolute()
            report, code = dispatch(repo, args)
        print(render(report) if args.action in {"handoff", "report"} else json.dumps(report, indent=2))
        return code
    except (Blocked, OSError, subprocess.TimeoutExpired, KeyError, TypeError, ValueError) as error:
        print(json.dumps({"status": "BLOCKED", "reason": str(error)}))
        return 2


if __name__ == "__main__":
    # Let the Wiki module use the same helpers and exception type when run as a script.
    sys.modules["convergence"] = sys.modules[__name__]
    raise SystemExit(main())
