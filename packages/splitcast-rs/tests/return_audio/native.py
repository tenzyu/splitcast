"""Private, policy-only PipeWire graph and local OpenSSH forwarding for two Agents."""
import contextlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

GRAPH = """
context.properties = { core.daemon = true core.name = return-test }
context.spa-libs = {
    audio.convert.* = audioconvert/libspa-audioconvert
    support.* = support/libspa-support
}
context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-metadata }
    { name = libpipewire-module-spa-node-factory }
    { name = libpipewire-module-client-node }
    { name = libpipewire-module-access }
    { name = libpipewire-module-adapter }
    { name = libpipewire-module-link-factory }
    { name = libpipewire-module-session-manager }
]
context.objects = [
    { factory = spa-node-factory args = { factory.name = support.node.driver node.name = Dummy-Driver priority.driver = 20000 } }
    { factory = adapter args = { factory.name = support.null-audio-sink node.name = return.selected media.class = Audio/Sink audio.position = [ FL FR ] priority.session = 100 } }
    { factory = adapter args = { factory.name = support.null-audio-sink node.name = return.vanish media.class = Audio/Sink audio.position = [ FL FR ] priority.session = 50 } }
    { factory = adapter args = { factory.name = support.null-audio-sink node.name = game.local media.class = Audio/Sink audio.position = [ FL FR ] priority.session = 2000 } }
]
"""


def until(check):
    end = time.monotonic() + 6
    while time.monotonic() < end:
        if check():
            return
        time.sleep(0.05)
    raise RuntimeError("private graph did not become ready")


@contextlib.contextmanager
def process(argv, env, **kw):
    with subprocess.Popen(argv, env=env, **kw) as child:
        try:
            yield child
        finally:
            child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()


def ssh_config(root):
    for name in ("client", "host"):
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f",
                        root / name], check=True)
    (root / "hosts").write_text("production-host " + (root / "host.pub").read_text())
    (root / "sshd.conf").write_text(f"""HostKey "{root}/host"
AuthorizedKeysFile "{root}/client.pub"
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM no
StrictModes no
MaxSessions 0
AllowTcpForwarding local
AllowAgentForwarding no
X11Forwarding no
""")
    server = shlex.join([shutil.which("sshd"), "-i", "-f", str(root / "sshd.conf")])
    (root / "ssh.conf").write_text(f"""Host production-host
 HostKeyAlias production-host
 ProxyCommand {server}
 IdentityFile "{root}/client"
 IdentityAgent none
 IdentitiesOnly yes
 UserKnownHostsFile "{root}/hosts"
 GlobalKnownHostsFile /dev/null
 StrictHostKeyChecking yes
""")


def main(root, test):
    env = dict(PATH=os.environ["PATH"], HOME=str(root), XDG_RUNTIME_DIR=str(root),
               XDG_STATE_HOME=str(root), XDG_CONFIG_HOME=str(root), XDG_CONFIG_DIRS=str(root),
               PIPEWIRE_RUNTIME_DIR=str(root), PIPEWIRE_REMOTE="return-test",
               DBUS_SESSION_BUS_ADDRESS=f"unix:path={root}/absent-bus",
               DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={root}/absent-bus",
               SPLITCAST_RETURN_LAB=str(root))
    ssh_config(root)
    graph = root / "graph.conf"
    graph.write_text(GRAPH)
    with contextlib.ExitStack() as stack:
        stack.enter_context(process(["pipewire", "-c", str(graph)], env))
        until(lambda: (root / "return-test").is_socket())
        # 'policy' loads linking policy without any hardware monitors.
        stack.enter_context(process(["wireplumber", "-p", "policy"], env))

        def objects():
            return json.loads(subprocess.check_output(["pw-dump"], env=env, timeout=3))

        def node_names():
            return {o["info"]["props"].get("node.name") for o in objects()
                    if o["type"] == "PipeWire:Interface:Node"}

        until(lambda: {"return.selected", "game.local"} <= node_names())
        vanishing = next(o["id"] for o in objects() if o["type"] == "PipeWire:Interface:Node"
                         and o["info"]["props"].get("node.name") == "return.vanish")
        (root / "vanishing-id").write_text(str(vanishing))
        sinks = {}
        for target in ["return.selected", "game.local", "return.vanish"]:
            sinks[target] = root / (target + ".pcm")
            output = stack.enter_context(sinks[target].open("wb"))
            stack.enter_context(process(["pw-record", "--raw", "--format=s16", "--rate=48000",
                                         "--channels=2", "--target=" + target, "--properties",
                                         "{ stream.capture.sink = true node.dont-fallback = true node.dont-reconnect = true }", "-"], env, stdout=output))
        until(lambda: all(f.stat().st_size > 4096 for f in sinks.values()))
        subprocess.run([test, "--ignored", "--exact", "return_tests::native_session", "--nocapture"],
                       check=True, env=env, timeout=100)
        time.sleep(0.2)
    selected, local, vanishing = (f.read_bytes() for f in sinks.values())
    for marker in ("first", "recovered"):
        # Match an interior block at unity gain independently of graph latency/padding.
        sample = (root / marker).read_bytes()[8192:12288]
        assert len(sample) == 4096 and selected.count(sample) == 1 and sample not in local, marker
    removed = (root / "removed").read_bytes()
    assert removed[8192:12288] in vanishing
    assert all(removed[i:i+4096] not in local for i in range(0, len(removed)-4096, 8192))
    missing = (root / "missing").read_bytes()[8192:12288]
    assert len(missing) == 4096 and missing not in selected and missing not in local
    game = (root / "local-marker").read_bytes()[8192:12288]
    assert len(game) == 4096 and game in local and game not in selected
    print("Independent null-sink monitors: first and recovered PCM only on requested sink; "
          "local marker only on game sink after transport loss.", flush=True)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="sc-return-") as directory:
        main(Path(directory), sys.argv[1])
