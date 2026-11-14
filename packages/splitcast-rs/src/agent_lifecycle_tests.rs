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
    send_as(port, command, args, TOKEN)
}
fn send_as(port: u16, command: &str, args: Value, token: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let body = json!({"command":command,"args":args}).to_string();
    write!(stream, "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stream
}
fn response(mut stream: TcpStream) -> Value {
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap()
}
fn get_status(port: u16) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(stream, "GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n").unwrap();
    stream
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
        ..Limits::default()
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
        agent.session.control(),
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
        ..Limits::default()
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
        agent.session.control(),
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
fn heartbeat_and_expiry_progress_while_the_effect_owner_does_not_drain_work() {
    let fixture = Fixture::new();
    let mut agent = Agent::new(fixture.0.clone()).unwrap();
    let owner = OwnerId::parse("live-owner").unwrap();
    agent
        .session
        .prepare(
            Some(owner.clone()),
            Instant::now(),
            Duration::from_millis(150),
        )
        .unwrap();
    agent.session.prepared().unwrap();
    let control = agent.session.control();
    let cancel = Arc::new(AtomicBool::new(false));
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let (server, queue) = ListenerOwner::start(
        socket,
        TOKEN.into(),
        1000,
        assets(),
        Arc::clone(&cancel),
        Limits {
            lease: Duration::from_millis(250),
            ..Limits::default()
        },
        control.clone(),
    )
    .unwrap();
    // The effect owner deliberately never receives from this queue. Renewal
    // must still take effect on the canonical lease, not on a status mirror.
    let queued = send(
        port,
        "start",
        json!({"_owner":"live-owner","output":"recording"}),
    );
    for _ in 0..4 {
        thread::sleep(Duration::from_millis(60));
        assert_eq!(
            response(send(port, "heartbeat", json!({"_owner":"live-owner"})))["renewed"],
            true
        );
        assert!(!agent.session.lease_expired(Instant::now()));
    }
    assert_eq!(
        response(send(port, "heartbeat", json!({"_owner":"intruder"})))["outcome"],
        "not_executed"
    );
    assert_eq!(agent.session.state(), Lifecycle::Ready);
    assert!(agent.session.output().is_none());
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        response(send(port, "heartbeat", json!({"_owner":"live-owner"})))["outcome"],
        "not_executed"
    );
    assert_eq!(
        response(send(port, "stop", json!({"_owner":"live-owner"})))["outcome"],
        "not_executed"
    );
    let work = queue.recv_timeout(Duration::from_secs(1)).unwrap();
    agent.execute_work(work, &cancel, Limits::default());
    assert_eq!(response(queued)["outcome"], "not_executed");
    assert!(agent.session.start(Output::Recording).is_err());
    agent.stop(None).unwrap();
    control.finish_stop((200, agent.status()));
    assert_eq!(agent.session.state(), Lifecycle::Idle);
    assert!(agent.children.names().is_empty());
    server.join().unwrap();
}

#[test]
fn completed_http_response_closes_even_when_listener_reaping_is_delayed() {
    let fixture = Fixture::new();
    let agent = Agent::new(fixture.0.clone()).unwrap();
    let control = agent.session.control();
    let cancel = Arc::new(AtomicBool::new(false));
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let (server, queue) = ListenerOwner::start(
        socket,
        TOKEN.into(),
        1000,
        assets(),
        cancel,
        Limits::default(),
        control.clone(),
    )
    .unwrap();
    let mut client = send(port, "status", json!({}));
    client
        .set_read_timeout(Some(Duration::from_millis(150)))
        .unwrap();
    let work = queue.recv_timeout(Duration::from_secs(1)).unwrap();
    let (locked, ready) = std::sync::mpsc::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let mut received = String::new();
    let result = thread::scope(|scope| {
        let generation = control.generation();
        let held = scope.spawn(move || {
            // Deliberately suspend listener progress, not a production effect.
            // Production dispatch closures contain only a nonblocking syscall.
            control
                .dispatch(generation, || {
                    locked.send(()).unwrap();
                    resume.recv_timeout(Duration::from_secs(2)).unwrap();
                    Ok(())
                })
                .unwrap();
        });
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        thread::sleep(Duration::from_millis(30));
        work.ticket.finish((200, r#"{"state":"idle"}"#.into()));
        let result = client.read_to_string(&mut received);
        release.send(()).unwrap();
        held.join().unwrap();
        result
    });
    server.join().unwrap();
    assert!(
        result.is_ok(),
        "complete HTTP response waited for listener reaping: {result:?}; received {received:?}"
    );
    assert!(received.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(received.ends_with(r#"{"state":"idle"}"#));
}

#[test]
fn stop_acknowledges_during_failure_cleanup_before_remote_ack_is_released() {
    let mut fixture = Fixture::new();
    fixture.0.role = "capture".into();
    fixture.0.remote_token_file = fixture.0.token_file.clone();
    fixture.0.timeout = 10;
    let peer = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer_port = peer.local_addr().unwrap().port();
    fixture.0.forward_control_port = peer_port;
    fixture.0.remote_control_port = peer_port;
    let mut agent = Agent::new(fixture.0.clone()).unwrap();
    agent
        .session
        .prepare(None, Instant::now(), Duration::from_secs(30))
        .unwrap();
    agent
        .session
        .record_remote_attempt(OwnerId::parse("failure-cleanup-owner").unwrap())
        .unwrap();
    agent.session.prepared().unwrap();
    agent.session.start(Output::Recording).unwrap();
    let marker = agent.cfg.state_dir.join("tunnel.pid");
    agent
        .spawn(
            Worker::Tunnel,
            vec![
                "sh".into(),
                "-c".into(),
                "printf %s $$ > \"$1\"; exec sleep 30".into(),
                "stand-in".into(),
                marker.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
    agent.spawn(Worker::Game, vec!["true".into()]).unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while !marker.exists() || !agent.children.any_exited() {
        assert!(Instant::now() < end, "owned worker fault was not observed");
        thread::sleep(Duration::from_millis(5));
    }
    let pid = fs::read_to_string(&marker).unwrap();
    let control = agent.session.control();
    let cancel = Arc::new(AtomicBool::new(false));
    let probe = crate::request::Operation::new(Arc::clone(&cancel), None, Duration::from_secs(10))
        .with_control(control.clone(), control.generation());
    agent.operation = Some(probe.clone());
    let socket = listener();
    let port = socket.local_addr().unwrap().port();
    let (server, queue) = ListenerOwner::start(
        socket,
        TOKEN.into(),
        1000,
        assets(),
        cancel,
        Limits::default(),
        control.clone(),
    )
    .unwrap();
    let (entered, boundary) = std::sync::mpsc::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let (
        agent,
        ack,
        status,
        admission,
        unauthorized_did_not_cancel,
        intent_observation,
        cleanup_started,
    ) = thread::scope(|scope| {
        let peer = scope.spawn(move || {
            let (mut socket, _) = peer.accept().unwrap();
            let request = control::recv(&mut socket, peer_port, TOKEN).unwrap();
            let request: Value = serde_json::from_str(&request.body).unwrap();
            assert_eq!(request["command"], "stop");
            assert_eq!(request["args"]["_owner"], "failure-cleanup-owner");
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(5)).unwrap();
            control::reply(
                &mut socket,
                200,
                r#"{"state":"idle","stop":"completed","owner":"failure-cleanup-owner"}"#,
            )
            .unwrap();
        });
        let owner = scope.spawn(move || {
            // Exact owner call boundary: tick -> exited worker -> fail ->
            // cleanup_owned -> remote Stop -> withheld acknowledgment.
            agent.last_heartbeat = Instant::now();
            agent.tick();
            assert!(control.stop_pending());
            agent.stop(None).unwrap();
            let mut body: Value = serde_json::from_str(&agent.status()).unwrap();
            body["stop"] = json!("completed");
            body["outcome"] = json!("completed");
            control.finish_stop((200, body.to_string()));
            agent
        });
        boundary.recv_timeout(Duration::from_secs(2)).unwrap();
        let refused = response(send(port, "stop", json!({"_owner":"intruder"})));
        let unauthorized_did_not_cancel =
            refused["outcome"] == "not_executed" && probe.check().is_ok();
        let started = Instant::now();
        let mut client = send(port, "stop", json!({}));
        client
            .set_read_timeout(Some(Duration::from_millis(450)))
            .unwrap();
        let intent_deadline = started + Duration::from_millis(450);
        while probe.check().is_ok() && Instant::now() < intent_deadline {
            thread::sleep(Duration::from_millis(1));
        }
        let intent_observation = (probe.check().is_err(), started.elapsed());
        let mut ack = String::new();
        let read = client.read_to_string(&mut ack);
        let admission = started.elapsed();
        let mut snapshot = get_status(port);
        snapshot
            .set_read_timeout(Some(Duration::from_millis(450)))
            .unwrap();
        let mut status = String::new();
        let status_read = snapshot.read_to_string(&mut status);
        release.send(()).unwrap();
        peer.join().unwrap();
        let agent = owner.join().unwrap();
        (
            agent,
            (read, ack),
            (status_read, status),
            admission,
            unauthorized_did_not_cancel,
            intent_observation,
            started,
        )
    });
    server.join().unwrap();
    drop(queue);
    assert_eq!(agent.session.state(), Lifecycle::Idle);
    assert!(agent.children.names().is_empty());
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let cleanup = cleanup_started.elapsed();
    assert!(unauthorized_did_not_cancel);
    assert!(
        intent_observation.0,
        "authorized intent was not observed while cleanup was blocked"
    );
    assert!(
        ack.0.is_ok(),
        "tick failure cleanup blocked HTTP acknowledgment: {:?}; bytes {:?}",
        ack.0,
        ack.1
    );
    assert!(
        admission <= Duration::from_millis(500),
        "failure-cleanup Stop acknowledgment took {admission:?}"
    );
    assert!(ack.1.starts_with("HTTP/1.1 202 Accepted\r\n"), "{}", ack.1);
    assert!(
        intent_observation.1 <= admission,
        "intent must be observed independently before reading its acknowledgment"
    );
    assert!(
        status.0.is_ok(),
        "status was hidden behind the blocked effect owner: {:?}",
        status.0
    );
    let status: Value = serde_json::from_str(status.1.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(status["stop"], "requested");
    assert_eq!(status["state"], "stopping");
    assert_eq!(status["remote_release_pending"], true);
    assert!(
        status["children"].is_null(),
        "pending snapshot must not fabricate an empty worker list"
    );
    assert_eq!(status["resource_observation"], "unknown");
    eprintln!("failure cleanup Stop intent observed: {:?}; original Operation revoked before acknowledgment read", intent_observation.1);
    eprintln!("failure cleanup Stop acknowledgment: {admission:?}; remote ACK withheld until after acknowledgment and status observation");
    eprintln!("failure cleanup Stop cleanup observed after {cleanup:?}; owned tunnel PID absent and session Idle");
}

#[test]
fn shutdown_during_delayed_prepare_closes_connections_and_stops_receiver() {
    delayed_prepare_control_case("prepare", "agent::lifecycle_tests::shutdown_during_delayed_prepare_closes_connections_and_stops_receiver");
}

#[test]
fn stop_during_implicit_prepare_of_start_prevents_output() {
    delayed_prepare_control_case(
        "start",
        "agent::lifecycle_tests::stop_during_implicit_prepare_of_start_prevents_output",
    );
}

fn delayed_prepare_control_case(command: &str, test_name: &str) {
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
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD, &fixture.0.state_dir)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child fixture failed\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
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
    let args = if command == "prepare" {
        json!({"_owner":"lease-owner"})
    } else {
        json!({})
    };
    let mut prepare = send(port, command, args.clone());
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
    assert_eq!(
        response(send_as(port, "stop", json!({}), "invalid-token"))["error"],
        "unauthorized or malformed request"
    );
    let wrong = response(send(port, "stop", json!({"_owner":"intruder"})));
    assert_eq!(wrong["outcome"], "not_executed");
    prepare
        .set_read_timeout(Some(Duration::from_millis(80)))
        .unwrap();
    let mut byte = [0];
    assert!(
        matches!(prepare.read(&mut byte), Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)),
        "wrong-owner Stop cancelled Prepare"
    );
    prepare
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let queued: Vec<_> = (0..6)
        .map(|_| {
            let mut queued_args = args.clone();
            queued_args["output"] = json!("recording");
            send(port, "start", queued_args)
        })
        .collect();
    thread::sleep(Duration::from_millis(100));
    let admitted = Instant::now();
    let stop = response(send(port, "stop", args.clone()));
    let admission = admitted.elapsed();
    assert_eq!(
        stop["stop"], "requested",
        "Stop must distinguish admission from cleanup"
    );
    assert!(
        admission <= Duration::from_millis(500),
        "Stop admission took {admission:?}"
    );
    eprintln!("delayed Prepare Stop intent admission: {admission:?}");
    assert_eq!(response(send(port, "stop", json!({})))["stop"], "requested");
    thread::sleep(Duration::from_millis(600));
    let _ =
        delayed.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    drop(delayed);
    assert_eq!(response(prepare)["outcome"], "unknown");
    let mut refused_capacity = false;
    let mut fenced = false;
    for client in queued {
        let reply = response(client);
        refused_capacity |= reply["error"]
            .as_str()
            .is_some_and(|e| e.contains("capacity"));
        fenced |= reply["error"]
            .as_str()
            .is_some_and(|e| e.contains("superseded"));
        assert_eq!(
            reply["outcome"], "not_executed",
            "pre-Stop queued Start escaped the fence"
        );
    }
    assert!(
        refused_capacity && fenced,
        "fixture must exercise both saturation and old queued commands"
    );
    let status = response(send(port, "status", json!({})));
    assert_eq!(status["state"], "idle", "{status}");
    assert_eq!(status["children"], json!([]));
    assert!(status["output"].is_null());
    let marker = std::path::PathBuf::from(std::env::var_os(CHILD).unwrap()).join("receiver.pid");
    let pid = fs::read_to_string(marker).unwrap();
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "receiver still exists at cleanup timestamp"
    );
    eprintln!(
        "delayed Prepare Stop cleanup observed after {:?} (includes deliberate 600 ms reply hold)",
        admitted.elapsed()
    );
    let mut partial = TcpStream::connect(("127.0.0.1", port)).unwrap();
    partial.write_all(b"POST /api/").unwrap();
    server.join();
    {
        partial
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut received = Vec::new();
        if let Err(error) = partial.read_to_end(&mut received) {
            assert!(matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ));
        }
    }
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    drop(Agent::new(fixture.0.clone()).unwrap());
}
