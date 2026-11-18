"""Exercise release Studio on a private, device-free native Wayland display."""
import argparse
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import socket
import struct
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageDraw

import gui

OUTPUT_SIZE = (1740, 920)


def store(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def output(*args, env=None):
    return subprocess.check_output(args, cwd=gui.ROOT, env=env, text=True, timeout=10).strip()


def sway(env, *args):
    return json.loads(output("swaymsg", "-r", *args, env=env))


def tree_nodes(node):
    yield node
    for child in node["nodes"] + node["floating_nodes"]:
        yield from tree_nodes(child)


def window(env, pid):
    return next((node for node in tree_nodes(sway(env, "-t", "get_tree"))
                 if node.get("pid") == pid and node.get("shell") == "xdg_shell"), None)


def command(env, commands):
    result = sway(env, commands)
    assert result and all(item["success"] for item in result), (commands, result)


class VirtualPointer:
    """The fixture's private zwlr_virtual_pointer_v1 uses its fallback seat."""
    pointer = 5

    def __init__(self, stack, display):
        self.peer = stack.enter_context(socket.socket(socket.AF_UNIX))
        self.peer.settimeout(5)
        self.peer.connect(str(display))
        stream = stack.enter_context(self.peer.makefile("rb"))
        registry, callback, manager = 2, 3, 4
        self.send(1, 1, struct.pack("=I", registry))  # wl_display.get_registry
        self.send(1, 0, struct.pack("=I", callback))  # wl_display.sync
        interface = b"zwlr_virtual_pointer_manager_v1\0"
        global_name = None
        while True:
            sender, header = struct.unpack("=II", stream.read(8))
            opcode, length = header & 0xffff, header >> 16
            body = stream.read(length - 8)
            if sender == callback:  # wl_callback.done
                break
            if sender == registry and opcode == 0:  # wl_registry.global
                name, size = struct.unpack_from("=II", body)
                if body[8:8 + size] == interface:
                    global_name = name
        assert global_name is not None, "private compositor lacks the Wayland virtual pointer protocol"
        padded = interface + bytes(-len(interface) % 4)
        self.send(registry, 0, struct.pack("=II", global_name, len(interface)) + padded
                  + struct.pack("=II", 1, manager))  # wl_registry.bind, version 1
        self.send(manager, 0, struct.pack("=II", 0, self.pointer))  # create_virtual_pointer

    def send(self, sender, opcode, payload=b""):
        self.peer.sendall(struct.pack("=II", sender, ((8 + len(payload)) << 16) | opcode) + payload)

    def event(self, opcode, *args):
        milliseconds = (time.monotonic_ns() // 1_000_000) & 0xffffffff
        self.send(self.pointer, opcode, struct.pack("=" + "I" * (len(args) + 1), milliseconds, *args))
        self.send(self.pointer, 4)  # frame

    def move(self, x, y):
        self.event(1, x, y, *OUTPUT_SIZE)  # motion_absolute

    def button(self, pressed):
        self.event(2, 272, int(pressed))  # button, Linux BTN_LEFT


class WaylandStudio(gui.Studio):
    def __init__(self, *args, pointer):
        self.pointer = pointer
        super().__init__(*args)

    def focus_window(self):
        self.window = gui.eventually(lambda: window(self.env, self.process.pid),
                                     "native Studio xdg_toplevel")
        command(self.env, f"[con_id={self.window['id']}] focus")

    def point(self, label):
        node = window(self.env, self.process.pid)
        x, y = map(int, self.target(label))
        rect, client = node["rect"], node["window_rect"]
        return rect["x"] + client["x"] + x, rect["y"] + client["y"] + y

    def click(self, label):
        self.pointer.move(*self.point(label))
        self.pointer.button(True)
        self.pointer.button(False)

    def drag(self, label, dx, dy):
        x, y = self.point(label)
        self.pointer.move(x, y)
        self.pointer.button(True)
        self.pointer.move(x + dx, y + dy)
        self.pointer.button(False)

    def edit(self, label, value):
        self.click(label)
        env = {name: value for name, value in self.env.items() if name != "WAYLAND_DEBUG"}
        # Clear the bounded overlay field using editing keys. The virtual
        # keyboard's generated keymap can deliver Ctrl+A as ordinary text.
        keys = ["-k", "End"] + ["-k", "BackSpace"] * 80
        subprocess.run(["wtype", *keys, "--", value], env=env, check=True, timeout=5)
        time.sleep(0.15)


class Acceptance:
    def __init__(self, evidence):
        self.evidence = evidence

    def display(self, stack, root):
        runtime = root / "runtime"
        runtime.mkdir(mode=0o700)
        font_config = root / "fonts.conf"
        font_config.write_text(f"""<?xml version="1.0"?><fontconfig>
<dir>{os.environ["SPLITCAST_TEST_FONTS"]}</dir><cachedir>{root / "font-cache"}</cachedir>
</fontconfig>""")
        config = root / "sway.conf"
        width, height = OUTPUT_SIZE
        config.write_text(f"""xwayland disable
output * mode {width}x{height}
focus_follows_mouse no
default_border none
default_floating_border none
seat seat0 fallback true
for_window [title="^SplitCast Studio$"] floating enable, move position 25 35
for_window [app_id="^splitcast-focus-fixture$"] floating enable, move position 1240 45
""")
        shutil.copyfile(config, self.evidence / "sway.conf")
        excluded = {"DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "SWAYSOCK",
                    "WAYLAND_DEBUG", "DBUS_SESSION_BUS_ADDRESS", "DRI_PRIME", "GBM_BACKEND",
                    "WLR_DRM_DEVICES", "LIBSEAT_BACKEND"}
        env = {name: value for name, value in os.environ.items()
               if name not in excluded and not name.startswith(("WLR_", "VK_"))}
        env.update(XDG_RUNTIME_DIR=str(runtime), XDG_SESSION_TYPE="wayland",
                   XDG_CONFIG_HOME=str(root / "config"), XDG_CACHE_HOME=str(root / "cache"),
                   WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman",
                   FONTCONFIG_FILE=str(font_config), XKB_CONFIG_ROOT=os.environ["SPLITCAST_TEST_XKB_ROOT"])
        log = self.evidence / "sway.log"
        compositor = gui.spawn(stack, ["sway", "-V", "-c", config], env, log)

        def sockets():
            assert compositor.poll() is None, log.read_text()[-4000:]
            ipc = next(runtime.glob("sway-ipc.*.sock"), None)
            display = next((path for path in runtime.glob("wayland-*") if path.is_socket()), None)
            return (ipc, display) if ipc and display else None

        ipc, display = gui.eventually(sockets, "private Sway IPC and Wayland sockets")
        env.update(SWAYSOCK=str(ipc), WAYLAND_DISPLAY=display.name)
        outputs = sway(env, "-t", "get_outputs")
        active = [item for item in outputs if item["active"]]
        assert len(active) == 1 and active[0]["name"].startswith("HEADLESS-"), active
        assert (active[0]["rect"]["width"], active[0]["rect"]["height"]) == OUTPUT_SIZE
        store(self.evidence / "outputs.json", outputs)
        # The headless backend has no physical seat devices. Keep a native
        # virtual keyboard attached so wl_keyboard enter/leave are observable.
        self.pointer = VirtualPointer(stack, display)
        keyboard = gui.spawn(stack, ["wtype", "-k", "Shift_L", "-s", "300000"], env,
                             self.evidence / "virtual-keyboard.log")
        gui.eventually(lambda: {"keyboard", "pointer"} <= {
            item["type"] for item in sway(env, "-t", "get_inputs")
        }, "private virtual keyboard and pointer")
        assert keyboard.poll() is None
        env["WAYLAND_DEBUG"] = "client"

        def preserve():
            for path in root.glob("*-studio.log"):
                shutil.copyfile(path, self.evidence / path.name)

        stack.callback(preserve)
        return env

    def studio(self, *args):
        return WaylandStudio(*args, pointer=self.pointer)

    def sample(self, studio, name):
        # Use one-second PSS samples and process (all threads) CPU ticks.
        proc = Path("/proc") / str(studio.process.pid)

        def ticks():
            fields = (proc / "stat").read_text().rsplit(")", 1)[1].split()
            return int(fields[11]) + int(fields[12])

        time.sleep(3)
        redraws = studio.render_count()
        before, start = ticks(), time.monotonic()
        pss_kib = []
        for _ in range(20):
            time.sleep(1)
            rollup = (proc / "smaps_rollup").read_text()
            pss_kib.append(int(re.search(r"^Pss:\s+(\d+) kB$", rollup, re.M)[1]))
        elapsed = time.monotonic() - start
        total_ticks = ticks() - before
        hz = os.sysconf("SC_CLK_TCK")
        cpu = total_ticks * 100 / (hz * elapsed)
        redraws = studio.render_count() - redraws
        store(self.evidence / f"{name}.json",
              dict(elapsed_seconds=elapsed, pss_kib=pss_kib, cpu_ticks=total_ticks,
                   ticks_per_second=hz, cpu_one_core_percent=cpu, redraws=redraws))
        print(f"{name}: {elapsed:.3f}s, PSS {min(pss_kib)/1024:.3f}..{max(pss_kib)/1024:.3f} MiB,"
              f" CPU {cpu:.3f}% of one core, redraws={redraws}", flush=True)
        return max(pss_kib) / 1024, cpu, redraws

    def idle(self, studio):
        pss, cpu, redraws = self.sample(studio, "idle")
        return pss <= 80 and cpu <= 0.5 and redraws == 0

    def screenshot(self, env, name):
        path = self.evidence / f"{name}.png"
        subprocess.run(["grim", path], env=env, check=True, timeout=5)
        with Image.open(path) as image:
            return image.convert("RGB")

    def unfocused(self, stack, root, studio, relay):
        preview = Image.new("RGB", (480, 270), "#265272")
        draw = ImageDraw.Draw(preview)
        for i in range(1, 12):
            draw.rectangle((i * 40, 0, i * 40 + 19, 269), fill="#5e9184")
        draw.text((20, 20), "Generated 480 x 270 preview", fill="white")
        buffer = io.BytesIO()
        preview.save(buffer, format="PNG")
        png = buffer.getvalue()
        (self.evidence / "preview-fixture.png").write_bytes(png)
        relay.preview = json.dumps({"image": "data:image/png;base64," + base64.b64encode(png).decode()}).encode()
        relay.chat_count = 128
        studio.wait("chat=128")
        _, _, redraws = self.sample(studio, "loaded-focused")
        assert redraws == 0, "unchanged 480×270 preview or chat redrew in the focused sample"

        foot_log = self.evidence / "second-client.log"
        foot = gui.spawn(stack, ["foot", "--app-id=splitcast-focus-fixture", "--title=Second native client",
                                 "--font=DejaVu Sans Mono", "--window-size-pixels=390x230",
                                 "--hold", "--", "true"], studio.env, foot_log)
        other = gui.eventually(lambda: window(studio.env, foot.pid), "second native xdg_toplevel")
        command(studio.env, f"[con_id={studio.window['id']}] focus")
        gui.eventually(lambda: window(studio.env, studio.process.pid)["focused"], "Studio focused")
        assert any("get_toplevel(" in line for line in studio.lines()), "Studio has no xdg_toplevel trace"
        mark = len(studio.lines())
        foot_offset = foot_log.stat().st_size
        command(studio.env, f"[con_id={other['id']}] focus")
        gui.eventually(lambda: re.search(r"wl_keyboard[#@]\d+\.leave\(", "\n".join(studio.lines()[mark:])),
                       "Studio wl_keyboard.leave")
        gui.eventually(lambda: re.search(r"wl_keyboard[#@]\d+\.enter\(",
                                         foot_log.read_text(errors="replace")[foot_offset:]),
                       "second client's wl_keyboard.enter")

        def focused_elsewhere():
            display_tree = sway(studio.env, "-t", "get_tree")
            nodes = {node["id"]: node for node in tree_nodes(display_tree)}
            primary, secondary = nodes[studio.window["id"]], nodes[other["id"]]
            assert all(node["visible"] and node["shell"] == "xdg_shell" for node in (primary, secondary))
            assert not primary["focused"] and secondary["focused"]
            rect, peer = primary["rect"], secondary["rect"]
            assert rect["x"] + rect["width"] <= peer["x"], (rect, peer)
            return rect, display_tree

        # Let the platform redraw from activation settle before injecting chat.
        time.sleep(0.4)
        rect, before_tree = focused_elsewhere()
        store(self.evidence / "unfocused-tree-before.json", before_tree)
        before = self.screenshot(studio.env, "unfocused-before")
        mark = len(studio.lines())
        chat_at = len(relay.times("chat"))
        preview_at = len(relay.times("preview-small"))
        relay.chat_count = 129
        try:
            studio.wait("active=false | chat=128", after=mark, seconds=6)
        except AssertionError:
            assert studio.process.poll() is None, studio.lines()[-10:]
        else:
            gui.eventually(lambda: len(relay.times("preview-small")) > preview_at,
                           "preview while Studio is visible and unfocused", seconds=3)
        _, after_tree = focused_elsewhere()
        store(self.evidence / "unfocused-tree-after.json", after_tree)
        after = self.screenshot(studio.env, "unfocused-after")
        _, hide_chat_y = studio.point("Hide chat")
        chat_box = (rect["x"] + 540, hide_chat_y + 43,
                    rect["x"] + rect["width"] - 18, rect["y"] + 475)
        assert chat_box[0] < chat_box[2] and chat_box[1] < chat_box[3], chat_box
        chat_before, chat_after = before.crop(chat_box), after.crop(chat_box)
        chat_before.save(self.evidence / "chat-before.png")
        chat_after.save(self.evidence / "chat-after.png")
        changed = ImageChops.difference(chat_before, chat_after).getbbox()
        deltas = len(relay.times("chat")) - chat_at
        requests = len(relay.times("preview-small")) - preview_at
        rendered = [line for line in studio.lines()[mark:]
                    if line.startswith("studio-render Running ") and "active=false | chat=128" in line]
        passed = bool(changed and deltas and rendered and requests)
        store(self.evidence / "focus.json",
              dict(passed=passed, injected_cursor=129, chat_requests=deltas, preview_requests=requests,
                   chat_pixels_changed=changed, unfocused_render_lines=rendered))
        print(f"Unfocused native chat {'PASS' if passed else 'FAIL'}: chat={deltas} preview={requests},"
              f" changed chat pixels={changed}, unfocused redraws={len(rendered)}", flush=True)
        if passed:
            _, _, redraws = self.sample(studio, "loaded-unfocused")
            assert redraws == 0, "unchanged preview or chat redrew in the unfocused sample"
            focused_elsewhere()
        gaps = [b - a for a, b in zip(relay.times("preview-small"), relay.times("preview-small")[1:])]
        assert gaps and min(gaps) >= 1, "preview exceeded 1 Hz"
        store(self.evidence / "preview-gaps.json", gaps)
        command(studio.env, f"[con_id={studio.window['id']}] focus")
        return passed


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--studio-binary", type=Path)
    parser.add_argument("--studio-source", help="Full Git SHA used to build --studio-binary")
    args = parser.parse_args()
    if bool(args.studio_binary) != bool(args.studio_source):
        parser.error("--studio-binary and --studio-source must be supplied together")
    if os.geteuid() == 0:
        parser.error("the private compositor must run unprivileged")
    os.umask(0o077)
    evidence = args.evidence.resolve()
    evidence.mkdir(mode=0o700)
    head = output("git", "rev-parse", "HEAD")
    binary_source = args.studio_source or head
    assert output("git", "rev-parse", f"{binary_source}^{{commit}}") == binary_source
    source = dict(command=shlex.join(["python3", *sys.argv]), checkout=head,
                  tree=output("git", "rev-parse", "HEAD^{tree}"),
                  dirty=output("git", "status", "--porcelain=v1"),
                  studio_source=binary_source, kernel=os.uname().release,
                  sway=output("sway", "--version"), foot=output("foot", "--version"),
                  rustc=output("rustc", "--version"), cargo=output("cargo", "--version"),
                  locks={path: digest(gui.ROOT / path) for path in
                         ("Cargo.lock", "packages/splitcast-studio/Cargo.lock", "flake.lock")})
    store(evidence / "source.json", source)
    binary = args.studio_binary or Path(gui.build(
        ["build", "--release", "--manifest-path", str(gui.MANIFEST)], "splitcast-studio"
    ))
    binary = binary.resolve()
    wrapped = binary.with_name(".splitcast-studio-wrapped")
    source["binary"] = str(binary)
    source["sha256"] = {path.name: digest(path) for path in (binary, wrapped) if path.is_file()}
    store(evidence / "source.json", source)
    test = Acceptance(evidence)
    gui.verify(str(binary), display=test.display, studio_type=test.studio,
               idle=test.idle, unfocused=test.unfocused)


if __name__ == "__main__":
    main()
