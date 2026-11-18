//! Native subprocess command specification. Uses the OS's existing hardware
//! codecs / GSR / FFmpeg. No game injection, no glReadPixels or frame capture
//! in the official osu! process. All arguments are passed as argv, not shell.
use crate::config::Config;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaObservation {
    pub ready: bool,
    pub bytes_received: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    track_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MediaProgress {
    previous_bytes: Option<u64>,
    observed_at: Option<Instant>,
    kind: FeedKind,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum FeedKind {
    #[default]
    Game,
    Mic,
    Camera,
}

pub struct ReceiveMonitor {
    game: MediaProgress,
    mic: MediaProgress,
    camera: Option<MediaProgress>,
}

impl ReceiveMonitor {
    pub fn new(camera: bool) -> Self {
        Self {
            game: MediaProgress::new(FeedKind::Game),
            mic: MediaProgress::new(FeedKind::Mic),
            camera: camera.then(|| MediaProgress::new(FeedKind::Camera)),
        }
    }

    pub fn observe(&mut self, document: &str, now: Instant) -> io::Result<bool> {
        let mut healthy = true;
        for (name, progress) in [("game", &mut self.game), ("mic", &mut self.mic)]
            .into_iter()
            .chain(self.camera.iter_mut().map(|progress| ("camera", progress)))
        {
            let sample = parse_media_observation(document, name)?;
            healthy &= progress.observe_at(&sample, now);
        }
        Ok(healthy)
    }
}

#[cfg(test)]
pub fn receive_snapshot(cfg: &Config) -> io::Result<String> {
    receive_operation(cfg, None)
}
pub(crate) fn receive_operation(
    cfg: &Config,
    operation: Option<&crate::request::Operation>,
) -> io::Result<String> {
    let timeout = match operation {
        Some(operation) => operation
            .deadline(Duration::from_secs(2))?
            .saturating_duration_since(Instant::now()),
        None => Duration::from_secs(2),
    };
    crate::http::get_json(cfg.api, "/v3/paths/list", timeout, 1024 * 1024).map(|v| v.to_string())
}

impl MediaObservation {
    fn usable(&self, kind: FeedKind) -> bool {
        self.ready
            && self.bytes_received > 0
            && match kind {
                FeedKind::Game => {
                    self.video_codec.as_deref() == Some("H264")
                        && self.audio_codec.as_deref() == Some("MPEG-4 Audio")
                }
                FeedKind::Mic => self.audio_codec.as_deref() == Some("Opus"),
                FeedKind::Camera => {
                    self.video_codec.as_deref() == Some("H264") && self.track_count == 1
                }
            }
    }
}

impl MediaProgress {
    pub fn new(kind: FeedKind) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }
    #[cfg(test)]
    pub fn observe(&mut self, sample: &MediaObservation) -> bool {
        self.observe_at(sample, Instant::now())
    }
    pub fn observe_at(&mut self, sample: &MediaObservation, now: Instant) -> bool {
        let progressing = self
            .previous_bytes
            .is_some_and(|n| sample.bytes_received > n);
        let recent = self.observed_at.is_some_and(|then| {
            now.checked_duration_since(then)
                .is_some_and(|d| d <= Duration::from_secs(3))
        });
        self.previous_bytes = Some(sample.bytes_received);
        self.observed_at = Some(now);
        sample.usable(self.kind) && progressing && recent
    }
}

pub fn parse_media_observation(json: &str, path: &str) -> io::Result<MediaObservation> {
    if json.len() > 1024 * 1024 {
        return Err(err("MediaMTX response too large"));
    }
    let document: serde_json::Value =
        serde_json::from_str(json).map_err(|_| err("invalid MediaMTX JSON"))?;
    let paths = document["items"]
        .as_array()
        .ok_or_else(|| err("MediaMTX items required"))?;
    let mut matches = paths.iter().filter(|p| p["name"].as_str() == Some(path));
    let item = matches.next().ok_or_else(|| err("media path absent"))?;
    if matches.next().is_some() {
        return Err(err("ambiguous media path"));
    }
    let ready = item["ready"]
        .as_bool()
        .ok_or_else(|| err("media readiness missing"))?;
    let bytes_received = item["bytesReceived"]
        .as_u64()
        .ok_or_else(|| err("invalid media byte counter"))?;
    let tracks = item["tracks"]
        .as_array()
        .ok_or_else(|| err("media tracks missing"))?;
    if tracks.iter().any(|v| !v.is_string()) {
        return Err(err("invalid media track"));
    }
    let video_codec = tracks
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| *v == "H264")
        .map(str::to_owned);
    let audio_codec = tracks
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| matches!(*v, "MPEG-4 Audio" | "Opus"))
        .map(str::to_owned);
    Ok(MediaObservation {
        ready,
        bytes_received,
        video_codec,
        audio_codec,
        track_count: tracks.len(),
    })
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
#[cfg(test)]
pub fn ready_output(buf: &str, name: &str) -> bool {
    let kind = match name {
        "game" => FeedKind::Game,
        "mic" => FeedKind::Mic,
        "camera" => FeedKind::Camera,
        _ => return false,
    };
    parse_media_observation(buf, name).is_ok_and(|o| o.usable(kind))
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

    #[test]
    fn camera_requires_only_one_h264_track() {
        assert!(ready_output(
            r#"{"items":[{"name":"camera","ready":true,"bytesReceived":200,"tracks":["H264"]}]}"#,
            "camera"
        ));
        assert!(!ready_output(
            r#"{"items":[{"name":"camera","ready":true,"bytesReceived":200,"tracks":["H264","G711"]}]}"#,
            "camera"
        ));
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
        let result = receive_snapshot(&cfg);
        server.join().unwrap();
        assert_eq!(result.unwrap(), "{\"items\":[]}");
    }
    #[test]
    fn readiness_requires_typed_fields_and_progress() {
        let mut progress = MediaProgress::default();
        let sample=parse_media_observation(r#"{"items":[{"name":"game","ready":true,"bytesReceived":4,"tracks":["H264","MPEG-4 Audio"]}]}"#,"game").unwrap();
        assert!(!progress.observe(&sample));
        let next=parse_media_observation(r#"{"items":[{"name":"game","ready":true,"bytesReceived":5,"tracks":["H264","MPEG-4 Audio"]}]}"#,"game").unwrap();
        assert!(progress.observe(&next));
        assert!(!progress.observe(&next));
    }
    #[test]
    fn a_path_name_is_not_readiness() {
        assert!(!ready_output(r#"{"items":[{"name":"game"}]}"#, "game"));
    }
    #[test]
    fn microphone_readiness_does_not_require_a_video_track() {
        assert!(
            ready_output(
                r#"{"items":[{"name":"mic","ready":true,"bytesReceived":42,"tracks":["Opus"]}]}"#,
                "mic"
            ),
            "valid audio-only microphone feed was refused"
        );
    }
}
