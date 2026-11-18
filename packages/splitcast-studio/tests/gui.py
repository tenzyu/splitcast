"""Drive release Studio on private Xvfb, through authenticated loopback to two real Agents."""
import contextlib
import http.client
import http.server
import json
import os
from pathlib import Path
import secrets
import shlex
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "packages/splitcast-studio/Cargo.toml"
APP = ROOT / "packages/splitcast-rs"
WAIT = 15


def build(args, target):
    output = subprocess.check_output(
        ["cargo", *args, "--locked", "--offline", "--message-format=json"], cwd=ROOT
    )
    for line in reversed(output.splitlines()):
        event = json.loads(line)
        if event.get("executable") and event["target"]["name"] == target:
            return event["executable"]
    raise AssertionError(f"missing Cargo executable for {target}")


def eventually(check, label, seconds=WAIT):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.05)
    raise AssertionError(f"timed out: {label}")


def private_file(path, value):
    path.write_text(value)
    path.chmod(0o600)
    return path


def cleanup(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
            raise


def spawn(stack, args, env, log):
    with log.open("wb") as output:
        process = subprocess.Popen(args, env=env, stdout=output, stderr=output)
    stack.callback(cleanup, process)
    return process


def exchange_http(port, headers, command=None, args=None):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=12)
    try:
        if command is None:
            conn.request("GET", "/api/status", headers=headers)
        else:
            body = json.dumps({"command": command, "args": args or {}})
            conn.request("POST", "/api/command", body, {
                **headers, "Content-Type": "application/json"
            })
        reply = conn.getresponse()
        return reply.status, reply.read(1024 * 1024)
    finally:
        conn.close()


def request(port, token, command=None, args=None):
    host = f"127.0.0.1:{port}"
    status, body = exchange_http(port, {
        "Host": host, "Origin": f"http://{host}", "Authorization": f"Bearer {token}"
    }, command, args)
    return status, json.loads(body)


class Relay(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, upstream):
        self.upstream = upstream
        self.records = []
        self.stop_responses = []
        self.hold = None
        self.drop = None
        self.chat_delta = False
        self.held = threading.Event()
        self.release = threading.Event()
        super().__init__(("127.0.0.1", 0), Handler)

    def times(self, name):
        return [at for cmd, at in self.records if cmd == name]


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        self.exchange(None, None)

    def do_POST(self):
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.exchange(payload["command"], payload["args"])

    def exchange(self, command, args):
        self.server.records.append((command, time.monotonic()))
        hold = self.server.hold == command and command is not None
        drop = self.server.drop == command and command is not None
        try:
            headers = {key: self.headers[key] for key in ("Host", "Origin", "Authorization")}
            code, body = exchange_http(self.server.upstream, headers, command, args)
            if command == "stop":
                self.server.stop_responses.append((code, json.loads(body).get("stop_id")))
            if self.server.chat_delta and command == "chat" and code == 200:
                # Inject one bounded delta at the authenticated relay boundary.
                # The real Agent call above still verifies authority and routing.
                page = {
                    "enabled": True, "epoch": "fixture-epoch", "cursor": 1,
                    "more": False, "reset": False, "overrun": False,
                    "subscription": {"state": "subscribed", "notifications": 1,
                                     "duplicates": 0, "dropped": 0, "reconnects": 0, "error": None},
                    "messages": [] if args.get("after") == 1 else [
                        {"cursor": 1, "id": "generated-1", "display_name": "fixture", "text": "generated delta"}
                    ],
                }
                body = json.dumps(page).encode()
            if hold:
                self.server.held.set()
                self.server.release.wait(8)
                self.server.hold = None
            if drop:
                self.server.drop = None
                return
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except (OSError, http.client.HTTPException):
            pass


def relay(stack, port):
    server = Relay(port)
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    def stop():
        server.release.set()
        server.shutdown()
        server.server_close()
        thread.join(2)
    stack.callback(stop)
    return server


def agent_config(root, role, ports, token_paths, worker):
    work = root / role
    work.mkdir()
    tools = work / "tools"
    tools.mkdir()
    for name in ("env", "timeout"):
        (tools / name).symlink_to(shutil.which(name))
    for name in ("obs", "mediamtx", "ssh", "ffmpeg", "ffprobe", "gpu-screen-recorder", "pulseaudio", "pactl"):
        executable = tools / name
        # Use the checked tool path from the local test shell.
        program = f"#!{shutil.which('sh')}\n"
        if name == "pactl":
            program += """case "$4" in
sources) printf '%s\n' '[{"name":"virtual_microphone","properties":{"device.api":"alsa","device.class":"sound"}}]' ;;
sink-inputs) printf '%s\n' '[{"corked":false,"properties":{"media.name":"SplitCast processed microphone"}}]' ;;
*) printf '%s\n' '[{"name":"splitcast_mic"}]' ;;
esac
"""
        else:
            program += (
                f"export SPLITCAST_VIRTUAL_KIND={name}\n"
                f"exec {shlex.quote(worker)} --exact virtual_worker --ignored --nocapture"
                f" >>{shlex.quote(str(work / (name + '.log')))} 2>&1\n"
            )
        executable.write_text(program)
        executable.chmod(0o700)

    cfg = json.loads((APP / "tests/agent-example.json").read_text())
    cfg["role"] = role
    cfg["peer"].update(
        control_port=ports[role], remote_control_port=ports["production"],
        forward_control_port=ports["production"] if role == "capture" else ports["unused"],
        ssh_port=ports["ssh"], timeout_seconds=10, token_file=str(token_paths[role]),
        remote_token_file=str(token_paths["production"]), state_dir=str(work / "state"),
    )
    cfg["capture"].update(
        rtmp_port=ports["rtmp"], rtsp_port=ports["rtsp"], api_port=ports["api"],
        target="virtual-window", game_audio="app:virtual-game",
    )
    cfg["audio"]["mic_source"] = "virtual_microphone"
    cfg["obs"].update(
        binary=str(tools / "obs"), websocket_port=ports["obs"],
        password_file=str(private_file(work / "password", "virtual-obs-private-password")),
        recording_dir=str(work / "recordings"),
    )
    if role == "production":
        cfg["camera"] = {"location": "capture", "input_format": "h264", "encoder": "copy",
                         "width": 320, "height": 180, "fps": 15}
    cfg["tts"].update(credentials_file=str(work / "absent-credentials"),
                      voicevox_url=f"http://127.0.0.1:{ports['unused']}")
    config = private_file(work / "config.json", json.dumps(cfg))
    env = {
        **os.environ, "PATH": str(tools), "SPLITCAST_VIRTUAL_DIR": str(work),
        "SPLITCAST_VIRTUAL_API": str(ports["api"]), "SPLITCAST_VIRTUAL_OBS": str(ports["obs"]),
    }
    return work, config, env


def display(stack, root):
    font_config = root / "fonts.conf"
    font_config.write_text(f"""<?xml version="1.0"?><fontconfig>
<dir>{os.environ["SPLITCAST_TEST_FONTS"]}</dir><cachedir>{root / "font-cache"}</cachedir>
</fontconfig>""")
    cookie = secrets.token_bytes(16)
    number = str(2000 + secrets.randbelow(9000))
    authority = root / "xauthority"
    fields = (b"", number.encode(), b"MIT-MAGIC-COOKIE-1", cookie)
    authority.write_bytes(struct.pack("!H", 65535) + b"".join(
        struct.pack("!H", len(value)) + value for value in fields
    ))
    authority.chmod(0o600)
    env = {
        **os.environ, "DISPLAY": f":{number}", "XAUTHORITY": str(authority),
        "FONTCONFIG_FILE": str(font_config), "VK_DRIVER_FILES": os.environ["SPLITCAST_TEST_MESA_ICD"],
        "VK_LOADER_LAYERS_DISABLE": "~implicit~", "LP_NUM_THREADS": "2",
        "XKB_CONFIG_ROOT": os.environ["SPLITCAST_TEST_XKB_ROOT"],
        "XDG_CONFIG_HOME": str(root / "config"), "XDG_CACHE_HOME": str(root / "cache"),
        "XDG_RUNTIME_DIR": str(root / "runtime"),
    }
    (root / "runtime").mkdir(mode=0o700)
    for key in ("WAYLAND_DISPLAY", "VK_ICD_FILENAMES", "VK_INSTANCE_LAYERS", "DRI_PRIME", "DBUS_SESSION_BUS_ADDRESS"):
        env.pop(key, None)
    log = root / "Xvfb.log"
    process = spawn(stack, ["Xvfb", env["DISPLAY"], "-screen", "0", "1440x920x24",
                             "-nolisten", "tcp", "-noreset", "-auth", str(authority)], env, log)
    def ready():
        assert process.poll() is None, log.read_text()
        return subprocess.run(["xdotool", "getdisplaygeometry"], env=env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    eventually(ready, "private Xvfb display")
    return env


class Studio:
    def __init__(self, stack, root, binary, role, port, agent_port, token_path, env):
        self.env = env
        self.log = root / f"{role}-studio.log"
        self.process = spawn(stack, [
            binary, "--port", str(port), "--agent-port", str(agent_port),
            "--token-file", str(token_path), "--trace-render",
        ], env, self.log)
        def find_window():
            assert self.process.poll() is None, self.lines()[-8:]
            result = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "^SplitCast Studio$"],
                                    text=True, env=env, capture_output=True)
            return result.stdout.strip().splitlines()[-1] if result.returncode == 0 else None
        self.window = eventually(find_window, "Studio window", 8)
        self.xdo("windowfocus", "--sync", self.window)
        self.wait("active=true")
        print(next((line for line in self.lines() if line.startswith("studio-renderer ")),
                   "renderer not reported"), flush=True)

    def lines(self):
        return self.log.read_text(errors="replace").splitlines()

    def render_count(self):
        return sum(line.startswith("studio-render ") for line in self.lines())

    def wait(self, text, after=0, seconds=WAIT):
        def visible():
            assert self.process.poll() is None, self.lines()[-8:]
            return any(line.startswith("studio-render ") and text in line for line in self.lines()[after:])
        eventually(visible, text, seconds)

    def xdo(self, *args):
        subprocess.run(["xdotool", *args], env=self.env, check=True, timeout=5)

    def target(self, label):
        prefix = f"studio-target {label}|"
        def shown():
            latest = []
            for line in reversed(self.lines()):
                if line.startswith("studio-render "):
                    break
                latest.append(line)
            for line in latest:
                if line.startswith(prefix):
                    return line.removeprefix(prefix).split("|")
            return None
        return eventually(shown, f"GUI target {label}")

    def click(self, label):
        x, y = self.target(label)
        self.xdo("mousemove", "--window", self.window, x, y, "click", "1")

    def drag(self, label, dx, dy):
        x, y = self.target(label)
        self.xdo("mousemove", "--window", self.window, x, y, "mousedown", "1")
        self.xdo("mousemove_relative", "--", str(dx), str(dy))
        self.xdo("mouseup", "1")

    def edit(self, label, value):
        self.click(label)
        self.xdo("key", "ctrl+a")
        self.xdo("type", "--clearmodifiers", value)
        time.sleep(0.15)


def sample_idle(studio, count=20):
    proc = Path("/proc") / str(studio.process.pid)
    def ticks():
        fields = (proc / "stat").read_text().rsplit(")", 1)[1].split()
        return int(fields[11]) + int(fields[12])
    time.sleep(3)
    before, draws, at = ticks(), studio.render_count(), time.monotonic()
    pss = []
    for _ in range(count):
        time.sleep(1)
        status = (proc / "smaps_rollup").read_text()
        pss.append(int(next(line for line in status.splitlines() if line.startswith("Pss:")).split()[1]) / 1024)
    running = time.monotonic() - at
    cpu = (ticks() - before) * 100 / (os.sysconf("SC_CLK_TCK") * running)
    renders = studio.render_count() - draws
    print(f"idle samples={count} elapsed={running:.3f}s PSS MiB={min(pss):.3f}..{max(pss):.3f}"
          f" CPU={cpu:.3f}% of one core redraws={renders}", flush=True)
    return max(pss) <= 80 and cpu <= 0.5 and renders == 0


def verify():
    os.umask(0o077)
    if len(sys.argv) > 2:
        raise SystemExit("Usage: gui.py [STUDIO_RELEASE_BINARY]")
    studio_bin = sys.argv[1] if len(sys.argv) == 2 else build(
        ["build", "--release", "--manifest-path", str(MANIFEST)], "splitcast-studio"
    )
    agent_bin = build(["build", "-p", "splitcast-rs-core"], "splitcast-rs-core")
    worker = build(["test", "-p", "splitcast-rs-core", "--test", "two_agents", "--no-run"], "two_agents")
    keys = ["production", "capture", "rtmp", "rtsp", "api", "obs", "ssh", "unused"]
    with contextlib.ExitStack() as stack:
        root = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="splitcast-studio-")))
        env = display(stack, root)
        reservations = [stack.enter_context(contextlib.closing(socket.socket())) for _ in keys]
        for sock in reservations:
            sock.bind(("127.0.0.1", 0))
        ports = {key: sock.getsockname()[1] for key, sock in zip(keys, reservations)}
        tokens = {role: secrets.token_hex(24) for role in ("production", "capture")}
        paths = {role: private_file(root / f"{role}.token", token) for role, token in tokens.items()}
        for sock in reservations:
            sock.close()
        roles = {role: agent_config(root, role, ports, paths, worker) for role in tokens}
        production_relay = relay(stack, ports["production"])
        direct = Studio(stack, root, studio_bin, "production", production_relay.server_port,
                        ports["production"], paths["production"], env)
        direct.edit("Overlay text", "Private fixture draft")

        agents = {}
        for role, (work, config, agent_env) in roles.items():
            agents[role] = spawn(stack, [agent_bin, "--config", config, "agent"], agent_env, work / "agent.log")
        def agent(role, cmd=None, args=None):
            return request(ports[role], tokens[role], cmd, args)
        def okay(role, command, args=None):
            status, body = agent(role, command, args)
            assert status == 200, (command, status, body.get("code"))
            return body
        direct.wait("Idle")
        direct.wait("Local draft")
        direct.click("Apply")
        eventually(lambda: okay("production", "scene")["draft"]["overlay"]["text"] == "Private fixture draft",
                   "offline draft applied in Idle")
        direct.wait("Agent draft · unsaved")
        budget = sample_idle(direct)
        assert not production_relay.times("preview-small"), "Idle preview request"
        old_camera = okay("production", "scene")["draft"]["camera"]
        direct.drag("Camera guide", -30, 14)
        direct.click("Apply")
        eventually(lambda: okay("production", "scene")["draft"]["camera"]["x"] < old_camera["x"],
                   "offline camera guide moved in Agent draft")
        cleanup(direct.process)

        # A's owner-bound production fixture supplies a synthetic camera feed.
        # Test the two-Agent Start with matching camera-free role configs, so no
        # worker requires a camera device on the capture role.
        cleanup(agents["production"])
        work, config, agent_env = roles["production"]
        production_config = json.loads(config.read_text())
        production_config.pop("camera")
        private_file(config, json.dumps(production_config))
        agents["production"] = spawn(stack, [agent_bin, "--config", config, "agent"],
                                     agent_env, work / "agent-restart.log")
        direct = Studio(stack, root, studio_bin, "production-stage", production_relay.server_port,
                        ports["production"], paths["production"], env)
        direct.wait("Idle")
        direct.edit("Overlay text", "Private fixture draft")
        direct.click("Apply")
        direct.wait("Agent draft · unsaved")
        cleanup(direct.process)

        forwarded = relay(stack, ports["capture"])
        studio = Studio(stack, root, studio_bin, "capture", forwarded.server_port,
                        ports["capture"], paths["capture"], env)
        studio.wait("Idle")
        studio.click("Prepare")
        studio.wait("Ready")
        studio.click("Live")
        studio.target("Type LIVE")
        assert not forwarded.times("start"), "selecting Live submitted Start"
        studio.click("Recording")
        studio.click("Start")
        studio.wait("Running")
        assert okay("capture", "scene")["applied"]["overlay"]["text"] == "Private fixture draft"
        studio.wait("| frame |")
        eventually(lambda: len(forwarded.times("preview-small")) >= 4, "four preview requests")
        intervals = [b - a for a, b in zip(forwarded.times("preview-small"), forwarded.times("preview-small")[1:])]
        print(f"preview requests={len(intervals) + 1} minimum gap={min(intervals):.3f}s", flush=True)
        assert min(intervals) >= 1, "snapshot exceeded 1 Hz"
        studio.click("Hide editor")
        studio.target("Show editor")
        baseline = len(forwarded.times("preview-small"))
        time.sleep(2.3)
        assert len(forwarded.times("preview-small")) == baseline, "snapshots while editor hidden"
        studio.click("Show editor")
        studio.click("Hide preview")
        studio.target("Show preview")
        baseline = len(forwarded.times("preview-small"))
        time.sleep(2.3)
        assert len(forwarded.times("preview-small")) == baseline, "snapshots while preview hidden"
        studio.click("Show preview")
        assert forwarded.times("chat"), "visible chat did not read the Agent"
        studio.click("Hide chat")
        studio.target("Show chat")
        baseline = len(forwarded.times("chat"))
        time.sleep(4.1)
        assert len(forwarded.times("chat")) == baseline, "chat polled while hidden"
        forwarded.chat_delta = True
        studio.click("Show chat")
        studio.wait("chat=1")
        time.sleep(0.5)
        steady_draws = studio.render_count()
        time.sleep(4.2)
        assert studio.render_count() == steady_draws, "unchanged preview and empty chat deltas redrew the UI"

        studio.edit("Overlay text", "Verified OBS edit")
        other = okay("capture", "scene")
        other["draft"]["overlay"]["text"] = "Concurrent Agent draft"
        okay("capture", "scene-apply", {
            "generation": other["generation"], "operation": "external-fixture-edit", "layout": other["draft"],
        })
        mark = len(studio.lines())
        studio.click("Apply")
        studio.wait("Conflict", after=mark)
        studio.click("Keep local draft")
        studio.click("Apply")
        studio.wait("Applied · unsaved")
        studio.click("OBS readback")
        studio.click("Save")
        studio.wait("Saved · OBS verified")
        saved = okay("capture", "scene")
        assert saved["applied"] == saved["saved"] == saved["draft"]
        assert saved["saved"]["overlay"]["text"] == "Verified OBS edit"

        forwarded.drop = "scene-apply"
        studio.edit("Overlay text", "Lost reply")
        mark = len(studio.lines())
        studio.click("Apply")
        studio.wait("Outcome unknown", after=mark)
        assert forwarded.drop is None
        mark = len(studio.lines())
        studio.click("Read Agent draft")
        studio.wait("Applied · unsaved", after=mark)

        forwarded.hold = "scene-apply"
        studio.edit("Overlay text", "Reply held past Stop")
        studio.click("Apply")
        assert forwarded.held.wait(8), "real Agent scene result was not held"
        mark = len(studio.lines())
        status_before = len(forwarded.times(None))
        studio.click("Status")
        eventually(lambda: len(forwarded.times(None)) > status_before, "Status during pending Apply")
        studio.click("Stop")
        eventually(lambda: forwarded.times("stop"), "Stop during pending Apply")
        status, _ = agent("capture", "stop")
        assert status in (200, 202), "concurrent Stop declined"
        studio.wait("Stop completed", after=mark)
        [(admission, stop_id)] = forwarded.stop_responses
        if admission == 202:
            studio.wait("Stop admitted", after=mark)
        stop_proof = agent("capture")[1]
        assert stop_proof["stop_id"] == stop_id and stop_proof["remote_release_pending"] is False
        print(f"concurrent Stop: GUI HTTP {admission}; matching completed Idle proof", flush=True)
        assert agent("production")[1]["state"] == "idle"
        assert agent("production")[1]["obs_shutdown"]["output_inactive_confirmed"]
        ended_workers = [
            Path("/proc") / file.read_text().strip()
            for work, _, _ in roles.values() for file in work.glob("*.pid")
        ]
        assert ended_workers, "fixture recorded no owned worker"
        eventually(lambda: all(not worker.exists() for worker in ended_workers), "owned workers exited")
        mark = len(studio.lines())
        forwarded.release.set()
        studio.wait("Stop completed · Agent confirmed cleanup | Outcome unknown", after=mark)
        baseline = len(forwarded.times("preview-small"))
        time.sleep(2.3)
        assert len(forwarded.times("preview-small")) == baseline, "snapshot after Stop"
        mark = len(studio.lines())
        cleanup(agents["capture"])
        studio.wait("offline", after=mark)  # The relay drops upstream failures; no stale phase.
        assert okay("production", "scene")["saved"] == saved["saved"]
        cleanup(studio.process)
        direct = Studio(stack, root, studio_bin, "production-reload", production_relay.server_port,
                        ports["production"], paths["production"], env)
        direct.wait("Idle")
        direct.click("Reload saved")
        direct.wait("Draft matches saved layout")
        assert okay("production", "scene")["draft"] == saved["saved"]
        print("GUI workflow passed: offline draft and camera drag, authenticated forward, Start,"
              " preview bounds/visibility, chat visibility and injected delta, generation conflict,"
              " apply/readback/save/reload, lost and late replies, independent Status/Stop, and disconnect", flush=True)
        if not budget:
            raise AssertionError("release idle budget exceeded")


if __name__ == "__main__":
    verify()
