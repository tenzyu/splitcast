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
}

impl Agent {
    fn command(&self, command: &str, args: Value) -> (u16, Value) {
        let body = json!({"command":command,"args":args}).to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        stream.set_write_timeout(Some(LIMIT)).unwrap();
        write!(stream, "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", self.port, self.port, self.token, body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (code, serde_json::from_str(body).unwrap())
    }

    fn ok(&self, command: &str, args: Value) -> Value {
        let (code, body) = self.command(command, args);
        assert_eq!(code, 200, "{command}: {body}");
        body
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
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
            let end = Instant::now() + LIMIT;
            while Instant::now() < end {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
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

impl Fixture {
    fn new() -> Self {
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
                        "#!{}\nprintf '%s\\n' '[{{\"name\":\"splitcast_mic\"}}]'\n",
                        executable("sh").display()
                    )
                } else {
                    format!("#!{}\nexport SPLITCAST_VIRTUAL_KIND='{name}'\nexec {} --exact virtual_worker --ignored --nocapture\n", executable("sh").display(), quote(&std::env::current_exe().unwrap()))
                };
                let path = tools.join(name);
                fs::write(&path, text).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            private_file(&dir.join("token"), &format!("virtual-{role}-private-token"));
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
                ports[1]
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
            cfg["obs"]["twitch_key_file"] = json!(dir.join("absent-twitch-key"));
            cfg["tts"]["credentials_file"] = json!(dir.join("absent-credentials"));
            cfg["tts"]["voicevox_url"] = json!(format!("http://127.0.0.1:{}", ports[8]));
            fs::write(dir.join("config.json"), cfg.to_string()).unwrap();
        }
        drop(reservations);
        let spawn = |role: &str, port| {
            let dir = root.join(role);
            let mut agent = Agent {
                child: Command::new(env!("CARGO_BIN_EXE_splitcast-rs-core"))
                    .args([
                        "--config",
                        dir.join("config.json").to_str().unwrap(),
                        "agent",
                    ])
                    .env("PATH", dir.join("tools"))
                    .env("SPLITCAST_VIRTUAL_DIR", &dir)
                    .env("SPLITCAST_VIRTUAL_API", ports[6].to_string())
                    .env("SPLITCAST_VIRTUAL_OBS", ports[7].to_string())
                    .env_remove("SPLITCAST_WEB_ASSETS")
                    .stdin(Stdio::null())
                    .stdout(File::create(dir.join("stdout.log")).unwrap())
                    .stderr(File::create(dir.join("stderr.log")).unwrap())
                    .spawn()
                    .unwrap(),
                port,
                token: format!("virtual-{role}-private-token"),
            };
            let end = Instant::now() + LIMIT;
            while TcpStream::connect(("127.0.0.1", port)).is_err() {
                assert!(
                    agent.child.try_wait().unwrap().is_none(),
                    "{role} exited: {}",
                    fs::read_to_string(dir.join("stderr.log")).unwrap()
                );
                assert!(Instant::now() < end, "{role} did not listen");
                thread::sleep(Duration::from_millis(20));
            }
            agent
        };
        let production = spawn("production", ports[1]);
        let capture = spawn("capture", ports[0]);
        println!(
            "two real agents, stand-in workers only; evidence {}",
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
            assert!(
                TcpListener::bind(("127.0.0.1", self.ports[index])).is_ok(),
                "worker port not released"
            );
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
        assert!(
            TcpListener::bind(("127.0.0.1", *port)).is_ok(),
            "port {port} survived agent shutdown"
        );
    }
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
            send(&mut socket, json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":data}}));
        }
        });
        }
    });
}
