#![cfg(target_os = "linux")]

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::Message;

const PASSWORD: &str = "virtual-obs-private-password";
const LIMIT: Duration = Duration::from_secs(30);
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Agent {
    child: Child,
    port: u16,
    token: String,
    evidence: PathBuf,
}

impl Agent {
    fn spawn(dir: &Path, port: u16, api: u16, obs: u16) -> Self {
        Self::spawn_binary(
            dir,
            port,
            api,
            obs,
            Path::new(env!("CARGO_BIN_EXE_splitcast-rs-core")),
        )
    }

    fn spawn_binary(dir: &Path, port: u16, api: u16, obs: u16, binary: &Path) -> Self {
        let role = dir.file_name().unwrap().to_str().unwrap();
        let mut agent = Self {
            child: Command::new(binary)
                .args([
                    "--config",
                    dir.join("config.json").to_str().unwrap(),
                    "agent",
                ])
                .env("PATH", dir.join("tools"))
                .env("SPLITCAST_VIRTUAL_DIR", dir)
                .env("SPLITCAST_VIRTUAL_API", api.to_string())
                .env("SPLITCAST_VIRTUAL_OBS", obs.to_string())
                .stdin(Stdio::null())
                .stdout(File::create(dir.join("stdout.log")).unwrap())
                .stderr(File::create(dir.join("stderr.log")).unwrap())
                .spawn()
                .unwrap(),
            port,
            token: fs::read_to_string(dir.join("token")).unwrap(),
            evidence: dir.to_path_buf(),
        };
        let end = Instant::now() + LIMIT;
        while !splitcast_rs_core::control::request(
            port,
            port,
            &agent.token,
            "status",
            "{}",
            Duration::from_millis(500),
        )
        .ok()
        .and_then(|body| serde_json::from_str::<Value>(&body).ok())
        .is_some_and(|body| body["role"] == role && body["state"] == "idle")
        {
            assert!(
                agent.child.try_wait().unwrap().is_none(),
                "{role} exited: {}",
                fs::read_to_string(dir.join("stderr.log")).unwrap()
            );
            assert!(Instant::now() < end, "{role} did not listen");
            thread::sleep(Duration::from_millis(20));
        }
        agent
    }

    fn trace(&self, event: Value) {
        // Fixture-only private diagnostics; never record the bearer or password.
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.evidence.join("control-transitions.jsonl"))
            .unwrap();
        writeln!(
            log,
            "{}",
            json!({"at":format!("{:?}", Instant::now()), "pid":self.child.id(), "event":event})
        )
        .unwrap();
    }

    fn command(&self, command: &str, args: Value) -> (u16, Value) {
        self.trace(json!({"request":command,"args":args}));
        let body = json!({"command":command,"args":args}).to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        stream.set_write_timeout(Some(LIMIT)).unwrap();
        write!(stream, "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", self.port, self.port, self.token, body.len()).unwrap();
        let mut response = String::new();
        if let Err(error) = stream.read_to_string(&mut response) {
            self.trace(json!({"request_failed":command,"error":error.to_string(),"partial_response":response,
                "process_status":fs::read_to_string(format!("/proc/{}/status",self.child.id())).ok()}));
            if let Ok(tasks) = fs::read_dir(format!("/proc/{}/task", self.child.id())) {
                for task in tasks.flatten() {
                    let path = task.path();
                    eprintln!(
                        "agent task {}: name={} wait={} syscall={}",
                        path.display(),
                        fs::read_to_string(path.join("comm"))
                            .unwrap_or_default()
                            .trim(),
                        fs::read_to_string(path.join("wchan"))
                            .unwrap_or_default()
                            .trim(),
                        fs::read_to_string(path.join("syscall"))
                            .unwrap_or_default()
                            .trim()
                    );
                }
            }
            panic!(
                "control port {} {command} {args}: {error}; received {response:?}",
                self.port
            );
        }
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        self.trace(json!({"response":command,"code":code,"body":body}));
        (code, body)
    }

    fn ok(&self, command: &str, args: Value) -> Value {
        let (code, body) = self.command(command, args);
        if command == "stop" && code == 202 {
            assert_eq!(body["stop"], "requested");
            assert_eq!(body["outcome"], "accepted");
            assert!(body["stop_id"].as_str().is_some_and(|id| !id.is_empty()));
            let completed = self.wait_for_stop();
            assert_eq!(completed["stop_id"], body["stop_id"]);
            return completed;
        }
        assert_eq!(code, 200, "{command}: {body}");
        body
    }

    fn wait_for_stop(&self) -> Value {
        let end = Instant::now() + LIMIT;
        loop {
            // Observe completion; never retransmit Stop or restart an
            // incomplete cleanup attempt merely to make a test pass.
            let status = self.ok("status", json!({}));
            assert_ne!(
                status["stop"], "incomplete",
                "Stop cleanup failed: {status}"
            );
            assert_ne!(
                status["cleanup"], "unknown",
                "Stop cleanup is explicitly unknown: {status}"
            );
            if status["stop"] == "completed" {
                assert_eq!(status["state"], "idle");
                assert_eq!(status["children"], json!([]));
                assert_eq!(status["remote_release_pending"], false);
                return status;
            }
            assert!(
                Instant::now() < end,
                "accepted Stop never resolved: {status}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn idle_prepare_id(&self) -> String {
        let routes = self.ok("routes", json!({}));
        assert_eq!(routes["transport"]["version"], 3);
        let id = routes["prepare_id"].as_str().unwrap();
        assert_eq!(self.state("idle")["start_id"], id);
        id.to_owned()
    }

    fn idle_owner(&self, nonce: u128) -> String {
        format!("{}-{nonce:032x}", self.idle_prepare_id())
    }

    fn stop_owner(&self, owner: &str) -> Value {
        let (code, initial) = self.command("stop", json!({"_owner":owner}));
        let id = initial["stop_id"].as_str().unwrap();
        let (instance, _) = owner.split_once('-').unwrap();
        assert_eq!(id.split_once('-').unwrap().0, instance);
        let proof = if code == 202 {
            assert_eq!(initial["stop"], "requested");
            assert_eq!(initial["outcome"], "accepted");
            let end = Instant::now() + LIMIT;
            loop {
                let observation = self.ok("status", json!({"_owner":owner}));
                assert_eq!(observation["stop_id"], id);
                assert_eq!(observation["owner"], owner);
                if observation["stop"] == "completed" {
                    break observation;
                }
                assert_eq!(observation["outcome"], "accepted");
                assert!(
                    Instant::now() < end,
                    "remote Stop did not complete: {observation}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        } else {
            assert_eq!(code, 200, "{initial}");
            initial
        };
        assert_eq!(proof["owner"], owner);
        assert_eq!(proof["state"], "idle");
        assert_eq!(proof["stop"], "completed");
        assert_eq!(proof["outcome"], "completed");
        proof
    }

    fn state(&self, expected: &str) -> Value {
        let status = self.ok("status", json!({}));
        assert_eq!(status["state"], expected, "{status}");
        if matches!(expected, "idle" | "failed") {
            assert_eq!(status["children"], json!([]), "{status}");
            assert!(status["output"].is_null(), "{status}");
        }
        status
    }

    fn shutdown(&mut self) {
        if self.child.try_wait().unwrap().is_some() {
            return;
        }
        // The unreaped owned Child pins this PID until wait completes.
        assert_eq!(
            unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) },
            0
        );
        let end = Instant::now() + LIMIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                self.trace(json!({"shutdown_exit":status.to_string()}));
                assert!(status.success(), "agent shutdown: {status}");
                return;
            }
            assert!(Instant::now() < end, "agent shutdown timeout");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let observed = self.child.try_wait();
        self.trace(json!({"drop_exit_observation":format!("{observed:?}")}));
        if !matches!(observed, Ok(Some(_))) {
            unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
            let end = Instant::now() + LIMIT;
            while Instant::now() < end {
                if let Ok(Some(status)) = self.child.try_wait() {
                    self.trace(json!({"drop_exit":status.to_string()}));
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            let _ = self.child.kill();
            let status = self.child.wait();
            self.trace(json!({"forced_drop_exit":format!("{status:?}")}));
        }
    }
}

struct Fixture {
    capture: Agent,
    production: Agent,
    root: PathBuf,
    ports: Vec<u16>,
    _port_reservations: Vec<OwnedFd>,
}

fn executable(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|dir| dir.join(name))
        .find(|path| path.is_file() && path.metadata().unwrap().permissions().mode() & 0o111 != 0)
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn private_file(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn bind_loopback_port(port: u16, reuse_address: bool) -> io::Result<(OwnedFd, u16)> {
    // SAFETY: socket takes only integer arguments. On success it gives us a
    // fresh descriptor, which we transfer to OwnedFd.
    let socket = unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedFd::from_raw_fd(fd)
    };
    let reuse = libc::c_int::from(reuse_address);
    let mut address = libc::sockaddr_in {
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets()),
        },
        sin_zero: [0; 8],
    };
    let mut size = size_of::<libc::sockaddr_in>() as libc::socklen_t;
    let fd = socket.as_raw_fd();
    // SAFETY: socket stays owned. The option points to an initialized c_int.
    // bind reads, and getsockname writes, at most size bytes in address.
    unsafe {
        if libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            (&raw const reuse).cast(),
            size_of_val(&reuse) as libc::socklen_t,
        ) < 0
            || libc::bind(fd, (&raw const address).cast(), size) < 0
            || libc::getsockname(fd, (&raw mut address).cast(), &mut size) < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((socket, u16::from_be(address.sin_port)))
}

fn assert_port_not_listening(port: u16) {
    // A closed listener can leave accepted TCP connections in TIME_WAIT.
    // Rebinding immediately without SO_REUSEADDR reports EADDRINUSE even
    // though no process still listens on the port.
    match TcpStream::connect(("127.0.0.1", port)) {
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
        Ok(_) => panic!("port {port} still accepts TCP connections"),
        Err(error) => panic!("cannot determine listener state on port {port}: {error}"),
    }
}

#[path = "return_audio/session.rs"]
mod return_tests;

impl Fixture {
    fn new() -> Self {
        Self::with_speech(false, None)
    }

    fn with_speech(speech: bool, native_lab: Option<&Path>) -> Self {
        Self::with_options(speech, native_lab, false)
    }

    fn with_options(speech: bool, native_lab: Option<&Path>, camera: bool) -> Self {
        let root = std::env::current_dir().unwrap().join(format!(
            "../../artifacts/verification/two-agents-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.parent().unwrap()).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let root = root.canonicalize().unwrap();
        // Keep the port assignments through both Agents' teardown. A bound
        // socket prevents automatic allocation by another fixture. With
        // SO_REUSEADDR and no listen(), the assigned worker can still bind
        // and own its listener on Linux, including on a later Prepare.
        let (reservations, ports): (Vec<_>, Vec<_>) =
            (0..9).map(|_| bind_loopback_port(0, true).unwrap()).unzip();
        for role in ["capture", "production"] {
            let dir = root.join(role);
            fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let tools = dir.join("tools");
            fs::create_dir(&tools).unwrap();
            std::os::unix::fs::symlink(executable("env"), tools.join("env")).unwrap();
            std::os::unix::fs::symlink(executable("timeout"), tools.join("timeout")).unwrap();
            for name in [
                "ssh",
                "ffmpeg",
                "ffprobe",
                "gpu-screen-recorder",
                "mediamtx",
                "pulseaudio",
                "obs",
                "pactl",
            ] {
                let text = if name == "pactl" {
                    format!(
                        r#"#!{}
if [ "$4" = sources ]; then
    printf '%s\n' '[{{"name":"virtual_microphone","properties":{{"device.api":"alsa","device.class":"sound"}}}}]'
elif [ "$4" = sink-inputs ]; then
    printf '%s\n' '[{{"corked":false,"properties":{{"media.name":"SplitCast processed microphone"}}}}]'
else
    printf '%s\n' '[{{"name":"splitcast_mic"}}]'
fi
"#,
                        executable("sh").display()
                    )
                } else {
                    format!("#!{}\nexport SPLITCAST_VIRTUAL_KIND='{name}'\nexec {} --exact virtual_worker --ignored --nocapture >> {} 2>&1\n", executable("sh").display(), quote(&std::env::current_exe().unwrap()), quote(&dir.join(format!("{name}.log"))))
                };
                let path = tools.join(name);
                fs::write(&path, text).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let fixture_id = root.file_name().unwrap().to_str().unwrap();
            private_file(&dir.join("token"), &format!("virtual-{fixture_id}-{role}"));
            private_file(&dir.join("password"), PASSWORD);
            let mut cfg: Value = serde_json::from_str(include_str!("agent-example.json")).unwrap();
            cfg["role"] = json!(role);
            cfg["peer"]["control_port"] = json!(if role == "capture" {
                ports[0]
            } else {
                ports[1]
            });
            cfg["peer"]["remote_control_port"] = json!(ports[1]);
            cfg["peer"]["forward_control_port"] = json!(if role == "capture" {
                if native_lab.is_some() {
                    ports[2]
                } else {
                    ports[1]
                }
            } else {
                ports[2]
            });
            cfg["peer"]["ssh_port"] = json!(ports[3]);
            cfg["peer"]["timeout_seconds"] = json!(10);
            cfg["peer"]["token_file"] = json!(dir.join("token"));
            cfg["peer"]["remote_token_file"] = json!(root.join("production/token"));
            cfg["peer"]["state_dir"] = json!(dir.join("state"));
            cfg["capture"]["rtmp_port"] = json!(ports[4]);
            cfg["capture"]["rtsp_port"] = json!(ports[5]);
            cfg["capture"]["api_port"] = json!(ports[6]);
            cfg["capture"]["target"] = json!("virtual-window");
            cfg["capture"]["game_audio"] = json!("app:virtual-game");
            cfg["audio"]["mic_source"] = json!("virtual_microphone");
            cfg["obs"]["binary"] = json!(tools.join("obs"));
            cfg["obs"]["websocket_port"] = json!(ports[7]);
            cfg["obs"]["password_file"] = json!(dir.join("password"));
            cfg["obs"]["recording_dir"] = json!(dir.join("recordings"));
            if role == "production" {
                if camera {
                    cfg["camera"] = json!({
                        "location":"capture","input_format":"h264","encoder":"copy",
                        "width":320,"height":180,"fps":15
                    });
                    cfg["obs"]["overlay_text"] = json!("legacy overlay");
                }
                let key = dir.join("virtual-stream-key");
                private_file(&key, "isolated-fixture-key");
                cfg["twitch"] = json!({
                    "ingest_host":"apn10.contribute.live-video.net",
                    "key_file":key,"allow_public":false,
                });
            }
            cfg["tts"]["credentials_file"] = json!(dir.join("absent-credentials"));
            cfg["tts"]["voicevox_url"] = json!(format!("http://127.0.0.1:{}", ports[8]));
            if speech {
                cfg["tts"]["enabled"] = json!(true);
                cfg["tts"]["queue_limit"] = json!(2);
                cfg["tts"]["expiry_seconds"] = json!(5);
                if role == "capture" {
                    cfg["audio"]["tts_sink"] = json!("return.selected");
                    if native_lab.is_none() {
                        cfg["audio"]["tts_volume_percent"] = json!(60);
                    }
                    return_tests::install_player(&dir, native_lab);
                }
            }
            fs::write(dir.join("config.json"), cfg.to_string()).unwrap();
        }
        let production = Agent::spawn(&root.join("production"), ports[1], ports[6], ports[7]);
        let capture = Agent::spawn(&root.join("capture"), ports[0], ports[6], ports[7]);
        println!(
            "two real agents, isolated worker environment; evidence {}",
            root.display()
        );
        Self {
            capture,
            production,
            root,
            ports,
            _port_reservations: reservations,
        }
    }

    fn pids(&self) -> Vec<i32> {
        ["capture", "production"]
            .into_iter()
            .flat_map(|role| {
                fs::read_dir(self.root.join(role))
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .filter(|p| p.extension().is_some_and(|e| e == "pid"))
                    .map(|p| fs::read_to_string(p).unwrap().trim().parse().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn released(&self, pids: &[i32]) {
        for pid in pids {
            assert_eq!(unsafe { libc::kill(*pid, 0) }, -1, "worker {pid} survived");
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }
        for index in [6, 7] {
            assert_port_not_listening(self.ports[index]);
        }
    }
}

#[test]
fn production_watchdog_requires_stream_frames_as_well_as_bytes() {
    for fault in ["missing", "wrong-type", "unchanged", "decreasing"] {
        let mut fixture = Fixture::new();
        let agent = &fixture.production;
        agent.ok("prepare", json!({}));
        agent.ok("start", json!({"output":"twitch-test"}));
        let baseline = agent.state("running")["output_progress"].clone();
        let end = Instant::now() + Duration::from_secs(4);
        loop {
            let status = agent.state("running");
            let progress = &status["output_progress"];
            if progress["bytes"].as_u64() > baseline["bytes"].as_u64() {
                assert!(
                    progress["frames"].as_u64().unwrap() > baseline["frames"].as_u64().unwrap()
                );
                assert_eq!(progress["frame_health"], "progressing");
                break;
            }
            assert!(
                Instant::now() < end,
                "valid stream counters did not advance: {status}"
            );
            thread::sleep(Duration::from_millis(20));
        }

        let pids = fixture.pids();
        assert!(!pids.is_empty());
        fs::write(fixture.root.join("production/frame-counter-fault"), fault).unwrap();
        let end = Instant::now() + Duration::from_secs(4);
        loop {
            let status = agent.ok("status", json!({}));
            if status["state"] == "failed" {
                break;
            }
            assert!(
                Instant::now() < end,
                "watchdog accepted {fault} stream frames: {status}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        let failed = agent.state("failed");
        assert_eq!(failed["obs_shutdown"]["output_inactive_confirmed"], true);
        fixture.released(&pids);
        let log = fs::read_to_string(fixture.root.join("production/obs-requests.log")).unwrap();
        assert_eq!(log.lines().filter(|line| *line == "StartStream").count(), 1);
        assert_eq!(log.lines().filter(|line| *line == "StopStream").count(), 1);
        agent.ok("stop", json!({}));
        fixture.production.shutdown();
        fixture.capture.shutdown();
    }
}

#[test]
fn two_actual_agents_control_failure_and_explicit_recovery() {
    let mut f = Fixture::new();
    f.capture.state("idle");
    f.production.state("idle");
    for failure in [
        None,
        Some(("capture", "gpu-screen-recorder")),
        Some(("production", "mediamtx")),
        None,
    ] {
        f.capture.ok("prepare", json!({}));
        f.capture.state("ready");
        f.production.state("ready");
        f.capture.ok("start", json!({"output":"recording"}));
        let before = f.capture.state("running");
        let production_before = f.production.state("running");
        assert_eq!(before["output"], "recording");
        assert_eq!(
            production_before["output_progress"]["frame_health"],
            "unknown"
        );
        assert!(production_before["output_progress"]["frames"].is_null());
        assert!(
            production_before["output_progress"]["bytes"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(production_before["output"], "recording");
        f.capture.ok("start", json!({"output":"recording"}));
        assert_eq!(f.capture.state("running"), before);
        assert_eq!(f.production.state("running"), production_before);
        for command in ["prepare", "start", "stop"] {
            let args = if command == "start" {
                json!({"_owner":"ffffffffffffffffffffffffffffffff", "output":"recording"})
            } else {
                json!({"_owner":"ffffffffffffffffffffffffffffffff"})
            };
            let (code, body) = f.production.command(command, args);
            assert_eq!(code, 409, "conflicting owner {command}: {body}");
        }
        thread::sleep(Duration::from_millis(1200));
        f.capture.state("running");
        f.production.state("running");
        let pids = f.pids();
        assert_eq!(pids.len(), 7, "both agents must own their worker processes");
        if let Some((role, worker)) = failure {
            let pid: i32 = fs::read_to_string(f.root.join(role).join(format!("{worker}.pid")))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
            let end = Instant::now() + LIMIT;
            while f.capture.ok("status", json!({}))["state"] != "failed" {
                assert!(
                    Instant::now() < end,
                    "capture did not react to {role} worker death"
                );
                thread::sleep(Duration::from_millis(20));
            }
            f.capture.state("failed");
            f.production.state("idle");
            assert_eq!(
                f.capture.command("prepare", json!({})).0,
                409,
                "failure requires explicit Stop before Prepare"
            );
        }
        f.capture.ok("stop", json!({}));
        f.capture.state("idle");
        f.production.state("idle");
        f.released(&pids);
    }
    let obs_log = fs::read_to_string(f.root.join("production/obs-requests.log")).unwrap();
    assert_eq!(
        obs_log
            .lines()
            .filter(|line| *line == "StartRecord")
            .count(),
        4,
        "repeated Start must not restart output"
    );
    assert!(!obs_log.contains("StartStream"));
    f.capture.shutdown();
    f.production.shutdown();
    for port in &f.ports {
        assert_port_not_listening(*port);
    }
}

#[test]
fn lost_stop_reply_and_new_session_do_not_fabricate_old_completion_history() {
    let mut f = Fixture::new();
    let first = f.production.idle_owner(1);
    f.production.ok("prepare", json!({"_owner":first}));
    f.production
        .ok("start", json!({"_owner":first,"output":"recording"}));
    let pids = f.pids();
    assert!(!pids.is_empty());
    // Force the 202 path, then lose its body before learning the ticket.
    let hold = f.root.join("production/hold-stop-ack");
    fs::write(&hold, "hold").unwrap();
    let body = json!({"command":"stop","args":{"_owner":first}}).to_string();
    let mut lost = TcpStream::connect(("127.0.0.1", f.production.port)).unwrap();
    write!(lost, "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", f.production.port, f.production.port, f.production.token, body.len()).unwrap();
    lost.set_read_timeout(Some(LIMIT)).unwrap();
    let mut status_line = Vec::new();
    while !status_line.ends_with(b"\r\n") {
        let mut byte = [0];
        lost.read_exact(&mut byte).unwrap();
        status_line.push(byte[0]);
        assert!(status_line.len() < 80);
    }
    assert_eq!(status_line, b"HTTP/1.1 202 Accepted\r\n");
    drop(lost);
    fs::remove_file(hold).unwrap();
    let end = Instant::now() + LIMIT;
    let first_stop = loop {
        let (code, observation) = f.production.command("status", json!({"_owner":first}));
        assert_eq!(code, 200, "{observation}");
        if observation["stop"] == "completed" {
            assert_eq!(observation["owner"], first);
            assert_eq!(observation["state"], "idle");
            assert_eq!(observation["children"], json!([]));
            assert!(observation["stop_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty()));
            break observation["stop_id"].clone();
        }
        assert!(
            Instant::now() < end,
            "lost-reply Stop did not resolve: {observation}"
        );
        thread::sleep(Duration::from_millis(10));
    };
    f.released(&pids);
    // An independent status observer saw the release. The client that lost
    // the body can obtain proof via a fresh explicit Stop of the same owner.
    let (code, retry) = f.production.command("stop", json!({"_owner":first}));
    assert_eq!(code, 200);
    assert_eq!(retry["stop_id"], first_stop);
    assert_eq!(retry["owner"], first);
    assert_eq!(retry["outcome"], "completed");
    let second = f.production.idle_owner(2);
    f.production.ok("prepare", json!({"_owner":second}));
    let current = f.production.ok("status", json!({}));
    assert_eq!(current["state"], "ready");
    assert!(
        current["stop"].is_null(),
        "new session fabricated old completion"
    );
    assert!(current["owner"].is_null());
    assert!(current["stop_id"].is_null());
    assert!(!current["children"].as_array().unwrap().is_empty());
    for command in ["status", "stop", "prepare", "heartbeat"] {
        assert_eq!(
            f.production.command(command, json!({"_owner":first})).0,
            409,
            "old {command} interfered with the newer owner"
        );
    }
    f.production
        .ok("start", json!({"_owner":second,"output":"recording"}));
    f.production.state("running");
    f.production.stop_owner(&second);
    let overwritten = f.production.ok("status", json!({"_owner":first}));
    assert_eq!(overwritten["stop"], "completed");
    assert_eq!(overwritten["owner"], second);
    assert_ne!(overwritten["stop_id"], first_stop);
    assert_ne!(
        overwritten["owner"], first,
        "another session became the first owner's release proof"
    );
    assert_eq!(f.production.command("stop", json!({"_owner":first})).0, 409);
    f.released(&f.pids());
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn completed_stop_fences_remote_prepare_replay_across_sessions_and_restart() {
    let mut f = Fixture::new();
    let first_revision = f.production.idle_prepare_id();
    let owner = format!("{first_revision}-{:032x}", 1);
    let (instance, _) = first_revision.split_once('-').unwrap();
    let invalid_owners = [
        format!("{:032x}", 1),
        first_revision.clone(),
        format!("{first_revision}-{:031x}", 1),
        format!("{first_revision}-{:032X}", 0xAB),
        format!("{instance}-9-idle-{:032x}", 1),
    ];
    for invalid in invalid_owners {
        let (status, body) = f.production.command("prepare", json!({"_owner":invalid}));
        assert_eq!(status, 409, "{body}");
        assert_eq!(body["outcome"], "not_executed");
        assert!(f.pids().is_empty());
    }
    assert_eq!(f.production.idle_prepare_id(), first_revision);
    assert_eq!(
        f.production.ok("prepare", json!({"_owner":owner}))["state"],
        "ready"
    );
    let pids = f.pids();
    assert_eq!(pids.len(), 1);
    let first_stop = f.production.stop_owner(&owner);
    f.released(&pids);

    let refused = |f: &Fixture| {
        let workers = f.pids();
        let old = f.production.state("idle");
        let (code, body) = f.production.command("prepare", json!({"_owner":owner}));
        assert_eq!(code, 409, "{body}");
        assert_eq!(body["outcome"], "not_executed");
        assert_eq!(f.production.state("idle"), old);
        assert_eq!(f.pids(), workers, "rejected Prepare spawned a worker");
        assert!(!f.root.join("production/obs-requests.log").exists());
        f.released(&workers);
    };
    refused(&f);

    let fresh = f.production.idle_owner(2);
    assert_ne!(fresh, owner);
    f.production.ok("prepare", json!({"_owner":fresh}));
    let fresh_stop = f.production.stop_owner(&fresh);
    assert_ne!(first_stop["stop_id"], fresh_stop["stop_id"]);
    f.released(&f.pids());
    refused(&f);

    f.production.shutdown();
    f.production = Agent::spawn(
        &f.root.join("production"),
        f.ports[1],
        f.ports[6],
        f.ports[7],
    );
    let restart = f.production.idle_prepare_id();
    assert_eq!(
        restart.split_once('-').unwrap().1,
        first_revision.split_once('-').unwrap().1
    );
    assert_ne!(restart.split_once('-').unwrap().0, instance);
    refused(&f);
    let (code, proof) = f.production.command("stop", json!({"_owner":owner}));
    assert_eq!(code, 409, "{proof}");
    assert_eq!(proof["outcome"], "not_executed");

    f.production.ok("prepare", json!({}));
    f.production.state("ready");
    f.production.ok("stop", json!({}));
    assert_ne!(f.production.idle_prepare_id(), restart);
    f.released(&f.pids());
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn one_of_two_nonces_for_an_idle_revision_acquires_and_can_retry() {
    let mut f = Fixture::with_speech(true, None);
    let revision = f.production.idle_prepare_id();
    let speech_routes = f.production.ok("routes", json!({"media":false}));
    assert_eq!(speech_routes["prepare_id"], revision);
    assert_eq!(speech_routes["transport"]["version"], 3);
    assert!(speech_routes["transport"]["media"].is_null());
    let owners = [
        format!("{revision}-{:032x}", 1),
        format!("{revision}-{:032x}", 2),
    ];
    let ready = std::sync::Barrier::new(3);
    let outcomes = thread::scope(|scope| {
        let production = &f.production;
        let start = &ready;
        let tasks = owners.each_ref().map(|owner| {
            scope.spawn(move || {
                start.wait();
                production.command("prepare", json!({"_owner":owner}))
            })
        });
        ready.wait();
        tasks.map(|task| task.join().unwrap())
    });
    let winner = match &outcomes {
        [(200, ready), (409, denied)] => {
            assert_eq!(ready["state"], "ready");
            assert_eq!(denied["outcome"], "not_executed");
            &owners[0]
        }
        [(409, denied), (200, ready)] => {
            assert_eq!(ready["state"], "ready");
            assert_eq!(denied["outcome"], "not_executed");
            &owners[1]
        }
        _ => panic!("expected one winner, got {outcomes:?}"),
    };
    let pids = f.pids();
    assert_eq!(pids.len(), 1);
    assert!(!f.root.join("production/obs-requests.log").exists());
    assert!(f.production.ok("routes", json!({}))["prepare_id"].is_null());
    let ready = f.production.ok("prepare", json!({"_owner":winner}));
    assert_eq!(ready["state"], "ready");
    let (code, wrong_purpose) = f
        .production
        .command("prepare", json!({"_owner":winner,"media":false}));
    assert_eq!(code, 409);
    assert_eq!(wrong_purpose["outcome"], "not_executed");
    assert_eq!(f.pids(), pids);
    f.production
        .ok("start", json!({"_owner":winner,"output":"recording"}));
    let running = f.pids();
    assert_eq!(
        f.production.ok("prepare", json!({"_owner":winner}))["state"],
        "running"
    );
    assert_eq!(f.pids(), running);
    f.production.stop_owner(winner);
    f.released(&running);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn late_owned_start_cannot_recreate_a_completed_remote_session() {
    let mut f = Fixture::new();
    f.capture.ok("prepare", json!({}));
    f.capture.ok("start", json!({"output":"recording"}));
    f.capture.ok("stop", json!({}));
    let completed = f.production.state("idle");
    assert_eq!(completed["stop"], "completed");
    let owner = completed["owner"].as_str().unwrap();
    assert_eq!(
        f.production
            .command("start", json!({"_owner":owner,"output":"recording"}))
            .0,
        409,
        "a late Start recreated a completed remote session"
    );
    f.production.state("idle");
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn bearer_holder_can_use_unowned_emergency_stop_on_peer_owned_session() {
    let mut f = Fixture::new();
    let revision = f.production.idle_prepare_id();
    f.capture.ok("prepare", json!({}));
    f.capture.ok("start", json!({"output":"recording"}));
    let pids = f.pids();
    // Capture possesses this same production bearer for peer RPCs. Omitting
    // _owner intentionally exercises its existing full operator authority.
    let body = splitcast_rs_core::control::request(
        f.production.port,
        f.production.port,
        &f.production.token,
        "stop",
        "{}",
        LIMIT,
    )
    .unwrap();
    let body: Value = serde_json::from_str(&body).unwrap();
    assert!(matches!(
        body["stop"].as_str(),
        Some("requested" | "completed")
    ));
    let proof = if body["stop"] == "requested" {
        f.production.wait_for_stop()
    } else {
        body.clone()
    };
    assert_eq!(proof["stop_id"], body["stop_id"]);
    assert_eq!(
        proof["owner"].as_str().unwrap().rsplit_once('-').unwrap().0,
        revision
    );
    f.production.state("idle");
    f.capture.ok("stop", json!({}));
    f.capture.state("idle");
    f.released(&pids);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn committed_prepare_is_stoppable_while_receiver_readiness_is_held() {
    let mut f = Fixture::new();
    let owner = f.production.idle_owner(1);
    let dir = f.root.join("production");
    fs::write(dir.join("hold-readiness"), "hold").unwrap();
    let pids = thread::scope(|scope| {
        let preparation = scope.spawn(|| f.production.command("prepare", json!({"_owner":owner})));
        let end = Instant::now() + LIMIT;
        while !dir.join("prepare-readiness-entered").exists() {
            assert!(Instant::now() < end, "Prepare did not reach the receiver");
            thread::sleep(Duration::from_millis(10));
        }
        let pids = f.pids();
        assert_eq!(pids.len(), 1);
        assert!(f.production.ok("routes", json!({}))["prepare_id"].is_null());
        let (code, denied) = f
            .production
            .command("stop", json!({"_owner":"wrong-owner"}));
        assert_eq!(code, 409);
        assert_eq!(denied["outcome"], "not_executed");
        f.production.stop_owner(&owner);
        let (code, interrupted) = preparation.join().unwrap();
        assert_eq!(code, 409);
        assert_eq!(interrupted["outcome"], "unknown");
        pids
    });
    f.production.state("idle");
    f.released(&pids);
    assert!(!dir.join("obs-requests.log").exists());
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn camera_contract_mismatch_rejects_capture_prepare_before_dispatch() {
    let mut f = Fixture::with_options(false, None, true);
    let idle = f.production.state("idle");
    let (code, refused) = f.capture.command("prepare", json!({}));
    assert_eq!(code, 409);
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("peer media or speech contract differs"));
    assert_eq!(f.capture.state("failed")["remote_release_pending"], false);
    assert_eq!(f.production.state("idle"), idle);
    assert!(!f.root.join("production/mediamtx.pid").exists());
    assert!(!f.root.join("production/obs-requests.log").exists());
    f.released(&f.pids());
    f.capture.ok("stop", json!({}));
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn capture_cannot_prepare_against_compatible_but_held_production() {
    let mut f = Fixture::new();
    f.production.ok("prepare", json!({}));
    let receiver = fs::read_to_string(f.root.join("production/mediamtx.pid")).unwrap();
    let (code, refused) = f.capture.command("prepare", json!({}));
    assert_eq!(code, 409);
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("no available Idle revision"));
    assert_eq!(f.capture.state("failed")["remote_release_pending"], false);
    f.production.state("ready");
    assert_eq!(
        fs::read_to_string(f.root.join("production/mediamtx.pid")).unwrap(),
        receiver
    );
    assert!(!f.root.join("production/obs-requests.log").exists());
    f.capture.ok("stop", json!({}));
    f.production.ok("stop", json!({}));
    f.released(&f.pids());
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
#[ignore = "requires a separately built pinned v2 Agent in SPLITCAST_TEST_V2_AGENT"]
fn paired_v2_v3_mismatches_refuse_both_directions_before_production_effects() {
    let v2 = PathBuf::from(std::env::var_os("SPLITCAST_TEST_V2_AGENT").unwrap())
        .canonicalize()
        .unwrap();
    for version_two_role in ["capture", "production"] {
        let mut f = Fixture::with_speech(true, None);
        if version_two_role == "capture" {
            f.capture.shutdown();
            f.capture = Agent::spawn_binary(
                &f.root.join("capture"),
                f.ports[0],
                f.ports[6],
                f.ports[7],
                &v2,
            );
        } else {
            f.production.shutdown();
            f.production = Agent::spawn_binary(
                &f.root.join("production"),
                f.ports[1],
                f.ports[6],
                f.ports[7],
                &v2,
            );
        }
        let idle = f.production.state("idle");
        for media in [true, false] {
            let (code, refused) = f.capture.command("prepare", json!({"media":media}));
            assert_eq!(
                code, 409,
                "v2 {version_two_role} with media={media}: {refused}"
            );
            assert!(refused["error"]
                .as_str()
                .unwrap()
                .contains("peer media or speech contract differs"));
            assert_eq!(f.capture.state("failed")["remote_release_pending"], false);
            assert_eq!(f.production.state("idle"), idle);
            assert!(!f.root.join("production/mediamtx.pid").exists());
            assert!(!f.root.join("production/obs-requests.log").exists());
            f.released(&f.pids());
            f.capture.ok("stop", json!({}));
            println!("v2 {version_two_role}, media={media}: contract refusal, unchanged production Idle, no remote attempt or worker");
        }
        f.capture.shutdown();
        f.production.shutdown();
    }
}

#[test]
fn sigterm_running_capture_releases_both_agents_workers() {
    let mut fixture = Fixture::new();
    let receiver_port = fixture.ports[6];
    assert_port_not_listening(receiver_port);
    assert_eq!(
        bind_loopback_port(receiver_port, false)
            .expect_err("fixture must reserve the receiver port before Prepare")
            .kind(),
        io::ErrorKind::AddrInUse,
    );
    fixture.capture.ok("prepare", json!({}));
    fixture.capture.ok("start", json!({"output":"recording"}));
    fixture.capture.state("running");
    fixture.production.state("running");
    let pids = fixture.pids();
    assert_eq!(pids.len(), 7);
    fixture.capture.shutdown();
    fixture.production.state("idle");
    fixture.released(&pids);
    fixture.production.shutdown();
}

#[test]
fn stop_during_start_acknowledgment_retains_uncertainty_and_reconciles() {
    let mut f = Fixture::new();
    f.capture.ok("prepare", json!({}));
    let production_dir = f.root.join("production");
    fs::write(production_dir.join("hold-start-ack"), "hold").unwrap();
    let capture_port = f.capture.port;
    let token = f.capture.token.clone();
    let start = thread::spawn(move || {
        splitcast_rs_core::control::request(
            capture_port,
            capture_port,
            &token,
            "start",
            r#"{"output":"recording"}"#,
            LIMIT,
        )
    });
    let end = Instant::now() + LIMIT;
    while !production_dir.join("start-committed").exists() {
        assert!(
            Instant::now() < end,
            "Start did not reach OBS effect boundary"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let pids = f.pids();
    let admitted = Instant::now();
    let (code, body) = f.capture.command("stop", json!({}));
    assert_eq!(code, 202, "{body}");
    assert_eq!(body["stop"], "requested");
    assert!(admitted.elapsed() <= Duration::from_millis(500));
    let error = start.join().unwrap().unwrap_err();
    assert!(error.to_string().contains("outcome unknown"), "{error}");
    // The publication effect was applied but the foreground acknowledgment is
    // deliberately withheld. Production can accept Stop on another connection.
    // Its accepted reply is not capture's proof of remote release.
    let status = f.capture.state("stopping");
    assert_eq!(status["remote_release_pending"], true);
    assert_eq!(status["stop"], "requested");
    assert_eq!(status["resource_observation"], "unknown");
    fs::remove_file(production_dir.join("hold-start-ack")).unwrap();
    let end = Instant::now() + LIMIT;
    loop {
        let status = f.production.ok("status", json!({}));
        assert_ne!(
            status["state"], "running",
            "cancelled Start committed Running"
        );
        if status["state"] == "idle" {
            break;
        }
        assert!(
            Instant::now() < end,
            "production cleanup did not complete: {status}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    f.capture.wait_for_stop();
    f.capture.state("idle");
    f.released(&pids);
    let log = fs::read_to_string(production_dir.join("obs-requests.log")).unwrap();
    assert_eq!(log.lines().filter(|line| *line == "StartRecord").count(), 1);
    assert!(!log.contains("StartStream"));
    // Fresh authorization is a positive control, not automatic recovery.
    f.capture.ok("prepare", json!({}));
    f.capture.ok("start", json!({"output":"recording"}));
    f.capture.state("running");
    f.capture.ok("stop", json!({}));
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn sigterm_during_delayed_prepare_releases_receiver_and_accepted_connections() {
    let mut f = Fixture::new();
    let dir = f.root.join("production");
    fs::write(dir.join("hold-readiness"), "hold").unwrap();
    let port = f.production.port;
    let token = f.production.token.clone();
    let prepare = thread::spawn(move || {
        splitcast_rs_core::control::request(port, port, &token, "prepare", "{}", LIMIT)
    });
    let end = Instant::now() + LIMIT;
    while !dir.join("prepare-readiness-entered").exists() {
        assert!(
            Instant::now() < end,
            "Prepare never reached the deliberately stalled readiness request"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let pids = f.pids();
    assert_eq!(pids.len(), 1, "only the production receiver should exist");
    let signalled = Instant::now();
    f.production.shutdown();
    assert!(prepare
        .join()
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("outcome unknown"));
    f.released(&pids);
    assert_port_not_listening(port);
    println!(
        "SIGTERM during delayed Prepare: {:?}; receiver and listener released",
        signalled.elapsed()
    );
    f.capture.shutdown();
}

#[test]
#[ignore = "subprocess entrypoint used only by the virtual two-agent fixture"]
fn virtual_worker() {
    let kind = std::env::var("SPLITCAST_VIRTUAL_KIND").expect("owned worker kind");
    let dir = PathBuf::from(std::env::var_os("SPLITCAST_VIRTUAL_DIR").unwrap());
    fs::write(
        dir.join(format!("{kind}.pid")),
        std::process::id().to_string(),
    )
    .unwrap();
    match kind.as_str() {
        "mediamtx" => {
            let port: u16 = std::env::var("SPLITCAST_VIRTUAL_API")
                .unwrap()
                .parse()
                .unwrap();
            let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
            for (n, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                stream.set_read_timeout(Some(LIMIT)).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 8192);
                }
                assert!(request.starts_with(b"GET /v3/paths/list HTTP/1.1"));
                if dir.join("hold-readiness").exists() {
                    fs::write(
                        dir.join("prepare-readiness-entered"),
                        "request observed; response withheld",
                    )
                    .unwrap();
                    let end = Instant::now() + LIMIT;
                    while dir.join("hold-readiness").exists() && Instant::now() < end {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                let body = json!({"items":[
                    {"name":"game","ready":true,"bytesReceived":(n+1)*1024,"tracks":["H264","MPEG-4 Audio"]},
                    {"name":"mic","ready":true,"bytesReceived":(n+1)*512,"tracks":["Opus"]},
                    {"name":"camera","ready":true,"bytesReceived":(n+1)*256,"tracks":["H264"]}
                ]}).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        }
        "obs" => virtual_obs(&dir),
        "ssh" | "ffmpeg" | "gpu-screen-recorder" | "pulseaudio" => loop {
            thread::sleep(Duration::from_secs(1));
        },
        other => panic!("unexpected stand-in worker {other}"),
    }
}

#[derive(Default)]
struct VirtualScene {
    camera: Option<Value>,
    camera_visible: bool,
    overlay: Option<Value>,
    overlay_text: Option<Value>,
}

impl VirtualScene {
    fn item_list(&self) -> Value {
        let mut items = vec![json!({
            "sourceName":"GameFeed","sceneItemId":1,"sceneItemEnabled":true,
            "sceneItemTransform":{"sourceWidth":1278.0,"sourceHeight":720.0}
        })];
        if let Some(transform) = &self.camera {
            let mut transform = transform.clone();
            transform["sourceWidth"] = json!(320.0);
            transform["sourceHeight"] = json!(180.0);
            items.push(json!({
                "sourceName":"CameraFeed","sceneItemId":2,"sceneItemEnabled":self.camera_visible,
                "sceneItemTransform":transform
            }));
        }
        if let Some(transform) = &self.overlay {
            items.push(json!({
                "sourceName":"SplitCastOverlay","sceneItemId":3,"sceneItemEnabled":true,
                "sceneItemTransform":transform
            }));
        }
        json!({"sceneItems":items})
    }

    fn inputs(&self) -> Value {
        let mut inputs = vec![
            json!({"inputName":"GameFeed"}),
            json!({"inputName":"MicProcessed"}),
        ];
        if self.camera.is_some() {
            inputs.push(json!({"inputName":"CameraFeed"}));
        }
        if self.overlay_text.is_some() {
            inputs.push(json!({"inputName":"SplitCastOverlay"}));
        }
        json!({"inputs":inputs})
    }

    fn input(&mut self, data: &Value) {
        match data["inputName"].as_str().unwrap() {
            "CameraFeed" => self.camera_visible = true,
            "SplitCastOverlay" => self.overlay_text = Some(data["inputSettings"]["text"].clone()),
            "GameFeed" | "MicProcessed" => {}
            other => panic!("unexpected OBS input {other}"),
        }
    }
}

fn virtual_obs(dir: &Path) {
    let port: u16 = std::env::var("SPLITCAST_VIRTUAL_OBS")
        .unwrap()
        .parse()
        .unwrap();
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    let state = Arc::new(Mutex::new((None, 0u64, VirtualScene::default())));
    let log = Arc::new(Mutex::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("obs-requests.log"))
            .unwrap(),
    ));
    thread::scope(|scope| {
        for stream in listener.incoming() {
            let state = Arc::clone(&state);
            let log = Arc::clone(&log);
            scope.spawn(move || {
        let stream = stream.unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        stream.set_write_timeout(Some(LIMIT)).unwrap();
        let Ok(mut socket) = tungstenite::accept(stream) else { return };
        let send = |socket: &mut tungstenite::WebSocket<TcpStream>, value: Value| {
            socket.send(Message::Text(value.to_string().into())).unwrap();
        };
        send(&mut socket, json!({"op":0,"d":{"authentication":{"salt":"virtual-salt","challenge":"virtual-challenge"}}}));
        let identify: Value = serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
        let secret = STANDARD.encode(Sha256::digest(format!("{PASSWORD}virtual-salt").as_bytes()));
        let proof = STANDARD.encode(Sha256::digest(format!("{secret}virtual-challenge").as_bytes()));
        assert_eq!(identify["d"]["authentication"], proof);
        send(&mut socket, json!({"op":2,"d":{"negotiatedRpcVersion":1}}));
        while let Ok(message) = socket.read() {
            let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            let method = request["d"]["requestType"].as_str().unwrap();
            writeln!(log.lock().unwrap(), "{method}").unwrap();
            let mut state = state.lock().unwrap();
            let (active, count, scene) = &mut *state;
            let args = &request["d"]["requestData"];
            let fault_path = dir.join("scene-fault");
            let fault = fs::read_to_string(&fault_path).unwrap_or_default();
            let mut result = true;
            let data = match method {
                "GetVersion" => json!({"obsVersion":"virtual"}),
                "GetSceneList" => json!({"scenes":[{"sceneName":"SplitCast"}]}),
                "GetSceneItemId" => json!({"sceneItemId":match args["sourceName"].as_str().unwrap() {
                    "GameFeed" => 1, "CameraFeed" => 2, "SplitCastOverlay" => 3,
                    other => panic!("unexpected scene item {other}"),
                }}),
                "GetSceneItemTransform" => {
                    let (width, height) = match args["sceneItemId"].as_u64().unwrap() {
                        1 => (1278, 720), 2 => (320, 180), other => panic!("unexpected transform {other}"),
                    };
                    json!({"sceneItemTransform":{"sourceWidth":width,"sourceHeight":height}})
                }
                "GetVideoSettings" => json!({"baseWidth":1280,"baseHeight":720}),
                "GetSceneItemList" => scene.item_list(),
                "GetInputList" => scene.inputs(),
                "CreateInput" | "SetInputSettings" => { scene.input(args); json!({}) }
                "GetInputSettings" => {
                    assert_eq!(args["inputName"], "SplitCastOverlay");
                    json!({"inputSettings":{"text":scene.overlay_text}})
                }
                "RemoveInput" => {
                    assert_eq!(args["inputName"], "SplitCastOverlay");
                    scene.overlay = None;
                    scene.overlay_text = None;
                    json!({})
                }
                "SetSceneItemTransform" => {
                    match args["sceneItemId"].as_u64().unwrap() {
                        2 => scene.camera = Some(args["sceneItemTransform"].clone()),
                        3 => scene.overlay = Some(args["sceneItemTransform"].clone()),
                        other => panic!("unexpected transform edit {other}"),
                    }
                    if fault == "lose-transform-reply" {
                        fs::remove_file(&fault_path).unwrap();
                        return;
                    }
                    json!({})
                }
                "SetSceneItemEnabled" => {
                    assert_eq!(args["sceneItemId"], 2);
                    if fault == "reject-visibility" {
                        fs::remove_file(&fault_path).unwrap();
                        result = false;
                    } else {
                        scene.camera_visible = args["sceneItemEnabled"].as_bool().unwrap();
                    }
                    json!({})
                }
                "GetSourceScreenshot" => {
                    assert_eq!(args["sourceName"], "SplitCast");
                    assert_eq!(args["imageFormat"], "png");
                    assert!(matches!((args["imageWidth"].as_u64(), args["imageHeight"].as_u64()), (Some(480),Some(270)) | (Some(960),Some(540))));
                    // RGBA scanlines with filter byte 0 and valid IDAT and CRCs.
                    // Keep the oversized case decodable as well so it tests dimensions alone.
                    let png = if fault == "oversize-preview" {
                        "iVBORw0KGgoAAAANSUhEUgAAAeEAAAABCAYAAADpeRY4AAAAGklEQVR4nGNQaHj4fxSP4lE8ikfxKB7F9McAXu+yvc2RzNkAAAAASUVORK5CYII="
                    } else {
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNQaHj4HwAExgKBNCGZBwAAAABJRU5ErkJggg=="
                    };
                    json!({"imageData":format!("data:image/png;base64,{png}")})
                }
                "StartRecord" | "StartStream" => {
                    assert!(active.is_none());
                    *active = Some(if method == "StartRecord" { "GetRecordStatus" } else { "GetStreamStatus" });
                    json!({})
                },
                "StopRecord" | "StopStream" => { *active = None; json!({}) },
                "SetStreamServiceSettings" => {
                    assert_eq!(request["d"]["requestData"]["streamServiceSettings"]["key"], "isolated-fixture-key?bandwidthtest=true");
                    json!({})
                },
                "GetRecordStatus" | "GetStreamStatus" => {
                    *count += 1;
                    let mut status = json!({"outputActive":*active == Some(method),"outputBytes":*count*1024,"outputDuration":*count*1000});
                    if method == "GetStreamStatus" {
                        match fs::read_to_string(dir.join("frame-counter-fault")).as_deref() {
                            Ok("missing") => {},
                            Ok("wrong-type") => status["outputTotalFrames"] = json!("9000"),
                            Ok("unchanged") => status["outputTotalFrames"] = json!((*count - 1) * 30),
                            Ok("decreasing") => status["outputTotalFrames"] = json!(0),
                            _ => status["outputTotalFrames"] = json!(*count * 30),
                        }
                    }
                    status
                }
                "SetInputAudioSyncOffset" | "SetInputAudioMonitorType" | "SetCurrentProgramScene" => json!({}),
                other => panic!("unexpected OBS request {other}"),
            };
             drop(state);
             if method == "SetSceneItemTransform" && dir.join("hold-scene-ack").exists() {
                 fs::write(dir.join("scene-transform-committed"), "OBS transform accepted").unwrap();
                 let end = Instant::now() + LIMIT;
                 while dir.join("hold-scene-ack").exists() && Instant::now() < end {
                     thread::sleep(Duration::from_millis(10));
                 }
             }
             if method == "StartRecord" && dir.join("hold-start-ack").exists() {
                 fs::write(dir.join("start-committed"), "output active; acknowledgment pending").unwrap();
                 let end = Instant::now() + LIMIT;
                 while dir.join("hold-start-ack").exists() && Instant::now() < end {
                     thread::sleep(Duration::from_millis(10));
                 }
             }
             if method == "StopRecord" && dir.join("hold-stop-ack").exists() {
                 let hold = dir.join("hold-stop-ack");
                 let end = Instant::now() + LIMIT;
                 while hold.exists() && Instant::now() < end {
                     thread::sleep(Duration::from_millis(10));
                 }
             }
             if socket.send(Message::Text(json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":result,"code":if result {100} else {600}},"responseData":data}}).to_string().into())).is_err() { return; }
        }
        });
        }
    });
}

fn scene_args(scene: &Value, operation: &str) -> Value {
    json!({"generation":scene["generation"],"operation":operation})
}

fn scene_edit(scene: &Value, operation: &str, layout: &Value) -> Value {
    let mut args = scene_args(scene, operation);
    args["layout"] = layout.clone();
    args
}

#[test]
fn studio_scene_edits_observe_partial_obs_effects_and_persist_only_verified_layouts() {
    let mut fixture = Fixture::with_options(false, None, true);
    let agent = &fixture.production;
    let root = fixture.root.join("production");
    let layout_file = root.join("state/production/studio-scene-v1.json");
    let fault = root.join("scene-fault");
    let initial = agent.ok("scene", json!({}));
    assert_eq!(initial["mode"], "draft");
    assert_eq!(initial["draft"]["overlay"]["text"], "legacy overlay");
    assert_eq!(
        initial["draft"]["camera"],
        json!({"x":952,"y":8,"width":320,"height":180,"visible":true})
    );
    assert!(initial["sources"].is_null());
    assert!(splitcast_rs_core::control::request(
        agent.port,
        agent.port,
        "wrong-local-bearer",
        "scene-apply",
        &scene_edit(&initial, "unauthenticated", &initial["draft"]).to_string(),
        LIMIT
    )
    .is_err());
    assert_eq!(
        agent.command("preview-small", json!({})).1["outcome"],
        "not_executed"
    );
    let mut layout = initial["draft"].clone();
    layout["camera"]["visible"] = false.into();
    layout["overlay"] = json!({"text":"画面","x":24,"y":32});
    let rejected = agent.command("scene-apply", {
        let mut args = scene_edit(&initial, "old-owner", &layout);
        args["_owner"] = json!("released-owner");
        args
    });
    assert_eq!(rejected.1["outcome"], "not_executed");

    let (first, second) = thread::scope(|scope| {
        let a =
            scope.spawn(|| agent.command("scene-apply", scene_edit(&initial, "racing-a", &layout)));
        let b =
            scope.spawn(|| agent.command("scene-apply", scene_edit(&initial, "racing-b", &layout)));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert!(matches!((first.0, second.0), (200, 409) | (409, 200)));
    let draft = agent.ok("scene", json!({}));
    assert_eq!(draft["draft"], layout);
    assert_eq!(draft["operation"]["state"], "draft");
    assert_eq!(draft["persistence"], "unsaved");
    assert!(!layout_file.exists());
    assert_eq!(
        agent
            .command("scene-save", scene_args(&draft, "pre-start-save"))
            .1["outcome"],
        "not_executed"
    );

    let owner = agent.idle_owner(1);
    agent.ok("prepare", json!({"_owner":owner}));
    agent.ok("start", json!({"_owner":owner}));
    let before = agent.ok("scene", json!({}));
    assert_ne!(before["generation"], draft["generation"]);
    assert_eq!(before["mode"], "live");
    assert_eq!(before["applied"], layout);
    assert_eq!(
        before["sources"],
        json!({"game":{"width":1278,"height":720},"camera":{"width":320,"height":180}})
    );
    assert_eq!(
        agent
            .command("scene-apply", scene_edit(&before, "unowned", &layout))
            .1["outcome"],
        "not_executed"
    );
    let with_owner = |mut args: Value| {
        args["_owner"] = json!(owner);
        args
    };
    for name in ["preview", "preview-small"] {
        assert!(agent.ok(name, json!({}))["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }
    fs::write(&fault, "oversize-preview").unwrap();
    assert_ne!(agent.command("preview-small", json!({})).0, 200);
    assert_eq!(agent.command("preview", json!({})).0, 200);

    layout["camera"]["x"] = 128.into();
    layout["camera"]["visible"] = true.into();
    layout["overlay"]["text"] = "changed".into();
    fs::write(&fault, "lose-transform-reply").unwrap();
    let lost = agent.command(
        "scene-apply",
        with_owner(scene_edit(&before, "lost", &layout)),
    );
    assert_eq!(lost.1["outcome"], "unknown");
    let uncertain = agent.ok("scene", json!({}));
    assert_eq!(
        uncertain["operation"],
        json!({"id":"lost","state":"unknown"})
    );
    assert!(uncertain["applied"].is_null());
    assert_eq!(uncertain["draft"], layout);
    assert_eq!(
        agent
            .command(
                "scene-apply",
                with_owner(scene_edit(&before, "stale", &layout))
            )
            .1["outcome"],
        "not_executed"
    );
    assert_eq!(
        agent
            .command(
                "scene-save",
                with_owner(scene_args(&uncertain, "unsafe-save"))
            )
            .1["outcome"],
        "not_executed"
    );
    let observed = agent.ok(
        "scene-reconcile",
        with_owner(scene_args(&uncertain, "read-lost")),
    );
    assert_eq!(observed["applied"]["camera"]["x"], 128);
    assert_eq!(observed["applied"]["camera"]["visible"], false);
    assert_eq!(observed["applied"]["overlay"]["text"], "画面");

    layout["camera"]["x"] = 144.into();
    fs::write(&fault, "reject-visibility").unwrap();
    let partial = agent.command(
        "scene-apply",
        with_owner(scene_edit(&observed, "partial", &layout)),
    );
    assert_eq!(partial.1["outcome"], "unknown");
    let state = agent.ok("scene", json!({}));
    let observed = agent.ok(
        "scene-reconcile",
        with_owner(scene_args(&state, "read-partial")),
    );
    assert_eq!(observed["applied"]["camera"]["x"], 144);
    assert_eq!(observed["applied"]["camera"]["visible"], false);
    assert_eq!(observed["operation"]["state"], "reconciled");

    let applied = agent.ok(
        "scene-apply",
        with_owner(scene_edit(&observed, "explicit-edit", &layout)),
    );
    assert_eq!(applied["applied"], layout);
    assert_eq!(applied["persistence"], "unsaved");
    fs::create_dir(&layout_file).unwrap();
    assert_ne!(
        agent
            .command("scene-save", with_owner(scene_args(&applied, "save-fail")))
            .0,
        200
    );
    let failing_save = agent.ok("scene", json!({}));
    assert_eq!(
        failing_save["operation"],
        json!({"id":"save-fail","state":"failed"})
    );
    assert_eq!(failing_save["applied"], layout);
    assert_eq!(failing_save["persistence"], "unsaved");
    assert!(layout_file.is_dir());
    fs::remove_dir(&layout_file).unwrap();

    let saved = agent.ok("scene-save", with_owner(scene_args(&failing_save, "save")));
    assert_eq!(saved["saved"], layout);
    assert_eq!(saved["persistence"], "saved");
    assert_eq!(
        fs::metadata(&layout_file).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let pids = fixture.pids();
    agent.ok("stop", json!({"_owner":owner}));
    fixture.released(&pids);
    let stopped = agent.ok("scene", json!({}));
    assert_ne!(stopped["generation"], saved["generation"]);
    assert!(stopped["applied"].is_null());
    assert_eq!(stopped["draft"], layout);
    let mut disposable = layout.clone();
    disposable["overlay"] = json!({"text":null,"x":16,"y":16});
    let edited = agent.ok(
        "scene-apply",
        scene_edit(&stopped, "draft-only", &disposable),
    );
    let reloaded = agent.ok("scene-reload", scene_args(&edited, "reload"));
    assert_eq!(reloaded["draft"], layout);
    assert_eq!(reloaded["persistence"], "saved");

    fixture.production.shutdown();
    fixture.production = Agent::spawn(&root, fixture.ports[1], fixture.ports[6], fixture.ports[7]);
    let restarted = fixture.production.ok("scene", json!({}));
    assert_ne!(restarted["generation"], reloaded["generation"]);
    assert_eq!(restarted["draft"], layout);
    assert_eq!(restarted["saved"], layout);
    assert_eq!(restarted["persistence"], "saved");
}

#[test]
fn production_held_scene_edit() {
    // The long arm pins the baseline false stop. When sampling is independent
    // of the effect owner, both edits must leave a progressing recording.
    for (hold_ms, expected) in [(100, "running"), (3500, "failed")] {
        let mut fixture = Fixture::with_options(false, None, true);
        let agent = &fixture.production;
        agent.ok("prepare", json!({}));
        agent.ok("start", json!({}));
        let scene = agent.ok("scene", json!({}));
        let mut layout = scene["draft"].clone();
        layout["camera"]["x"] = json!(128);
        let dir = fixture.root.join("production");
        let hold = dir.join("hold-scene-ack");
        let committed = dir.join("scene-transform-committed");
        fs::write(&hold, "hold").unwrap();
        thread::scope(|scope| {
            let edit = scope
                .spawn(|| agent.command("scene-apply", scene_edit(&scene, "held-edit", &layout)));
            let end = Instant::now() + Duration::from_secs(4);
            while !committed.exists() && Instant::now() < end {
                thread::sleep(Duration::from_millis(5));
            }
            let accepted = committed.exists();
            if accepted {
                thread::sleep(Duration::from_millis(hold_ms));
            }
            fs::remove_file(hold).unwrap();
            assert!(accepted, "scene edit did not reach its OBS write");
            let (code, result) = edit.join().unwrap();
            assert_eq!(code, 200, "{result}");
            assert_eq!(result["operation"]["state"], "applied");
        });

        let pids = fixture.pids();
        assert!(!pids.is_empty());
        let end = Instant::now() + Duration::from_secs(4);
        let mut bytes = agent.ok("status", json!({}))["output_progress"]["bytes"].as_u64();
        let mut advanced = 0;
        let status = loop {
            let status = agent.ok("status", json!({}));
            if expected == "failed" && status["state"] == "failed" {
                assert_eq!(status["children"], json!([]));
                assert!(status["output"].is_null());
                assert!(status["error"]
                    .as_str()
                    .unwrap()
                    .contains("stopped progressing"));
                fixture.released(&pids);
                break status;
            }
            if expected == "running" {
                assert_eq!(status["state"], "running", "{status}");
                let current = status["output_progress"]["bytes"].as_u64().unwrap();
                if bytes.is_none_or(|previous| current > previous) {
                    bytes = Some(current);
                    advanced += 1;
                    if advanced == 2 {
                        break status;
                    }
                }
            }
            assert!(
                Instant::now() < end,
                "hold_ms={hold_ms} expected {expected}: {status}"
            );
            thread::sleep(Duration::from_millis(20));
        };
        println!(
            "hold_ms={hold_ms} edit=applied state={} later_samples={advanced} error={}",
            status["state"], status["error"]
        );
        agent.ok("stop", json!({}));
        fixture.released(&pids);
        fixture.capture.shutdown();
        fixture.production.shutdown();
    }
}

#[test]
fn studio_scene_proxy_preserves_stop_under_withheld_obs_acknowledgment() {
    let mut fixture = Fixture::new();
    fixture.capture.ok("prepare", json!({}));
    assert_eq!(fixture.capture.ok("chat", json!({}))["enabled"], false);
    let ready = fixture.capture.ok("scene", json!({}));
    assert_eq!(ready["mode"], "draft");
    fixture.capture.ok("start", json!({}));
    let live = fixture.capture.ok("scene", json!({}));
    assert_eq!(live["mode"], "live");
    assert_ne!(live["generation"], ready["generation"]);
    assert_eq!(
        fixture.capture.ok("preview-small", json!({}))["image"],
        fixture.production.ok("preview-small", json!({}))["image"]
    );

    let dir = fixture.root.join("production");
    let hold = dir.join("hold-scene-ack");
    fs::write(&hold, "hold").unwrap();
    let mut layout = live["draft"].clone();
    layout["overlay"] = json!({"text":"pending","x":32,"y":16});
    let port = fixture.capture.port;
    let token = fixture.capture.token.clone();
    let args = scene_edit(&live, "pending-edit", &layout).to_string();
    let edit = thread::spawn(move || {
        splitcast_rs_core::control::request(port, port, &token, "scene-apply", &args, LIMIT)
    });
    let end = Instant::now() + Duration::from_secs(4);
    while !dir.join("scene-transform-committed").exists() {
        assert!(
            Instant::now() < end,
            "scene edit did not reach its OBS write"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let pids = fixture.pids();
    let requested = Instant::now();
    let (code, ack) = fixture.capture.command("stop", json!({}));
    assert!(matches!(code, 200 | 202), "{ack}");
    assert!(requested.elapsed() <= Duration::from_millis(500));
    let error = edit.join().unwrap().unwrap_err();
    assert!(error.to_string().contains("outcome unknown"), "{error}");
    fs::remove_file(hold).unwrap();
    let stopped = fixture.capture.wait_for_stop();
    assert_eq!(stopped["stop_id"], ack["stop_id"]);
    assert_eq!(
        fixture.production.state("idle")["obs_shutdown"]["output_inactive_confirmed"],
        true
    );
    fixture.released(&pids);
    let after = fixture.production.ok("scene", json!({}));
    assert_ne!(after["generation"], live["generation"]);
    assert_eq!(after["mode"], "draft");
    assert!(after["applied"].is_null());
    assert!(after["saved"].is_null());
    fixture.capture.shutdown();
    fixture.production.shutdown();
}
