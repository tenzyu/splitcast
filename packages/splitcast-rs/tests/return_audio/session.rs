use super::*;
use std::os::unix::process::CommandExt;

fn script(path: &Path, body: String) {
    fs::write(
        path,
        format!("#!{}\nset -eu\n{body}\n", executable("sh").display()),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

pub(super) fn install_player(dir: &Path, native: Option<&Path>) {
    // Capture the complete raw input, then open exactly that file as native player stdin.
    // Keep a start ledger as well: a short/empty/repeated attempt must be observable.
    let play = if let Some(root) = native {
        script(
            &dir.join("tools/ssh"),
            format!(
                "echo $$ > {}\nexec {} -F {} \"$@\"",
                quote(&dir.join("ssh.pid")),
                quote(&executable("ssh")),
                quote(&root.join("ssh.conf"))
            ),
        );
        format!(
            "exec {} \"$@\" < \"$dir/input-$$\"",
            quote(&executable("pw-play"))
        )
    } else {
        "exit 0".into()
    };
    script(
        &dir.join("tools/pw-play"),
        format!(
            r#"dir={}
echo "$$ $*" >> "$dir/starts"
{} > "$dir/input-$$.tmp"
{} "$dir/input-$$.tmp" "$dir/input-$$"
if test -f "$dir/hang"; then exec {} 40; fi
if test -f "$dir/reject"; then exit 19; fi
{play}"#,
            quote(dir),
            quote(&executable("cat")),
            quote(&executable("mv")),
            quote(&executable("sleep"))
        ),
    );
}

fn pcm(seed: u32, frames: usize) -> Vec<u8> {
    let mut word = seed;
    (0..frames * 2)
        .flat_map(|_| {
            word = word.wrapping_mul(1664525).wrapping_add(1013904223);
            ((word >> 17) as i16 - 16384).to_le_bytes()
        })
        .collect()
}

fn wav(pcm: &[u8]) -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend((36 + pcm.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt \x10\0\0\0\x01\0\x02\0");
    out.extend(48000u32.to_le_bytes());
    out.extend(192000u32.to_le_bytes());
    out.extend(b"\x04\0\x10\0data");
    out.extend((pcm.len() as u32).to_le_bytes());
    out.extend(pcm);
    out
}

fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(
            Instant::now() < end,
            "timed out awaiting fixture observation"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

struct Voice(TcpListener);
impl Voice {
    fn new(port: u16) -> Self {
        let l = TcpListener::bind(("127.0.0.1", port)).unwrap();
        l.set_nonblocking(true).unwrap();
        Self(l)
    }
    fn respond(&self, prefix: &str, response: &[u8]) {
        let mut connection = None;
        wait(|| match self.0.accept() {
            Ok((s, _)) => {
                connection = Some(s);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(e) => panic!("{e}"),
        });
        let mut s = connection.unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            s.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() <= 8192);
        }
        assert!(request.starts_with(prefix.as_bytes()));
        if prefix.contains("/synthesis") {
            let mut query = [0; 2];
            s.read_exact(&mut query).unwrap();
            assert_eq!(&query, b"{}");
        }
        write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            response.len()
        )
        .unwrap();
        s.write_all(response).unwrap();
    }
    fn synthesize(&self, audio: &[u8]) {
        self.respond("POST /audio_query?text=fixture&speaker=", b"{}");
        self.respond("POST /synthesis?speaker=", audio);
    }
}

fn starts(dir: &Path) -> Vec<u32> {
    match fs::read_to_string(dir.join("starts")) {
        Ok(s) => s
            .lines()
            .map(|line| line.split_once(' ').unwrap().0.parse().unwrap())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => panic!("{e}"),
    }
}
fn next_id(agent: &Agent) -> String {
    agent.ok("tts", json!({}))["next_id"]
        .as_str()
        .unwrap()
        .into()
}
fn submit(agent: &Agent, voice: &Voice, bytes: &[u8]) -> String {
    let id = next_id(agent);
    agent.ok("say", json!({"id":id,"text":"fixture"}));
    voice.synthesize(&wav(bytes));
    id
}
fn observed(dir: &Path, n: usize, pcm: &[u8]) -> u32 {
    wait(|| starts(dir).len() >= n);
    let pids = starts(dir);
    assert_eq!(pids.len(), n);
    let pid = pids[n - 1];
    let path = dir.join(format!("input-{pid}"));
    wait(|| path.exists());
    assert!(
        fs::read(path).unwrap() == pcm,
        "different PCM at player stdin"
    );
    let ledger = fs::read_to_string(dir.join("starts")).unwrap();
    let argv = ledger.lines().nth(n - 1).unwrap();
    assert!(argv.contains("--raw --format=s16 --rate=48000 --channels=2 --target="));
    let volume = if std::env::var_os("SPLITCAST_RETURN_LAB").is_some() {
        "--volume=1.00"
    } else {
        "--volume=0.60"
    };
    assert!(argv.contains(volume));
    assert!(argv.contains("node.dont-fallback = true node.dont-reconnect = true"));
    println!(
        "player input #{n}: {} PCM bytes sha256 {:x}",
        pcm.len(),
        Sha256::digest(pcm)
    );
    pid
}

fn gone(pid: u32) {
    wait(|| !Path::new(&format!("/proc/{pid}")).exists());
}
fn rejected(agent: &Agent, cmd: &str, args: Value) {
    assert_eq!(agent.command(cmd, args).0, 409, "accepted {cmd}");
}

fn discard_reply(agent: &Agent, cmd: &str, args: Value) {
    let mut s = TcpStream::connect(("127.0.0.1", agent.port)).unwrap();
    let body = json!({"command":cmd,"args":args}).to_string();
    write!(s,"POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", agent.token, body.len(), port=agent.port).unwrap();
    // Close without reading the response, including any audio it contains.
}

struct ClientProcess {
    child: Child,
    transcript: PathBuf,
}

impl ClientProcess {
    fn spawn(mut command: Command, dir: &Path, label: &str, stdin: Stdio) -> Self {
        let transcript = dir.join(format!("{label}.stdout"));
        let child = command
            .process_group(0)
            .stdin(stdin)
            .stdout(File::create(&transcript).unwrap())
            .stderr(File::create(dir.join(format!("{label}.stderr"))).unwrap())
            .spawn()
            .unwrap();
        Self { child, transcript }
    }

    fn send(&mut self, input: &str) {
        let stdin = self.child.stdin.as_mut().unwrap();
        stdin.write_all(input.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    fn text(&self) -> String {
        fs::read_to_string(&self.transcript).unwrap()
    }

    fn finish(mut self) -> (bool, String) {
        let mut status = None;
        wait(|| {
            status = self.child.try_wait().unwrap();
            status.is_some()
        });
        (status.unwrap().success(), self.text())
    }
}

impl Drop for ClientProcess {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            // SAFETY: the unreaped ClientProcess leader pins its private process group.
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
            let _ = self.child.wait();
        }
    }
}

fn control_binary(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_splitcast-rs-core"));
    command.arg("--config").arg(dir.join("config.json"));
    command
}

fn cli(dir: &Path, args: &[&str], label: &str) -> (bool, String) {
    let mut command = control_binary(dir);
    command.args(args);
    ClientProcess::spawn(command, dir, label, Stdio::null()).finish()
}

fn assert_stop_proof(agent: &Agent, receipt: &Value) {
    assert_eq!(receipt["stop"], "completed");
    assert_eq!(receipt["outcome"], "completed");
    assert_eq!(receipt["state"], "idle");
    assert_eq!(receipt["children"], json!([]));
    assert_eq!(receipt["remote_release_pending"], false);
    assert!(receipt["stop_id"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(receipt["stop_id"], agent.state("idle")["stop_id"]);
}

#[test]
fn cli_and_tui_speak_and_wait_for_their_own_stop() {
    let mut f = Fixture::with_speech(true, None);
    let dir = f.root.join("capture");
    let voice = Voice::new(f.ports[8]);
    let samples = pcm(79, 4800);
    fs::write(dir.join("hang"), b"").unwrap();
    let (ok, _) = cli(&dir, &["prepare", "--speech-only"], "cli-prepare");
    assert!(ok);
    let (ok, _) = cli(
        &dir,
        &[
            "start",
            "--output",
            "twitch-live",
            "--duration-seconds",
            "60",
        ],
        "cli-unconfirmed-public",
    );
    assert!(!ok);
    f.capture.state("ready");

    // This reservation provides an independent oracle for the ID the CLI
    // must acquire from the Agent itself before sending its bare "say TEXT".
    let id = next_id(&f.production);
    let (ok, utterance) = cli(&dir, &["say", "fixture"], "cli-speak");
    assert!(ok);
    let utterance: Value = serde_json::from_str(&utterance).unwrap();
    assert_eq!(utterance["id"], id);
    voice.synthesize(&wav(&samples));
    let cli_pid = observed(&dir, 1, &samples);
    let (ok, _) = cli(&dir, &["say", "fixture", "--id", &id], "cli-stale");
    assert!(!ok, "CLI accepted a consumed speech ID");
    assert_eq!(
        f.capture
            .command("say", json!({"text":"fixture","id":id}))
            .1["outcome"],
        "not_executed"
    );
    assert_eq!(starts(&dir), [cli_pid]);
    let (ok, stopped) = cli(&dir, &["stop"], "cli-stop");
    assert!(ok);
    let stopped: Value = serde_json::from_str(&stopped).unwrap();
    assert_stop_proof(&f.capture, &stopped);
    gone(cli_pid);
    f.production.state("idle");

    // Run the shipped terminal menu in a separate process and drive its
    // stdin in operator order. Wait for returned PCM before requesting Stop.
    let mut command = control_binary(&dir);
    command.arg("tui");
    let mut tui = ClientProcess::spawn(command, &dir, "tui", Stdio::piped());
    tui.send("5\n");
    wait(|| tui.text().contains("\"state\": \"ready\""));
    let id = next_id(&f.production);
    tui.send("14\n60\nno\n12\nfixture\n");
    wait(|| tui.text().contains(&format!("\"id\": \"{id}\"")));
    voice.synthesize(&wav(&samples));
    let tui_pid = observed(&dir, 2, &samples);
    tui.send("7\n");
    wait(|| tui.text().contains("\"stop\": \"completed\""));
    tui.send("0\n");
    let (ok, transcript) = tui.finish();
    assert!(ok);
    assert!(!transcript.contains("\"output\": \"twitch-live\""));
    let proof = f.capture.state("idle");
    assert_stop_proof(&f.capture, &proof);
    let tui_stop_id = proof["stop_id"].as_str().unwrap();
    assert_ne!(stopped["stop_id"], tui_stop_id);
    assert!(transcript.contains(&format!("\"stop_id\": \"{tui_stop_id}\"")));
    gone(tui_pid);
    f.production.state("idle");
    assert_eq!(starts(&dir), [cli_pid, tui_pid]);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn tui_can_stop_while_start_reply_is_held() {
    let mut f = Fixture::new();
    let dir = f.root.join("capture");
    let production = f.root.join("production");
    let hold = production.join("hold-start-ack");
    f.capture.ok("prepare", json!({}));
    fs::write(&hold, b"").unwrap();
    let mut command = control_binary(&dir);
    command.arg("tui");
    let mut tui = ClientProcess::spawn(command, &dir, "tui-urgent-stop", Stdio::piped());
    tui.send("6\n");
    wait(|| production.join("start-committed").exists());
    let pids = f.pids();
    tui.send("7\n");
    // The terminal must send Stop before the adapter releases Start's reply.
    // A serialized menu waiting on Start cannot satisfy this assertion.
    wait(|| {
        let (code, status) = f.capture.command("status", json!({}));
        match code {
            200 => matches!(status["stop"].as_str(), Some("requested" | "completed")),
            // A Status queued before Stop can lose its generation fence.
            // A fresh Status is observation only and can be submitted again.
            503 if status["outcome"] == "not_executed" => false,
            _ => panic!("unexpected Stop observation response: {code} {status}"),
        }
    });
    fs::remove_file(hold).unwrap();
    wait(|| tui.text().contains("\"stop\": \"completed\""));
    tui.send("0\n");
    let (ok, transcript) = tui.finish();
    assert!(ok);
    let proof = f.capture.state("idle");
    assert_stop_proof(&f.capture, &proof);
    let stop_id = proof["stop_id"].as_str().unwrap();
    assert!(transcript.contains(&format!("\"stop_id\": \"{stop_id}\"")));
    f.production.state("idle");
    f.released(&pids);
    let obs_requests = fs::read_to_string(production.join("obs-requests.log")).unwrap();
    assert_eq!(
        obs_requests
            .lines()
            .filter(|method| *method == "StartRecord")
            .count(),
        1
    );
    assert!(!obs_requests.contains("StartStream"));
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
#[ignore = "requires Node.js and a headless Chromium executable"]
fn chromium_speaks_through_the_agent_and_observes_stop() {
    let mut f = Fixture::with_speech(true, None);
    let dir = f.root.join("capture");
    let voice = Voice::new(f.ports[8]);
    f.capture.ok("prepare", json!({"media":false}));
    let mut command =
        Command::new(std::env::var_os("SPLITCAST_TEST_NODE").unwrap_or_else(|| "node".into()));
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/return_audio/web.cjs"))
        .arg(std::env::var_os("SPLITCAST_TEST_CHROMIUM").unwrap_or_else(|| "chromium".into()))
        .arg(dir.join("browser-profile"))
        .arg(format!("http://127.0.0.1:{}/", f.capture.port))
        .arg(dir.join("token"));
    let mut browser = ClientProcess::spawn(command, &dir, "chromium", Stdio::piped());
    let first = pcm(87, 4800);
    voice.synthesize(&wav(&first));
    gone(observed(&dir, 1, &first));
    let second = pcm(88, 4800);
    fs::write(dir.join("hang"), b"").unwrap();
    voice.synthesize(&wav(&second));
    let pid = observed(&dir, 2, &second);
    browser.send("stop\n");
    let (ok, body) = browser.finish();
    assert!(ok, "see {}", dir.join("chromium.stderr").display());
    let receipt: Value = serde_json::from_str(&body).unwrap();
    assert_stop_proof(&f.capture, &receipt["stop"]);
    gone(pid);
    f.production.state("idle");
    assert_eq!(starts(&dir).len(), 2);
    f.capture.shutdown();
    f.production.shutdown();
}

#[test]
fn return_faults_and_replay() {
    let mut f = Fixture::with_speech(true, None);
    let dir = f.root.join("capture");
    let voice = Voice::new(f.ports[8]);
    let ready = f.capture.ok("prepare", json!({"media":false}));
    assert_eq!(ready["children"], json!(["ssh"]));
    assert_eq!(f.production.ok("status", json!({}))["children"], json!([]));
    rejected(&f.capture, "start", json!({}));
    let id = next_id(&f.production);
    let args = json!({"text":"fixture","id":id});
    let actual = pcm(13, 4800);

    let saved = std::mem::replace(&mut f.production.token, "invalid-bearer".into());
    assert_eq!(f.production.command("say", args.clone()).0, 403);
    f.production.token = saved;
    rejected(
        &f.production,
        "say",
        json!({"id":id,"text":"fixture","target":"game.local"}),
    );
    rejected(
        &f.production,
        "say",
        json!({"id":id,"text":"fixture","_owner":"wrong-session"}),
    );
    rejected(&f.capture, "tts-next", json!({}));
    rejected(&f.production, "tts-next", json!({}));
    rejected(&f.production, "say", json!({"text":"fixture"}));
    assert_eq!(next_id(&f.production), id);
    assert!(starts(&dir).is_empty());
    discard_reply(&f.production, "say", args.clone());
    voice.synthesize(&wav(&actual));
    rejected(&f.production, "say", args);
    gone(observed(&dir, 1, &actual));

    // A non-PCM synthesis response is rejected even when retried by the synth worker.
    let invalid = next_id(&f.production);
    f.production
        .ok("say", json!({"text":"fixture","id":invalid}));
    for _ in 0..3 {
        voice.synthesize(b"not a WAV");
    }
    wait(|| f.production.ok("tts", json!({}))["active"] == false);
    assert_eq!(starts(&dir).len(), 1);

    // Concurrent clients holding one admission ID can start at most one utterance.
    let id = next_id(&f.production);
    let args = json!({"text":"fixture","id":id});
    let (a, b) = thread::scope(|s| {
        let a = s.spawn(|| f.production.command("say", args.clone()).0);
        let b = f.production.command("say", args.clone()).0;
        (a.join().unwrap(), b)
    });
    assert!(matches!((a, b), (200, 409) | (409, 200)));
    fs::write(dir.join("reject"), b"").unwrap();
    voice.synthesize(&wav(&actual));
    gone(observed(&dir, 2, &actual));
    fs::remove_file(dir.join("reject")).unwrap();
    rejected(&f.production, "say", args);
    wait(|| {
        f.capture.ok("status", json!({}))["warnings"]
            .to_string()
            .contains("player failed")
    });

    // A ten-second WAV with a five-second grant reaches the player, which stalls.
    // Expiry must reap it earlier than the ordinary duration-plus-startup deadline.
    fs::write(dir.join("hang"), b"").unwrap();
    let long = pcm(9, 480_000);
    let now = Instant::now();
    submit(&f.production, &voice, &long);
    gone(observed(&dir, 3, &long));
    assert!(now.elapsed() < Duration::from_secs(10));
    // Stop during playback owns cancellation and reaps before final completion.
    submit(&f.production, &voice, &long);
    let pid = observed(&dir, 4, &long);
    let now = Instant::now();
    f.capture.ok("stop", json!({}));
    assert!(now.elapsed() < Duration::from_secs(5));
    gone(pid);
    fs::remove_file(dir.join("hang")).unwrap();
    f.capture.ok("prepare", json!({"media":false}));
    rejected(&f.production, "say", json!({"text":"fixture","id":id}));
    let recovered = pcm(18, 4800);
    submit(&f.production, &voice, &recovered);
    gone(observed(&dir, 5, &recovered));
    f.capture.ok("stop", json!({}));

    // Lost pull reply: interrogate the real production Agent under a fresh test owner.
    let owner = json!({"_owner":"test-capture-lease"});
    f.production
        .ok("prepare", json!({"_owner":owner["_owner"],"media":false}));
    let expired_id = next_id(&f.production);
    wait(|| next_id(&f.production) != expired_id);
    rejected(
        &f.production,
        "say",
        json!({"id":expired_id,"text":"fixture"}),
    );
    submit(&f.production, &voice, &actual);
    wait(|| f.production.ok("tts", json!({}))["ready"] == 1);
    discard_reply(&f.production, "tts-next", owner.clone());
    wait(|| f.production.ok("tts", json!({}))["ready"] == 0);
    assert!(f.production.ok("tts-next", owner.clone()).is_null());
    submit(&f.production, &voice, &actual);
    wait(|| f.production.ok("tts", json!({}))["ready"] == 1);
    submit(&f.production, &voice, &actual);
    wait(|| f.production.ok("tts", json!({}))["ready"] == 2);
    let full = next_id(&f.production);
    rejected(&f.production, "say", json!({"id":full,"text":"fixture"}));
    wait(|| f.production.ok("tts", json!({}))["ready"] == 0);
    assert!(f.production.ok("tts-next", owner.clone()).is_null());
    f.production.ok("stop", owner);
    assert_eq!(starts(&dir).len(), 5);
}

#[test]
#[ignore = "runs owned OpenSSH and a private device-free PipeWire graph"]
fn native_return() {
    assert!(Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/return_audio/native.py"))
        .arg(std::env::current_exe().unwrap())
        .status()
        .unwrap()
        .success());
}

fn reselect_capture(f: &mut Fixture, target: &str) {
    f.capture.shutdown();
    let dir = f.root.join("capture");
    let config = dir.join("config.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    document["audio"]["tts_sink"] = json!(target);
    fs::write(config, document.to_string()).unwrap();
    f.capture = Agent::spawn(&dir, f.ports[0], f.ports[6], f.ports[7]);
}

#[test]
#[ignore = "entry point for native_return's isolated private graph"]
fn native_session() {
    let lab = PathBuf::from(std::env::var_os("SPLITCAST_RETURN_LAB").unwrap());
    assert_eq!(std::env::var("PIPEWIRE_REMOTE").unwrap(), "return-test");
    assert_eq!(
        PathBuf::from(std::env::var_os("PIPEWIRE_RUNTIME_DIR").unwrap()),
        lab
    );
    let mut f = Fixture::with_speech(true, Some(&lab));
    let voice = Voice::new(f.ports[8]);
    let dir = f.root.join("capture");
    f.capture.ok("prepare", json!({"media":false}));
    let first = pcm(31, 48000);
    fs::write(lab.join("first"), &first).unwrap();
    let id = submit(&f.production, &voice, &first);
    gone(observed(&dir, 1, &first));
    let interrupted = pcm(32, 192000);
    submit(&f.production, &voice, &interrupted);
    let pid = observed(&dir, 2, &interrupted);
    let tunnel: i32 = fs::read_to_string(dir.join("ssh.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // The owned SSH worker's PID is pinned by its live Agent for the lifetime of this session.
    assert_eq!(unsafe { libc::kill(tunnel, libc::SIGKILL) }, 0);
    wait(|| f.capture.ok("status", json!({}))["state"] == "failed");
    gone(pid);
    // While the forward is gone, the unrelated native game playback still works.
    assert_port_not_listening(f.ports[2]);
    let marker = lab.join("local-marker");
    fs::write(&marker, pcm(47, 48000)).unwrap();
    assert!(Command::new("pw-play")
        .args([
            "--raw",
            "--format=s16",
            "--channels=2",
            "--rate=48000",
            "--target=game.local",
            "-"
        ])
        .stdin(File::open(marker).unwrap())
        .status()
        .unwrap()
        .success());
    f.capture.ok("stop", json!({}));
    reselect_capture(&mut f, "return.absent");
    f.capture.ok("prepare", json!({"media":false}));
    rejected(&f.production, "say", json!({"id":id,"text":"fixture"}));
    let missing = pcm(91, 48000);
    fs::write(lab.join("missing"), &missing).unwrap();
    submit(&f.production, &voice, &missing);
    gone(observed(&dir, 3, &missing));
    f.capture.ok("stop", json!({}));
    reselect_capture(&mut f, "return.selected");
    f.capture.ok("prepare", json!({"media":false}));
    let recovered = pcm(77, 48000);
    fs::write(lab.join("recovered"), &recovered).unwrap();
    submit(&f.production, &voice, &recovered);
    gone(observed(&dir, 4, &recovered));
    f.capture.ok("stop", json!({}));
    reselect_capture(&mut f, "return.vanish");
    f.capture.ok("prepare", json!({"media":false}));
    let removed = pcm(79, 192000);
    fs::write(lab.join("removed"), &removed).unwrap();
    submit(&f.production, &voice, &removed);
    let pid = observed(&dir, 5, &removed);
    // Wait for a non-silent marker in this exact sink before removing the node.
    // Search only the advancing tail; both monitor and marker are stereo PCM16.
    let monitor = lab.join("return.vanish.pcm");
    let mut offset = 0;
    wait(|| {
        use std::io::{Seek, SeekFrom};
        let mut file = File::open(&monitor).unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        offset += (bytes.len().saturating_sub(4096) / 4 * 4) as u64;
        bytes
            .windows(4096)
            .step_by(4)
            .any(|s| s == &removed[8192..12288])
    });
    let id = fs::read_to_string(lab.join("vanishing-id")).unwrap();
    assert!(Command::new("pw-cli")
        .args(["-r", "return-test", "destroy", &id])
        .status()
        .unwrap()
        .success());
    gone(pid);
    f.capture.ok("stop", json!({}));
    f.production.state("idle");
    assert_port_not_listening(f.ports[2]);
}
