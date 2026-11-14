use super::*;
use crate::server::{Limits, ListenerOwner};
use std::io::Write;
use std::net::{TcpListener, TcpStream};

const TOKEN: &str = "lifecycle-test-token-long-enough";
struct Fixture(config::Config);
impl Fixture {
    fn new() -> Self {
        let mut cfg = config::Config::parse(include_bytes!("../tests/agent-example.json")).unwrap();
        cfg.role = "production".into();
        cfg.state_dir = std::env::temp_dir().join(format!(
            "splitcast-lifecycle-{}-{}",
            std::process::id(),
            rand_hex().unwrap()
        ));
        crate::state::private_directory(&cfg.state_dir).unwrap();
        cfg.token_file = cfg.state_dir.join("token");
        crate::state::atomic_file(&cfg.token_file, TOKEN).unwrap();
        Self(cfg)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0.state_dir).unwrap();
    }
}
fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}
fn assets() -> Arc<crate::web::Assets> {
    Arc::new(crate::web::Assets::load(None).unwrap())
}
fn send(port: u16, command: &str, args: Value) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let body = json!({"command":command,"args":args}).to_string();
    write!(stream, "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stream
}
fn response(mut stream: TcpStream) -> Value {
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap()
}
struct Running {
    cancel: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<io::Result<()>>>,
}
impl Running {
    fn join(mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[test]
fn expired_and_abandoned_queue_entries_never_write_settings() {
    let fixture = Fixture::new();
    let mut agent = Agent::new(fixture.0.clone()).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let limits = Limits {
        queue: Duration::from_millis(80),
        operation: Duration::from_secs(1),
    };
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let (server, queue) = ListenerOwner::start(
        socket,
        TOKEN.into(),
        1000,
        assets(),
        Arc::clone(&cancel),
        limits,
    )
    .unwrap();
    for abandon in [false, true] {
        let client = send(
            port,
            "routes",
            json!({"settings":{"audio":{"mic_offset_ms":123}}}),
        );
        let work = queue.recv_timeout(Duration::from_secs(2)).unwrap();
        if abandon {
            assert!(work.ticket.cancel_queued());
        }
        let reply = response(client);
        assert_eq!(reply["outcome"], "not_executed");
        agent.execute_work(work, &cancel, limits);
        assert!(!agent.cfg.state_dir.join("settings.json").exists());
        assert_ne!(agent.cfg.mic_offset, 123);
    }
    server.join().unwrap();
    drop(agent);
}

#[test]
fn started_request_timeout_reports_unknown_and_cancels_next_effect() {
    let fixture = Fixture::new();
    let mut agent = Agent::new(fixture.0.clone()).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let limits = Limits {
        queue: Duration::from_millis(80),
        operation: Duration::from_millis(80),
    };
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let (server, queue) = ListenerOwner::start(
        socket,
        TOKEN.into(),
        1000,
        assets(),
        Arc::clone(&cancel),
        limits,
    )
    .unwrap();
    let client = send(
        port,
        "routes",
        json!({"settings":{"audio":{"mic_offset_ms":123}}}),
    );
    let work = queue.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(work.ticket.begin());
    agent.operation = Some(crate::request::Operation::new(
        Arc::clone(&cancel),
        Some(Arc::clone(&work.ticket)),
        Duration::from_secs(5),
    ));
    assert_eq!(response(client)["outcome"], "unknown");
    assert!(agent.execute(work.invocation).is_err());
    assert!(!agent.cfg.state_dir.join("settings.json").exists());
    server.join().unwrap();
    drop(agent);
}

#[test]
fn repeated_server_instances_close_partial_requests_and_release_role_lock() {
    let fixture = Fixture::new();
    for _ in 0..2 {
        let socket = listener();
        let port = socket.local_addr().unwrap().port();
        let cfg = fixture.0.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::clone(&cancel);
        let server = Running {
            cancel: Arc::clone(&cancel),
            thread: Some(thread::spawn(move || {
                serve_bound(
                    cfg,
                    socket,
                    TOKEN.into(),
                    assets(),
                    shutdown,
                    Limits::default(),
                )
            })),
        };
        assert_eq!(response(send(port, "status", json!({})))["state"], "idle");
        let mut partial = TcpStream::connect(("127.0.0.1", port)).unwrap();
        partial
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        partial.write_all(b"POST /api/").unwrap();
        cancel.store(true, Ordering::Relaxed);
        server.join();
        let mut byte = [0];
        match partial.read(&mut byte) {
            Ok(0) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
                ) => {}
            result => panic!("partial request socket remained open: {result:?}"),
        }
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    }
}

#[test]
fn readiness_shutdown_stops_before_the_next_worker_spawn() {
    let fixture = Fixture::new();
    let mut agent = Agent::new(fixture.0.clone()).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    agent.operation = Some(crate::request::Operation::new(
        Arc::clone(&cancel),
        None,
        Duration::from_secs(5),
    ));
    let service = TcpListener::bind("127.0.0.1:0").unwrap();
    agent.cfg.api = service.local_addr().unwrap().port();
    let shutdown = Arc::clone(&cancel);
    let delayed = thread::spawn(move || {
        let (mut stream, _) = service.accept().unwrap();
        let mut bytes = [0; 4096];
        assert!(stream.read(&mut bytes).unwrap() > 0);
        shutdown.store(true, Ordering::Relaxed);
        thread::sleep(Duration::from_millis(80));
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    });
    let result =
        agent.wait_ready(|| media::receive_operation(&agent.cfg, agent.operation.as_ref()).is_ok());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert!(agent
        .spawn(Worker::Receiver, vec!["sleep".into(), "30".into()])
        .is_err());
    assert!(agent.children.names().is_empty());
    delayed.join().unwrap();
    drop(agent);
}

#[test]
fn shutdown_during_delayed_prepare_closes_connections_and_stops_receiver() {
    use std::os::unix::fs::PermissionsExt;
    const CHILD: &str = "SPLITCAST_LIFECYCLE_FIXTURE";
    if std::env::var_os(CHILD).is_none() {
        let fixture = Fixture::new();
        let bin = fixture.0.state_dir.join("bin");
        fs::create_dir(&bin).unwrap();
        for name in ["ffmpeg", "ffprobe", "pactl", "mediamtx", "pulseaudio"] {
            let script = bin.join(name);
            fs::write(&script, "#!/bin/sh\nprintf '%s' \"$$\" > \"$SPLITCAST_LIFECYCLE_FIXTURE/receiver.pid\"\nexec sleep 30\n").unwrap();
            fs::set_permissions(script, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "agent::lifecycle_tests::shutdown_during_delayed_prepare_closes_connections_and_stops_receiver", "--nocapture"])
            .env(CHILD, &fixture.0.state_dir)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output().unwrap();
        assert!(
            output.status.success(),
            "child fixture failed\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let pid = fs::read_to_string(fixture.0.state_dir.join("receiver.pid")).unwrap();
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "owned receiver survived shutdown"
        );
        return;
    }
    let mut fixture = Fixture::new();
    let service = TcpListener::bind("127.0.0.1:0").unwrap();
    fixture.0.api = service.local_addr().unwrap().port();
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let cancel = Arc::new(AtomicBool::new(false));
    let cfg = fixture.0.clone();
    let shutdown = Arc::clone(&cancel);
    let server = Running {
        cancel: Arc::clone(&cancel),
        thread: Some(thread::spawn(move || {
            serve_bound(
                cfg,
                socket,
                TOKEN.into(),
                assets(),
                shutdown,
                Limits::default(),
            )
        })),
    };
    let mut prepare = send(port, "prepare", json!({}));
    service.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    let (mut delayed, _) = loop {
        match service.accept() {
            Ok(connection) => break connection,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < end => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(e) => panic!("Prepare did not reach receiver readiness: {e}"),
        }
    };
    delayed
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = [0; 4096];
    assert!(delayed.read(&mut request).unwrap() > 0);
    let mut stop = send(port, "stop", json!({}));
    stop.set_read_timeout(Some(Duration::from_millis(80)))
        .unwrap();
    let mut byte = [0];
    assert!(
        matches!(stop.read(&mut byte), Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)),
        "queued Stop unexpectedly completed during delayed Prepare"
    );
    cancel.store(true, Ordering::Relaxed);
    let _ =
        delayed.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    drop(delayed);
    server.join();
    for stream in [&mut prepare, &mut stop] {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut received = Vec::new();
        if let Err(error) = stream.read_to_end(&mut received) {
            assert!(matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ));
        }
    }
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    drop(Agent::new(fixture.0.clone()).unwrap());
}
