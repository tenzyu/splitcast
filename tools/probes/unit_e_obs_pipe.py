"""Unit E startup gate for the pinned SplitCast OBS package (Linux only)."""

import array
import contextlib
import fcntl
from itertools import groupby
import json
import os
from pathlib import Path
import secrets
import select
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import time


MIC_NAME = "MicProcessed"
PULSE_SINK = "splitcast_unit_e"
DECOY_HZ = 2710
START_SECONDS = 30
TIMING_MARKS = ((20, 900, 0.35), (30, 1300, 0.60), (40, 1700, 0.85))
TIMING_WINDOW = 480


def report(event, **fields):
    print(json.dumps({"event": event, **fields}, sort_keys=True), flush=True)


def check(condition, detail):
    if not condition:
        raise RuntimeError(detail)


def start(run, name, argv, **kwargs):
    report("spawn", name=name, argv=[os.fspath(arg) for arg in argv])
    return run.spawn(name, argv, **kwargs)


def listen_rows():
    return [
        fields[1]
        for name in ("tcp", "tcp6")
        for line in Path("/proc/net", name).read_text().splitlines()[1:]
        if (fields := line.split())[3] == "0A"
    ]


def request(client, kind, params=None):
    client.sequence += 1
    request_id = str(client.sequence)
    message = {"requestType": kind, "requestId": request_id}
    if params is not None:
        message["requestData"] = params
    client.send(6, message)
    data = client.receive(7)
    check(data["requestId"] == request_id and data["requestType"] == kind,
          f"mismatched reply to {kind}")
    return data["requestStatus"], data.get("responseData", {})


def rpc(client, kind, params=None):
    status, result = request(client, kind, params)
    report("obs-request", request=kind, data=params, status=status, result=result)
    check(status["result"], f"OBS rejected {kind}: {status}")
    return result


def until(label, observe, seconds=START_SECONDS):
    started = time.monotonic()
    while time.monotonic() - started < seconds:
        value = observe()
        if value:
            report("ready", condition=label, elapsed_ms=round(1000 * (time.monotonic() - started), 3))
            return value
        time.sleep(0.02)
    raise RuntimeError(f"{label} timed out after {seconds}s")


def connect_to_scene(child, port, password):
    from media_acceptance import ObsStats

    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        check(child.poll() is None, f"OBS exited before scene readiness: {child.returncode}")
        try:
            client = ObsStats(port, password)
        except (OSError, TimeoutError):
            time.sleep(0.05)
            continue
        try:
            status, ready = request(client, "GetSceneList")
        except BaseException:
            client.socket.close()
            raise
        if status["result"] and any(scene["sceneName"] == "SplitCast" for scene in ready["scenes"]):
            return client
        report("scene-not-ready", status=status)
        client.socket.close()
        time.sleep(0.05)
    raise RuntimeError("OBS did not load the SplitCast scene")


def display(run):
    Path("/tmp/.X11-unix").mkdir(exist_ok=True)
    auth = run.directory / "xauthority"
    fields = (b"", b"", b"MIT-MAGIC-COOKIE-1", secrets.token_bytes(16))
    auth.write_bytes(struct.pack(">H", 65535) + b"".join(struct.pack(">H", len(v)) + v for v in fields))
    chosen = run.directory / "display-number"
    with chosen.open("wb") as ready:
        fd = ready.fileno()
        child = start(run, "display", ["Xvfb", "-displayfd", str(fd), "-screen", "0", "1280x720x24",
                                       "-xkbdir", os.environ["SPLITCAST_XKB_ROOT"],
                                       "-nolisten", "tcp", "-noreset", "-auth", auth], pass_fds=(fd,))
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        check(child.poll() is None, "Xvfb exited before allocating a display")
        value = chosen.read_text()
        if "\n" in value:
            number = value.splitlines()[0]
            check(number.isdigit(), "Xvfb display allocation failed")
            run.env.update(DISPLAY=":" + number, XAUTHORITY=str(auth))
            return
        time.sleep(0.02)
    raise RuntimeError("Xvfb did not allocate a display")


def profile(directory, port):
    # Software profile and blank scene from obs.rs::launch_args, with no
    # DesktopAudioDevice or AuxAudioDevice entries in the scene collection.
    studio = directory / "config-home/obs-studio"
    plugin = studio / "plugin_config/obs-websocket"
    scenes = studio / "basic/scenes"
    ini = studio / "basic/profiles/SplitCast"
    recordings = directory / "recordings"
    for path in (plugin, scenes, ini, recordings):
        path.mkdir(parents=True)
    password = secrets.token_hex(24)
    (plugin / "config.json").write_text(json.dumps({
        "server_enabled": True, "server_port": port, "auth_required": True,
        "server_password": password, "first_load": False,
    }))
    (studio / "user.ini").write_text(
        "[Basic]\nProfile=SplitCast\nProfileDir=SplitCast\nSceneCollection=SplitCast\n"
        "SceneCollectionFile=SplitCast\n[General]\nFirstRun=true\n"
    )
    (scenes / "SplitCast.json").write_text(json.dumps({
        "name": "SplitCast", "current_scene": "SplitCast", "current_program_scene": "SplitCast",
        "scene_order": [{"name": "SplitCast"}], "groups": [],
        "sources": [{"name": "SplitCast", "id": "scene", "versioned_id": "scene",
                     "settings": {"items": []}, "mixers": 0}],
        "transitions": [],
    }))
    (ini / "basic.ini").write_text(
        "[General]\nName=SplitCast\n[Video]\nBaseCX=1280\nBaseCY=720\nOutputCX=1280\n"
        "OutputCY=720\nFPSType=0\nFPSCommon=30\n[Output]\nMode=Advanced\nReconnect=false\n"
        "[AdvOut]\nEncoder=obs_x264\nRecType=Standard\n"
        f"RecFilePath={recordings}\nRecFormat2=mkv\nRecEncoder=obs_x264\n"
        "TrackIndex=1\nRecTracks=7\nRescale=false\n[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n"
    )
    (ini / "streamEncoder.json").write_text(
        json.dumps({"rate_control": "CBR", "bitrate": 5000, "keyint_sec": 2})
    )
    return password


def observe_obs(child, expected, pipe):
    from media_acceptance import digest

    proc = Path("/proc", str(child.pid))
    exe = Path(os.readlink(proc / "exe"))
    check(exe == expected, f"unexpected running OBS binary: {exe}")
    mappings = set()
    for line in (proc / "maps").read_text().splitlines():
        path = line.split(maxsplit=5)[-1]
        if path.startswith("/") and Path(path).name.startswith(
            ("libobs.so.", "obs-ffmpeg.so", "libavformat.so.", "obs-websocket.so")
        ):
            mappings.add(path)
    libraries = {path: digest(path) for path in sorted(mappings)}
    fd = proc / "fd/0"
    stat = fd.stat()
    same_pipe = pipe is not None and (stat.st_dev, stat.st_ino) == pipe
    report("scene-process", pid=child.pid, exe=str(exe), sha256=digest(exe),
           mapped_libraries=libraries, fd0=os.readlink(fd), same_pipe=same_pipe,
           listeners=listen_rows())
    return same_pipe


def media_receiver(run):
    rtmp, rtsp, api = run.ports[2:5]
    path = run.directory / "mediamtx.json"
    path.write_text(json.dumps({
        "logLevel": "info", "readTimeout": "5s", "writeTimeout": "5s", "writeQueueSize": 128,
        "rtsp": True, "rtspAddress": f"127.0.0.1:{rtsp}", "rtspTransports": ["tcp"],
        "rtmp": True, "rtmpAddress": f"127.0.0.1:{rtmp}",
        "api": True, "apiAddress": f"127.0.0.1:{api}", "hls": False, "webrtc": False, "srt": False,
        "metrics": False, "pprof": False, "playback": False,
        "paths": {name: {"source": "publisher"} for name in ("game", "mic")},
    }))
    child = start(run, "receiver", ["mediamtx", path])

    def ready():
        check(child.poll() is None, "MediaMTX exited during startup")
        try:
            return run.transport()
        except OSError:
            return None

    until("receiver API", ready, 10)


class TimingSource:
    """Generate all three inputs from the same finite FFmpeg timeline."""

    def __init__(self, run):
        game = "0.08*sin(2*PI*440*t)+" + "+".join(
            f"0.30*between(t,{onset},{onset + 0.45})*sin(2*PI*{hz}*t)"
            for onset, hz, _ in TIMING_MARKS
        )
        envelope = "+".join(f"between(t,{onset},{onset + width})" for onset, _, width in TIMING_MARKS)
        mic = f"({envelope})*0.15*(sin(2*PI*180*t)+0.5*sin(2*PI*360*t)+0.3*sin(2*PI*540*t))"
        source = f"color=c=gray:size=1280x720:rate=30:duration=65[out0];aevalsrc='{game}|{mic}':s=48000:d=65[out1]"
        self.child = start(run, "synchronized-publisher", [
            "ffmpeg", "-v", "error", "-nostdin", "-re", "-f", "lavfi", "-i", source,
            "-filter_complex_threads", "1", "-filter_complex",
            "[0:a]asplit=2[g][m];[g]pan=mono|c0=FL[game];[m]pan=mono|c0=FR[mic]",
            "-map", "0:v:0", "-map", "[game]", "-c:v", "libx264", "-threads", "2",
            "-preset", "ultrafast", "-tune", "zerolatency", "-g", "30", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-ar", "48000", "-f", "flv", f"rtmp://127.0.0.1:{run.ports[2]}/game",
            "-map", "[mic]", "-c:a", "libopus", "-ar", "48000",
            "-f", "rtsp", "-rtsp_transport", "tcp", f"rtsp://127.0.0.1:{run.ports[3]}/mic",
        ])

    def observe(self, phase):
        check(self.child.poll() is None, f"synchronized publisher exited during {phase}: {self.child.returncode}")


def pulse_server(run, name, native, sink):
    directory = run.directory / name
    runtime = directory / "runtime"
    runtime.mkdir(parents=True)
    native.parent.mkdir(exist_ok=True)
    cookie = directory / "cookie"
    cookie.write_bytes(secrets.token_bytes(256))
    script = directory / "server.pa"
    script.write_text(
        f'load-module module-native-protocol-unix socket="{native}" auth-anonymous=0 auth-cookie="{cookie}"\n'
        f"load-module module-null-sink sink_name={sink} rate=48000 channels=1\n"
        f"set-default-sink {sink}\nset-default-source {sink}.monitor\n"
    )
    child = start(run, name, [
        "env", f"PULSE_RUNTIME_PATH={runtime}", f"PULSE_STATE_PATH={directory}",
        "pulseaudio", "-n", "--daemonize=no", "--use-pid-file=no", "--exit-idle-time=-1",
        "--realtime=no", "--high-priority=no", "--disable-shm=yes",
        "--log-target=stderr", f"--file={script}",
    ])
    client = ["env", f"PULSE_SERVER=unix:{native}", f"PULSE_COOKIE={cookie}"]
    until(f"{name} socket", lambda: child.poll() is None and native.is_socket(), 5)
    sinks = json.loads(run.tool([*client, "pactl", "-f", "json", "list", "sinks"]))
    check([entry["name"] for entry in sinks] == [sink], f"unexpected sink inventory for {name}")
    return client


def write_pcm(run, pulse, pipe_write):
    plugin = os.environ["SPLITCAST_RNNOISE_PLUGIN"]
    filters = (
        f"aresample=48000:async=1000,aformat=channel_layouts=mono,ladspa=file={plugin}:"
        "plugin=noise_suppressor_mono:controls=0|200|0|0|0,highpass=f=80,volume=0dB,alimiter=limit=0.95"
    )
    ffmpeg = [
        "ffmpeg", "-hide_banner", "-nostdin", "-loglevel", "error", "-rtsp_transport", "tcp",
        "-i", f"rtsp://127.0.0.1:{run.ports[3]}/mic", "-vn", "-af", filters, "-ar", "48000", "-ac", "1",
    ]
    if pipe_write is None:
        label = "SplitCast processed microphone"
        return start(run, "rnnoise", [*pulse, *ffmpeg, "-f", "pulse", "-buffer_duration", "100",
                                      "-prebuf", "1920", "-stream_name", label, "-device", PULSE_SINK, label])
    # The fixture uses Run.spawn for other children. Its stdout is a log, so
    # the pipe writer has an explicitly separate stdout and stderr here.
    argv = [*ffmpeg, "-c:a", "pcm_s16le", "-f", "s16le", "pipe:1"]
    with (run.directory / "rnnoise.log").open("wb") as log:
        report("spawn", name="rnnoise", argv=argv, stdout=os.readlink(f"/proc/self/fd/{pipe_write}"))
        child = subprocess.Popen(argv, env=run.env, stdin=subprocess.DEVNULL, stdout=pipe_write,
                                 stderr=log, start_new_session=True)
    run.children.append(("rnnoise", child))
    return child


def queued_bytes(fd, writer):
    poll = select.poll()
    poll.register(fd, select.POLLIN)
    events = poll.poll(0)
    check(writer.poll() is None and not any(flags & (select.POLLHUP | select.POLLERR | select.POLLNVAL)
                                            for _, flags in events), "PCM writer exited or pipe disconnected")
    count = array.array("i", [0])
    fcntl.ioctl(fd, termios.FIONREAD, count, True)
    return count[0]


def attach_inputs(client, run, writer, read_fd, pulse):
    if read_fd is None:
        def pulse_ready():
            check(writer.poll() is None, "Pulse PCM writer exited during startup")
            inputs = json.loads(run.tool([*pulse, "pactl", "-f", "json", "list", "sink-inputs"]))
            return len(inputs) == 1 and not inputs[0]["corked"] and (
                inputs[0]["properties"]["media.name"] == "SplitCast processed microphone"
            )
        until("private Pulse writer uncorked", pulse_ready)
        mic_kind = "pulse_input_capture"
        settings = {"device_id": f"{PULSE_SINK}.monitor"}
    else:
        until("first queued PCM", lambda: queued_bytes(read_fd, writer))
        mic_kind = "ffmpeg_source"
        settings = {
            "is_local_file": False, "input": "pipe:0", "input_format": "s16le",
            "ffmpeg_options": "sample_rate=48000 ch_layout=mono", "hw_decode": False,
            "restart_on_activate": False, "close_when_inactive": False, "seekable": False,
        }
    rpc(client, "CreateInput", {
        "sceneName": "SplitCast", "inputName": "GameFeed", "inputKind": "ffmpeg_source",
        "inputSettings": {
            "is_local_file": False, "input": f"rtsp://127.0.0.1:{run.ports[3]}/game",
            "input_format": "rtsp", "ffmpeg_options": "rtsp_transport=tcp",
            "hw_decode": False, "buffering_mb": 1,
        }, "sceneItemEnabled": True,
    })
    before = None if read_fd is None else queued_bytes(read_fd, writer)
    if before is not None:
        check(before > 0, "queued PCM vanished before attaching MicProcessed")
        report("before-attach", queued_bytes=before, writer_pid=writer.pid)
    rpc(client, "CreateInput", {
        "sceneName": "SplitCast", "inputName": MIC_NAME, "inputKind": mic_kind,
        "inputSettings": settings, "sceneItemEnabled": True,
    })
    if before is not None:
        observed = [before, before, 0]
        states = set()

        def playing_and_drained():
            count = queued_bytes(read_fd, writer)
            observed[0] = min(observed[0], count)
            observed[1] = max(observed[1], count)
            observed[2] += 1
            status, response = request(client, "GetMediaInputStatus", {"inputName": MIC_NAME})
            check(status["result"], f"OBS media status failed: {status}")
            state = response["mediaState"]
            states.add(state)
            return observed[0] < before and state == "OBS_MEDIA_STATE_PLAYING"

        try:
            until("packaged OBS reading the PCM pipe and playing", playing_and_drained)
        finally:
            report("drain-observation", before_bytes=before, minimum_bytes=observed[0],
                   maximum_bytes=observed[1], samples=observed[2], media_states=sorted(states),
                   writer_exit=writer.poll())
    for name, game in (("GameFeed", True), (MIC_NAME, False)):
        rpc(client, "SetInputAudioTracks", {
            "inputName": name, "inputAudioTracks": {"1": True, "2": game, "3": not game},
        })
    rpc(client, "SetInputAudioSyncOffset", {"inputName": MIC_NAME, "inputAudioSyncOffset": 0})
    rpc(client, "SetInputAudioMonitorType", {
        "inputName": MIC_NAME, "monitorType": "OBS_MONITORING_TYPE_NONE",
    })
    rpc(client, "SetCurrentProgramScene", {"sceneName": "SplitCast"})


def decode_recording(run, path):
    from media_acceptance import analyze_audio, digest, tone_power, video_cadence

    probe = json.loads(run.tool(["ffprobe", "-v", "error", "-count_packets", "-show_streams",
                                "-show_format", "-of", "json", path]))
    (run.directory / "ffprobe.json").write_text(json.dumps(probe, indent=2))
    streams = probe["streams"]
    check([s["codec_name"] for s in streams] == ["h264", "aac", "aac", "aac"], "wrong output codecs")
    check(all(int(s["nb_read_packets"]) > 100 for s in streams), "incomplete recording streams")
    run.tool(["ffmpeg", "-v", "error", "-xerror", "-i", path, "-map", "0", "-f", "null", "-"])
    frames = json.loads(run.tool([
        "ffprobe", "-v", "error", "-select_streams", "v:0", "-show_frames", "-show_entries",
        "frame=best_effort_timestamp_time", "-of", "json", path,
    ]))["frames"]
    cadence = video_cadence([float(frame["best_effort_timestamp_time"]) for frame in frames])
    tracks = []
    for index, name in enumerate(("mix", "game", "mic")):
        pcm = array.array("h", run.tool(["ffmpeg", "-v", "error", "-xerror", "-i", path, "-map",
                                        f"0:a:{index}", "-ac", "1", "-ar", "48000", "-f", "s16le", "pipe:1"]))
        windows, coverage = analyze_audio(pcm, 0)
        stats = {"track": name, **coverage}
        for label in ("game", "mic", "decoy"):
            count, absence, longest = 0, 0, 0
            for index, window in enumerate(windows):
                power = (tone_power(pcm[index * 4800:(index + 1) * 4800], DECOY_HZ)
                         if label == "decoy" else window[2][label])
                present = power > max(100, window[2]["noise"] * 10)
                absence = 0 if present else absence + 1
                count += present
                longest = max(longest, absence)
            stats[label] = {"present": count, "max_absent": longest}
        tracks.append(stats)
    report("decoded", sha256=digest(path), duration=probe["format"]["duration"],
           packets=[int(s["nb_read_packets"]) for s in streams], cadence=cadence, tracks=tracks)
    for stats in tracks:
        check(stats["complete_windows"] >= 90, f"{stats['track']} track too short")
        for label in ("game", "mic", "decoy"):
            # The broad game-frequency search also classifies mic energy near
            # the 540 Hz harmonic. Keep its mic-track count as a diagnostic.
            if label == "game" and stats["track"] == "mic":
                continue
            expected = label != "decoy" and stats["track"] in ("mix", label)
            counts = stats[label]
            check((counts["max_absent"] <= 3 and counts["present"] >= 0.8 * stats["complete_windows"])
                  if expected else counts["present"] == 0,
                  f"{stats['track']} track {label} coverage failed: {counts}")


def decode_timing(run, path):
    from media_acceptance import audio_clock, digest, tone_power

    run.tool(["ffmpeg", "-v", "error", "-xerror", "-i", path, "-map", "0", "-f", "null", "-"], timeout=60)
    results = []
    for index in (1, 2):
        frames = json.loads(run.tool([
            "ffprobe", "-v", "error", "-select_streams", f"a:{index}", "-show_frames",
            "-show_entries", "frame=best_effort_timestamp_time,nb_samples", "-of", "json", path,
        ]))["frames"]
        clock = audio_clock(frames)
        pcm = array.array("h", run.tool([
            "ffmpeg", "-v", "error", "-xerror", "-i", path, "-map", f"0:a:{index}",
            "-ac", "1", "-ar", "48000", "-f", "s16le", "pipe:1",
        ]))
        check(len(pcm) == clock["samples"], "timing track PCM and timestamp sample counts differ")
        def read_bursts(hz):
            powers = [tone_power(pcm[i:i + TIMING_WINDOW], hz)
                      for i in range(0, len(pcm) - TIMING_WINDOW + 1, TIMING_WINDOW)]
            peak = max(powers)
            check(peak > 1e5, f"missing timing marker {hz} Hz; peak={peak}")
            active = (i for i, power in enumerate(powers) if power > peak * 0.10)
            runs = [[pair[1] for pair in group]
                    for _, group in groupby(enumerate(active), key=lambda pair: pair[1] - pair[0])]
            return [
                {"hz": hz, "onset_seconds": clock["origin_seconds"] + burst[0] * TIMING_WINDOW / 48000,
                 "seconds": len(burst) * TIMING_WINDOW / 48000, "peak": peak}
                for burst in runs if len(burst) >= 26
            ]

        marks = []
        if index == 1:
            for _, hz, _ in TIMING_MARKS:
                bursts = read_bursts(hz)
                check(len(bursts) == 1 and abs(bursts[0]["seconds"] - 0.45) <= 0.08,
                      f"game timing burst {hz} Hz is ambiguous: {bursts}")
                marks.extend(bursts)
        else:
            # The existing voiced mic identity survives RNNoise. Give each
            # burst a distinct duration and calibrate it against its own PTS.
            bursts = read_bursts(180)
            check(len(bursts) == len(TIMING_MARKS), f"mic timing burst count differs: {bursts}")
            for _, _, width in TIMING_MARKS:
                matches = [burst for burst in bursts if abs(burst["seconds"] - width) <= 0.08]
                check(len(matches) == 1, f"mic timing burst {width}s is ambiguous: {bursts}")
                marks.extend(matches)
        results.append({"track": index, "clock": clock, "markers": marks})
    game, mic = results
    # Each 10 ms window brackets a sampled onset. Allow one adjacent window
    # for threshold placement per track and include measured PTS quantization.
    uncertainty = 2 * TIMING_WINDOW / 48000 + sum(
        track["clock"]["max_pts_error_seconds"] for track in results
    )
    measurements = [
        {"source_seconds": mark[0], "offset_seconds": m["onset_seconds"] - g["onset_seconds"],
         "uncertainty_seconds": uncertainty}
        for mark, g, m in zip(TIMING_MARKS, game["markers"], mic["markers"])
    ]
    report("timing", sha256=digest(path), tracks=results, markers=measurements, sync_seconds=0)
    (Path("/evidence") / "timing.json").write_text(json.dumps(measurements, indent=2) + "\n")


def recording(client, run, source, writer, timing):
    def game_geometry():
        item = rpc(client, "GetSceneItemId", {"sceneName": "SplitCast", "sourceName": "GameFeed"})
        status, result = request(client, "GetSceneItemTransform", {
            "sceneName": "SplitCast", "sceneItemId": item["sceneItemId"],
        })
        check(status["result"], "OBS scene geometry rejected")
        transform = result["sceneItemTransform"]
        return transform["sourceWidth"] == 1280 and transform["sourceHeight"] == 720

    until("GameFeed decoded geometry", game_geometry)
    rpc(client, "StartRecord")
    until("recording active", lambda: rpc(client, "GetRecordStatus")["outputActive"], 5)
    started = time.monotonic()
    sample_times = range(0, 46, 3) if timing else (0, 2, 5, 8, 10)
    for second in sample_times:
        time.sleep(max(0, started + second - time.monotonic()))
        source.observe(f"recording second {second}")
        check(writer.poll() is None, "RNNoise exited during recording")
        check(rpc(client, "GetRecordStatus")["outputActive"], "OBS recording became inactive")
        rpc(client, "GetStats")
    stopped = rpc(client, "StopRecord")
    until("recording inactive after StopRecord",
          lambda: not rpc(client, "GetRecordStatus")["outputActive"], 12)
    return Path(stopped["outputPath"])


def cleanup(run, port):
    released = True
    for name, child in reversed(run.children):
        forced = False
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGINT if name == "obs" else signal.SIGTERM)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                forced = True
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
        try:
            def group_gone():
                try:
                    os.killpg(child.pid, 0)
                except ProcessLookupError:
                    return True
                return False
            until(f"{name} group reaped", group_gone, 2)
        except RuntimeError:
            gone = False
        else:
            gone = True
        released &= gone and not forced
        report("cleanup", name=name, pid=child.pid, exit=child.returncode,
               group_gone=gone, forced=forced)
    with socket.socket() as conn:
        conn.settimeout(0.2)
        listening = conn.connect_ex(("127.0.0.1", port)) == 0
    report("after-cleanup", listeners=listen_rows(), obs_port_open=listening)
    check(released and not listening and not listen_rows(), "owned group or listener cleanup failed")


def inner(arm, timing):
    from media_acceptance import Run, Source, digest, free_ports

    interfaces = {line.split(":")[0].strip()
                  for line in Path("/proc/net/dev").read_text().splitlines()[2:]}
    devices = sorted(path.name for path in Path("/dev").iterdir())
    report("sandbox", arm=arm, interfaces=sorted(interfaces), devices=devices,
           pid_one=Path("/proc/1/comm").read_text().strip())
    check(Path("/proc/1/comm").read_text().strip() == "bwrap"
          and interfaces == {"lo"} and not any(name.startswith(("snd", "dri", "video", "media"))
                                               for name in devices),
          "device-hidden private Bubblewrap namespace required")
    directory = Path(tempfile.mkdtemp(prefix="unit-e-", dir="/tmp"))
    run = Run(directory, None)
    runtime = directory / "runtime"
    isolated_pipewire = directory / "isolated-pipewire"
    isolated_pipewire.mkdir()
    run.env.update(PIPEWIRE_RUNTIME_DIR=str(isolated_pipewire), PIPEWIRE_REMOTE="no-pipewire")
    run.ports = free_ports(6)
    port = run.ports[5]
    password = profile(directory, port)
    wrapper = Path(shutil.which("obs")).resolve()
    expected = wrapper.with_name(".obs-wrapped")
    trace = directory / "obs-connects.log"
    trace_tool = os.environ["SPLITCAST_UNIT_E_STRACE"]
    ffmpeg = Path(shutil.which("ffmpeg")).resolve()
    report("arm-tools", obs_wrapper=str(wrapper), obs_wrapper_sha256=digest(wrapper),
           trace_tool=trace_tool, trace_sha256=digest(trace_tool), ffmpeg=str(ffmpeg),
           ffmpeg_sha256=digest(ffmpeg), rnnoise=os.environ["SPLITCAST_RNNOISE_PLUGIN"],
           rnnoise_sha256=digest(os.environ["SPLITCAST_RNNOISE_PLUGIN"]))
    decoy_pulse = runtime / "pulse/native"
    decoy_pw = runtime / "pipewire-0"
    source = None
    try:
        display(run)
        decoy = pulse_server(run, "decoy-pulse", decoy_pulse, "splitcast_unit_e_decoy")
        start(run, "decoy-tone", [*decoy, "ffmpeg", "-nostdin", "-v", "error", "-re", "-f", "lavfi",
                                 "-i", f"sine=frequency={DECOY_HZ}:sample_rate=48000", "-t", "45",
                                 "-f", "pulse", "-device", "splitcast_unit_e_decoy", "Unit E decoy"])
        with contextlib.ExitStack() as close, socket.socket(socket.AF_UNIX) as decoy_socket:
            decoy_socket.bind(str(decoy_pw))
            decoy_socket.listen(8)
            pulse, pipe, read_fd, write_fd = [], None, None, None
            if arm == "pulse":
                pulse = pulse_server(run, "private-pulse", directory / "private-pulse/native.sock", PULSE_SINK)
            else:
                read_fd, write_fd = os.pipe()
                capacity = fcntl.fcntl(read_fd, fcntl.F_SETPIPE_SZ, 8192)
                check(0 < capacity <= 8192, f"unexpected PCM pipe capacity: {capacity}")
                pstat = os.fstat(read_fd)
                pipe = pstat.st_dev, pstat.st_ino
                report("pipe-created", capacity=capacity, inode=pstat.st_ino,
                       read_end=os.readlink(f"/proc/self/fd/{read_fd}"),
                       write_end=os.readlink(f"/proc/self/fd/{write_fd}"))
            close.callback(lambda: os.close(read_fd) if read_fd is not None else None)
            close.callback(lambda: os.close(write_fd) if write_fd is not None else None)
            media_receiver(run)
            source = TimingSource(run) if timing else Source(run, 1)

            def published():
                source.observe("publish readiness")
                paths = run.transport()["items"]
                return all(any(path["name"] == name and path["ready"] and path["bytesReceived"] > 0
                               for path in paths) for name in ("game", "mic"))

            until("game and microphone publishers", published, 10)
            child = start(run, "obs", [trace_tool, "-D", "-f", "-o", trace, "-e", "trace=connect,execve",
                                       *pulse, wrapper, "--multi", "--disable-shutdown-check",
                                       "--profile", "SplitCast", "--collection", "SplitCast"], stdin=read_fd)
            client = connect_to_scene(child, port, password)
            try:
                version = client.request("GetVersion")
                kinds = client.request("GetInputKindList")["inputKinds"]
                report("scene-ready", obs=version["obsVersion"], rpc=version["obsWebSocketVersion"],
                       ffmpeg_source="ffmpeg_source" in kinds,
                       pulse_input_capture="pulse_input_capture" in kinds)
                same_pipe = observe_obs(child, expected, pipe)
                check(arm == "pulse" or same_pipe, "packaged OBS lost the inherited PCM pipe on fd 0")
                check({"ffmpeg_source", "pulse_input_capture"}.issubset(kinds), "required OBS inputs unavailable")
                writer = write_pcm(run, pulse, write_fd)
                if write_fd is not None:
                    os.close(write_fd)
                    write_fd = None
                attach_inputs(client, run, writer, read_fd, pulse)
                if read_fd is not None:
                    os.close(read_fd)
                    read_fd = None
                artifact = recording(client, run, source, writer, timing)
            finally:
                client.socket.close()
    finally:
        try:
            try:
                if isinstance(source, Source):
                    source.expected_stops.update(("game", "mic"))
                    source.close()
            finally:
                cleanup(run, port)
        finally:
            for log in directory.glob("*.log"):
                shutil.copyfile(log, Path("/evidence", log.name))
            if trace.is_file():
                attempts = [line for line in trace.read_text().splitlines()
                            if "connect(" in line and (str(decoy_pulse) in line or str(decoy_pw) in line)]
                report("decoy-connections", count=len(attempts), connections=attempts)
                check(not attempts, "OBS attempted to connect to a decoy audio route")
            for artifact_file in (directory / "recordings").glob("*.mkv"):
                shutil.copyfile(artifact_file, Path("/evidence", artifact_file.name))
            (Path("/evidence") / "commands.json").write_text(json.dumps(run.commands, indent=2, default=str))
    try:
        if timing:
            decode_timing(run, artifact)
        else:
            decode_recording(run, artifact)
    finally:
        (Path("/evidence") / "commands.json").write_text(json.dumps(run.commands, indent=2, default=str))
        if (directory / "ffprobe.json").exists():
            shutil.copyfile(directory / "ffprobe.json", "/evidence/ffprobe.json")
    report("arm-pass", arm=arm)


def compare_timing(evidence):
    pulse, pipe = (json.loads((evidence / arm / "timing.json").read_text()) for arm in ("pulse", "pipe"))
    failed = []
    for baseline, candidate in zip(pulse, pipe):
        upper = abs(candidate["offset_seconds"]) + candidate["uncertainty_seconds"]
        lower = max(0, abs(candidate["offset_seconds"]) - candidate["uncertainty_seconds"])
        pulse_upper = abs(baseline["offset_seconds"]) + baseline["uncertainty_seconds"]
        pulse_lower = max(0, abs(baseline["offset_seconds"]) - baseline["uncertainty_seconds"])
        passed = upper <= 0.5 and upper - pulse_lower <= 0.1
        exceeded = lower > 0.5 or lower - pulse_upper > 0.1
        report("timing-comparison", candidate=candidate, baseline=baseline,
               absolute_error_bounds=[lower, upper], excess_bounds=[lower - pulse_upper, upper - pulse_lower],
               result="PASS" if passed else ("EXCEEDED" if exceeded else "INCONCLUSIVE"))
        if not passed:
            failed.append(candidate["source_seconds"])
    check(not failed, f"packaged OBS timing did not satisfy the matched limits at source markers {failed}")


def launch(timing):
    root = Path(__file__).resolve().parents[2]
    runs = root / "artifacts/unit-e-obs-pcm/runs"
    runs.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="attempt-", dir=runs))
    build = ["nix", "build", "--no-link", "--print-out-paths", "--no-write-lock-file", "--impure", "--expr",
             "(builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux.strace"]
    report("tool-build", argv=build)
    outputs = subprocess.check_output(build, text=True, timeout=60).splitlines()
    trace_tool, = (str(Path(output) / "bin/strace") for output in outputs
                   if (Path(output) / "bin/strace").is_file())
    # Synthetic passwd/group entries avoid inheriting host account metadata.
    (evidence / "passwd").write_text(
        f"splitcast-test:x:{os.getuid()}:{os.getgid()}:SplitCast fixture:/tmp:/bin/sh\n"
    )
    (evidence / "group").write_text(f"splitcast-test:x:{os.getgid()}:\n")
    store_path = ":".join(path for path in os.environ["PATH"].split(":")
                          if path.startswith("/nix/store/"))
    vars_to_keep = ("SPLITCAST_XKB_ROOT", "LIBGL_DRIVERS_PATH", "__EGL_VENDOR_LIBRARY_FILENAMES",
                    "SPLITCAST_RNNOISE_PLUGIN")
    args = [
        "bwrap", "--unshare-all", "--unshare-user", "--die-with-parent", "--new-session",
        "--hostname", "splitcast-fixture",
        "--ro-bind", "/nix/store", "/nix/store", "--dev", "/dev", "--proc", "/proc",
        "--tmpfs", "/tmp", "--dir", "/run", "--dir", "/etc", "--clearenv",
        "--ro-bind", str(Path(shutil.which("bash")).resolve()), "/bin/sh",
        "--ro-bind", str(evidence / "passwd"), "/etc/passwd",
        "--ro-bind", str(evidence / "group"), "/etc/group",
        "--ro-bind", str(Path(__file__).resolve()), "/probe/unit_e_obs_pipe.py",
        "--ro-bind", str(root / "tools/media_acceptance.py"), "/probe/media_acceptance.py",
        "--setenv", "PATH", store_path, "--setenv", "LANG", "C",
        "--setenv", "TMPDIR", "/tmp", "--chdir", "/probe",
    ]
    for name in vars_to_keep:
        args.extend(("--setenv", name, os.environ[name]))
    report("evidence-directory", path=str(evidence.relative_to(root)))
    for arm in ("pulse", "pipe"):
        output = evidence / arm
        output.mkdir()
        argv = [*args, "--bind", str(output), "/evidence", "--setenv", "SPLITCAST_UNIT_E_STRACE", trace_tool,
                "python3", "-B", "/probe/unit_e_obs_pipe.py", "--sandbox", arm, "timing" if timing else "short"]
        (output / "sandbox-argv.json").write_text(json.dumps(argv, indent=2) + "\n")
        with (output / "sandbox.log").open("wb") as log:
            result = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=log, stderr=log, timeout=80)
        (output / "sandbox.exit").write_text(str(result.returncode) + "\n")
        sys.stdout.buffer.write((output / "sandbox.log").read_bytes())
        sys.stdout.flush()
        if result.returncode:
            return result.returncode
    if timing:
        compare_timing(evidence)
    return 0


if __name__ == "__main__":
    os.umask(0o077)
    if (len(sys.argv) == 4 and sys.argv[1] == "--sandbox"
            and sys.argv[2] in ("pulse", "pipe") and sys.argv[3] in ("short", "timing")):
        inner(sys.argv[2], sys.argv[3] == "timing")
    elif sys.argv[1:] in ([], ["--timing"]):
        raise SystemExit(launch(sys.argv[1:] == ["--timing"]))
    else:
        raise SystemExit("Run from the pinned media-test devShell with optional --timing")
