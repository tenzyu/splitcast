#![cfg(target_os = "linux")]

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
        let role = dir.file_name().unwrap().to_str().unwrap();
        let mut agent = Self {
            child: Command::new(env!("CARGO_BIN_EXE_splitcast-rs-core"))
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
            return self.wait_for_stop();
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
        let root = std::env::current_dir().unwrap().join(format!(
            "../../artifacts/verification/two-agents-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.parent().unwrap()).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let reservations: Vec<_> = (0..10)
            .map(|_| TcpListener::bind("127.0.0.1:0").unwrap())
            .collect();
        let ports: Vec<_> = reservations
            .iter()
            .map(|l| l.local_addr().unwrap().port())
            .collect();
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
        drop(reservations);
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
    let first = "first-observer-owner";
    let second = "second-observer-owner";
    f.production.ok("prepare", json!({"_owner":first}));
    f.production
        .ok("start", json!({"_owner":first,"output":"recording"}));
    let pids = f.pids();
    assert!(!pids.is_empty());
    // Observe only the 202 status line, then lose the JSON reply. This proves
    // Stop admission before polling, without observing completion or resending.
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
    let end = Instant::now() + LIMIT;
    loop {
        let (code, observation) = f.production.command("status", json!({"_owner":first}));
        assert_eq!(code, 200, "{observation}");
        if observation["stop"] == "completed" {
            assert_eq!(observation["owner"], first);
            assert_eq!(observation["state"], "idle");
            assert_eq!(observation["children"], json!([]));
            break;
        }
        assert!(
            Instant::now() < end,
            "lost-reply Stop did not resolve: {observation}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    f.released(&pids);
    f.production.ok("prepare", json!({"_owner":second}));
    let current = f.production.ok("status", json!({}));
    assert_eq!(current["state"], "ready");
    assert!(
        current["stop"].is_null(),
        "new session fabricated old completion"
    );
    assert!(current["owner"].is_null());
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
    f.production.ok("stop", json!({"_owner":second}));
    let overwritten = f.production.ok("status", json!({"_owner":first}));
    assert_eq!(overwritten["stop"], "completed");
    assert_eq!(overwritten["owner"], second);
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
fn completed_stop_is_not_a_cross_host_prepare_replay_fence() {
    let mut f = Fixture::new();
    let owner = "replayed-prepare-owner";
    f.production.ok("prepare", json!({"_owner":owner}));
    f.production.ok("stop", json!({"_owner":owner}));
    f.released(&f.pids());
    // The bearer still authorizes Prepare. Owner IDs are constraints, not a
    // durable request registry. A later admitted Prepare creates fresh effects.
    // Supported clients must not replay an uncertain Prepare automatically.
    f.production.ok("prepare", json!({"_owner":owner}));
    let fresh = f.production.state("ready");
    assert!(fresh["stop"].is_null());
    assert!(fresh["output"].is_null(), "Prepare alone started output");
    assert!(!fresh["children"].as_array().unwrap().is_empty());
    let pids = f.pids();
    f.production.ok("stop", json!({"_owner":owner}));
    f.released(&pids);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn late_owned_start_cannot_recreate_a_completed_remote_session() {
    let mut f = Fixture::new();
    f.capture.ok("prepare", json!({}));
    f.capture.ok("start", json!({"output":"recording"}));
    f.capture.ok("stop", json!({}));
    let completed = f.production.ok("stop", json!({}));
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
    if body["stop"] == "requested" {
        f.production.wait_for_stop();
    }
    f.production.state("idle");
    f.capture.ok("stop", json!({}));
    f.capture.state("idle");
    f.released(&pids);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn sigterm_running_capture_releases_both_agents_workers() {
    let mut fixture = Fixture::new();
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
                    {"name":"mic","ready":true,"bytesReceived":(n+1)*512,"tracks":["Opus"]}
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

fn virtual_obs(dir: &Path) {
    let port: u16 = std::env::var("SPLITCAST_VIRTUAL_OBS")
        .unwrap()
        .parse()
        .unwrap();
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    let state = Arc::new(Mutex::new((false, 0u64)));
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
            let (active, count) = &mut *state;
            let data = match method {
                "GetVersion" => json!({"obsVersion":"virtual"}),
                "GetSceneList" => json!({"scenes":[{"sceneName":"SplitCast"}]}),
                "GetSceneItemId" => json!({"sceneItemId":1}),
                "GetSceneItemTransform" => json!({"sceneItemTransform":{"sourceWidth":1280,"sourceHeight":720}}),
                "GetInputList" => json!({"inputs":[{"inputName":"GameFeed"},{"inputName":"MicProcessed"},{"inputName":"Overlay"}]}),
                "StartRecord" => { assert!(!*active); *active = true; json!({}) },
                "StopRecord" => { *active = false; json!({}) },
                "GetRecordStatus" | "GetStreamStatus" => {
                    *count += 1;
                    json!({"outputActive":method == "GetRecordStatus" && *active,"outputBytes":*count*1024,"outputTotalFrames":*count*30,"outputDuration":*count*1000})
                }
                "SetInputSettings" | "SetInputAudioSyncOffset" | "SetInputAudioMonitorType" | "SetCurrentProgramScene" => json!({}),
                other => panic!("unexpected OBS request {other}"),
            };
             drop(state);
             if method == "StartRecord" && dir.join("hold-start-ack").exists() {
                 fs::write(dir.join("start-committed"), "output active; acknowledgment pending").unwrap();
                 let end = Instant::now() + LIMIT;
                 while dir.join("hold-start-ack").exists() && Instant::now() < end {
                     thread::sleep(Duration::from_millis(10));
                 }
             }
             if socket.send(Message::Text(json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":data}}).to_string().into())).is_err() { return; }
        }
        });
        }
    });
}
