use splitcast_rs_core::{process::OwnedProcess, run_bounded};
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: Duration = Duration::from_secs(20);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "splitcast-transport-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Proxy {
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<usize>>>,
}
impl Proxy {
    fn start(listener: TcpListener, destination: SocketAddr, disconnect: bool) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + LIMIT;
            listener.set_nonblocking(true)?;
            let mut source = loop {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "proxy accept cancelled or timed out",
                    ));
                }
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => return Err(e),
                }
            };
            let mut sink = loop {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "proxy connect cancelled or timed out",
                    ));
                }
                match TcpStream::connect_timeout(&destination, Duration::from_millis(100)) {
                    Ok(stream) => break stream,
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::ConnectionRefused | io::ErrorKind::TimedOut
                        ) =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => return Err(e),
                }
            };
            source.set_nonblocking(true)?;
            sink.set_nonblocking(true)?;
            let mut buffer = [0u8; 8192];
            let mut total = 0;
            loop {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "proxy transfer cancelled or timed out",
                    ));
                }
                match source.read(&mut buffer) {
                    Ok(0) => {
                        sink.shutdown(Shutdown::Write)?;
                        return Ok(total);
                    }
                    Ok(n) => {
                        thread::sleep(Duration::from_millis(2));
                        let mut offset = 0;
                        while offset < n {
                            if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                                return Err(io::Error::new(
                                    io::ErrorKind::TimedOut,
                                    "proxy write cancelled or timed out",
                                ));
                            }
                            match sink.write(&buffer[offset..n]) {
                                Ok(0) => {
                                    return Err(io::Error::new(
                                        io::ErrorKind::WriteZero,
                                        "proxy write returned zero",
                                    ))
                                }
                                Ok(written) => offset += written,
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    thread::sleep(Duration::from_millis(5))
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        total += n;
                        if disconnect && total >= 16 * 1024 {
                            source.shutdown(Shutdown::Both)?;
                            sink.shutdown(Shutdown::Both)?;
                            return Ok(total);
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => return Err(e),
                }
            }
        });
        Self {
            cancel,
            thread: Some(thread),
        }
    }
    fn join(&mut self) -> io::Result<usize> {
        self.thread
            .take()
            .unwrap()
            .join()
            .expect("proxy thread panicked")
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_owned()).collect()
}

fn wait(process: &mut OwnedProcess) -> std::process::ExitStatus {
    let deadline = Instant::now() + LIMIT;
    loop {
        if let Some(status) = process.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "ffmpeg did not exit before deadline"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn run(recording: &Path, disconnect: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_address = listener.local_addr().unwrap();
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let receiver_address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut receiver_args = args(&["ffmpeg", "-nostdin", "-v", "error", "-y", "-i"]);
    receiver_args.push(format!("tcp://{receiver_address}?listen=1"));
    receiver_args.extend(args(&[
        "-map", "0:v:0", "-map", "0:a:0", "-c", "copy", "-f", "matroska",
    ]));
    receiver_args.push(recording.to_str().unwrap().into());
    let mut receiver = OwnedProcess::spawn(&receiver_args).unwrap();
    let mut proxy = Proxy::start(listener, receiver_address, disconnect);
    let mut sender_args = args(&[
        "ffmpeg",
        "-nostdin",
        "-v",
        "error",
        "-filter_threads",
        "1",
        "-re",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=320x180:rate=15",
        "-re",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000",
        "-t",
        "2",
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-c:v",
        "libx264",
        "-threads",
        "1",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-f",
        "mpegts",
    ]);
    sender_args.push(format!("tcp://{proxy_address}"));
    let mut sender = OwnedProcess::spawn(&sender_args).unwrap();
    let bytes = proxy.join().expect("proxy transfer failed");
    assert!(bytes >= 16 * 1024, "no substantial media reached the proxy");
    let sender_status = wait(&mut sender);
    let receiver_status = wait(&mut receiver);
    if disconnect {
        assert!(
            !sender_status.success(),
            "sender completed despite disconnect"
        );
    } else {
        assert!(sender_status.success(), "sender failed: {sender_status}");
        assert!(
            receiver_status.success(),
            "receiver failed: {receiver_status}"
        );
    }
    for process in [&mut sender, &mut receiver] {
        let pid = process.id();
        process.stop(Duration::ZERO).unwrap();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "owned process was not reaped"
        );
    }
    assert!(
        TcpStream::connect_timeout(&proxy_address, Duration::from_millis(100)).is_err(),
        "proxy listener survived join"
    );
    assert!(
        TcpStream::connect_timeout(&receiver_address, Duration::from_millis(100)).is_err(),
        "receiver listener survived exit"
    );
}

#[test]
#[ignore = "runs generated media through local TCP and FFmpeg"]
fn generated_transport_disconnect_and_recovery() {
    let fixture = Fixture::new();
    run(&fixture.0.join("interrupted.mkv"), true);
    let recording = fixture.0.join("recovered.mkv");
    run(&recording, false);
    let mut probe_args = args(&[
        "-v",
        "error",
        "-count_packets",
        "-show_entries",
        "stream=codec_name,nb_read_packets:format=duration",
        "-of",
        "json",
    ]);
    probe_args.push(recording.to_str().unwrap().into());
    let output = run_bounded(
        "ffprobe",
        &probe_args,
        None,
        Duration::from_secs(10),
        1024 * 1024,
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let probe: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let streams = probe["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 2);
    assert!(streams
        .iter()
        .any(|s| s["codec_name"] == "h264" && s["nb_read_packets"] == "30"));
    assert!(streams.iter().any(|s| s["codec_name"] == "aac"
        && s["nb_read_packets"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0));
    let duration: f64 = probe["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((duration - 2.0).abs() < 0.15, "duration {duration}");
    let mut decode_args = args(&["-nostdin", "-v", "error", "-xerror", "-threads", "1", "-i"]);
    decode_args.push(recording.to_str().unwrap().into());
    decode_args.extend(args(&["-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"]));
    let output = run_bounded(
        "ffmpeg",
        &decode_args,
        None,
        Duration::from_secs(10),
        1024 * 1024,
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
