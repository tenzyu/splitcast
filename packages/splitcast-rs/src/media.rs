//! Native subprocess command specification. Uses the OS's existing hardware
//! codecs / GSR / FFmpeg. No game injection, no glReadPixels or frame capture
//! in the official osu! process. All arguments are passed as argv, not shell.
use crate::config::Config;
use crate::request::Operation;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|x| x.to_string()).collect()
}
fn err(s: &str) -> io::Error {
    io::Error::other(s)
}
pub(crate) fn command_operation(
    argv: &[String],
    operation: Option<&crate::request::Operation>,
) -> io::Result<String> {
    let (exe, rest) = argv.split_first().ok_or_else(|| err("empty command"))?;
    let r = crate::process::run_operation(
        exe,
        rest,
        None,
        Duration::from_secs(5),
        8 * 1024 * 1024,
        operation,
    )?;
    if !r.status.success() {
        return Err(err("native helper exited nonzero; inspect its logs"));
    }
    Ok(String::from_utf8_lossy(&r.stdout).trim().to_string())
}

// Prepare checks API availability before any publishers are required.
pub(crate) fn receive_operation(cfg: &Config, operation: Option<&Operation>) -> io::Result<String> {
    Ok(decode_paths_document(&read_paths(cfg.api, operation)?)?.to_string())
}

pub(crate) fn receive_snapshot(
    api_port: u16,
    require_camera: bool,
    operation: Option<&Operation>,
) -> io::Result<ReceiveSnapshot> {
    ReceiveSnapshot::decode(&read_paths(api_port, operation)?, require_camera)
}

fn read_paths(api_port: u16, operation: Option<&Operation>) -> io::Result<Vec<u8>> {
    let timeout = match operation {
        Some(operation) => operation
            .deadline(Duration::from_secs(2))?
            .saturating_duration_since(Instant::now()),
        None => Duration::from_secs(2),
    };
    crate::http::get_body(api_port, "/v3/paths/list", timeout, 1024 * 1024)
}

fn decode_paths_document(body: &[u8]) -> io::Result<Value> {
    serde_json::from_slice(body).map_err(|_| err("invalid loopback JSON"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceivePathError {
    Absent,
    Ambiguous,
    InvalidReadiness,
    InvalidByteCounter,
    MissingTracks,
    InvalidTrack,
}

impl fmt::Display for ReceivePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Absent => "media path absent",
            Self::Ambiguous => "ambiguous media path",
            Self::InvalidReadiness => "media readiness missing",
            Self::InvalidByteCounter => "invalid media byte counter",
            Self::MissingTracks => "media tracks missing",
            Self::InvalidTrack => "invalid media track",
        })
    }
}

impl std::error::Error for ReceivePathError {}

enum FeedKind {
    Game,
    Mic,
    Camera,
}

#[derive(Clone, Copy, Debug)]
struct FeedSample {
    bytes_received: u64,
    usable: bool,
}

type FeedObservation = Result<FeedSample, ReceivePathError>;

/// The required feed observations from one receiver response.
#[derive(Debug)]
pub(crate) struct ReceiveSnapshot {
    game: FeedObservation,
    mic: FeedObservation,
    camera: Option<FeedObservation>,
}

impl ReceiveSnapshot {
    fn decode(body: &[u8], require_camera: bool) -> io::Result<Self> {
        let document = decode_paths_document(body)?;
        let paths = document["items"]
            .as_array()
            .ok_or_else(|| err("MediaMTX items required"))?;
        Ok(Self {
            game: parse_feed(paths, FeedKind::Game),
            mic: parse_feed(paths, FeedKind::Mic),
            camera: require_camera.then(|| parse_feed(paths, FeedKind::Camera)),
        })
    }
}

fn parse_feed(paths: &[Value], kind: FeedKind) -> FeedObservation {
    let name = match kind {
        FeedKind::Game => "game",
        FeedKind::Mic => "mic",
        FeedKind::Camera => "camera",
    };
    let mut matches = paths.iter().filter(|p| p["name"].as_str() == Some(name));
    let item = matches.next().ok_or(ReceivePathError::Absent)?;
    if matches.next().is_some() {
        return Err(ReceivePathError::Ambiguous);
    }
    let ready = item["ready"]
        .as_bool()
        .ok_or(ReceivePathError::InvalidReadiness)?;
    let bytes_received = item["bytesReceived"]
        .as_u64()
        .ok_or(ReceivePathError::InvalidByteCounter)?;
    let tracks = item["tracks"]
        .as_array()
        .ok_or(ReceivePathError::MissingTracks)?;
    if tracks.iter().any(|v| !v.is_string()) {
        return Err(ReceivePathError::InvalidTrack);
    }
    let contains_h264 = || tracks.iter().any(|track| track == "H264");
    let first_audio_codec = || {
        tracks
            .iter()
            .filter_map(Value::as_str)
            .find(|v| matches!(*v, "MPEG-4 Audio" | "Opus"))
    };
    let codecs_match = match kind {
        FeedKind::Game => contains_h264() && first_audio_codec() == Some("MPEG-4 Audio"),
        FeedKind::Mic => first_audio_codec() == Some("Opus"),
        FeedKind::Camera => tracks.len() == 1 && contains_h264(),
    };
    Ok(FeedSample {
        bytes_received,
        usable: ready && bytes_received > 0 && codecs_match,
    })
}

#[derive(Default)]
struct FeedProgress {
    previous: Option<(u64, Instant)>,
}

impl FeedProgress {
    fn observe(&mut self, sample: FeedSample, now: Instant) -> bool {
        let previous = self.previous.replace((sample.bytes_received, now));
        sample.usable
            && previous.is_some_and(|(bytes, at)| {
                sample.bytes_received > bytes
                    && now
                        .checked_duration_since(at)
                        .is_some_and(|age| age <= Duration::from_secs(3))
            })
    }
}

pub(crate) struct ReceiveMonitor {
    game: FeedProgress,
    mic: FeedProgress,
    camera: Option<FeedProgress>,
}

impl ReceiveMonitor {
    pub(crate) fn new(require_camera: bool) -> Self {
        Self {
            game: FeedProgress::default(),
            mic: FeedProgress::default(),
            camera: require_camera.then(FeedProgress::default),
        }
    }

    pub(crate) fn observe(
        &mut self,
        snapshot: &ReceiveSnapshot,
        now: Instant,
    ) -> Result<bool, ReceivePathError> {
        // Readiness retries keep each typed sample up to the first path error,
        // including samples whose counters or codecs make them unusable.
        let mut healthy = self.game.observe(snapshot.game?, now);
        healthy &= self.mic.observe(snapshot.mic?, now);
        if let Some(camera) = &mut self.camera {
            let sample = snapshot.camera.ok_or(ReceivePathError::Absent)??;
            healthy &= camera.observe(sample, now);
        }
        Ok(healthy)
    }
}
pub fn game(cfg: &Config) -> io::Result<Vec<String>> {
    if cfg.target.is_empty() || cfg.game_audio.is_empty() {
        return Err(err("explicit capture.target and game_audio required"));
    }
    if !cfg.game_audio.starts_with("app:")
        || cfg.game_audio[4..].is_empty()
        || cfg.game_audio.contains(['|', ';', '\n', '\r', '\0'])
        || cfg.game_audio.eq_ignore_ascii_case("app:splitcast-tts")
    {
        return Err(err(
            "game audio must select only the authorized game application",
        ));
    }
    let mut a = strings(&["gpu-screen-recorder", "-w"]);
    a.push(cfg.target.clone());
    a.push("-s".into());
    a.push(format!("{}x{}", cfg.width, cfg.height));
    a.extend(strings(&[
        "-f",
        &cfg.fps.to_string(),
        "-fm",
        "cfr",
        "-k",
        "h264",
        "-encoder",
        "gpu",
        "-fallback-cpu-encoding",
        "no",
        "-tune",
        "performance",
        "-bm",
        "cbr",
        "-q",
        &cfg.bitrate.to_string(),
        "-keyint",
        "2",
        "-a",
    ]));
    a.push(cfg.game_audio.clone());
    a.extend(strings(&[
        "-ac", "aac", "-ab", "192", "-cursor", "no", "-c", "flv", "-o",
    ]));
    a.push(format!("rtmp://127.0.0.1:{}/game", cfg.rtmp));
    Ok(a)
}
pub(crate) fn mic(
    cfg: &Config,
    operation: Option<&crate::request::Operation>,
) -> io::Result<Vec<String>> {
    if cfg.mic_source.is_empty() {
        return Err(err("explicit mic_source required"));
    }
    let sources = command_operation(
        &strings(&["pactl", "-f", "json", "list", "sources"]),
        operation,
    )?;
    let sources: serde_json::Value =
        serde_json::from_str(&sources).map_err(|_| err("invalid microphone inventory"))?;
    if !sources.as_array().is_some_and(|sources| {
        sources.iter().any(|source| {
            source["name"].as_str() == Some(cfg.mic_source.as_str())
                && source["properties"]["device.api"] == "alsa"
                && source["properties"]["device.class"] == "sound"
        })
    }) {
        return Err(err(
            "selected microphone must be an existing hardware ALSA source",
        ));
    }
    let mut a = strings(&[
        "ffmpeg",
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-thread_queue_size",
        "256",
        "-f",
        "pulse",
        "-sample_rate",
        "48000",
        "-channels",
        "1",
        "-i",
    ]);
    a.push(cfg.mic_source.clone());
    a.extend(strings(&[
        "-vn",
        "-ac",
        "1",
        "-ar",
        "48000",
        "-c:a",
        "libopus",
        "-b:a",
        "96k",
        "-application",
        "lowdelay",
        "-f",
        "rtsp",
        "-rtsp_transport",
        "tcp",
    ]));
    a.push(format!("rtsp://127.0.0.1:{}/mic", cfg.rtsp));
    Ok(a)
}
pub fn rnnoise(cfg: &Config) -> io::Result<Vec<String>> {
    if cfg.processed_sink.is_empty() || !cfg.processed_sink.starts_with("splitcast_") {
        return Err(err(
            "processed audio sink must be explicitly SplitCast-owned",
        ));
    }
    let a=format!("aresample=48000:async=1000,aformat=channel_layouts=mono,ladspa=file={}:plugin={}:controls={},highpass=f=80,volume={}dB,alimiter=limit=0.95",
        cfg.rnnoise_plugin.display(),cfg.rnnoise_label,cfg.rnnoise_control,cfg.gain_db);
    Ok(vec![
        "ffmpeg".into(),
        "-hide_banner".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-rtsp_transport".into(),
        "tcp".into(),
        "-i".into(),
        format!("rtsp://127.0.0.1:{}/mic", cfg.rtsp),
        "-vn".into(),
        "-af".into(),
        a,
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "1".into(),
        "-f".into(),
        "pulse".into(),
        // Keep the private playback buffer below the startup continuity budget.
        "-buffer_duration".into(),
        "100".into(),
        "-prebuf".into(),
        "1920".into(), // 20 ms at 48 kHz, mono s16le.
        "-stream_name".into(),
        "SplitCast processed microphone".into(),
        "-device".into(),
        cfg.processed_sink.clone(),
        "SplitCast processed microphone".into(),
    ])
}
pub(crate) fn camera(cfg: &Config) -> io::Result<Option<Vec<String>>> {
    use std::os::unix::fs::FileTypeExt;
    let Some(camera) = cfg
        .camera
        .as_ref()
        .filter(|camera| camera.location == cfg.role)
    else {
        return Ok(None);
    };
    if !camera
        .device
        .metadata()
        .is_ok_and(|m| m.file_type().is_char_device())
    {
        return Err(err("selected V4L2 camera is unavailable"));
    }
    let mut args = strings(&["ffmpeg", "-hide_banner", "-nostdin", "-loglevel", "error"]);
    if let Some(render) = camera
        .render_device
        .as_ref()
        .filter(|_| camera.encoder == "h264_vaapi")
    {
        args.extend(strings(&["-vaapi_device", &render.to_string_lossy()]));
    }
    args.extend(strings(&[
        "-f",
        "v4l2",
        "-input_format",
        &camera.input_format,
        "-video_size",
        &format!("{}x{}", camera.width, camera.height),
        "-framerate",
        &camera.fps.to_string(),
        "-i",
        &camera.device.to_string_lossy(),
        "-map",
        "0:v:0",
        "-an",
        "-c:v",
        &camera.encoder,
    ]));
    match camera.encoder.as_str() {
        "h264_vaapi" => args.extend(strings(&["-vf", "format=nv12,hwupload"])),
        "libx264" => args.extend(strings(&[
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
        ])),
        _ => {}
    }
    if camera.encoder != "copy" {
        args.extend(strings(&[
            "-b:v",
            "2M",
            "-g",
            &camera.fps.to_string(),
            "-bf",
            "0",
        ]));
    }
    args.extend(strings(&[
        "-f",
        "rtsp",
        "-rtsp_transport",
        "tcp",
        &format!("rtsp://127.0.0.1:{}/camera", cfg.rtsp),
    ]));
    Ok(Some(args))
}

pub fn tunnel(cfg: &Config, media: bool) -> Vec<String> {
    let mut a = vec!["ssh".into()];
    if let Some(file) = &cfg.ssh_config {
        a.extend(["-F".into(), file.to_string_lossy().into_owned()]);
    }
    a.extend(strings(&[
        "-N",
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ControlPath=none",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ServerAliveInterval=5",
        "-o",
        "ServerAliveCountMax=2",
        "-o",
        "ConnectTimeout=5",
        "-p",
        &cfg.ssh_port.to_string(),
    ]));
    let control = [(cfg.forward_control_port, cfg.remote_control_port)];
    let transport = [(cfg.rtmp, cfg.rtmp), (cfg.rtsp, cfg.rtsp)];
    for (local, remote) in control.iter().chain(transport.iter().filter(|_| media)) {
        a.push("-L".into());
        a.push(format!("127.0.0.1:{local}:127.0.0.1:{remote}"));
    }
    a.push("--".into());
    a.push(cfg.ssh_host.clone());
    a
}
pub fn receiver(cfg: &Config) -> io::Result<Vec<String>> {
    let root = &cfg.state_dir;
    fs::create_dir_all(root)?;
    let filename = root.join("mediamtx-rs.json");
    // All ports come from bounded unsigned integers and never from user shell.
    let mut contents = serde_json::json!({
        "logLevel":"warn","readTimeout":"5s","writeTimeout":"5s","writeQueueSize":128,
        "rtsp":true,"rtspAddress":format!("127.0.0.1:{}",cfg.rtsp),"rtspTransports":["tcp"],
        "rtmp":true,"rtmpAddress":format!("127.0.0.1:{}",cfg.rtmp),
        "api":true,"apiAddress":format!("127.0.0.1:{}",cfg.api),
        "hls":false,"webrtc":false,"srt":false,"metrics":false,"pprof":false,"playback":false,
        "paths":{"game":{"source":"publisher"},"mic":{"source":"publisher"}}
    });
    if cfg.camera.is_some() {
        contents["paths"]["camera"] = serde_json::json!({"source":"publisher"});
    }
    fs::write(&filename, contents.to_string())?;
    Ok(vec![
        "mediamtx".into(),
        filename.to_string_lossy().to_string(),
    ])
}
pub(crate) fn missing_tools(cfg: &Config, media: bool, start: bool) -> Vec<String> {
    let mut req = Vec::new();
    if cfg.role == "capture" {
        req.push("ssh");
        if cfg.tts_enabled {
            req.push("pw-play");
        }
        if media && start {
            req.extend(["gpu-screen-recorder", "ffmpeg", "pactl"]);
        }
    } else if media {
        req.push("mediamtx");
        if start {
            req.extend(["pulseaudio", "pactl", "ffmpeg"]);
        }
    }
    let mut missing = vec![];
    for name in req {
        let present = std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| {
                let path = dir.join(name);
                path.is_file()
                    && path
                        .metadata()
                        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
            })
        });
        if !present {
            missing.push(name.into());
        }
    }
    missing
}
pub(crate) fn devices_operation(
    role: &str,
    operation: Option<&crate::request::Operation>,
) -> io::Result<serde_json::Value> {
    use serde_json::{json, Value};
    let run = |program: &str, args: &[&str]| {
        let mut argv = vec![program.to_owned()];
        argv.extend(args.iter().map(|s| (*s).to_owned()));
        command_operation(&argv, operation)
    };
    let mut result = json!({"role":role,"sources":[],"sinks":[],"capture_targets":"","applications":"","errors":[]});
    let mut errors = Vec::new();
    for kind in ["sources", "sinks"] {
        if let Some(operation) = operation {
            operation.check()?;
        }
        let devices = run("pactl", &["-f", "json", "list", kind]).and_then(|s| {
            serde_json::from_str::<Value>(&s).map_err(|_| err("invalid audio discovery"))
        });
        match devices {
            Ok(Value::Array(items)) => {
                result[kind]=Value::Array(items.iter().filter_map(|v|v["name"].as_str().map(|name|json!({"name":name,"description":v["description"].as_str().unwrap_or(name)}))).collect());
            }
            _ => errors.push(format!("cannot discover Pulse {kind}")),
        }
    }
    if role == "capture" {
        for (key, option) in [
            ("capture_targets", "--list-capture-options"),
            ("applications", "--list-application-audio"),
        ] {
            if let Some(operation) = operation {
                operation.check()?;
            }
            match run("gpu-screen-recorder", &[option]) {
                Ok(value) => result[key] = json!(value),
                Err(_) => errors.push(format!("GSR {key} discovery unavailable")),
            }
        }
    }
    result["errors"] = json!(errors);
    let mut cameras: Vec<_> = fs::read_dir("/dev/v4l/by-id")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.path().into_os_string().into_string().ok())
        .collect();
    cameras.sort();
    result["cameras"] = json!(cameras);
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feed(name: &str, bytes: u64, tracks: Value) -> Value {
        json!({"name": name, "ready": true, "bytesReceived": bytes, "tracks": tracks})
    }

    fn feeds(game: u64, mic: u64, camera: u64) -> [Value; 3] {
        [
            feed("game", game, json!(["H264", "MPEG-4 Audio"])),
            feed("mic", mic, json!(["Opus"])),
            feed("camera", camera, json!(["H264"])),
        ]
    }

    fn snapshot(items: &[Value], camera: bool) -> ReceiveSnapshot {
        ReceiveSnapshot::decode(json!({"items": items}).to_string().as_bytes(), camera).unwrap()
    }

    #[test]
    fn snapshot_requires_a_json_array_of_paths() {
        for body in [b"{".as_slice(), b"\xff"] {
            assert_eq!(
                ReceiveSnapshot::decode(body, false)
                    .unwrap_err()
                    .to_string(),
                "invalid loopback JSON"
            );
        }
        for body in [b"{}".as_slice(), br#"{"items":{}}"#] {
            assert_eq!(
                ReceiveSnapshot::decode(body, false)
                    .unwrap_err()
                    .to_string(),
                "MediaMTX items required"
            );
        }
        let empty = ReceiveSnapshot::decode(br#"{"items":[]}"#, false).unwrap();
        assert_eq!(empty.game.unwrap_err(), ReceivePathError::Absent);
        assert_eq!(empty.mic.unwrap_err(), ReceivePathError::Absent);
        assert!(empty.camera.is_none());
    }

    #[test]
    fn required_paths_are_unique_and_fields_are_typed() {
        use ReceivePathError::*;

        let [game, mic, camera] = feeds(4, 4, 4);
        for (field, bad, expected) in [
            ("ready", json!("true"), InvalidReadiness),
            ("bytesReceived", json!("4"), InvalidByteCounter),
            ("bytesReceived", json!(-1), InvalidByteCounter),
            ("bytesReceived", json!(4.0), InvalidByteCounter),
            ("tracks", Value::Null, MissingTracks),
            ("tracks", json!(["H264", false]), InvalidTrack),
        ] {
            let mut row = game.clone();
            row[field] = bad;
            assert_eq!(snapshot(&[row], false).game.unwrap_err(), expected);
        }
        let malformed = json!({"name": "game"});
        assert_eq!(
            snapshot(std::slice::from_ref(&malformed), false)
                .game
                .unwrap_err(),
            InvalidReadiness
        );
        assert_eq!(
            snapshot(&[malformed, game], false).game.unwrap_err(),
            Ambiguous
        );
        assert_eq!(
            snapshot(&[mic.clone(), mic], false).mic.unwrap_err(),
            Ambiguous
        );
        assert_eq!(
            snapshot(&[camera.clone(), camera], true)
                .camera
                .unwrap()
                .unwrap_err(),
            Ambiguous
        );
    }

    #[test]
    fn codecs_and_optional_camera_follow_the_existing_feed_rules() {
        let [game, mic, camera] = feeds(4, 4, 4);
        let required = snapshot(&[game.clone(), mic.clone(), camera], true);
        assert!(required.game.unwrap().usable);
        assert!(required.mic.unwrap().usable);
        assert!(required.camera.unwrap().unwrap().usable);

        for (kind, name, tracks, usable) in [
            (
                FeedKind::Game,
                "game",
                json!(["h264", "MPEG-4 Audio"]),
                false,
            ),
            (FeedKind::Game, "game", json!(["H264", "Opus"]), false),
            (
                FeedKind::Game,
                "game",
                json!(["H264", "Opus", "MPEG-4 Audio"]),
                false,
            ),
            (
                FeedKind::Game,
                "game",
                json!(["G711", "MPEG-4 Audio", "Opus", "H264"]),
                true,
            ),
            (FeedKind::Mic, "mic", json!(["MPEG-4 Audio", "Opus"]), false),
            (FeedKind::Mic, "mic", json!(["H264", "Opus"]), true),
            (FeedKind::Camera, "camera", json!(["H264", "G711"]), false),
            (FeedKind::Camera, "camera", json!(["H264", "H264"]), false),
        ] {
            assert_eq!(
                parse_feed(&[feed(name, 4, tracks)], kind).unwrap().usable,
                usable
            );
        }

        let rows = [
            Value::Null,
            json!({"name": 1}),
            json!({"name": "Game"}),
            json!({"name": "other"}),
            json!({"name": "other", "tracks": [false]}),
            json!({"name": "camera"}),
            json!({"name": "camera", "ready": false}),
            mic,
            game,
        ];
        let optional = snapshot(&rows, false);
        assert!(optional.game.unwrap().usable);
        assert!(optional.mic.unwrap().usable);
        assert!(optional.camera.is_none());
        assert_eq!(
            snapshot(&rows, true).camera.unwrap().unwrap_err(),
            ReceivePathError::Ambiguous
        );
        assert_eq!(
            snapshot(&rows[7..], true).camera.unwrap().unwrap_err(),
            ReceivePathError::Absent
        );
    }
    #[test]
    fn receiver_api_needs_no_external_http_tool() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut cfg = Config::parse(include_bytes!("../tests/agent-example.json")).unwrap();
        cfg.api = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut input = [0; 4096];
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    assert!(stream.read(&mut input).unwrap() > 0);
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"items\":[]}").unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let result = receive_operation(&cfg, None);
        server.join().unwrap();
        assert_eq!(result.unwrap(), "{\"items\":[]}");
    }
    #[test]
    fn progress_uses_strict_bytes_and_an_inclusive_three_second_window() {
        let mut monitor = ReceiveMonitor::new(true);
        let start = Instant::now();
        for (bytes, millis, expected) in [
            (10, 0, false),
            (11, 3000, true),
            (12, 6001, false),
            (13, 6000, false),
            (13, 7000, false),
            (12, 8000, false),
            (13, 9000, true),
            (0, 10000, false),
            (1, 11000, true),
        ] {
            let sample = snapshot(&feeds(bytes, bytes, bytes), true);
            let observed = monitor
                .observe(&sample, start + Duration::from_millis(millis))
                .unwrap();
            assert_eq!(observed, expected, "bytes={bytes} millis={millis}");
        }
    }

    #[test]
    fn a_path_error_advances_only_the_typed_prefix_of_the_snapshot() {
        let start = Instant::now();
        // Two recovery samples isolate the effects on the earlier and later paths.
        for (game_bytes, expected) in [(20, false), (21, true)] {
            let mut monitor = ReceiveMonitor::new(true);
            assert!(!monitor
                .observe(&snapshot(&feeds(10, 10, 10), true), start)
                .unwrap());
            let [game, _, camera] = feeds(20, 20, 20);
            let malformed = snapshot(&[camera, json!({"name": "mic"}), game], true);
            assert_eq!(
                monitor.observe(&malformed, start + Duration::from_secs(1)),
                Err(ReceivePathError::InvalidReadiness)
            );
            let recovered = snapshot(&feeds(game_bytes, 11, 11), true);
            assert_eq!(
                monitor.observe(&recovered, start + Duration::from_secs(2)),
                Ok(expected)
            );
        }
    }

    #[test]
    fn unusable_typed_feeds_still_advance_all_baselines() {
        let start = Instant::now();
        let mut monitor = ReceiveMonitor::new(false);
        let [mut game, mic, _] = feeds(20, 20, 0);
        game["ready"] = json!(false);
        let unusable = snapshot(&[game, mic], false);
        assert!(!unusable.game.unwrap().usable);
        assert_eq!(monitor.observe(&unusable, start), Ok(false));
        let stalled_mic = snapshot(&feeds(21, 20, 0), false);
        assert_eq!(
            monitor.observe(&stalled_mic, start + Duration::from_secs(1)),
            Ok(false)
        );
        let advancing = snapshot(&feeds(22, 21, 0), false);
        assert_eq!(
            monitor.observe(&advancing, start + Duration::from_secs(2)),
            Ok(true)
        );
    }
}
