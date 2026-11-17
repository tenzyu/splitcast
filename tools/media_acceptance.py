#!/usr/bin/env python3
"""Opt-in real production media acceptance. No capture devices or public output."""

import argparse
import array
import base64
from collections import deque
from fractions import Fraction
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import secrets
import select
import shutil
import signal
import socket
import statistics
import struct
import subprocess
import threading
import time


ROOT = Path(__file__).resolve().parent.parent
WIDTH, HEIGHT, FPS = 1280, 720, 30
AUDIO_SECONDS = 45
AUDIO_FREQUENCIES = tuple(440 + 40 * second for second in range(AUDIO_SECONDS))
GAME_AUDIO = "aevalsrc='0.18*sin(2*PI*(440+40*floor(t))*t)':s=48000"
MIC_AUDIO = "aevalsrc=0.15*(sin(2*PI*180*t)+0.5*sin(2*PI*360*t)+0.3*sin(2*PI*540*t)):s=48000"
CASES = ("normal", "frozen", "delay", "disconnect", "stall", "worker-death")
LIMITS = {
    "record_seconds": 10,
    "duration_min": 9,
    "duration_max": 13,
    "valid_frame_fraction": 0.95,
    "content_span_seconds": 7,
    "max_scene_age_seconds": 3,
    "max_av_offset_seconds": 0.5,
    "av_measurement_uncertainty_seconds": 0.2,
    "av_negative_delay_seconds": 0.8,
    "max_audio_pts_error_seconds": 0.005,
    "fault_seconds": 20,
    "source_seconds": AUDIO_SECONDS,
    "backlog_delay_seconds": 4,
    "startup_pipe_seconds": 40,
    "scene_startup_samples": 2,
    "max_progress_gap_seconds": 0.5,
    "cadence_startup_seconds": 1,
    "cadence_tail_seconds": 0.2,
    "min_video_density_fps": 28.5,
    "min_video_second_frames": 27,
    "max_video_pts_gap_seconds": 0.101,
    "max_audio_absence_windows": 3,
}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def source_identity():
    names = subprocess.check_output(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"], cwd=ROOT
    ).decode().split("\0")
    files = {
        name: digest(ROOT / name)
        for name in sorted(set(names))
        if name and not name.startswith(("docs/", ".audit/"))
        and name not in ("README.md", "MEMORY.md", "LICENSE", "THIRD_PARTY.md")
        and (ROOT / name).is_file()
    }
    return {
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
        "files": files,
        "tree_sha256": hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest(),
    }


def process_identity(pid):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return {"pid": pid, "state": fields[0], "ppid": int(fields[1]), "start": fields[19]}
    except FileNotFoundError:
        return None


def descendants(pid):
    found = {}
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            item = process_identity(int(entry.name))
            if item:
                found[item["pid"]] = item
    owned = {pid}
    while True:
        more = {p for p, item in found.items() if item["ppid"] in owned}
        if more <= owned:
            return {p: found[p] for p in owned if p in found}
        owned |= more


def still_owned(item):
    now = process_identity(item["pid"])
    return now is not None and now["start"] == item["start"]


def port_open(port):
    with socket.socket() as stream:
        stream.settimeout(0.1)
        return stream.connect_ex(("127.0.0.1", port)) == 0


def free_ports(count):
    reservations = []
    try:
        for _ in range(count):
            stream = socket.socket()
            stream.bind(("127.0.0.1", 0))
            reservations.append(stream)
        return [stream.getsockname()[1] for stream in reservations]
    finally:
        for stream in reservations:
            stream.close()


def frame(tick, epoch, ordinal):
    pixels = bytearray([40]) * (WIDTH * HEIGHT)
    x = 40 + (ordinal * 11) % 1050
    for y in range(180, 300):
        pixels[y * WIDTH + x:y * WIDTH + x + 120] = bytes([220]) * 120
    values = [1, 0] + [(tick >> bit) & 1 for bit in range(9)]
    values += [(epoch >> bit) & 1 for bit in range(4)]
    values += [sum(values[2:]) % 2]
    for bit, value in enumerate(values):
        for y in range(600, 700):
            start = y * WIDTH + 20 + bit * 70
            pixels[start:start + 45] = bytes([235 if value else 16]) * 45
    return pixels


def barcode(pixels):
    require(len(pixels) == WIDTH * HEIGHT, "truncated decoded frame")
    levels = [pixels[650 * WIDTH + 42 + bit * 70] for bit in range(16)]
    if any(60 < value < 180 for value in levels):
        return None
    bits = [int(value >= 180) for value in levels]
    if bits[:2] != [1, 0] or sum(bits[2:15]) % 2 != bits[15]:
        return None
    tick = sum(bits[2 + bit] << bit for bit in range(9))
    epoch = sum(bits[11 + bit] << bit for bit in range(4))
    return tick, epoch


def tone_power(samples, frequency):
    coefficient = 2 * math.cos(2 * math.pi * frequency / 48000)
    previous, older = 0.0, 0.0
    for sample in samples:
        current = sample + coefficient * previous - older
        older, previous = previous, current
    return (previous * previous + older * older - coefficient * previous * older) / len(samples) ** 2


def assess_marks(marks, epoch):
    valid = [(index / 10, mark[0]) for index, mark in enumerate(marks) if mark and mark[1] == epoch]
    require(marks and len(valid) >= len(marks) * LIMITS["valid_frame_fraction"], "missing or wrong source barcodes")
    require(all(b[1] >= a[1] for a, b in zip(valid, valid[1:])), "backward content identity")
    last_tick = None
    last_change = 0.0
    for at, tick in valid:
        if tick != last_tick:
            last_tick, last_change = tick, at
        require(at - last_change <= LIMITS["max_progress_gap_seconds"], "frozen content progression gap")
    require((len(marks) - 1) / 10 - valid[-1][0] <= LIMITS["max_progress_gap_seconds"], "missing content tail")
    span = (valid[-1][1] - valid[0][1]) / 10
    require(span >= LIMITS["content_span_seconds"], f"frozen or insufficient content progression: {span}")
    return valid, span


def av_alignment(valid, windows):
    require(abs(valid[0][0] - windows[0][0]) <= LIMITS["max_av_offset_seconds"]
            and abs(valid[-1][0] - (windows[-1][0] + 0.1)) <= LIMITS["max_av_offset_seconds"],
            "A/V analysis coverage endpoints exceed envelope")
    for at, source_second, _ in windows:
        require(any(tick // 10 == source_second and abs(video_at - (at + 0.05)) <= LIMITS["max_av_offset_seconds"]
                    for video_at, tick in valid), "A/V audio source identity has no nearby video observation")
    video_edges = [((previous[0] + current[0]) / 2, current[1] // 10, (current[0] - previous[0]) / 2)
                   for previous, current in zip(valid, valid[1:])
                   if previous[1] // 10 != current[1] // 10]
    boundary_edges = [edge for edge in video_edges
                      if not windows[0][0] + 0.1 <= edge[0] <= windows[-1][0]]
    video_edges = [edge for edge in video_edges
                   if windows[0][0] + 0.1 <= edge[0] <= windows[-1][0]]
    audio_edges = [(current[0], current[1]) for previous, current in zip(windows, windows[1:])
                   if previous[1] != current[1]]
    require(len(video_edges) >= 3, "insufficient A/V transition markers")
    offsets = []
    upper_bounds = []
    used = set()
    for at, source_second, half_width in video_edges:
        matches = [(index, audio_at) for index, (audio_at, audio_second) in enumerate(audio_edges)
                   if audio_second == source_second and abs(audio_at - at) < 0.75]
        require(len(matches) == 1, "missing or ambiguous A/V transition")
        index, audio_at = matches[0]
        require(index not in used, "reused A/V transition")
        used.add(index)
        offsets.append(audio_at - at)
        upper_bounds.append(abs(audio_at - at) + LIMITS["av_measurement_uncertainty_seconds"]
                            + max(0, half_width - 0.05))
    require(all(index in used for index, (at, _) in enumerate(audio_edges)
                if valid[0][0] + 0.5 < at < valid[-1][0] - 0.5), "unexpected A/V transition")
    uncertainty = LIMITS["av_measurement_uncertainty_seconds"]
    upper = max(upper_bounds)
    require(upper <= LIMITS["max_av_offset_seconds"], f"A/V offset upper bound exceeded: {upper}")
    return {"estimate_seconds": statistics.median(offsets), "edge_offsets_seconds": offsets,
            "uncertainty_seconds": uncertainty, "absolute_upper_seconds": upper,
            "video_edges": video_edges, "audio_edges": audio_edges,
            "unpaired_boundary_video_edges": boundary_edges}


def audio_clock(frames):
    require(frames, "missing decoded audio frames")
    origin = float(frames[0]["best_effort_timestamp_time"])
    samples = 0
    errors = []
    for frame in frames:
        count = int(frame["nb_samples"])
        require(count > 0, "invalid decoded audio sample count")
        error = abs(float(frame["best_effort_timestamp_time"]) - origin - samples / 48000)
        require(error <= LIMITS["max_audio_pts_error_seconds"], f"A/V audio PTS discontinuity: {error}")
        errors.append(error)
        samples += count
    return {"origin_seconds": origin, "samples": samples, "max_pts_error_seconds": max(errors)}


def video_cadence(timestamps, fault_tail=False):
    require(timestamps and all(b > a for a, b in zip(timestamps, timestamps[1:])),
            "video cadence: missing, duplicate or backward PTS")
    start = timestamps[0] + LIMITS["cadence_startup_seconds"]
    end = timestamps[-1] + 1 / FPS - LIMITS["cadence_tail_seconds"]
    if fault_tail:
        end = min(end, timestamps[0] + LIMITS["record_seconds"] - LIMITS["cadence_tail_seconds"])
    require(end - start >= 7, "video cadence: insufficient steady-state coverage")
    steady = [at for at in timestamps if start <= at < end]
    density = len(steady) / (end - start)
    counts = [sum(start + second <= at < start + second + 1 for at in steady)
              for second in range(math.floor(end - start))]
    gap = max((b - a for a, b in zip(steady, steady[1:])), default=math.inf)
    require(density >= LIMITS["min_video_density_fps"], f"video cadence: low density {density}")
    require(min(counts) >= LIMITS["min_video_second_frames"], f"video cadence: sparse second {counts}")
    require(gap <= LIMITS["max_video_pts_gap_seconds"], f"video cadence: dropped-frame interval {gap}")
    return {"start_seconds": start, "end_seconds": end, "density_fps": density,
            "frames_per_full_second": counts, "max_pts_gap_seconds": gap,
            "scope": "pre-fault prefix" if fault_tail else "steady-state recording"}


def analyze_audio(pcm, origin):
    windows = []
    for offset in range(0, len(pcm) - 4800 + 1, 4800):
        samples = pcm[offset:offset + 4800]
        powers = [tone_power(samples, frequency) for frequency in AUDIO_FREQUENCIES]
        source_second = max(range(AUDIO_SECONDS), key=lambda second: powers[second])
        identity = {"game": powers[source_second], "mic": tone_power(samples, 180),
                    "noise": tone_power(samples, 1230), "frequency": AUDIO_FREQUENCIES[source_second]}
        windows.append((origin + offset / 48000, source_second, identity))
    return windows, {"complete_windows": len(windows), "partial_tail_samples": len(pcm) % 4800,
                     "partial_tail_policy": "not classified; less than 100 ms; stream endpoints checked separately"}


def audio_presence(windows):
    require(windows, "missing complete audio windows")
    result = {}
    for kind, fraction in (("mic", 0.7), ("game", 0.8)):
        present = [w[2][kind] > max(100, w[2]["noise"] * 10) for w in windows]
        require(sum(present) >= len(present) * fraction, f"{kind} audio identity absent")
        longest, current = 0, 0
        for found in present:
            current = 0 if found else current + 1
            longest = max(longest, current)
        require(longest <= LIMITS["max_audio_absence_windows"],
                f"{kind} audio consecutive dropout: {longest} windows")
        result[kind] = {"present_windows": sum(present), "max_absent_windows": longest}
    return result


def source_health(errors, publishers, expected_stops):
    require(not any(not error["expected"] for error in errors), f"unexpected source feeder failure: {errors}")
    for name, code in publishers.items():
        require(code is None or name in expected_stops, f"unexpected source publisher exit: {name}={code}")


class Run:
    def __init__(self, directory, binary):
        self.directory, self.binary = directory, binary
        self.children = []
        self.commands = []
        self.inventory = {}
        self.agent = None
        self.port = None
        self.env = os.environ.copy()
        self.env.pop("DBUS_SESSION_BUS_ADDRESS", None)
        self.env.pop("WAYLAND_DISPLAY", None)
        self.env.pop("SPLITCAST_WEB_ASSETS", None)
        self.env.update({
            "HOME": str(directory / "home"),
            "XDG_RUNTIME_DIR": str(directory / "runtime"),
            "XDG_CONFIG_HOME": str(directory / "config-home"),
            "QT_QPA_PLATFORM": "xcb", "LIBGL_ALWAYS_SOFTWARE": "1",
            "PULSE_SERVER": "unix:" + str(directory / "no-shared-pulse"),
        })
        for name in ("home", "runtime", "config-home"):
            (directory / name).mkdir(mode=0o700)

    def spawn(self, name, argv, **kwargs):
        log = (self.directory / (name + ".log")).open("wb")
        self.commands.append({"name": name, "argv": [str(a) for a in argv], "at": time.monotonic()})
        try:
            child = subprocess.Popen(
                argv, env=self.env, cwd=ROOT, stdin=kwargs.pop("stdin", subprocess.DEVNULL),
                stdout=log, stderr=log, start_new_session=True, **kwargs
            )
        finally:
            log.close()
        self.children.append((name, child))
        return child

    def tool(self, argv, input_data=None, timeout=30):
        self.commands.append({"argv": [str(a) for a in argv], "at": time.monotonic()})
        result = subprocess.run(argv, input=input_data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=self.env, cwd=ROOT, timeout=timeout)
        self.commands[-1]["returncode"] = result.returncode
        require(result.returncode == 0, f"{argv[0]} failed: {result.stderr.decode(errors='replace')[-2000:]}")
        return result.stdout

    def control(self, command, args=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=60)
        try:
            connection.request("POST", "/api/command", json.dumps({"command": command, "args": args or {}}), {
                "Authorization": "Bearer " + self.token,
                "Origin": f"http://127.0.0.1:{self.port}", "Content-Type": "application/json",
            })
            response = connection.getresponse()
            body = json.loads(response.read(4 * 1024 * 1024))
            if command != "preview":
                self.commands.append({"control": command, "code": response.status, "body": body,
                                      "at": time.monotonic()})
            require(response.status in (200, 202), f"{command} rejected: {response.status} {body}")
            return body
        finally:
            connection.close()

    def stop(self, allow_retry=False):
        body = self.control("stop")
        deadline = time.monotonic() + 30
        while body.get("stop") == "requested":
            require(time.monotonic() < deadline, "Stop completion timed out")
            time.sleep(0.05)
            body = self.control("status")
        if allow_retry and body.get("stop") == "incomplete":
            require(body.get("state") == "stopping" and body.get("outcome") == "unknown",
                    f"incomplete cleanup obligation was lost: {body}")
            self.commands.append({"explicit_cleanup_retry": body, "at": time.monotonic()})
            self.stop()
            return
        require(body.get("state") == "idle" and body.get("children") == []
                and body.get("stop") == "completed" and not body.get("remote_release_pending"),
                f"incomplete Stop: {body}")

    def transport(self):
        connection = http.client.HTTPConnection("127.0.0.1", self.ports[4], timeout=2)
        try:
            connection.request("GET", "/v3/paths/list")
            response = connection.getresponse()
            require(response.status == 200, "MediaMTX API rejected observation")
            return json.loads(response.read(1024 * 1024))
        finally:
            connection.close()

    def display(self):
        Path("/tmp/.X11-unix").mkdir(exist_ok=True)
        read_fd, write_fd = os.pipe()
        self.env["XAUTHORITY"] = str(self.directory / "xauthority")
        # FamilyWild is confined by a random MIT cookie and TCP-disabled Xvfb.
        cookie = secrets.token_bytes(16)
        fields = [b"", b"", b"MIT-MAGIC-COOKIE-1", cookie]
        auth = struct.pack(">H", 65535) + b"".join(struct.pack(">H", len(v)) + v for v in fields)
        Path(self.env["XAUTHORITY"]).write_bytes(auth)
        try:
            self.spawn("display", ["Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1280x720x24",
                                   "-xkbdir", os.environ["SPLITCAST_XKB_ROOT"],
                                   "-nolisten", "tcp", "-auth", self.env["XAUTHORITY"]], pass_fds=(write_fd,))
        finally:
            os.close(write_fd)
        try:
            require(select.select([read_fd], [], [], 10)[0], "Xvfb did not allocate a display")
            number = os.read(read_fd, 64).decode().strip()
            require(number.isdigit(), "Xvfb display allocation failed")
            self.env["DISPLAY"] = ":" + number
        finally:
            os.close(read_fd)

    def prepare(self):
        ports = free_ports(6)
        self.port = ports[0]
        self.ports = ports
        self.token = secrets.token_hex(24)
        for name, value in (("token", self.token), ("password", secrets.token_hex(24))):
            (self.directory / name).write_text(value)
        config = json.loads((ROOT / "packages/splitcast-rs/tests/agent-example.json").read_text())
        config["role"] = "production"
        config["peer"].update({
            "control_port": ports[0], "forward_control_port": ports[1],
            "token_file": str(self.directory / "token"), "remote_token_file": str(self.directory / "token"),
            "state_dir": str(self.directory / "state"), "timeout_seconds": 30,
        })
        config["capture"].update({"width": WIDTH, "height": HEIGHT, "fps": FPS,
                                   "rtmp_port": ports[2], "rtsp_port": ports[3], "api_port": ports[4]})
        config["audio"]["rnnoise_plugin"] = os.environ["SPLITCAST_RNNOISE_PLUGIN"]
        config["audio"]["rnnoise_control"] = "0|200|0|0|0"
        config["obs"].update({"binary": "obs", "websocket_port": ports[5], "video_mode": "software",
                              "password_file": str(self.directory / "password"),
                              "recording_dir": str(self.directory / "recordings"),
                              "twitch_key_file": str(self.directory / "absent-stream-key")})
        self.config = config
        write_json(self.directory / "config.json", config)
        self.agent = self.spawn("agent", [str(self.binary), "--config", str(self.directory / "config.json"), "agent"])
        deadline = time.monotonic() + 10
        while not port_open(self.port):
            require(self.agent.poll() is None, "Agent exited before control readiness")
            require(time.monotonic() < deadline, "Agent did not bind control")
            time.sleep(0.05)
        require(self.control("prepare")["state"] == "ready", "Prepare did not reach Ready")
        require(not list((self.directory / "recordings").glob("*.mkv")), "Prepare started recording")
        self.observe_resources()

    def observe_resources(self):
        owned = descendants(self.agent.pid) if self.agent else {}
        self.inventory.update(owned)
        opaque_fds = []
        for pid in owned:
            try:
                for fd in Path(f"/proc/{pid}/fd").iterdir():
                    try:
                        target = os.readlink(fd)
                    except FileNotFoundError:
                        continue
                    require(not target.startswith(("/dev/dri/", "/dev/snd/", "/dev/video")),
                            f"unauthorized device opened: {target}")
            except FileNotFoundError:
                pass
            except PermissionError:
                opaque_fds.append(pid)
        listeners = []
        for table in ("tcp", "tcp6"):
            for row in Path("/proc/net/" + table).read_text().splitlines()[1:]:
                fields = row.split()
                if fields[3] == "0A":
                    address = fields[1].split(":")[0]
                    require(address in ("0100007F", "00000000000000000000000001000000"),
                            f"non-loopback owned listener: {fields[1]}")
                    listeners.append({"pid_inode": fields[9], "address": fields[1]})
        write_json(self.directory / "resources.json", {"processes": self.inventory, "listeners": listeners,
                   "fd_inspection_denied": opaque_fds,
                   "physical_devices": "hidden by mandatory private mount namespace"})

    def preview(self, source):
        before = time.monotonic()
        body = self.control("preview")
        received = time.monotonic()
        image = base64.b64decode(body["image"].removeprefix("data:image/png;base64,"), validate=True)
        (self.directory / f"scene-{received:.6f}.png").write_bytes(image)
        raw = self.tool(["ffmpeg", "-v", "error", "-threads", "1", "-i", "pipe:0", "-vf",
                         "scale=1280:720", "-frames:v", "1", "-pix_fmt", "gray", "-f", "rawvideo", "pipe:1"], image)
        mark = barcode(raw)
        require(mark is not None and mark[1] == source.epoch, f"invalid scene identity: {mark}")
        with source.lock:
            stamp = source.ledger.get(mark[0])
        require(stamp is not None, "scene marker absent from source ledger")
        return {"tick": mark[0], "epoch": mark[1], "before": before, "received": received,
                "generated": stamp, "age_upper_seconds": received - stamp}

    def cleanup(self):
        errors = []
        if self.agent and self.agent.poll() is None:
            for action in (self.observe_resources, self.stop):
                try:
                    action()
                except Exception as error:
                    errors.append(str(error))
        for name, child in reversed(self.children):
            if child.poll() is None:
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
                    errors.append(name + " required forced termination")
                except ProcessLookupError:
                    child.wait(timeout=5)
        for item in self.inventory.values():
            if still_owned(item):
                errors.append(f"owned process survives: {item}")
        if hasattr(self, "ports"):
            for port in self.ports:
                if port_open(port):
                    errors.append(f"listener survives: {port}")
        pulse = self.directory / "state/production/production-audio/native.sock"
        if pulse.exists():
            errors.append("private Pulse socket survives")
        write_json(self.directory / "commands.json", self.commands)
        write_json(self.directory / "cleanup.json", {"errors": errors, "children": [
            {"name": name, "pid": child.pid, "returncode": child.poll()} for name, child in self.children]})
        return errors


class Source:
    def __init__(self, run, epoch, frozen=False, backlog_seconds=0):
        self.run, self.epoch, self.frozen = run, epoch, frozen
        self.backlog_frames = int(backlog_seconds * FPS)
        self.peak_queue_bytes = 0
        self.ledger, self.lock, self.errors = {}, threading.Lock(), []
        self.cancel = threading.Event()
        self.expected_stops = set()
        self.observations = []
        self.child = run.spawn(f"game-{epoch}", [
            "ffmpeg", "-v", "error", "-nostdin", "-filter_threads", "1", "-f", "rawvideo",
            "-pixel_format", "gray", "-video_size", "1280x720", "-framerate", "30", "-i", "pipe:0",
            "-f", "lavfi", "-i", GAME_AUDIO, "-t", str(LIMITS["source_seconds"]),
            "-c:v", "libx264", "-threads", "2", "-preset", "ultrafast", "-tune", "zerolatency",
            "-g", "30", "-pix_fmt", "yuv420p", "-c:a", "aac", "-ar", "48000", "-f", "flv",
            f"rtmp://127.0.0.1:{run.ports[2]}/game",
        ], stdin=subprocess.PIPE)
        self.thread = threading.Thread(target=self.feed, name="generated-frames")
        self.mic = run.spawn(f"mic-{epoch}", [
            "ffmpeg", "-v", "error", "-nostdin", "-re", "-f", "lavfi", "-i",
            MIC_AUDIO,
            "-t", str(LIMITS["source_seconds"]), "-c:a", "libopus", "-ar", "48000", "-f", "rtsp",
            "-rtsp_transport", "tcp", f"rtsp://127.0.0.1:{run.ports[3]}/mic",
        ])
        self.thread.start()

    def feed(self):
        stream = self.child.stdin
        origin = time.monotonic()
        queue = deque()
        try:
            os.set_blocking(stream.fileno(), False)
            for ordinal in range(FPS * LIMITS["source_seconds"]):
                if self.cancel.wait(max(0, origin + ordinal / FPS - time.monotonic())):
                    break
                tick = min(ordinal // 3, 20) if self.frozen else ordinal // 3
                with self.lock:
                    self.ledger.setdefault(tick, time.monotonic())
                queue.append(frame(tick, self.epoch, min(ordinal, 60) if self.frozen else ordinal))
                queued_bytes = len(queue) * WIDTH * HEIGHT
                self.peak_queue_bytes = max(self.peak_queue_bytes, queued_bytes)
                require(queued_bytes <= 128 * 1024 * 1024, "generated backlog exceeded 128 MiB")
                if len(queue) <= self.backlog_frames:
                    continue
                pixels = memoryview(queue.popleft())
                deadline = time.monotonic() + LIMITS["startup_pipe_seconds"]
                while pixels and not self.cancel.is_set():
                    require(time.monotonic() < deadline, "source pipe exceeded startup/backpressure budget")
                    if select.select([], [stream], [], 0.1)[1]:
                        try:
                            written = os.write(stream.fileno(), pixels)
                            pixels = pixels[written:]
                        except BlockingIOError:
                            pass
        except BrokenPipeError:
            self.errors.append({"message": "publisher closed raw input", "at": time.monotonic(),
                                "expected": "game" in self.expected_stops})
        except Exception as error:
            self.errors.append({"message": str(error), "at": time.monotonic(), "expected": False})
        finally:
            stream.close()

    def observe(self, phase):
        publishers = {"game": self.child.poll(), "mic": self.mic.poll()}
        observation = {"phase": phase, "at": time.monotonic(), "publishers": publishers,
                       "expected_stops": sorted(self.expected_stops), "errors": list(self.errors)}
        self.observations.append(observation)
        source_health(observation["errors"], publishers, self.expected_stops)

    def expect_stop(self, phase):
        self.observe("before " + phase)
        self.expected_stops.update(("game", "mic"))
        self.observations.append({"phase": phase, "at": time.monotonic(),
                                  "expected_stops": sorted(self.expected_stops)})

    def close(self):
        self.cancel.set()
        self.thread.join(timeout=12)
        require(not self.thread.is_alive(), "source feeder did not join")
        try:
            self.observe("feeder joined")
        finally:
            write_json(self.run.directory / f"source-{self.epoch}.json", {"epoch": self.epoch, "frozen": self.frozen,
                        "ledger": self.ledger, "errors": self.errors, "peak_queue_bytes": self.peak_queue_bytes,
                        "backlog_frames": self.backlog_frames, "observations": self.observations})


def inspect_recording(run, path, epoch, fault_tail=False, evidence_tag=None):
    tag = evidence_tag or str(epoch)
    probe = json.loads(run.tool(["ffprobe", "-v", "error", "-count_frames", "-count_packets",
                                "-show_streams", "-show_format", "-of", "json", str(path)]))
    write_json(run.directory / f"probe-{tag}.json", probe)
    streams = probe["streams"]
    video = [v for v in streams if v["codec_type"] == "video"]
    audio = [v for v in streams if v["codec_type"] == "audio"]
    require(len(video) == len(audio) == 1, "expected one video and one audio stream")
    require(video[0]["codec_name"] == "h264" and audio[0]["codec_name"] == "aac", "unexpected output codecs")
    require((video[0]["width"], video[0]["height"]) == (WIDTH, HEIGHT), "wrong output geometry")
    require(Fraction(video[0]["r_frame_rate"]) == FPS, "wrong output frame rate")
    require(int(audio[0]["sample_rate"]) == 48000 and audio[0]["channels"] == 2,
            "wrong output audio geometry")
    require(int(video[0]["nb_read_frames"]) >= 250 and int(audio[0]["nb_read_packets"]) > 100,
            "insufficient decoded frames or audio packets")
    duration = float(probe["format"]["duration"])
    maximum = LIMITS["duration_max"] + (LIMITS["fault_seconds"] if fault_tail else 0)
    require(LIMITS["duration_min"] <= duration <= maximum, f"wrong duration: {duration}")
    run.tool(["ffmpeg", "-v", "error", "-xerror", "-i", str(path), "-map", "0:v:0", "-map", "0:a:0",
              "-f", "null", "-"], timeout=60)
    packets = json.loads(run.tool(["ffprobe", "-v", "error", "-show_packets", "-show_entries",
                                  "packet=stream_index,pts_time,dts_time,duration_time", "-of", "json", str(path)]))["packets"]
    for stream in streams:
        timestamps = [float(p["dts_time"]) for p in packets
                      if p["stream_index"] == stream["index"] and "dts_time" in p]
        require(timestamps and all(b >= a for a, b in zip(timestamps, timestamps[1:])),
                "missing or backward decode timestamps")
    write_json(run.directory / f"packets-{tag}.json", packets)
    decoded_frames = json.loads(run.tool(["ffprobe", "-v", "error", "-show_frames", "-show_entries",
                                         "frame=media_type,best_effort_timestamp_time,nb_samples", "-of", "json", str(path)]))["frames"]
    timelines = {kind: [float(f["best_effort_timestamp_time"]) for f in decoded_frames
                        if f["media_type"] == kind] for kind in ("video", "audio")}
    for timestamps in timelines.values():
        require(timestamps and all(b >= a for a, b in zip(timestamps, timestamps[1:])),
                "missing or backward decoded presentation timestamps")
    write_json(run.directory / f"frame-timestamps-{tag}.json", timelines)
    cadence = video_cadence(timelines["video"], fault_tail)
    clock = audio_clock([f for f in decoded_frames if f["media_type"] == "audio"])
    endpoints = {"start_offset_seconds": timelines["audio"][0] - timelines["video"][0],
                 "end_offset_seconds": clock["origin_seconds"] + clock["samples"] / 48000
                 - timelines["video"][-1] - 1 / FPS}
    require(all(abs(offset) <= LIMITS["max_av_offset_seconds"] for offset in endpoints.values()),
            f"A/V stream endpoints exceed envelope: {endpoints}")
    prefix = ["-t", str(LIMITS["record_seconds"])] if fault_tail else []
    raw = run.tool(["ffmpeg", "-v", "error", "-i", str(path)] + prefix + ["-vf", "fps=10", "-pix_fmt", "gray",
                    "-f", "rawvideo", "pipe:1"], timeout=60)
    size = WIDTH * HEIGHT
    require(len(raw) % size == 0, "partial raw decoded frame")
    marks = [barcode(raw[offset:offset + size]) for offset in range(0, len(raw), size)]
    valid, span = assess_marks(marks, epoch)
    valid = [(at + timelines["video"][0], tick) for at, tick in valid]
    pcm = array.array("h", run.tool(["ffmpeg", "-v", "error", "-i", str(path)] + prefix + ["-vn", "-ac", "1", "-ar", "48000",
                                     "-f", "s16le", "pipe:1"], timeout=60))
    if not fault_tail:
        require(len(pcm) == clock["samples"], "decoded PCM sample count differs from presentation clock")
    windows, coverage = analyze_audio(pcm, timelines["audio"][0])
    presence = audio_presence(windows)
    alignment = av_alignment(valid, windows)
    alignment["first_video_pts_seconds"] = timelines["video"][0]
    alignment["first_audio_pts_seconds"] = timelines["audio"][0]
    result = {"sha256": digest(path), "duration": duration, "frames": int(video[0]["nb_read_frames"]),
              "content_span_seconds": span, "markers": valid, "audio_windows": windows,
              "av_alignment": alignment,
              "audio_clock": clock,
              "video_cadence": cadence, "audio_coverage": coverage, "audio_presence": presence,
              "stream_endpoints": endpoints,
              "av_resolution_seconds": 0.1, "full_decode_to_eof": True}
    result["content_analysis"] = "pre-fault ten-second prefix" if fault_tail else "whole recording"
    write_json(run.directory / f"decoded-{tag}.json", result)
    return result


def reject_recording(run, path, epoch, tag, reason):
    try:
        inspect_recording(run, path, epoch, evidence_tag=tag)
    except AssertionError as error:
        require(reason in str(error), f"wrong {tag} rejection: {error}")
        return {"sha256": digest(path), "rejection": str(error)}
    raise AssertionError("oracle accepted " + tag)


def scenario(directory, binary, case, epoch):
    run = Run(directory, binary)
    source = None
    result = {"case": case, "result": "FAIL", "limits": LIMITS}
    try:
        run.display()
        run.prepare()
        source = Source(run, epoch, frozen=case == "frozen",
                        backlog_seconds=LIMITS["backlog_delay_seconds"] if case == "delay" else 0)
        require(run.control("start", {"output": "recording"})["state"] == "running", "Start did not reach Running")
        run.observe_resources()
        samples, counters = [], []
        started = time.monotonic()
        for _ in range(LIMITS["record_seconds"]):
            time.sleep(max(0, started + len(samples) + 1 - time.monotonic()))
            samples.append(run.preview(source))
            counters.append(run.transport())
            source.observe("recording sample")
        result["scene_samples"] = samples
        write_json(directory / "scene.json", samples)
        write_json(directory / "transport.json", counters)
        if case in ("disconnect", "stall", "worker-death"):
            # The injected failure can make Agent teardown close both publishers.
            source.expect_stop("injected " + case)
            if case == "worker-death":
                candidates = []
                for pid, item in descendants(run.agent.pid).items():
                    try:
                        command = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
                        if any(b"ladspa=file=" in arg for arg in command):
                            candidates.append(item)
                    except FileNotFoundError:
                        pass
                require(len(candidates) == 1, f"ambiguous RNNoise worker identity: {candidates}")
                item = candidates[0]
                require(still_owned(item), "worker identity changed before fault")
                os.kill(item["pid"], signal.SIGKILL)
            elif case == "stall":
                os.kill(source.child.pid, signal.SIGSTOP)
            else:
                os.kill(source.child.pid, signal.SIGTERM)
            fault_at = time.monotonic()
            state = run.control("status")
            while state["state"] == "running":
                require(time.monotonic() - fault_at < LIMITS["fault_seconds"], "Agent failed to detect media fault")
                time.sleep(0.2)
                state = run.control("status")
            require(state["state"] in ("failed", "stopping"), f"unexpected fault state: {state}")
            result["fault"] = {"detected_seconds": time.monotonic() - fault_at, "status": state}
            if case == "stall":
                os.kill(source.child.pid, signal.SIGCONT)
        source.expect_stop("controller Stop")
        run.stop(allow_retry=case != "normal")
        recordings = list((directory / "recordings").glob("*.mkv"))
        require(len(recordings) == 1, f"expected one new OBS recording: {recordings}")
        if case in ("frozen", "delay"):
            require(any(s["age_upper_seconds"] > LIMITS["max_scene_age_seconds"] for s in samples),
                    "bad scene did not violate freshness envelope")
            game_bytes = [next(item["bytesReceived"] for item in c["items"] if item["name"] == "game") for c in counters]
            require(all(b > a for a, b in zip(game_bytes, game_bytes[1:])), "bad feed did not retain byte progress")
        if case == "frozen":
            try:
                inspect_recording(run, recordings[0], epoch)
            except AssertionError as error:
                require("frozen" in str(error), f"wrong negative-control rejection: {error}")
                result["expected_rejection"] = str(error)
            else:
                raise AssertionError("oracle accepted frozen content")
            result["agent_freshness_enforcement"] = "NOT IMPLEMENTED; byte-progress health admitted stale content"
        elif case == "delay":
            result["decoded"] = inspect_recording(run, recordings[0], epoch)
            result["independent_age_gate"] = "FAIL; scene age exceeded envelope despite byte progress"
            result["agent_freshness_enforcement"] = "NOT IMPLEMENTED; the independent observer detected stale content"
            result["backlog"] = {"boundary": "generated frame queue before RTMP publication",
                                 "frames": source.backlog_frames, "peak_bytes": source.peak_queue_bytes}
        else:
            require(all(s["age_upper_seconds"] <= LIMITS["max_scene_age_seconds"]
                        for s in samples[LIMITS["scene_startup_samples"]:]),
                    "live scene exceeded freshness envelope")
            result["decoded"] = inspect_recording(run, recordings[0], epoch,
                                                   fault_tail=case in ("disconnect", "stall", "worker-death"))
        if case == "normal":
            delayed = directory / "bad-av-offset.mkv"
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-itsoffset",
                      str(LIMITS["av_negative_delay_seconds"]), "-i", str(recordings[0]),
                      "-map", "0:v:0", "-map", "1:a:0", "-c", "copy", str(delayed)])
            try:
                inspect_recording(run, delayed, epoch, evidence_tag="audio-offset")
            except AssertionError as error:
                require("A/V" in str(error), f"wrong delayed-audio rejection: {error}")
                result["av_negative_control"] = {"delay_seconds": LIMITS["av_negative_delay_seconds"],
                                                 "sha256": digest(delayed), "rejection": str(error)}
            else:
                raise AssertionError("oracle accepted delayed audio")
            discontinuous = directory / "bad-audio-gap.mkv"
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-map", "0:v:0", "-map", "0:a:0",
                      "-c:v", "copy", "-af", f"asetpts='PTS+{LIMITS['av_negative_delay_seconds']}/TB*gte(T,0.5)'",
                      "-c:a", "aac", "-b:a", "160k", str(discontinuous)])
            try:
                inspect_recording(run, discontinuous, epoch, evidence_tag="audio-gap")
            except AssertionError as error:
                require("A/V audio PTS discontinuity" in str(error), f"wrong audio-gap rejection: {error}")
                result["audio_gap_negative_control"] = {"sha256": digest(discontinuous), "rejection": str(error)}
            else:
                raise AssertionError("oracle accepted audio timestamp gap")
            wrong_boundary = directory / "bad-audio-boundary.mkv"
            replacement = "0.18*sin(2*PI*1640*t)+0.15*sin(2*PI*180*t)"
            expressions = "|".join(f"if(lt(t,0.4),{replacement},val({channel}))" for channel in (0, 1))
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-map", "0:v:0", "-map", "0:a:0",
                      "-c:v", "copy", "-af", f"aeval=exprs='{expressions}':c=stereo,aformat=channel_layouts=stereo",
                      "-c:a", "aac", "-b:a", "160k", str(wrong_boundary)])
            try:
                inspect_recording(run, wrong_boundary, epoch, evidence_tag="audio-boundary")
            except AssertionError as error:
                require("A/V audio source identity" in str(error), f"wrong audio-boundary rejection: {error}")
                result["audio_boundary_negative_control"] = {"sha256": digest(wrong_boundary), "rejection": str(error)}
            else:
                raise AssertionError("oracle accepted wrong boundary audio")
            sparse = directory / "bad-video-vfr.mkv"
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-map", "0:v:0", "-map", "0:a:0",
                      "-vf", "select='mod(n,4)'", "-fps_mode", "vfr",
                      "-c:v", "libx264", "-threads", "2", "-preset", "ultrafast", "-c:a", "copy", str(sparse)])
            result["vfr_negative_control"] = reject_recording(run, sparse, epoch, "video-vfr", "video cadence:")
            tail = directory / "bad-audio-tail.mkv"
            tail_at = result["decoded"]["audio_clock"]["samples"] / 48000 - 0.4
            expressions = "|".join(f"if(gte(t,{tail_at}),{replacement},val({channel}))" for channel in (0, 1))
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-map", "0:v:0", "-map", "0:a:0",
                      "-c:v", "copy", "-af", f"aeval=exprs='{expressions}':c=stereo,aformat=channel_layouts=stereo",
                      "-c:a", "aac", "-b:a", "160k", str(tail)])
            result["audio_tail_negative_control"] = reject_recording(run, tail, epoch, "audio-tail", "A/V audio source identity")
            dropout = directory / "bad-mic-dropout.mkv"
            run.tool(["ffmpeg", "-v", "error", "-i", str(recordings[0]), "-map", "0:v:0", "-map", "0:a:0",
                      "-c:v", "copy", "-af", "bandreject=f=180:t=h:w=100:enable='between(t,3,6.5)'",
                      "-c:a", "aac", "-b:a", "160k", str(dropout)])
            result["mic_dropout_negative_control"] = reject_recording(run, dropout, epoch, "mic-dropout", "mic audio consecutive dropout")
        if case == "worker-death":
            source.close()
            for child in (source.child, source.mic):
                if child.poll() is None:
                    child.terminate()
                    child.wait(timeout=5)
            source = None
            require(run.control("prepare")["state"] == "ready", "explicit recovery Prepare failed")
            source = Source(run, 15)
            require(run.control("start", {"output": "recording"})["state"] == "running", "explicit recovery Start failed")
            recovered_samples = []
            recovery_started = time.monotonic()
            for _ in range(LIMITS["record_seconds"]):
                time.sleep(max(0, recovery_started + len(recovered_samples) + 1 - time.monotonic()))
                recovered_samples.append(run.preview(source))
                source.observe("recovery sample")
            source.expect_stop("recovery Stop")
            run.stop()
            fresh = set((directory / "recordings").glob("*.mkv")) - set(recordings)
            require(len(fresh) == 1, "recovery did not produce a distinct recording")
            result["recovery"] = inspect_recording(run, fresh.pop(), 15)
            result["recovery"]["scene_samples"] = recovered_samples
            require(all(sample["age_upper_seconds"] <= LIMITS["max_scene_age_seconds"]
                        for sample in recovered_samples[LIMITS["scene_startup_samples"]:]), "stale recovery scene")
        result["result"] = "PASS"
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {error}"
        if run.agent and run.agent.poll() is None:
            try:
                result["failure_status"] = run.control("status")
            except Exception as observation_error:
                result["failure_observation_error"] = str(observation_error)
        if "BLOCKED:" in str(error):
            result["result"] = "BLOCKED"
    finally:
        finalization_errors = []
        for owned in (source,):
            if owned:
                try:
                    owned.close()
                except Exception as error:
                    finalization_errors.append(str(error))
        cleanup = run.cleanup()
        cleanup.extend(finalization_errors)
        result["cleanup_errors"] = cleanup
        if cleanup:
            result["result"] = "FAIL"
        write_json(directory / "result.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-identity", action="store_true")
    parser.add_argument("--build-receipt", action="store_true")
    parser.add_argument("--case", choices=CASES, action="append", help="diagnostic subset, not complete acceptance")
    args = parser.parse_args()
    binary = ROOT / "target/debug/splitcast-rs-core"
    if args.source_identity:
        print(json.dumps(source_identity(), sort_keys=True))
        return 0
    if args.build_receipt:
        identity = source_identity()
        before = json.loads((Path(os.environ["SPLITCAST_MEDIA_EVIDENCE_DIR"]) / "build-source.json").read_text())
        require(before == identity, "source changed during native build")
        print(json.dumps({"source": identity, "binary_sha256": digest(binary),
                          "obs_closure_sha256": digest(Path(os.environ["SPLITCAST_MEDIA_EVIDENCE_DIR"]) / "obs-closure.txt"),
                          "command": "cargo build -p splitcast-rs-core --locked --offline",
                          "features": "default", "profile": "dev"}, sort_keys=True))
        return 0
    os.umask(0o077)
    directory = Path(os.environ.get("SPLITCAST_MEDIA_EVIDENCE_DIR", ROOT / "artifacts/media-plane/direct-blocked"))
    directory.mkdir(parents=True, mode=0o700, exist_ok=True)
    identity = source_identity()
    write_json(directory / "source-before.json", identity)
    write_json(directory / "limits.json", LIMITS)
    results = []
    try:
        require(os.environ.get("SPLITCAST_MEDIA_SANDBOX") == "1", "BLOCKED: run tools/verify-media.sh in media-test shell")
        require(not any(Path("/dev").glob("dri/*")) and not Path("/dev/snd").exists()
                and not any(Path("/dev").glob("video*")), "BLOCKED: physical media devices visible")
        interfaces = {line.split(":")[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()[2:]}
        require(interfaces == {"lo"}, "BLOCKED: non-loopback network interface visible")
        require(binary.is_file(), "BLOCKED: build the current native binary first")
        build = json.loads((directory / "build.json").read_text())
        require(build["source"] == identity and build["binary_sha256"] == digest(binary),
                "BLOCKED: source or native binary differs from build receipt")
        require(build["obs_closure_sha256"] == digest(directory / "obs-closure.txt"), "BLOCKED: OBS closure receipt changed")
        require(os.environ.get("SPLITCAST_RNNOISE_PLUGIN"), "BLOCKED: use the pinned media-test devShell")
        require(Path(os.environ["SPLITCAST_RNNOISE_PLUGIN"]).is_file(), "BLOCKED: RNNoise plugin unavailable")
        write_json(directory / "binary.json", {"sha256": digest(binary), "path": str(binary)})
        environment = {"load_average": os.getloadavg(), "cpu_count": os.cpu_count(),
                       "kernel": os.uname().release, "uid_map": Path("/proc/self/uid_map").read_text(),
                       "network_interfaces": sorted(interfaces), "tools": {}}
        for name in ("ffmpeg", "ffprobe", "mediamtx", "obs", "Xvfb", "pulseaudio", "bwrap", "python3"):
            path = Path(shutil.which(name)).resolve()
            environment["tools"][name] = {"path": str(path), "sha256": digest(path)}
        environment["rnnoise_sha256"] = digest(os.environ["SPLITCAST_RNNOISE_PLUGIN"])
        write_json(directory / "environment.json", environment)
        for epoch, case in enumerate(args.case or CASES, 1):
            target = directory / case
            target.mkdir(mode=0o700)
            result = scenario(target, binary.resolve(), case, epoch)
            results.append(result)
            print(case + ": " + result["result"] + (" " + result["error"] if "error" in result else ""), flush=True)
    except Exception as error:
        results.append({"result": "BLOCKED", "error": str(error)})
    after = source_identity()
    write_json(directory / "source-after.json", after)
    complete = not args.case and len(results) == len(CASES)
    passed = complete and all(r["result"] == "PASS" for r in results) and identity == after
    write_json(directory / "summary.json", {"result": "PASS" if passed else "INCOMPLETE", "cases": results,
                                            "complete_matrix": complete, "unchanged_source": identity == after,
                                            "source_tree_sha256": identity["tree_sha256"]})
    host_directory = Path(os.environ.get("SPLITCAST_MEDIA_HOST_EVIDENCE", directory))
    print("evidence: " + str(host_directory.relative_to(ROOT)), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
