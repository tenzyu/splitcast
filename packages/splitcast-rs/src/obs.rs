//! Authenticated OBS WebSocket v5 control over loopback TCP only.
use crate::config::{secret, Config, ObsVideoMode};
use crate::state::{atomic_file, private_directory};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tungstenite::{protocol::WebSocketConfig, Message, WebSocket};

const LIMIT: usize = 4 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

fn err(message: &str) -> io::Error {
    io::Error::other(message)
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "OBS control deadline exceeded"))
}

struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
    operation: Option<crate::request::Operation>,
}
impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if let Some(operation) = &self.operation {
            return operation.read(&mut self.stream, bytes, self.deadline);
        }
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(bytes).map_err(normalize_timeout)
    }
}
impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(operation) = &self.operation {
            return operation.write(&mut self.stream, bytes, self.deadline);
        }
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(bytes).map_err(normalize_timeout)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.flush().map_err(normalize_timeout)
    }
}
fn normalize_timeout(e: io::Error) -> io::Error {
    if matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) {
        io::Error::new(io::ErrorKind::TimedOut, "OBS control deadline exceeded")
    } else {
        e
    }
}
fn redacted_websocket_error(e: tungstenite::Error) -> io::Error {
    match e {
        tungstenite::Error::Io(e) => normalize_timeout(e),
        tungstenite::Error::Capacity(_) => err("oversized OBS response"),
        _ => err("invalid or closed OBS WebSocket"),
    }
}
fn base64_sha(input: &[u8]) -> String {
    STANDARD.encode(Sha256::digest(input))
}
fn auth_string<'a>(hello: &'a Value, key: &str) -> io::Result<&'a str> {
    hello["d"]["authentication"][key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| err("OBS authentication string required"))
}
fn response_data(response: &str) -> io::Result<Value> {
    let value: Value = serde_json::from_str(response).map_err(|_| err("invalid OBS JSON"))?;
    value["d"]["responseData"]
        .as_object()
        .map(|_| value["d"]["responseData"].clone())
        .ok_or_else(|| err("OBS response data required"))
}
#[derive(Debug, Clone, PartialEq)]
pub struct OutputStatus {
    pub active: bool,
    pub bytes: Option<u64>,
    pub frames: Option<u64>,
    pub duration_ms: Option<u64>,
}

// The profile and the requested output have the same lifetime. An attempted
// Start is recorded before its RPC because a lost reply can leave output active.
pub(crate) struct SessionProfile {
    directory: PathBuf,
    phase: OutputPhase,
}

enum OutputPhase {
    Configured,
    Requested(crate::Output),
    Observed(crate::Output, OutputStatus),
}

impl SessionProfile {
    pub(crate) fn create(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir(&directory)?;
        Ok(Self {
            directory,
            phase: OutputPhase::Configured,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_attempt(directory: PathBuf, output: crate::Output) -> Self {
        Self {
            directory,
            phase: OutputPhase::Requested(output),
        }
    }

    pub(crate) fn launch_args(&self, cfg: &Config) -> io::Result<Vec<String>> {
        launch_args(cfg, &self.directory)
    }

    pub(crate) fn request_start(&mut self, obs: &mut Obs, output: crate::Output) -> io::Result<()> {
        self.phase = OutputPhase::Requested(output);
        obs.start(output)
    }

    pub(crate) fn requested_output(&self) -> Option<crate::Output> {
        match self.phase {
            OutputPhase::Configured => None,
            OutputPhase::Requested(output) | OutputPhase::Observed(output, _) => Some(output),
        }
    }

    pub(crate) fn observe(&mut self, obs: &mut Obs) -> io::Result<bool> {
        let output = self
            .requested_output()
            .ok_or_else(|| err("owned OBS output unavailable"))?;
        let current = obs.observe(output)?;
        let progressing = match &self.phase {
            OutputPhase::Observed(_, previous) => {
                current.active
                    && current
                        .bytes
                        .zip(previous.bytes)
                        .is_some_and(|(now, old)| now > old)
                    && current
                        .frames
                        .zip(previous.frames)
                        .is_none_or(|(now, old)| now > old)
            }
            _ => {
                current.active
                    && current.bytes.is_some_and(|bytes| bytes > 0)
                    && current.frames.is_none_or(|frames| frames > 0)
            }
        };
        self.phase = OutputPhase::Observed(output, current);
        Ok(progressing)
    }

    pub(crate) fn progress(&self) -> Option<Value> {
        let OutputPhase::Observed(_, progress) = &self.phase else {
            return None;
        };
        Some(json!({
            "active": progress.active, "bytes": progress.bytes,
            "frames": progress.frames, "duration_ms": progress.duration_ms,
        }))
    }

    pub(crate) fn discard(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}
pub struct Obs {
    operation: Option<crate::request::Operation>,
    socket: WebSocket<DeadlineStream>,
    seq: u64,
    failed: bool,
}
impl Obs {
    #[cfg(test)]
    fn connect(cfg: &Config) -> io::Result<Self> {
        Self::connect_until(cfg, Instant::now() + TIMEOUT)
    }
    pub(crate) fn connect_operation(
        cfg: &Config,
        operation: Option<crate::request::Operation>,
    ) -> io::Result<Self> {
        let deadline = operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        let mut obs = Self::connect_context(cfg, deadline, operation.clone())?;
        obs.operation = operation;
        Ok(obs)
    }
    #[cfg(test)]
    fn connect_until(cfg: &Config, deadline: Instant) -> io::Result<Self> {
        Self::connect_context(cfg, deadline, None)
    }
    fn connect_context(
        cfg: &Config,
        deadline: Instant,
        operation: Option<crate::request::Operation>,
    ) -> io::Result<Self> {
        let password = secret(&cfg.obs_password, 4096)?;
        if password.len() < 16 {
            return Err(err("OBS password is missing or too short"));
        }
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, cfg.obs_port);
        let stream = TcpStream::connect_timeout(
            &addr.into(),
            remaining(deadline)?.min(Duration::from_secs(2)),
        )?;
        let stream = DeadlineStream {
            stream,
            deadline,
            operation,
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(LIMIT))
            .max_frame_size(Some(LIMIT))
            .max_write_buffer_size(LIMIT + 1024);
        let (socket, _) = tungstenite::client::client_with_config(
            format!("ws://127.0.0.1:{}/", cfg.obs_port),
            stream,
            Some(config),
        )
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => redacted_websocket_error(e),
            _ => err("OBS WebSocket handshake interrupted"),
        })?;
        let mut obs = Self {
            operation: None,
            socket,
            seq: 0,
            failed: false,
        };
        let hello = obs.next(deadline)?;
        if hello["op"].as_u64() != Some(0) {
            return Err(err("OBS WebSocket v5 Hello required"));
        }
        let salt = auth_string(&hello, "salt")?;
        let challenge = auth_string(&hello, "challenge")?;
        let secret = base64_sha(format!("{password}{salt}").as_bytes());
        let proof = base64_sha(format!("{secret}{challenge}").as_bytes());
        obs.send(
            &json!({"op":1,"d":{"rpcVersion":1,"authentication":proof,"eventSubscriptions":0}}),
            deadline,
        )?;
        if obs.next(deadline)?["op"].as_u64() != Some(2) {
            return Err(err("OBS WebSocket authentication rejected"));
        }
        Ok(obs)
    }
    fn send(&mut self, value: &Value, deadline: Instant) -> io::Result<()> {
        remaining(deadline)?;
        let text = value.to_string();
        if text.len() > LIMIT {
            return Err(err("oversized OBS request"));
        }
        self.socket.get_mut().deadline = deadline;
        self.socket
            .send(Message::Text(text.into()))
            .map_err(redacted_websocket_error)
    }
    fn next(&mut self, deadline: Instant) -> io::Result<Value> {
        loop {
            remaining(deadline)?;
            self.socket.get_mut().deadline = deadline;
            match self.socket.read().map_err(redacted_websocket_error)? {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_str()).map_err(|_| err("invalid OBS JSON"))
                }
                Message::Ping(_) | Message::Pong(_) => {}
                _ => return Err(err("OBS text message required")),
            }
        }
    }
    pub fn call(&mut self, name: &str, body: &str) -> io::Result<String> {
        let deadline = self
            .operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        self.call_until(name, body, deadline)
    }
    fn call_until(&mut self, name: &str, body: &str, deadline: Instant) -> io::Result<String> {
        if self.failed {
            return Err(err("OBS connection must be re-established"));
        }
        if body.len() > LIMIT / 2 || name.len() > 256 {
            return Err(err("oversized OBS request"));
        }
        let body: Value =
            serde_json::from_str(body).map_err(|_| err("invalid OBS request JSON"))?;
        if !body.is_object() {
            return Err(err("OBS request object required"));
        }
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or_else(|| err("OBS request sequence exhausted"))?;
        let id = self.seq.to_string();
        let result = (|| {
            self.send(
                &json!({"op":6,"d":{"requestType":name,"requestId":id,"requestData":body}}),
                deadline,
            )?;
            for _ in 0..64 {
                let answer = self.next(deadline)?;
                if answer["op"].as_u64() != Some(7) {
                    continue;
                }
                if answer["d"]["requestId"].as_str() != Some(id.as_str()) {
                    continue;
                }
                if answer["d"]["requestStatus"]["result"].as_bool() != Some(true) {
                    let code = answer["d"]["requestStatus"]["code"]
                        .as_u64()
                        .map_or_else(|| "unknown".into(), |code| code.to_string());
                    return Err(err(&format!("OBS request {name} rejected (code {code})")));
                }
                return Ok(answer.to_string());
            }
            Err(err("OBS request lost amidst other events"))
        })();
        // A timed-out fragmented exchange must never be reused for another request.
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn has(&mut self, method: &str, list: &str, key: &str, needle: &str) -> io::Result<bool> {
        let response = response_data(&self.call(method, "{}")?)?;
        let entries = response[list]
            .as_array()
            .ok_or_else(|| err("OBS list required"))?;
        Ok(entries
            .iter()
            .any(|entry| entry[key].as_str() == Some(needle)))
    }
    fn input(&mut self, name: &str, kind: &str, settings: Value) -> io::Result<()> {
        if !self.has("GetInputList", "inputs", "inputName", name)? {
            self.call("CreateInput", &json!({"sceneName":"SplitCast","inputName":name,"inputKind":kind,"inputSettings":settings,"sceneItemEnabled":true}).to_string())?;
        } else {
            self.call(
                "SetInputSettings",
                &json!({"inputName":name,"inputSettings":settings,"overlay":false}).to_string(),
            )?;
        }
        Ok(())
    }
    pub(crate) fn scene_ready(&mut self) -> io::Result<bool> {
        self.has("GetSceneList", "scenes", "sceneName", "SplitCast")
    }
    fn scene_item(&mut self, source: &str) -> io::Result<i64> {
        let item = response_data(&self.call(
            "GetSceneItemId",
            &json!({"sceneName":"SplitCast","sourceName":source}).to_string(),
        )?)?;
        item["sceneItemId"]
            .as_i64()
            .ok_or_else(|| err("OBS scene item required"))
    }
    fn source_geometry_matches(
        &mut self,
        source: &str,
        width: u16,
        height: u16,
    ) -> io::Result<bool> {
        let id = self.scene_item(source)?;
        let response = response_data(&self.call(
            "GetSceneItemTransform",
            &json!({"sceneName":"SplitCast","sceneItemId":id}).to_string(),
        )?)?;
        let transform = &response["sceneItemTransform"];
        Ok(transform["sourceWidth"].as_f64() == Some(f64::from(width))
            && transform["sourceHeight"].as_f64() == Some(f64::from(height)))
    }
    pub(crate) fn source_geometry_ready(&mut self, cfg: &Config) -> io::Result<bool> {
        Ok(
            self.source_geometry_matches("GameFeed", cfg.width, cfg.height)?
                && match &cfg.camera {
                    Some(camera) => {
                        self.source_geometry_matches("CameraFeed", camera.width, camera.height)?
                    }
                    None => true,
                },
        )
    }
    pub fn configure(&mut self, cfg: &Config) -> io::Result<()> {
        self.call("GetVersion", "{}")?;
        if !self.has("GetSceneList", "scenes", "sceneName", "SplitCast")? {
            self.call("CreateScene", "{\"sceneName\":\"SplitCast\"}")?;
        }
        let feed = |name| json!({"is_local_file":false,"input":format!("rtsp://127.0.0.1:{}/{name}",cfg.rtsp),"input_format":"rtsp","ffmpeg_options":"rtsp_transport=tcp","hw_decode":cfg.obs_video_mode == ObsVideoMode::Hardware,"buffering_mb":1});
        self.input("GameFeed", "ffmpeg_source", feed("game"))?;
        if cfg.camera.is_some() {
            self.input("CameraFeed", "ffmpeg_source", feed("camera"))?;
            let id = self.scene_item("CameraFeed")?;
            self.call(
                "SetSceneItemTransform",
                &json!({
                    "sceneName":"SplitCast","sceneItemId":id,
                    "sceneItemTransform":{
                        "alignment":5,"positionX":cfg.width - cfg.width / 4 - 8,
                    "positionY":8,
                        "boundsType":"OBS_BOUNDS_SCALE_INNER","boundsAlignment":0,
                        "boundsWidth":cfg.width / 4,"boundsHeight":cfg.height / 4
                    }
                })
                .to_string(),
            )?;
        }
        self.input(
            "MicProcessed",
            "pulse_input_capture",
            json!({"device_id":format!("{}.monitor",cfg.processed_sink)}),
        )?;
        if cfg.separate_audio_tracks {
            for (name, game, microphone) in
                [("GameFeed", true, false), ("MicProcessed", false, true)]
            {
                self.call(
                    "SetInputAudioTracks",
                    &json!({
                        "inputName":name, "inputAudioTracks":{"1":true,"2":game,"3":microphone}
                    })
                    .to_string(),
                )?;
            }
        }
        self.call(
            "SetInputAudioSyncOffset",
            &json!({"inputName":"MicProcessed","inputAudioSyncOffset":cfg.mic_offset}).to_string(),
        )?;
        self.call(
            "SetInputAudioMonitorType",
            "{\"inputName\":\"MicProcessed\",\"monitorType\":\"OBS_MONITORING_TYPE_NONE\"}",
        )?;
        self.call("SetCurrentProgramScene", "{\"sceneName\":\"SplitCast\"}")?;
        Ok(())
    }
    pub(crate) fn configure_output(
        &mut self,
        cfg: &Config,
        output: crate::Output,
    ) -> io::Result<()> {
        if output == crate::Output::Recording {
            return Ok(());
        }
        let twitch = cfg
            .twitch
            .as_ref()
            .ok_or_else(|| err("Twitch is not configured on production"))?;
        if output == crate::Output::TwitchLive && !twitch.allow_public {
            return Err(err("public Twitch output is disabled on production"));
        }
        let mut key = secret(&twitch.key_file, 512)?;
        if !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(err("invalid Twitch stream key"));
        }
        if output == crate::Output::TwitchTest {
            key.push_str("?bandwidthtest=true");
        }
        self.call(
            "SetStreamServiceSettings",
            &json!({
                "streamServiceType":"rtmp_custom",
                "streamServiceSettings":{
                    "server":format!("rtmps://{}/app/", twitch.ingest_host),
                    "key":key,"use_auth":false
                }
            })
            .to_string(),
        )?;
        Ok(())
    }
    pub fn start(&mut self, output: crate::Output) -> io::Result<()> {
        self.call(
            if output == crate::Output::Recording {
                "StartRecord"
            } else {
                "StartStream"
            },
            "{}",
        )
        .map(|_| ())
    }
    pub fn stop(&mut self, output: crate::Output) -> io::Result<()> {
        let deadline = self
            .operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        let mut requested = false;
        loop {
            let response =
                response_data(&self.call_until(status_request(output), "{}", deadline)?)?;
            match response["outputActive"].as_bool() {
                Some(false) => return Ok(()),
                Some(true) => {}
                None => return Err(err("OBS outputActive boolean required")),
            }
            if !requested {
                self.call_until(
                    if output == crate::Output::Recording {
                        "StopRecord"
                    } else {
                        "StopStream"
                    },
                    "{}",
                    deadline,
                )?;
                requested = true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    pub fn observe(&mut self, output: crate::Output) -> io::Result<OutputStatus> {
        let response = response_data(&self.call(status_request(output), "{}")?)?;
        Ok(OutputStatus {
            active: response["outputActive"]
                .as_bool()
                .ok_or_else(|| err("OBS outputActive boolean required"))?,
            bytes: response["outputBytes"].as_u64(),
            frames: response["outputTotalFrames"].as_u64(),
            duration_ms: response["outputDuration"].as_u64(),
        })
    }
    pub fn preview(&mut self) -> io::Result<String> {
        let response = response_data(&self.call("GetSourceScreenshot", "{\"sourceName\":\"SplitCast\",\"imageFormat\":\"png\",\"imageWidth\":960,\"imageHeight\":540}")?)?;
        let image = response["imageData"]
            .as_str()
            .filter(|image| image.starts_with("data:image/png;base64,"))
            .ok_or_else(|| err("OBS returned an invalid scene screenshot"))?;
        Ok(json!({"image":image}).to_string())
    }
}

fn status_request(output: crate::Output) -> &'static str {
    match output {
        crate::Output::Recording => "GetRecordStatus",
        crate::Output::TwitchTest | crate::Output::TwitchLive => "GetStreamStatus",
    }
}

fn launch_args(cfg: &Config, base: &Path) -> io::Result<Vec<String>> {
    let _port = TcpListener::bind((Ipv4Addr::LOCALHOST, cfg.obs_port))
        .map_err(|_| err("OBS port is already occupied"))?;
    let studio = base.join("obs-studio");
    let password = secret(&cfg.obs_password, 4096)?;
    if password.len() < 16 {
        return Err(err("OBS auth password must be at least 16 characters"));
    }
    private_directory(&cfg.state_dir)?;
    atomic_file(&studio.join("plugin_config/obs-websocket/config.json"), json!({"server_enabled":true,"server_port":cfg.obs_port,"auth_required":true,"server_password":password,"first_load":false}).to_string())?;
    fs::create_dir_all(&cfg.recording_dir)?;
    if cfg.recording_dir.to_string_lossy().contains(['\n', '\r']) {
        return Err(err("invalid recording directory"));
    }
    let encoder = match cfg.obs_video_mode {
        ObsVideoMode::Hardware => "obs_nvenc_h264_tex",
        ObsVideoMode::Software => "obs_x264",
    };
    let recording_tracks = if cfg.separate_audio_tracks {
        0b111
    } else {
        0b001
    };
    let profile = format!("[General]\nName=SplitCast\n[Video]\nBaseCX={width}\nBaseCY={height}\nOutputCX={width}\nOutputCY={height}\nFPSType=0\nFPSCommon={fps}\n[Output]\nMode=Advanced\nReconnect=false\n[AdvOut]\nEncoder={encoder}\nRecType=Standard\nRecFilePath={}\nRecFormat2=mkv\nRecEncoder={encoder}\nTrackIndex=1\nRecTracks={recording_tracks}\nRescale=false\n[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n", cfg.recording_dir.display(),width=cfg.width,height=cfg.height,fps=cfg.fps);
    atomic_file(&studio.join("basic/profiles/SplitCast/basic.ini"), &profile)?;
    atomic_file(
        &studio.join("basic/profiles/SplitCast/streamEncoder.json"),
        json!({"rate_control":"CBR","bitrate":cfg.obs_bitrate,"keyint_sec":2}).to_string(),
    )?;
    atomic_file(&studio.join("user.ini"), "[Basic]\nProfile=SplitCast\nProfileDir=SplitCast\nSceneCollection=SplitCast\nSceneCollectionFile=SplitCast\n[General]\nFirstRun=true\n")?;
    atomic_file(&studio.join("basic/scenes/SplitCast.json"), "{\"name\":\"SplitCast\",\"current_scene\":\"SplitCast\",\"current_program_scene\":\"SplitCast\",\"scene_order\":[{\"name\":\"SplitCast\"}],\"sources\":[{\"name\":\"SplitCast\",\"id\":\"scene\",\"versioned_id\":\"scene\",\"settings\":{\"items\":[]},\"mixers\":0}],\"groups\":[],\"transitions\":[]}")?;
    Ok(vec![
        "env".into(),
        format!("XDG_CONFIG_HOME={}", base.display()),
        cfg.obs_binary.to_string_lossy().to_string(),
        "--multi".into(),
        "--disable-shutdown-check".into(),
        "--profile".into(),
        "SplitCast".into(),
        "--collection".into(),
        "SplitCast".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::thread;

    struct Fixture {
        dir: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = PathBuf::from("/tmp/opencode").join(format!(
                "native-obs-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            private_directory(&dir).unwrap();
            atomic_file(&dir.join("password"), "a-private-password-long-enough").unwrap();
            Self { dir }
        }
        fn config(&self, port: u16) -> Config {
            config(&self.dir, port)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    #[test]
    fn profile_uses_the_selected_video_geometry() {
        let fixture = Fixture::new();
        // This configuration-only test starts no OBS. Let launch_args perform
        // its own ephemeral bind, without releasing a reservation first.
        let mut cfg = fixture.config(0);
        cfg.width = 1280;
        cfg.height = 720;
        cfg.fps = 30;
        launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
        let profile = fs::read_to_string(
            cfg.state_dir
                .join("obs-config/obs-studio/basic/profiles/SplitCast/basic.ini"),
        )
        .unwrap();
        for setting in [
            "BaseCX=1280\n",
            "BaseCY=720\n",
            "OutputCX=1280\n",
            "OutputCY=720\n",
            "FPSCommon=30\n",
        ] {
            assert!(
                profile.contains(setting),
                "missing selected video setting {setting:?}"
            );
        }
    }
    #[test]
    fn profile_uses_selected_encoder_for_stream_and_recording() {
        let fixture = Fixture::new();
        let mut cfg = fixture.config(0);
        for (mode, encoder) in [
            (ObsVideoMode::Hardware, "obs_nvenc_h264_tex"),
            (ObsVideoMode::Software, "obs_x264"),
        ] {
            cfg.obs_video_mode = mode;
            launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
            let profile = fs::read_to_string(
                cfg.state_dir
                    .join("obs-config/obs-studio/basic/profiles/SplitCast/basic.ini"),
            )
            .unwrap();
            assert!(profile
                .lines()
                .any(|line| line == format!("Encoder={encoder}")));
            assert!(profile
                .lines()
                .any(|line| line == format!("RecEncoder={encoder}")));
        }
    }
    fn config(dir: &Path, port: u16) -> Config {
        let mut value: Value =
            serde_json::from_str(include_str!("../tests/agent-example.json")).unwrap();
        value["role"] = json!("production");
        value["peer"]["state_dir"] = json!(dir);
        value["capture"]["width"] = json!(1920);
        value["capture"]["height"] = json!(1080);
        value["obs"]["password_file"] = json!(dir.join("password"));
        value["obs"]["recording_dir"] = json!(dir.join("recordings"));
        let mut cfg = Config::parse(value.to_string().as_bytes()).unwrap();
        cfg.obs_port = port; // zero requests an ephemeral port in profile-only tests
        cfg
    }
    fn read(socket: &mut WebSocket<TcpStream>) -> Value {
        serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap()
    }
    fn send(socket: &mut WebSocket<TcpStream>, value: Value) {
        socket
            .send(Message::Text(value.to_string().into()))
            .unwrap();
    }
    fn answer(socket: &mut WebSocket<TcpStream>, request: &Value, data: Value) {
        send(
            socket,
            json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":data}}),
        );
    }
    fn server(
        f: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
    ) -> (u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = thread::spawn(move || f(accept_obs(&listener)));
        (port, thread)
    }
    fn accept_obs(listener: &TcpListener) -> WebSocket<TcpStream> {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        send(
            &mut socket,
            json!({"op":0,"d":{"authentication":{"salt":"salt\"","challenge":"challenge\\"}}}),
        );
        let identify = read(&mut socket);
        let secret = base64_sha(b"a-private-password-long-enoughsalt\"");
        assert_eq!(
            identify["d"]["authentication"],
            base64_sha(format!("{secret}challenge\\").as_bytes())
        );
        assert_eq!(identify["d"]["eventSubscriptions"], 0);
        send(&mut socket, json!({"op":2,"d":{"negotiatedRpcVersion":1}}));
        socket
    }

    #[test]
    fn configure_sends_selected_decode_mode_over_websocket() {
        for (mode, hw_decode) in [
            (ObsVideoMode::Hardware, true),
            (ObsVideoMode::Software, false),
        ] {
            let fixture = Fixture::new();
            let (port, thread) = server(move |mut socket| {
                let mut feeds = Vec::new();
                loop {
                    let request = read(&mut socket);
                    let method = request["d"]["requestType"].as_str().unwrap();
                    if method == "CreateInput" {
                        let data = &request["d"]["requestData"];
                        if data["inputKind"] == "ffmpeg_source" {
                            assert_eq!(data["inputSettings"]["hw_decode"], hw_decode);
                            feeds.push(data["inputName"].as_str().unwrap().to_owned());
                        }
                    }
                    let data = match method {
                        "GetSceneList" => json!({"scenes":[{"sceneName":"SplitCast"}]}),
                        "GetInputList" => json!({"inputs":[]}),
                        _ => json!({}),
                    };
                    answer(&mut socket, &request, data);
                    if method == "SetCurrentProgramScene" {
                        break;
                    }
                }
                assert_eq!(feeds, ["GameFeed"]);
            });
            let mut cfg = fixture.config(port);
            cfg.obs_video_mode = mode;
            Obs::connect(&cfg).unwrap().configure(&cfg).unwrap();
            thread.join().unwrap();
        }
    }

    #[test]
    fn game_readiness_requires_decoded_source_geometry() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            for (width, height) in [(0, 0), (1280, 720), (1920, 1080)] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], "GetSceneItemId");
                answer(&mut socket, &request, json!({"sceneItemId":7}));
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], "GetSceneItemTransform");
                assert_eq!(request["d"]["requestData"]["sceneItemId"], 7);
                answer(
                    &mut socket,
                    &request,
                    json!({"sceneItemTransform":{"sourceWidth":width,"sourceHeight":height}}),
                );
            }
        });
        let mut cfg = fixture.config(port);
        cfg.width = 1280;
        cfg.height = 720;
        let mut obs = Obs::connect(&cfg).unwrap();
        assert!(!obs.source_geometry_ready(&cfg).unwrap());
        assert!(obs.source_geometry_ready(&cfg).unwrap());
        assert!(!obs.source_geometry_ready(&cfg).unwrap());
        thread.join().unwrap();
    }

    #[test]
    fn compound_configuration_uses_one_operation_budget() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            while let Ok(message) = socket.read() {
                let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                thread::sleep(Duration::from_millis(40));
                let response = json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":{"scenes":[{"sceneName":"SplitCast"}],"inputs":[{"inputName":"GameFeed"},{"inputName":"MicProcessed"}]}}});
                if socket
                    .send(Message::Text(response.to_string().into()))
                    .is_err()
                {
                    break;
                }
            }
        });
        let cfg = fixture.config(port);
        let operation = crate::request::Operation::new(
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            None,
            Duration::from_millis(140),
        );
        let mut obs = Obs::connect_operation(&cfg, Some(operation)).unwrap();
        assert_eq!(
            obs.configure(&cfg).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn cancellation_between_configuration_calls_prevents_the_next_rpc() {
        let fixture = Fixture::new();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown = std::sync::Arc::clone(&cancel);
        let (port, thread) = server(move |mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "GetVersion");
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
            answer(&mut socket, &request, json!({}));
            assert!(
                socket.read().is_err(),
                "configuration sent another RPC after cancellation"
            );
        });
        let cfg = fixture.config(port);
        let operation = crate::request::Operation::new(cancel, None, Duration::from_secs(2));
        let mut obs = Obs::connect_operation(&cfg, Some(operation)).unwrap();
        assert_eq!(
            obs.configure(&cfg).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn wrong_ids_are_ignored_and_result_must_be_boolean() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            send(
                &mut socket,
                json!({"op":7,"d":{"requestId":"wrong","requestStatus":{"result":true}}}),
            );
            send(&mut socket, json!({"op":5}));
            send(
                &mut socket,
                json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":"true"}}}),
            );
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert_eq!(
            obs.call("GetVersion", "{}").unwrap_err().to_string(),
            "OBS request GetVersion rejected (code unknown)"
        );
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("re-established"));
        thread.join().unwrap();
    }
    #[test]
    fn stalled_response_and_events_share_one_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            for _ in 0..20 {
                if socket.send(Message::Text("{\"op\":5}".into())).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn dripfed_partial_frame_cannot_extend_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream.write_all(&[0x81, 126, 0, 200]).unwrap();
            for _ in 0..200 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn stalled_response_is_bounded() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            thread::sleep(Duration::from_millis(250));
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn dripfed_http_upgrade_is_bounded() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let cfg = fixture.config(listener.local_addr().unwrap().port());
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            for byte in b"HTTP/1.1 101 Switching Protocols\r\n" {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        let result = Obs::connect_until(&cfg, start + Duration::from_millis(100));
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_millis(500));
        thread.join().unwrap();
    }
    #[test]
    fn unauthenticated_server_is_rejected() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let cfg = fixture.config(listener.local_addr().unwrap().port());
        let thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            send(&mut socket, json!({"op":0,"d":{"rpcVersion":1}}));
        });
        assert!(Obs::connect(&cfg)
            .err()
            .unwrap()
            .to_string()
            .contains("authentication string required"));
        thread.join().unwrap();
    }
    #[test]
    fn blocked_tcp_writes_share_one_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let start = Instant::now();
        let mut stream = DeadlineStream {
            stream,
            deadline: start + Duration::from_millis(100),
            operation: None,
        };
        assert_eq!(
            stream
                .write_all(&vec![0; 32 * 1024 * 1024])
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
    }
    #[test]
    fn dripfed_continuations_cannot_extend_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream.write_all(&[0x01, 1, b'{']).unwrap();
            for _ in 0..100 {
                if stream.write_all(&[0x00, 1, b' ']).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn oversized_frame_is_rejected_before_payload() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            socket
                .get_mut()
                .write_all(&[0x81, 127, 0, 0, 0, 0, 0, 0x40, 0, 1])
                .unwrap();
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("oversized"));
        thread.join().unwrap();
    }
    #[test]
    fn oversized_fragmented_message_is_rejected() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream
                .write_all(&[0x01, 127, 0, 0, 0, 0, 0, 0x20, 0, 0])
                .unwrap();
            stream.write_all(&vec![b'x'; LIMIT / 2]).unwrap();
            stream
                .write_all(&[0x80, 127, 0, 0, 0, 0, 0, 0x20, 0, 1])
                .unwrap();
            let _ = stream.write_all(&vec![b'x'; LIMIT / 2 + 1]);
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("oversized"));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn exact_names_only_not_settings_or_substrings() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"scenes":[{"sceneName":"SplitCastBackup"},{"sceneName":"Other","settings":"SplitCast"}]}),
            );
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"inputs":[{"inputName":"GameFeedBackup"},{"inputName":"Other","settings":"GameFeed"}]}),
            );
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"scenes":[{"sceneName":"SplitCast"}]}),
            );
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(!obs
            .has("GetSceneList", "scenes", "sceneName", "SplitCast")
            .unwrap());
        assert!(!obs
            .has("GetInputList", "inputs", "inputName", "GameFeed")
            .unwrap());
        assert!(obs
            .has("GetSceneList", "scenes", "sceneName", "SplitCast")
            .unwrap());
        thread.join().unwrap();
    }
    #[test]
    fn output_status_reports_progress() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "GetRecordStatus");
            answer(
                &mut socket,
                &request,
                json!({"outputActive":true,"outputBytes":123,"outputTotalFrames":42,"outputDuration":99}),
            );
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert_eq!(
            obs.observe(crate::Output::Recording).unwrap(),
            OutputStatus {
                active: true,
                bytes: Some(123),
                frames: Some(42),
                duration_ms: Some(99)
            }
        );
        thread.join().unwrap();
    }
    #[test]
    fn start_requests_private_recording() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "StartRecord");
            answer(&mut socket, &request, json!({}));
        });
        let cfg = fixture.config(port);
        let mut obs = Obs::connect(&cfg).unwrap();
        obs.start(crate::Output::Recording).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn stop_waits_for_recording_finalization_before_returning() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            for (method, data) in [
                ("GetRecordStatus", json!({"outputActive":true})),
                ("StopRecord", json!({})),
                ("GetRecordStatus", json!({"outputActive":true})),
                ("GetRecordStatus", json!({"outputActive":false})),
            ] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], method);
                answer(&mut socket, &request, data);
            }
        });
        Obs::connect(&fixture.config(port))
            .unwrap()
            .stop(crate::Output::Recording)
            .unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn twitch_test_selects_tls_and_stop_reconciles_a_lost_start_reply() {
        use crate::Output::{TwitchLive, TwitchTest};
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut cfg = fixture.config(listener.local_addr().unwrap().port());
        let key_file = fixture.dir.join("stream-key");
        atomic_file(&key_file, "live_fake_key").unwrap();
        cfg.twitch = Some(crate::config::Twitch {
            ingest_host: "apn10.contribute.live-video.net".into(),
            key_file,
            allow_public: false,
        });
        let thread = thread::spawn(move || {
            let mut socket = accept_obs(&listener);
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "SetStreamServiceSettings");
            assert_eq!(
                request["d"]["requestData"],
                json!({
                    "streamServiceType":"rtmp_custom",
                    "streamServiceSettings":{"server":"rtmps://apn10.contribute.live-video.net/app/",
                        "key":"live_fake_key?bandwidthtest=true","use_auth":false}
                })
            );
            answer(&mut socket, &request, json!({}));
            assert_eq!(read(&mut socket)["d"]["requestType"], "StartStream");
            drop(socket);

            let mut socket = accept_obs(&listener);
            for (method, data) in [
                ("GetStreamStatus", json!({"outputActive":true})),
                ("StopStream", json!({})),
                ("GetStreamStatus", json!({"outputActive":false})),
            ] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], method);
                answer(&mut socket, &request, data);
            }
        });
        let mut obs = Obs::connect(&cfg).unwrap();
        assert!(obs.configure_output(&cfg, TwitchLive).is_err());
        obs.configure_output(&cfg, TwitchTest).unwrap();
        assert!(obs.start(TwitchTest).is_err());
        Obs::connect(&cfg).unwrap().stop(TwitchTest).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn launch_refuses_occupied_port() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        assert!(launch_args(
            &fixture.config(listener.local_addr().unwrap().port()),
            &fixture.dir.join("obs-config"),
        )
        .unwrap_err()
        .to_string()
        .contains("occupied"));
    }
    #[test]
    fn launch_refuses_public_credentials_and_keeps_generated_files_private() {
        let fixture = Fixture::new();
        let cfg = fixture.config(0);
        fs::set_permissions(&cfg.obs_password, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(launch_args(&cfg, &cfg.state_dir.join("obs-config"),).is_err());
        fs::set_permissions(&cfg.obs_password, fs::Permissions::from_mode(0o600)).unwrap();
        launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
        let credentials = cfg
            .state_dir
            .join("obs-config/obs-studio/plugin_config/obs-websocket/config.json");
        assert_eq!(fs::metadata(&credentials).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(credentials.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::read_to_string(&cfg.obs_password).unwrap(),
            "a-private-password-long-enough"
        );
    }
    #[test]
    fn atomic_state_rejects_symlinks_and_ignores_predictable_temp_symlink() {
        let fixture = Fixture::new();
        let victim = fixture.dir.join("victim");
        fs::write(&victim, "untouched").unwrap();
        let state = fixture.dir.join("state.json");
        symlink(&victim, state.with_extension("tmp")).unwrap();
        atomic_file(&state, "private").unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "untouched");
        assert_eq!(fs::metadata(&state).unwrap().mode() & 0o777, 0o600);
        fs::remove_file(&state).unwrap();
        symlink(&victim, &state).unwrap();
        assert!(atomic_file(&state, "private").is_err());
        let alias = fixture.dir.join("alias");
        symlink(&fixture.dir, &alias).unwrap();
        assert!(atomic_file(&alias.join("other"), "private").is_err());
        assert_eq!(fs::read_to_string(victim).unwrap(), "untouched");
    }
}
