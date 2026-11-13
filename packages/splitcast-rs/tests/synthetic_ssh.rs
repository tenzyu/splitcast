//! Opt-in, real two-host synthetic transport validation, not an agent/OBS test.
use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Harness {
    source_host: String,
    identity: PathBuf,
    remote_ffmpeg: String,
    ffmpeg: OsString,
    ffprobe: OsString,
    evidence: PathBuf,
}

fn valid_remote_target(host: &str, tool: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._@-".contains(&b))
        && tool.starts_with('/')
        && tool
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._+-".contains(&b))
}

impl Harness {
    fn load() -> Self {
        assert_eq!(
            env::var("SPLITCAST_SYNTHETIC_AUTHORIZED").as_deref(),
            Ok("1"),
            "explicit synthetic-host authorization is required"
        );
        let source_host =
            env::var("SPLITCAST_SYNTHETIC_SOURCE_HOST").expect("source host is required");
        let identity = PathBuf::from(
            env::var_os("SPLITCAST_SYNTHETIC_IDENTITY").expect("SSH identity is required"),
        );
        let remote_ffmpeg = env::var("SPLITCAST_SYNTHETIC_REMOTE_FFMPEG")
            .expect("absolute remote FFmpeg path is required");
        assert!(
            valid_remote_target(&source_host, &remote_ffmpeg),
            "unsafe remote host or executable path"
        );
        assert!(identity.is_file(), "SSH identity must be an existing file");
        Self {
            source_host,
            identity,
            remote_ffmpeg,
            ffmpeg: env::var_os("SPLITCAST_FFMPEG").unwrap_or_else(|| "ffmpeg".into()),
            ffprobe: env::var_os("SPLITCAST_FFPROBE").unwrap_or_else(|| "ffprobe".into()),
            evidence: env::var_os("SPLITCAST_EVIDENCE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(env::temp_dir),
        }
    }
    fn ssh_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = ["-i"].into_iter().map(Into::into).collect();
        args.push(self.identity.as_os_str().to_owned());
        args.extend(
            [
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
                "--",
            ]
            .into_iter()
            .map(OsString::from),
        );
        args.push(self.source_host.clone().into());
        args
    }
    fn ssh(&self) -> Command {
        let mut command = Command::new("ssh");
        command.args(self.ssh_args());
        command
    }
}

struct Owned<'a> {
    harness: &'a Harness,
    unit: String,
    children: Vec<Child>,
    evidence: PathBuf,
}

impl Owned<'_> {
    fn stop(&mut self) -> bool {
        let remote = Command::new("timeout")
            .arg("20")
            .arg("ssh")
            .args(self.harness.ssh_args())
            .arg(format!(
                "systemctl --user stop {}.service >/dev/null 2>&1; systemctl --user show {}.service -p LoadState -p ActiveState -p MainPID",
                self.unit, self.unit
            ))
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", &format!("{}.service", self.unit)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        for c in &mut self.children {
            let _ = c.kill();
            let _ = c.wait();
        }
        let local = Command::new("systemctl")
            .args([
                "--user",
                "show",
                &format!("{}.service", self.unit),
                "-p",
                "LoadState",
                "-p",
                "ActiveState",
                "-p",
                "MainPID",
            ])
            .output();
        let stopped = |output: std::io::Result<std::process::Output>, name: &str| {
            output.is_ok_and(|o| {
                let _ = fs::write(self.evidence.join(name), &o.stdout);
                let text = String::from_utf8_lossy(&o.stdout);
                o.status.success()
                    && text.lines().any(|l| l == "ActiveState=inactive")
                    && text.lines().any(|l| l == "MainPID=0")
            })
        };
        stopped(remote, "remote-teardown.txt") && stopped(local, "local-teardown.txt")
    }
}

impl Drop for Owned<'_> {
    fn drop(&mut self) {
        self.stop();
    }
}

fn bounded(unit: &str) -> Command {
    let mut c = Command::new("systemd-run");
    c.args([
        "--user",
        "--quiet",
        "--pipe",
        "--wait",
        "--collect",
        "--unit",
        unit,
        "-p",
        "CPUQuota=100%",
        "-p",
        "MemoryMax=512M",
        "-p",
        "RuntimeMaxSec=660",
        "-p",
        "TimeoutStopSec=5",
        "-p",
        "KillMode=control-group",
        "--",
    ]);
    c
}

#[test]
#[ignore = "requires explicit two-host synthetic-only authorization and staged FFmpeg"]
fn synthetic_av_over_encrypted_ssh() {
    let harness = Harness::load();
    let seconds: u64 = env::var("SPLITCAST_SYNTHETIC_SECONDS")
        .unwrap_or_else(|_| "10".into())
        .parse()
        .unwrap();
    assert!(
        [10, 600].contains(&seconds),
        "only authorized 10s/600s runs"
    );
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let unit = format!("splitcast-synthetic-{}-{nonce}", std::process::id());
    let dir = harness.evidence.join(&unit);
    fs::create_dir_all(&harness.evidence).unwrap();
    fs::create_dir(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    }
    println!("evidence={} seconds={seconds} unit={unit}", dir.display());
    fs::write(dir.join("scope.txt"), format!("Synthetic testsrc2 320x180 15fps + sine 48000Hz, software H264/AAC; no device input, listener, GPU or production activation.\nseconds={seconds}\nunit={unit}\nCPUQuota=100% MemoryMax=512M RuntimeMaxSec=660 TimeoutStopSec=5 KillMode=control-group on both media units\n")).unwrap();
    let mut owned = Owned {
        harness: &harness,
        unit: unit.clone(),
        children: Vec::new(),
        evidence: dir.clone(),
    };
    // EXIT/HUP cleanup covers remote-session termination; finite input and the
    // service runtime bound also cover a disconnected client with no signal.
    let remote = format!("umask 077; trap 'systemctl --user stop {unit}.service >/dev/null 2>&1 || :' EXIT; trap 'exit 129' HUP; trap 'exit 130' INT TERM; systemd-run --user --quiet --pipe --wait --collect --unit {unit} -p CPUQuota=100% -p MemoryMax=512M -p RuntimeMaxSec=660 -p TimeoutStopSec=5 -p KillMode=control-group -- {remote_ffmpeg} -hide_banner -nostdin -loglevel warning -threads 1 -filter_threads 1 -re -f lavfi -i testsrc2=size=320x180:rate=15 -re -f lavfi -i sine=frequency=440:sample_rate=48000 -t {seconds} -map 0:v:0 -map 1:a:0 -c:v libx264 -threads:v 1 -preset ultrafast -tune zerolatency -pix_fmt yuv420p -g 30 -b:v 300k -c:a aac -b:a 64k -f mpegts pipe:1", remote_ffmpeg=harness.remote_ffmpeg);
    fs::write(dir.join("remote-command.txt"), &remote).unwrap();
    let mut source = harness
        .ssh()
        .arg(remote)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(File::create(dir.join("source.stderr")).unwrap())
        .spawn()
        .unwrap();
    let pipe = source.stdout.take().unwrap();
    owned.children.push(source);
    let recording = dir.join("synthetic.mkv");
    let sink = bounded(&unit)
        .arg(&harness.ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-loglevel",
            "warning",
            "-threads",
            "1",
            "-i",
            "pipe:0",
            "-map",
            "0:v:0",
            "-map",
            "0:a:0",
            "-c",
            "copy",
            "-f",
            "matroska",
        ])
        .arg(&recording)
        .stdin(Stdio::from(pipe))
        .stdout(Stdio::null())
        .stderr(File::create(dir.join("sink.stderr")).unwrap())
        .spawn()
        .unwrap();
    owned.children.push(sink);
    if env::var("SPLITCAST_SYNTHETIC_DISCONNECT").as_deref() == Ok("1") {
        assert_eq!(
            seconds, 10,
            "disconnect probe is restricted to the short run"
        );
        thread::sleep(Duration::from_secs(3));
        assert!(
            owned
                .children
                .iter_mut()
                .all(|c| c.try_wait().unwrap().is_none()),
            "transport was not live before disconnect"
        );
        owned.children[0].kill().unwrap();
        owned.children[0].wait().unwrap();
        assert!(
            owned.stop(),
            "disconnect explicit teardown verification failed"
        );
        fs::write(dir.join("result.txt"), "PASS: live SSH client forcibly disconnected; explicit independent remote stop and local stop verified both owned units inactive. Media validity not asserted for interrupted output.\n").unwrap();
        println!("PASS disconnect cleanup evidence={}", dir.display());
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(seconds + 45);
    let mut statuses = [None, None];
    loop {
        for (i, c) in owned.children.iter_mut().enumerate() {
            if statuses[i].is_none() {
                statuses[i] = c.try_wait().unwrap();
            }
        }
        if statuses.iter().all(Option::is_some) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "transport exceeded deadline; cleanup guard will stop both units"
        );
        thread::sleep(Duration::from_millis(200));
    }
    fs::write(
        dir.join("exit-status.txt"),
        format!("source={:?}\nsink={:?}\n", statuses[0], statuses[1]),
    )
    .unwrap();
    assert!(
        statuses.iter().all(|s| s.unwrap().success()),
        "transport failed; inspect evidence"
    );
    for (selector, name) in [("v:0", "video"), ("a:0", "audio")] {
        let output = bounded(&unit)
            .arg(&harness.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                selector,
                "-count_packets",
                "-show_entries",
                "stream=codec_name,nb_read_packets",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(&recording)
            .output()
            .unwrap();
        fs::write(dir.join(format!("{name}-probe.txt")), &output.stdout).unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let count: u64 = text
            .lines()
            .find_map(|l| l.strip_prefix("nb_read_packets="))
            .unwrap()
            .parse()
            .unwrap();
        assert!(count > 0, "no {name} packets");
    }
    let output = bounded(&unit)
        .arg(&harness.ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,size",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&recording)
        .output()
        .unwrap();
    fs::write(dir.join("format-probe.txt"), &output.stdout).unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let duration: f64 = text
        .lines()
        .find_map(|l| l.strip_prefix("duration="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (duration - seconds as f64).abs() < 1.0,
        "unexpected recorded duration: {duration}"
    );
    let decoded = bounded(&unit)
        .arg(&harness.ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-threads",
            "1",
            "-i",
        ])
        .arg(&recording)
        .args([
            "-map", "0:v:0", "-map", "0:a:0", "-threads", "1", "-f", "null", "-",
        ])
        .stdout(Stdio::null())
        .stderr(File::create(dir.join("decode.stderr")).unwrap())
        .status()
        .unwrap();
    assert!(decoded.success(), "recorded audio/video did not decode");
    assert!(owned.stop(), "explicit teardown verification failed");
    fs::write(dir.join("result.txt"), "PASS: synthetic SSH transport, packets, duration, full A/V decode, explicit owned-unit stop and inactive checks. Not Rust-agent, OBS, gameplay, DSO, physical audio/input or latency evidence.\n").unwrap();
    println!("PASS evidence={}", dir.display());
}

#[test]
fn synthetic_remote_arguments_reject_shell_and_option_injection() {
    assert!(valid_remote_target("capture-host", "/usr/bin/ffmpeg"));
    for host in ["", "-oProxyCommand=bad", "host;bad", "host\nother"] {
        assert!(!valid_remote_target(host, "/usr/bin/ffmpeg"));
    }
    for tool in ["ffmpeg", "/bin/ffmpeg;bad", "/bin/a b", "/bin/a'bad"] {
        assert!(!valid_remote_target("capture-host", tool));
    }
}
