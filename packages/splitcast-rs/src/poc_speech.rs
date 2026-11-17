//! Finite, explicitly selected playback for local fixtures and prepared Agent returns.
//!
//! Process completion is not confirmed listening. Cleanup has the process module's
//! group-escape, controller-death and kernel-I/O limitations; teardown can exceed
//! the operation deadline. There is no retry and no recording or cache output.

use crate::process::run_operation;
use crate::request::Operation;
use crate::tts::{PcmWav, AUDIO_LIMIT};
use serde_json::{json, Value};
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

const SAFETY_LIMIT: Duration = Duration::from_secs(30);
const STARTUP_ALLOWANCE: Duration = Duration::from_secs(2);
const OUTPUT_CAP: usize = 16 * 1024;
const PROPERTIES: &str =
    "{ node.dont-fallback = true node.dont-reconnect = true application.name = splitcast-tts }";

#[derive(Debug)]
pub enum PlaybackError {
    InvalidTarget,
    Input(io::Error),
    Operation(io::Error),
    PlayerExit { status: ExitStatus, stderr: String },
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget => write!(f, "an explicit safe PipeWire node name is required"),
            Self::Input(error) => write!(f, "invalid playback input: {error}"),
            Self::Operation(error) => write!(f, "playback operation failed: {error}"),
            Self::PlayerExit { status, stderr } => {
                write!(f, "native player failed ({status}): {stderr}")
            }
        }
    }
}

impl std::error::Error for PlaybackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) | Self::Operation(error) => Some(error),
            Self::InvalidTarget | Self::PlayerExit { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct FixturePlaybackReceipt {
    target: String,
    channels: u16,
    sample_rate: u32,
    pcm_bytes: usize,
    duration: Duration,
}

impl FixturePlaybackReceipt {
    pub fn to_json(&self) -> Value {
        json!({
            "provenance": "local_fixture",
            "completion_scope": "native_player_process",
            "player_process": "completed",
            "listening": "unverified",
            "recording_files_created": 0,
            "target": self.target,
            "channels": self.channels,
            "sample_rate": self.sample_rate,
            "pcm_bytes": self.pcm_bytes,
            "duration_seconds": self.duration.as_secs_f64(),
        })
    }
}

/// Validates a bounded local WAV, then runs one native player on its PCM stdin.
/// The caller owns the source file and shutdown flag; neither is mutated here.
pub fn play_fixture(
    input: &Path,
    target: &str,
    shutdown: Arc<AtomicBool>,
) -> Result<FixturePlaybackReceipt, PlaybackError> {
    let operation = Operation::new(shutdown, None, SAFETY_LIMIT);
    play_fixture_with(input, target, &operation, "pw-play", &[])
}

pub(crate) fn valid_target(target: &str) -> bool {
    if target.is_empty()
        || target.len() > 255
        || !target.as_bytes()[0].is_ascii_alphanumeric()
        || !target
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || [
            "auto",
            "default",
            "default_sink",
            "default_source",
            "default-sink",
            "default-source",
            "none",
            "null",
            "any",
        ]
        .iter()
        .any(|name| target.eq_ignore_ascii_case(name))
    {
        return false;
    }
    !target.bytes().all(|b| b.is_ascii_digit())
}

fn read_fixture(input: &Path, operation: &Operation) -> Result<Vec<u8>, PlaybackError> {
    operation.check().map_err(PlaybackError::Operation)?;
    // Open before inspecting metadata: a replaced final symlink cannot redirect
    // this descriptor, and a raced-in FIFO cannot block open before fstat.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(input)
        .map_err(PlaybackError::Input)?;
    let metadata = file.metadata().map_err(PlaybackError::Input)?;
    if !metadata.is_file() || metadata.len() > AUDIO_LIMIT as u64 {
        return Err(PlaybackError::Input(io::Error::new(
            io::ErrorKind::InvalidInput,
            "fixture must be a regular file of at most 4 MiB",
        )));
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        operation.check().map_err(PlaybackError::Operation)?;
        let remaining = (AUDIO_LIMIT + 1 - bytes.len()).min(buffer.len());
        let n = file
            .read(&mut buffer[..remaining])
            .map_err(PlaybackError::Input)?;
        if n == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > AUDIO_LIMIT {
            return Err(PlaybackError::Input(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fixture grew beyond 4 MiB",
            )));
        }
    }
}

fn play_fixture_with(
    input: &Path,
    target: &str,
    operation: &Operation,
    program: &str,
    prefix: &[String],
) -> Result<FixturePlaybackReceipt, PlaybackError> {
    if !valid_target(target) {
        return Err(PlaybackError::InvalidTarget);
    }
    let bytes = read_fixture(input, operation)?;
    let wav = PcmWav::parse(&bytes).map_err(PlaybackError::Input)?;
    play_pcm(&wav, target, operation, program, prefix)?;
    Ok(FixturePlaybackReceipt {
        target: target.into(),
        channels: wav.channels(),
        sample_rate: wav.sample_rate(),
        pcm_bytes: wav.pcm().len(),
        duration: wav.duration(),
    })
}

pub(crate) fn play_return(
    bytes: &[u8],
    target: &str,
    operation: &Operation,
) -> Result<(), PlaybackError> {
    let wav = PcmWav::parse(bytes).map_err(PlaybackError::Input)?;
    if !valid_target(target) {
        return Err(PlaybackError::InvalidTarget);
    }
    play_pcm(&wav, target, operation, "pw-play", &[])
}

fn play_pcm(
    wav: &PcmWav<'_>,
    target: &str,
    operation: &Operation,
    program: &str,
    prefix: &[String],
) -> Result<(), PlaybackError> {
    let mut args = prefix.to_vec();
    args.extend([
        "--raw".into(),
        "--format=s16".into(),
        format!("--rate={}", wav.sample_rate()),
        format!("--channels={}", wav.channels()),
        format!("--target={target}"),
        format!("--properties={PROPERTIES}"),
        "-".into(),
    ]);
    let deadline = operation
        .deadline((wav.duration() + STARTUP_ALLOWANCE).min(SAFETY_LIMIT))
        .map_err(PlaybackError::Operation)?;
    let output = run_operation(
        program,
        &args,
        Some(wav.pcm()),
        deadline.saturating_duration_since(Instant::now()),
        OUTPUT_CAP,
        Some(operation),
    )
    .map_err(PlaybackError::Operation)?;
    if !output.status.success() {
        return Err(PlaybackError::PlayerExit {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::symlink;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const HELPER: &str = "poc_speech::tests::native_player_helper";

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "splitcast-fixture-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("input.wav"), sample_wav()).unwrap();
            fs::write(path.join("sentinel"), b"not owned by playback").unwrap();
            Self(path)
        }

        fn input(&self) -> std::path::PathBuf {
            self.0.join("input.wav")
        }

        fn prefix(&self, mode: &str) -> Vec<String> {
            vec![
                "--ignored".into(),
                "--exact".into(),
                HELPER.into(),
                "--nocapture".into(),
                "--".into(),
                format!("splitcast-test-dir={}", self.0.display()),
                format!("splitcast-test-mode={mode}"),
            ]
        }

        fn play(
            &self,
            mode: &str,
            operation: &Operation,
        ) -> Result<FixturePlaybackReceipt, PlaybackError> {
            play_fixture_with(
                &self.input(),
                "fixture.selected",
                operation,
                std::env::current_exe().unwrap().to_str().unwrap(),
                &self.prefix(mode),
            )
        }

        fn pid(&self, marker: &str) -> u32 {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Ok(text) = fs::read_to_string(self.0.join(marker)) {
                    if let Ok(pid) = text.parse() {
                        return pid;
                    }
                }
                assert!(Instant::now() < deadline, "missing {marker} helper marker");
                thread::sleep(Duration::from_millis(5));
            }
        }

        fn starts(&self) -> usize {
            match fs::read_to_string(self.0.join("starts")) {
                Ok(text) => text.lines().count(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
                Err(error) => panic!("{error}"),
            }
        }

        fn assert_source_intact(&self) {
            assert_eq!(fs::read(self.input()).unwrap(), sample_wav());
            assert_eq!(
                fs::read(self.0.join("sentinel")).unwrap(),
                b"not owned by playback"
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn operation(budget: Duration) -> Operation {
        Operation::new(Arc::new(AtomicBool::new(false)), None, budget)
    }

    fn sample_wav() -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend(44u32.to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(48000u32.to_le_bytes());
        bytes.extend(192000u32.to_le_bytes());
        bytes.extend(4u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(8u32.to_le_bytes());
        bytes.extend([0, 128, 255, 127, 42, 0, 214, 255]);
        bytes
    }

    #[test]
    #[ignore = "inert child entry; invoked explicitly only by playback tests"]
    fn native_player_helper() {
        let args: Vec<String> = std::env::args().collect();
        let setting = |name: &str| args.iter().find_map(|arg| arg.strip_prefix(name)).unwrap();
        let directory = Path::new(setting("splitcast-test-dir="));
        let mode = setting("splitcast-test-mode=");
        let mut starts = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("starts"))
            .unwrap();
        writeln!(starts, "{mode}").unwrap();
        let marker = if mode == "descendant" {
            "descendant"
        } else {
            "leader"
        };
        fs::write(directory.join(marker), std::process::id().to_string()).unwrap();
        if mode == "hang" {
            let mut descendant_args = args[1..].to_vec();
            let setting = descendant_args
                .iter_mut()
                .find(|arg| arg.starts_with("splitcast-test-mode="))
                .unwrap();
            *setting = "splitcast-test-mode=descendant".into();
            // The descendant deliberately remains in the inherited owned group.
            // The controller's run_operation, not this test helper, owns teardown.
            let mut descendant = Command::new(std::env::current_exe().unwrap())
                .args(descendant_args)
                .stdin(Stdio::null())
                .spawn()
                .unwrap();
            let status = descendant.wait().unwrap();
            panic!("inert descendant exited before group teardown: {status}");
        }
        if matches!(mode, "hang" | "descendant" | "unrelated") {
            loop {
                thread::park();
            }
        }
        let mut pcm = Vec::new();
        io::stdin().read_to_end(&mut pcm).unwrap();
        let raw = args.iter().position(|arg| arg == "--raw").unwrap();
        fs::write(
            directory.join("observed.json"),
            json!({"argv": &args[raw..], "pcm": pcm}).to_string(),
        )
        .unwrap();
        if mode == "nonzero" {
            eprintln!("inert player rejection");
            std::process::exit(23);
        }
        if mode == "noisy" {
            io::stdout().write_all(&vec![b'x'; OUTPUT_CAP + 1]).unwrap();
        }
    }

    #[test]
    fn validated_pcm_and_exact_target_are_delivered_once_without_recordings() {
        let fixture = Fixture::new();
        let receipt = fixture
            .play("ok", &operation(SAFETY_LIMIT))
            .unwrap()
            .to_json();
        let observed: Value =
            serde_json::from_slice(&fs::read(fixture.0.join("observed.json")).unwrap()).unwrap();
        assert_eq!(observed["pcm"], json!([0, 128, 255, 127, 42, 0, 214, 255]));
        assert_eq!(
            observed["argv"],
            json!([
                "--raw",
                "--format=s16",
                "--rate=48000",
                "--channels=2",
                "--target=fixture.selected",
                "--properties={ node.dont-fallback = true node.dont-reconnect = true application.name = splitcast-tts }",
                "-",
            ])
        );
        assert_eq!(fixture.starts(), 1);
        assert_eq!(receipt["provenance"], "local_fixture");
        assert_eq!(receipt["completion_scope"], "native_player_process");
        assert_eq!(receipt["player_process"], "completed");
        assert_eq!(receipt["listening"], "unverified");
        assert_eq!(receipt["recording_files_created"], 0);
        fixture.assert_source_intact();
        let mut files: Vec<_> = fs::read_dir(&fixture.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        files.sort();
        assert_eq!(
            files,
            ["input.wav", "leader", "observed.json", "sentinel", "starts"]
                .map(std::ffi::OsString::from)
        );
        assert!(!Path::new(&format!("/proc/{}", fixture.pid("leader"))).exists());
    }

    #[test]
    fn invalid_targets_wavs_and_expired_operations_start_no_helpers() {
        let fixture = Fixture::new();
        for target in [
            "",
            "auto",
            "AUTO",
            "default",
            "default_sink",
            "none",
            "0",
            "000",
            "1",
            "1234",
            "-1",
            "4294967295",
            "9999999999999",
            "@DEFAULT_SINK@",
            "a b",
            "a\nb",
            "a/b",
            "a;exit",
            "--target=x",
        ] {
            let result = play_fixture_with(
                &fixture.input(),
                target,
                &operation(SAFETY_LIMIT),
                std::env::current_exe().unwrap().to_str().unwrap(),
                &fixture.prefix("ok"),
            );
            assert!(
                matches!(result, Err(PlaybackError::InvalidTarget)),
                "{target}: {result:?}"
            );
        }
        for target in ["fixture.selected", "alsa_output.fixture.analog-stereo"] {
            assert!(valid_target(target));
        }
        for bytes in [
            b"not a WAV".to_vec(),
            vec![],
            {
                let mut bytes = sample_wav();
                bytes[32] = 1;
                bytes
            },
            {
                let mut bytes = sample_wav();
                bytes.extend_from_slice(&sample_wav()[36..]);
                let length = (bytes.len() - 8) as u32;
                bytes[4..8].copy_from_slice(&length.to_le_bytes());
                bytes
            },
        ] {
            fs::write(fixture.input(), bytes).unwrap();
            assert!(matches!(
                fixture.play("ok", &operation(SAFETY_LIMIT)),
                Err(PlaybackError::Input(_))
            ));
        }
        fs::write(fixture.input(), sample_wav()).unwrap();
        assert!(
            matches!(fixture.play("ok", &operation(Duration::ZERO)), Err(PlaybackError::Operation(error)) if error.kind() == io::ErrorKind::TimedOut)
        );
        let cancelled = Operation::new(Arc::new(AtomicBool::new(true)), None, SAFETY_LIMIT);
        assert!(
            matches!(fixture.play("ok", &cancelled), Err(PlaybackError::Operation(error)) if error.kind() == io::ErrorKind::Interrupted)
        );
        assert_eq!(fixture.starts(), 0);
        fixture.assert_source_intact();
    }

    #[test]
    fn data_before_format_never_starts_player() {
        let fixture = Fixture::new();
        let wav = sample_wav();
        // Keep both complete chunks and the RIFF size. Change only their order.
        let reordered = [&wav[..12], &wav[36..], &wav[12..36]].concat();
        fs::write(fixture.input(), reordered).unwrap();
        let result = fixture.play("ok", &operation(SAFETY_LIMIT));
        assert_eq!(fixture.starts(), 0);
        assert!(matches!(result, Err(PlaybackError::Input(_))));
    }

    #[test]
    fn rejects_symlink_directory_fifo_and_oversized_files_without_spawn() {
        let fixture = Fixture::new();
        let link = fixture.0.join("link");
        symlink(fixture.input(), &link).unwrap();
        let fifo = fixture.0.join("fifo");
        // Pure filesystem fixture creation; no shell or device operation.
        let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
        assert!(status.success());
        let large = fixture.0.join("large");
        fs::File::create(&large)
            .unwrap()
            .set_len(AUDIO_LIMIT as u64 + 1)
            .unwrap();
        let start = Instant::now();
        for input in [&link, &fifo, &large, &fixture.0] {
            let result = play_fixture_with(
                input,
                "fixture.selected",
                &operation(SAFETY_LIMIT),
                std::env::current_exe().unwrap().to_str().unwrap(),
                &fixture.prefix("ok"),
            );
            assert!(
                matches!(result, Err(PlaybackError::Input(_))),
                "{input:?}: {result:?}"
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "special-file open blocked"
        );
        assert_eq!(fixture.starts(), 0);
        fixture.assert_source_intact();
    }

    #[test]
    fn missing_nonzero_and_output_limit_fail_without_retry() {
        let fixture = Fixture::new();
        let result = play_fixture_with(
            &fixture.input(),
            "fixture.selected",
            &operation(SAFETY_LIMIT),
            fixture.0.join("missing-player").to_str().unwrap(),
            &[],
        );
        assert!(
            matches!(result, Err(PlaybackError::Operation(error)) if error.kind() == io::ErrorKind::NotFound)
        );
        assert_eq!(fixture.starts(), 0);
        let result = fixture.play("nonzero", &operation(SAFETY_LIMIT));
        assert!(
            matches!(result, Err(PlaybackError::PlayerExit { status, stderr }) if status.code() == Some(23) && stderr.contains("inert player rejection"))
        );
        assert_eq!(fixture.starts(), 1);
        let result = fixture.play("noisy", &operation(SAFETY_LIMIT));
        assert!(
            matches!(result, Err(PlaybackError::Operation(error)) if error.to_string().contains("output limit"))
        );
        assert_eq!(fixture.starts(), 2);
        fixture.assert_source_intact();
    }

    fn assert_terminated(pid: u32) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                Ok(stat)
                    if stat
                        .rsplit_once(')')
                        .unwrap()
                        .1
                        .trim_start()
                        .starts_with('Z') =>
                {
                    return
                }
                _ => (),
            }
            assert!(Instant::now() < deadline, "owned process {pid} survived");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn cancellation_and_deadline_release_only_the_owned_group() {
        let unrelated_fixture = Fixture::new();
        let mut argv = vec![std::env::current_exe()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()];
        argv.extend(unrelated_fixture.prefix("unrelated"));
        let mut unrelated = crate::process::OwnedProcess::spawn(&argv).unwrap();
        unrelated_fixture.pid("leader");
        for cancel in [true, false] {
            let fixture = Fixture::new();
            let shutdown = Arc::new(AtomicBool::new(false));
            let operation = Operation::new(
                Arc::clone(&shutdown),
                None,
                if cancel {
                    SAFETY_LIMIT
                } else {
                    Duration::from_secs(1)
                },
            );
            thread::scope(|scope| {
                let worker = scope.spawn(|| fixture.play("hang", &operation));
                let leader = fixture.pid("leader");
                let descendant = fixture.pid("descendant");
                if cancel {
                    shutdown.store(true, Ordering::Relaxed);
                }
                let result = worker.join().unwrap();
                assert!(
                    matches!(result, Err(PlaybackError::Operation(error)) if error.kind() == if cancel { io::ErrorKind::Interrupted } else { io::ErrorKind::TimedOut })
                );
                assert!(
                    !Path::new(&format!("/proc/{leader}")).exists(),
                    "leader not reaped"
                );
                assert_terminated(descendant);
            });
            assert_eq!(
                fixture.starts(),
                2,
                "one leader and one owned descendant, no retry"
            );
            fixture.assert_source_intact();
            assert!(unrelated.try_wait().unwrap().is_none());
        }
        unrelated.stop(Duration::ZERO).unwrap();
    }

    #[test]
    fn player_deadline_uses_audio_duration_plus_finite_startup_allowance() {
        let fixture = Fixture::new();
        let start = Instant::now();
        let result = fixture.play("hang", &operation(SAFETY_LIMIT));
        assert!(
            matches!(result, Err(PlaybackError::Operation(error)) if error.kind() == io::ErrorKind::TimedOut)
        );
        assert!(start.elapsed() < Duration::from_secs(4));
        assert!(!Path::new(&format!("/proc/{}", fixture.pid("leader"))).exists());
        assert_terminated(fixture.pid("descendant"));
        assert_eq!(fixture.starts(), 2);
        fixture.assert_source_intact();
    }
}
