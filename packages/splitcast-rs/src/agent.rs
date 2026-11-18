use crate::protocol::{Command, Invocation, MicAction, OwnerId, Start};
use crate::request::rejected;
use crate::scene::{ResultState as SceneResult, Scene};
use crate::workers::{Worker, Workers};
use crate::{config, control, media, obs, return_audio};
use crate::{Lifecycle, Output, Role, Session};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::process::ExitStatusExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

fn err(s: &str) -> io::Error {
    io::Error::other(s)
}
fn json_state(s: Lifecycle) -> &'static str {
    match s {
        Lifecycle::Idle => "idle",
        Lifecycle::Preparing => "preparing",
        Lifecycle::Ready => "ready",
        Lifecycle::Running => "running",
        Lifecycle::Degraded => "degraded",
        Lifecycle::Stopping => "stopping",
        Lifecycle::Failed => "failed",
    }
}
fn rand_hex() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    let mut f = fs::File::open("/dev/urandom")?;
    f.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|x| format!("{x:02x}")).collect::<String>())
}
fn transport(cfg: &config::Config, media: bool) -> Value {
    let mut contract = json!({"version":2});
    if media {
        contract["media"] = json!({
            "video":{"codec":"H264","width":cfg.width,"height":cfg.height,"fps":cfg.fps},
            "audio":{"game":"MPEG-4 Audio","mic":"Opus","rate":48000},
            "ports":{"rtmp":cfg.rtmp,"rtsp":cfg.rtsp}
        });
        if let Some(camera) = &cfg.camera {
            contract["media"]["camera"] = json!({
                "location":camera.location,"codec":"H264","width":camera.width,
                "height":camera.height,"fps":camera.fps
            });
        }
    }
    if cfg.tts_enabled {
        contract["speech_return"] = json!({"version":1,"expiry_seconds":cfg.tts_expiry_seconds});
    }
    contract
}
fn require_rust_production(document: &str) -> io::Result<()> {
    let hello: Value = serde_json::from_str(document).map_err(|_| err("invalid peer status"))?;
    if hello["version"] != 1 || hello["role"] != "production" || hello["engine"] != "rust-candidate"
    {
        return Err(err(
            "the peer must be a compatible Rust production agent; Python delegation is forbidden",
        ));
    }
    Ok(())
}
pub fn doctor(cfg: &config::Config) -> io::Result<String> {
    let missing = media::missing_tools(cfg, true, true);
    let selected = cfg.role != "capture"
        || (!cfg.target.is_empty()
            && cfg.game_audio.starts_with("app:")
            && !cfg.mic_source.is_empty());
    Ok(json!({"ok":missing.is_empty()&&selected&&(!cfg.tts_enabled||cfg.role!="capture"||crate::poc_speech::valid_target(&cfg.tts_sink)),"engine":"rust-candidate","missing":missing,"capture_target_selected":!cfg.target.is_empty(),"game_only_audio_selected":cfg.game_audio.starts_with("app:"),"rnnoise_plugin_present":cfg.rnnoise_plugin.is_file(),"competitive_non_regression":"NOT VERIFIED","cutover_authorized":false}).to_string())
}
pub fn cleanup_after_exit(cfg: config::Config) -> io::Result<()> {
    let agent = Agent::new(cfg)?;
    if agent.cfg.role == "production" {
        crate::audio::cleanup_socket(&agent.cfg)?;
        for entry in fs::read_dir(&agent.cfg.state_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.strip_prefix("obs-session-"))
                    .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                fs::remove_dir_all(entry.path())?;
            }
        }
    }
    Ok(())
}
#[derive(Default)]
struct ObsShutdown {
    output_inactive: Option<bool>,
    exit: Option<std::process::ExitStatus>,
}

impl ObsShutdown {
    fn status(&self) -> Value {
        json!({
            "output_inactive_confirmed": self.output_inactive,
            "exit_code": self.exit.and_then(|status| status.code()),
            "signal": self.exit.and_then(|status| status.signal()),
            "normal_exit": self.exit.map(|status| status.success()),
        })
    }
}

struct Agent {
    operation: Option<crate::request::Operation>,
    cfg: config::Config,
    session: Session,
    children: Workers,
    error: String,
    warnings: Vec<String>,
    mic_muted: bool,
    last_heartbeat: Instant,
    _lock: fs::File,
    receive_monitor: Option<media::ReceiveMonitor>,
    last_health: Instant,
    media_prepared: bool,
    obs_profile: Option<obs::SessionProfile>,
    obs_shutdown: Option<ObsShutdown>,
    eventsub: Option<crate::eventsub::EventSub>,
    scene: Scene,
    speech_source: Option<return_audio::Source>,
    speech_receiver: Option<return_audio::Receiver>,
}
impl Agent {
    fn new(mut cfg: config::Config) -> io::Result<Self> {
        let role = if cfg.role == "capture" {
            Role::Capture
        } else {
            Role::Production
        };
        cfg.state_dir = cfg.state_dir.join(&cfg.role);
        let directory = crate::state::private_directory(&cfg.state_dir)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(
                std::path::PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
                    .join("agent.lock"),
            )?;
        let meta = lock.metadata()?;
        if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o077 != 0 {
            return Err(err("unsafe state lock file"));
        }
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(err("an agent already owns this role state directory"));
        }
        let instance = rand_hex()?;
        let scene = Scene::new(instance.clone(), &cfg);
        Ok(Self {
            operation: None,
            cfg,
            session: Session::for_agent(role, instance),
            children: Workers::default(),
            error: String::new(),
            warnings: vec![],
            mic_muted: false,
            last_heartbeat: Instant::now(),
            _lock: lock,
            receive_monitor: None,
            last_health: Instant::now(),
            media_prepared: false,
            obs_profile: None,
            obs_shutdown: None,
            eventsub: None,
            scene,
            speech_source: None,
            speech_receiver: None,
        })
    }
    fn status(&self) -> String {
        self.status_value().to_string()
    }

    fn status_value(&self) -> Value {
        let control = self.session.control();
        let (stop, outcome) = if control.stop_pending() {
            (Some("requested"), Some("accepted"))
        } else if self.session.state() == Lifecycle::Stopping {
            (Some("incomplete"), Some("unknown"))
        } else if control.stop_completed() {
            (Some("completed"), Some("completed"))
        } else {
            (None, None)
        };
        json!({
            "version": 1, "engine": "rust-candidate", "role": self.cfg.role,
            "state": json_state(self.session.state()), "output": self.session.output().map(Output::as_str),
            "start_id": self.session.start_id(),
            "output_seconds_remaining": self.session.output_seconds_remaining(),
            "output_progress": self.obs_profile.as_ref().and_then(obs::SessionProfile::progress),
            "error": self.error, "warnings": self.warnings, "mic_muted": self.mic_muted,
            "children": self.children.names(), "stop": stop, "outcome": outcome, "stop_id": control.stop_id(),
            "owner": control.completed_owner().as_ref().map(OwnerId::as_str),
            "remote_release_pending": self.session.remote_owner().is_some(),
            "obs_shutdown": self.obs_shutdown.as_ref().map(ObsShutdown::status),
        })
    }
    fn spawn(&mut self, worker: Worker, argv: Vec<String>) -> io::Result<()> {
        self.checkpoint()?;
        self.children.spawn(worker, &argv)
    }
    fn checkpoint(&self) -> io::Result<()> {
        self.operation
            .as_ref()
            .map_or(Ok(()), |operation| operation.check())
    }
    fn obs(&self) -> io::Result<obs::Obs> {
        self.checkpoint()?;
        obs::Obs::connect_operation(&self.cfg, self.operation.clone())
    }
    fn wait_ready<F>(&self, mut check: F) -> io::Result<()>
    where
        F: FnMut() -> bool,
    {
        let end = Instant::now() + Duration::from_secs(self.cfg.timeout as u64);
        while Instant::now() < end {
            self.checkpoint()?;
            if check() {
                self.checkpoint()?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(200));
        }
        Err(err("external service readiness timeout"))
    }
    fn rust_peer_ready(&self) -> bool {
        self.request_peer("status", "{}")
            .and_then(|s| require_rust_production(&s))
            .is_ok()
    }
    fn validate_peer_transport(&self, media: bool) -> io::Result<()> {
        self.checkpoint()?;
        let response: Value = serde_json::from_str(
            &self.request_peer("routes", &json!({"media": media}).to_string())?,
        )
        .map_err(|_| err("invalid peer route contract"))?;
        if response["transport"] != transport(&self.cfg, media) {
            return Err(err(
                "peer media or speech contract differs; no capture was started",
            ));
        }
        Ok(())
    }
    fn guard_lease(&self, owner: Option<&OwnerId>) -> io::Result<()> {
        self.session.authorize(owner).map_err(|_| {
            rejected("session is already owned; stop it locally before changing owners")
        })
    }
    fn renew_lease(&mut self, owner: &OwnerId) -> io::Result<()> {
        self.session
            .renew(owner, Instant::now(), self.lease_duration())
            .map_err(|_| err("no matching active production owner; prepare a new session"))
    }
    fn lease_duration(&self) -> Duration {
        Duration::from_secs((self.cfg.timeout as u64 * 8).max(30))
    }
    fn prepare(&mut self, owner: Option<&OwnerId>, media: bool) -> io::Result<()> {
        self.checkpoint()?;
        self.guard_lease(owner)?;
        if matches!(self.session.state(), Lifecycle::Ready | Lifecycle::Running) {
            return if self.media_prepared == media {
                Ok(())
            } else {
                Err(err("stop before changing the prepared session purpose"))
            };
        }
        if self.session.state() != Lifecycle::Idle {
            return Err(err("stop before preparing another session"));
        }
        if !media && !self.cfg.tts_enabled && self.cfg.chat_credentials.is_none() {
            return Err(err(
                "media-free Prepare requires speech return or production chat",
            ));
        }
        self.scene.reset()?;
        self.media_prepared = media;
        self.session
            .prepare(owner.cloned(), Instant::now(), self.lease_duration())
            .map_err(|_| err("invalid prepare transition"))?;
        let outcome = (|| -> io::Result<()> {
            let missing = media::missing_tools(&self.cfg, media, false);
            if !missing.is_empty() {
                return Err(err("SplitCast required native tools missing"));
            }
            if self.cfg.role == "capture" {
                if self.cfg.tts_enabled && !crate::poc_speech::valid_target(&self.cfg.tts_sink) {
                    return Err(err(
                        "return audio requires an explicit capture playback node",
                    ));
                }
                self.spawn(Worker::Tunnel, media::tunnel(&self.cfg, media))?;
                self.wait_ready(|| self.rust_peer_ready())?;
                self.validate_peer_transport(media)?;
                self.prepare_remote(OwnerId::parse(&rand_hex()?)?, media)?;
                if self.cfg.tts_enabled {
                    let status = self.remote_request(&Command::TtsStatus)?;
                    self.speech_receiver = Some(return_audio::Receiver::from_status(&status)?);
                }
            } else {
                if self.cfg.tts_enabled {
                    if owner.is_none() {
                        return Err(err("speech return requires capture-owned Prepare"));
                    }
                    self.speech_source = Some(return_audio::Source::new(&self.cfg, rand_hex()?)?);
                }
                if let Some(path) = &self.cfg.chat_credentials {
                    let queue = self
                        .cfg
                        .tts_enabled
                        .then_some(self.cfg.tts_queue_limit.into());
                    self.eventsub =
                        Some(crate::eventsub::EventSub::start(path, rand_hex()?, queue)?);
                }
                if media {
                    self.spawn(Worker::Receiver, media::receiver(&self.cfg)?)?;
                    self.wait_ready(|| {
                        media::receive_operation(&self.cfg, self.operation.as_ref()).is_ok()
                    })?;
                }
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            return Err(self.fail(e));
        }
        self.checkpoint().map_err(|e| self.fail(e))?;
        self.session.prepared().map_err(|_| err("prepare failed"))?;
        if let Some(o) = owner {
            self.renew_lease(o)?;
        }
        self.last_heartbeat = Instant::now();
        Ok(())
    }
    fn start(&mut self, start: Start, owner: Option<&OwnerId>) -> io::Result<()> {
        let output = start.output();
        self.checkpoint()?;
        self.guard_lease(owner)?;
        if owner.is_none() {
            if let Some(id) = start.session_id() {
                if self.session.start_id().as_deref() != Some(id) {
                    return Err(rejected("Start session ID is stale; refresh status"));
                }
            }
            if output == Output::TwitchLive
                && (start.session_id().is_none() || self.session.state() == Lifecycle::Idle)
            {
                return Err(rejected(
                    "public Start requires a prepared session and its current Start ID",
                ));
            }
        }

        if self.session.state() == Lifecycle::Running {
            return if self.session.output() == Some(output) {
                Ok(())
            } else {
                Err(rejected("stop before changing outputs"))
            };
        }
        if self.cfg.role == "production" && output != Output::Recording {
            let twitch = self
                .cfg
                .twitch
                .as_ref()
                .ok_or_else(|| rejected("Twitch is not configured on production"))?;
            if output == Output::TwitchLive && !twitch.allow_public {
                return Err(rejected("public Twitch output is disabled on production"));
            }
        }
        if self.session.state() == Lifecycle::Idle {
            if owner.is_some() {
                return Err(rejected(
                    "remote Start requires an existing matching preparation",
                ));
            }
            self.prepare(owner, true)?;
        }
        if self.session.state() != Lifecycle::Ready {
            return Err(rejected("not prepared"));
        }
        if !self.media_prepared {
            return Err(rejected(
                "stop the speech-only session before starting media",
            ));
        }
        self.scene.reset()?;
        let outcome = (|| -> io::Result<()> {
            if !media::missing_tools(&self.cfg, true, true).is_empty() {
                return Err(err("required native media executables are missing"));
            }
            let camera = media::camera(&self.cfg)?;
            if self.cfg.role == "capture" {
                let game = media::game(&self.cfg)?;
                let microphone = media::mic(&self.cfg, self.operation.as_ref())?;
                self.spawn(Worker::Game, game)?;
                self.spawn(Worker::Microphone, microphone)?;
                if let Some(argv) = camera {
                    self.spawn(Worker::Camera, argv)?;
                }
                self.remote_request(&Command::Start(start))?;
            } else {
                if let Some(argv) = camera {
                    self.spawn(Worker::Camera, argv)?;
                }
                let mut monitor = media::ReceiveMonitor::new(self.cfg.camera.is_some());
                self.wait_ready(|| {
                    media::receive_operation(&self.cfg, self.operation.as_ref())
                        .is_ok_and(|v| monitor.observe(&v, Instant::now()).unwrap_or(false))
                })?;
                self.spawn(Worker::AudioServer, crate::audio::server(&self.cfg)?)?;
                self.wait_ready(|| {
                    crate::audio::ready_operation(&self.cfg, self.operation.as_ref())
                        .unwrap_or(false)
                })?;
                let directory = self
                    .cfg
                    .state_dir
                    .join(format!("obs-session-{}", rand_hex()?));
                self.obs_profile = Some(obs::SessionProfile::create(directory)?);
                self.obs_shutdown = None;
                let argv = self
                    .obs_profile
                    .as_ref()
                    .expect("profile just allocated")
                    .launch_args(&self.cfg)?;
                self.spawn(Worker::Obs, crate::audio::client(&self.cfg, argv))?;
                self.wait_ready(|| {
                    self.obs()
                        .and_then(|mut o| o.scene_ready())
                        .unwrap_or(false)
                })?;
                self.spawn(
                    Worker::Rnnoise,
                    crate::audio::client(&self.cfg, media::rnnoise(&self.cfg)?),
                )?;
                self.wait_ready(|| {
                    crate::audio::processing_ready(&self.cfg, self.operation.as_ref())
                        .unwrap_or(false)
                })?;
                let mut o = self.obs()?;
                o.configure(&self.cfg, &self.scene.draft)?;
                self.wait_ready(|| {
                    self.obs()
                        .and_then(|mut o| o.source_geometry_ready(&self.cfg))
                        .unwrap_or(false)
                })?;
                o.finish_start_layout(&self.scene.draft)?;
                let layout = o.read_layout(&self.cfg)?;
                if layout.layout != self.scene.draft {
                    return Err(err("owned OBS Start layout did not match the draft"));
                }
                self.scene.live = Some(layout);
                o.configure_output(&self.cfg, output)?;
                self.session
                    .arm_output_limit(start.limit())
                    .map_err(io::Error::other)?;
                self.obs_profile
                    .as_mut()
                    .expect("profile allocated before launching OBS")
                    .request_start(&mut o, output)?;
                self.wait_ready(|| {
                    media::receive_operation(&self.cfg, self.operation.as_ref())
                        .is_ok_and(|v| monitor.observe(&v, Instant::now()).unwrap_or(false))
                        && self
                            .obs()
                            .and_then(|mut o| o.output_ready(output))
                            .unwrap_or(false)
                })?;
                self.receive_monitor = Some(monitor);
                let mut studio = self.obs()?;
                if !self
                    .obs_profile
                    .as_mut()
                    .expect("profile allocated before launching OBS")
                    .observe(&mut studio)?
                {
                    return Err(err(
                        "OBS output counters were not ready before confirming Start",
                    ));
                }
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            return Err(self.fail(e));
        }
        self.checkpoint().map_err(|e| self.fail(e))?;
        self.session
            .start(output)
            .map_err(|error| self.fail(io::Error::other(error)))?;
        self.last_health = Instant::now();
        if let Some(o) = owner {
            self.renew_lease(o)?;
        }
        Ok(())
    }
    fn cleanup(&mut self) -> io::Result<()> {
        let previous = self.operation.replace(crate::request::Operation::new(
            Arc::new(AtomicBool::new(false)),
            None,
            Duration::from_secs(15),
        ));
        let result = self.cleanup_owned();
        self.operation = previous;
        result
    }
    fn cleanup_owned(&mut self) -> io::Result<()> {
        self.receive_monitor = None;
        let scene_reset = self.scene.reset();
        self.eventsub.take(); // cancels and joins the session subscription
        self.speech_source.take(); // cancels and joins any synthesis worker
        let mut failed = scene_reset.is_err();
        if let Some(receiver) = &mut self.speech_receiver {
            match receiver.stop() {
                Ok(()) => self.speech_receiver = None,
                Err(error) => {
                    failed = true;
                    self.warn(&error.to_string());
                }
            }
        }
        if self.children.contains(Worker::Obs) {
            let mut shutdown = ObsShutdown::default();
            if let Some(output) = self
                .obs_profile
                .as_ref()
                .and_then(obs::SessionProfile::requested_output)
            {
                let inactive = self.obs().and_then(|mut o| o.stop(output)).is_ok();
                shutdown.output_inactive = Some(inactive);
                failed |= !inactive;
            }
            match self.children.close_obs() {
                Ok(exit) => shutdown.exit = exit,
                Err(_) => failed = true,
            }
            if shutdown.exit.is_some_and(|status| !status.success()) {
                self.warn("OBS exited abnormally; inspect obs_shutdown in status");
            }
            self.obs_shutdown = Some(shutdown);
        }
        if self.session.remote_owner().is_some() {
            let release = (|| -> io::Result<()> {
                if !self.children.contains(Worker::Tunnel) {
                    self.spawn(
                        Worker::Tunnel,
                        media::tunnel(&self.cfg, self.media_prepared),
                    )?;
                    self.wait_ready(|| self.rust_peer_ready())?;
                    self.validate_peer_transport(self.media_prepared)?;
                }
                let mut response: Value =
                    serde_json::from_str(&self.remote_request(&Command::Stop)?)
                        .map_err(|_| err("invalid remote Stop completion"))?;
                loop {
                    if response["stop"] == "completed"
                        && response["state"] == "idle"
                        && response["owner"].as_str()
                            == self.session.remote_owner().as_ref().map(OwnerId::as_str)
                    {
                        break;
                    }
                    if response["stop"] != "requested" || response["outcome"] != "accepted" {
                        return Err(err(
                            "remote Stop did not prove completion for the owned session",
                        ));
                    }
                    // Observe the same accepted intent. Never retransmit Start
                    // or Prepare, or silently restart incomplete cleanup.
                    self.checkpoint()?;
                    thread::sleep(Duration::from_millis(20));
                    response = serde_json::from_str(&self.remote_request(&Command::Status)?)
                        .map_err(|_| err("invalid remote Stop observation"))?;
                }
                self.session
                    .remote_released()
                    .map_err(|_| err("invalid remote release transition"))
            })();
            if release.is_err() {
                failed = true;
                self.warn("remote stop unconfirmed; explicit Stop may retry and the production lease must expire");
            }
        }
        failed |= self.children.stop_all(Duration::from_millis(200)).is_err();
        if !self.children.contains(Worker::Obs) {
            if let Some(profile) = &self.obs_profile {
                if profile.discard().is_ok() {
                    self.obs_profile = None;
                } else {
                    failed = true;
                }
            }
        }
        if self.cfg.role == "production" && crate::audio::cleanup_socket(&self.cfg).is_err() {
            failed = true;
        }
        if failed {
            Err(err("owned resources could not all be released; stop again and inspect the enclosing service cgroup"))
        } else {
            Ok(())
        }
    }
    fn fail(&mut self, cause: io::Error) -> io::Error {
        if self.session.control().stop_pending() {
            // The admitted Stop owns compensation. Do not perform two cleanup
            // attempts or overwrite Stopping while unwinding cancelled work.
            let _ = self.session.stop();
            self.error = cause.to_string();
            return cause;
        }
        let _ = self.session.fail();
        let error = match self.cleanup() {
            Ok(()) => err(&cause.to_string()),
            Err(cleanup) => err(&format!("{cause}; cleanup incomplete: {cleanup}")),
        };
        self.error = error.to_string();
        error
    }
    fn stop(&mut self, owner: Option<&OwnerId>) -> io::Result<()> {
        if owner.is_some() {
            self.guard_lease(owner)?;
        }
        if self.session.state() == Lifecycle::Idle {
            return Ok(());
        }
        self.session
            .stop()
            .map_err(|_| err("stop lifecycle error"))?;
        if let Err(error) = self.cleanup() {
            self.error = error.to_string();
            return Err(error);
        }
        self.session
            .stopped()
            .map_err(|_| err("stop completion error"))?;
        self.error.clear();
        self.mic_muted = false;
        Ok(())
    }
    fn tick(&mut self) {
        self.expire_lease();
        if matches!(self.session.state(), Lifecycle::Ready | Lifecycle::Running) {
            if self.cfg.role == "capture"
                && self.children.contains(Worker::Tunnel)
                && self.last_heartbeat.elapsed() >= Duration::from_secs(2)
            {
                self.last_heartbeat = Instant::now();
                if self.remote_request(&Command::Heartbeat).is_err() {
                    let _ = self.fail(err("production heartbeat failed; owned capture stopped"));
                    return;
                }
            }
            let dead = self.children.any_exited();
            if dead {
                let _ = self.fail(err(
                    "owned infrastructure or media process exited; explicit prepare required",
                ));
            }
            if let Err(error) = self.poll_speech() {
                self.warn(&format!("speech return: {error}"));
            }
            if self.cfg.role == "production"
                && self.session.state() == Lifecycle::Running
                && self.last_health.elapsed() >= Duration::from_secs(1)
            {
                self.last_health = Instant::now();
                let health = (|| -> io::Result<bool> {
                    let snapshot = media::receive_operation(&self.cfg, self.operation.as_ref())?;
                    if !self
                        .receive_monitor
                        .as_mut()
                        .ok_or_else(|| err("receive monitor unavailable"))?
                        .observe(&snapshot, Instant::now())?
                    {
                        return Ok(false);
                    }
                    let mut studio = self.obs()?;
                    self.obs_profile
                        .as_mut()
                        .ok_or_else(|| err("owned OBS profile unavailable"))?
                        .observe(&mut studio)
                })();
                if !matches!(health, Ok(true)) {
                    let _ = self.fail(err("incoming media or OBS output stopped progressing; output stopped without automatic publication"));
                }
            }
        }
    }
    fn poll_speech(&mut self) -> io::Result<()> {
        if let Some(source) = &mut self.speech_source {
            source.tick(self.eventsub.as_ref());
        }
        let Some(receiver) = &mut self.speech_receiver else {
            return Ok(());
        };
        if !receiver.poll_due()? {
            return Ok(());
        }
        let control = self.session.control();
        let generation = control.generation();
        let started = Instant::now();
        let reply = self.remote_request(&Command::TtsNext)?;
        self.checkpoint()?;
        self.speech_receiver
            .as_mut()
            .expect("prepared receiver owns this poll")
            .receive(&reply, started, &self.cfg, control, generation)
    }
    fn expire_lease(&mut self) {
        let expired = self.session.lease_expired(Instant::now());
        let control = self.session.control();
        control.expire();
        if !control.stop_pending() {
            return;
        }
        let response = match self.stop(None) {
            Ok(()) => {
                if expired {
                    self.warn("session watchdog expired; owned media stopped");
                }
                let mut body = self.status_value();
                body["stop"] = json!("completed");
                body["outcome"] = json!("completed");
                (200, body.to_string())
            }
            Err(error) => {
                if expired {
                    self.warn("session watchdog expired; cleanup is incomplete");
                }
                (
                    409,
                    json!({
                        "error":error.to_string(), "state":json_state(self.session.state()),
                        "stop":"incomplete", "outcome":"unknown", "code":"cleanup_incomplete",
                        "remote_release_pending":self.session.remote_owner().is_some(),
                        "obs_shutdown":self.obs_shutdown.as_ref().map(ObsShutdown::status),
                    })
                    .to_string(),
                )
            }
        };
        control.finish_stop(response);
    }
    fn warn(&mut self, message: &str) {
        if self.warnings.len() == 16 {
            self.warnings.remove(0);
        }
        self.warnings.push(message.into());
    }
    #[cfg(test)]
    fn command(&mut self, cmd: &str, args: &str) -> io::Result<String> {
        self.expire_lease();
        let args: Value = serde_json::from_str(args).map_err(|_| err("invalid command JSON"))?;
        let invocation = Invocation::parse(cmd, &args, self.cfg.tts_max_text as usize)?;
        self.execute(invocation)
    }
    fn execute(&mut self, invocation: Invocation) -> io::Result<String> {
        self.checkpoint()?;
        let owner = invocation.owner.as_ref();
        if owner.is_some() && self.cfg.role != "production" {
            return Err(rejected("only production accepts remote ownership"));
        }
        if owner.is_some() {
            self.guard_lease(owner)?;
        }
        if owner.is_some()
            && self.session.state() == Lifecycle::Idle
            && matches!(
                invocation.command,
                Command::Chat(_)
                    | Command::Scene
                    | Command::SceneApply { .. }
                    | Command::SceneReconcile(_)
                    | Command::SceneSave(_)
                    | Command::SceneReload(_)
                    | Command::PreviewSmall
            )
        {
            return Err(rejected("no current production owner"));
        }
        match invocation.command {
            Command::Status=>Ok(self.status()),
            Command::Heartbeat=>{self.renew_lease(owner.ok_or_else(||err("production owner required"))?)?;Ok(self.status())},
            Command::Doctor=>doctor(&self.cfg),
            Command::Prepare{media}=>{self.prepare(owner, media)?;Ok(self.status())},
            Command::Start(output)=>{self.start(output,owner)?;Ok(self.status())},
            Command::Stop=>{self.stop(owner)?;Ok(self.status())},
            Command::Diagnostics=>Ok(json!({"version":1,"role":self.cfg.role,"state":json_state(self.session.state()),"output":self.session.output().map(Output::as_str),"children":self.children.names(),"mic_muted":self.mic_muted,"retries":0}).to_string()),
            Command::Routes{media}=>Ok(json!({"transport": transport(&self.cfg,media)}).to_string()),
            Command::Devices{remote}=>self.devices(remote),
            Command::TtsNext => {
                if owner.is_none() { return Err(err("speech pull requires the production session owner")); }
                Ok(self.speech_source.as_mut().ok_or_else(||err("no active speech source"))?.take())
            }
            Command::Chat(cursor) if self.cfg.role == "production" => {
                Ok(match &self.eventsub {
                    Some(eventsub) => eventsub.page(cursor.as_ref()),
                    None => crate::chat::inactive(self.cfg.chat_credentials.is_some(), cursor.as_ref()),
                })
            }
            command @ (Command::Scene | Command::SceneApply { .. } | Command::SceneReconcile(_)
                        | Command::SceneSave(_) | Command::SceneReload(_) | Command::PreviewSmall) => {
                self.studio_command(command, owner)
            }
            command=>{
                if !matches!(self.session.state(),Lifecycle::Ready|Lifecycle::Running|Lifecycle::Degraded){return Err(rejected("prepare the session first"));}
                if self.cfg.role=="capture"{return self.peer_command(&command);}
                match command {
                    Command::Mic(action)=>{
                        if !self.children.contains(Worker::Obs){return Err(err("no owned OBS session"));}
                         let mut obs=self.obs()?;
                        if action!=MicAction::Status {
                            let muted=action==MicAction::Mute;
                            obs.call("SetInputMute",&json!({"inputName":"MicProcessed","inputMuted":muted}).to_string())?;self.mic_muted=muted;
                        }
                        let ret:Value=serde_json::from_str(&obs.call("GetInputMute","{\"inputName\":\"MicProcessed\"}")?).map_err(|_|err("invalid OBS mute response"))?;
                        let muted=ret["d"]["responseData"]["inputMuted"].as_bool().ok_or_else(||err("invalid OBS mute response"))?;
                        self.mic_muted=muted;Ok(json!({"muted":muted}).to_string())
                    },
                    Command::Preview => self.obs()?.preview(),
                    Command::TtsStatus => self.speech_source.as_mut()
                        .ok_or_else(||rejected("speech return is disabled"))?.status(self.eventsub.as_ref()),
                    Command::Say {text, id} => self.speech_source.as_mut()
                        .ok_or_else(||rejected("speech return is disabled"))?.say(&text, &id),
                    _=>Err(err("command is not a prepared-session control"))
                }
            }
        }
    }
    fn studio_obs(&self) -> io::Result<obs::Obs> {
        self.checkpoint()?;
        let operation = self
            .operation
            .clone()
            .map(|operation| operation.capped(Duration::from_secs(5)));
        obs::Obs::connect_operation(&self.cfg, operation)
    }

    fn studio_command(&mut self, command: Command, owner: Option<&OwnerId>) -> io::Result<String> {
        let lifecycle = self.session.state();
        let running = lifecycle == Lifecycle::Running;
        if self.cfg.role == "capture" {
            if !matches!(lifecycle, Lifecycle::Ready | Lifecycle::Running) {
                return Err(rejected(
                    "prepare the capture session before forwarding Studio controls",
                ));
            }
            return self.peer_command(&command);
        }
        if !matches!(
            lifecycle,
            Lifecycle::Idle | Lifecycle::Ready | Lifecycle::Running
        ) {
            return Err(rejected(
                "scene controls require idle, ready or running production",
            ));
        }
        let generation = self.session.control().generation();
        if command == Command::Scene {
            return Ok(self.scene.snapshot(&self.cfg, generation, running));
        }
        let owned = running && self.obs_profile.is_some() && self.children.contains(Worker::Obs);
        if command == Command::PreviewSmall {
            if !owned {
                return Err(rejected("Studio preview requires running owned OBS"));
            }
            return self.studio_obs()?.preview_small();
        }
        self.guard_lease(owner)?;
        if running && !owned {
            return Err(rejected("no owned running OBS session"));
        }
        let state = match command {
            Command::SceneApply { edit, layout } => {
                let canvas = self.scene.canvas(&self.cfg);
                layout.validate(canvas, self.cfg.camera.is_some())?;
                self.checkpoint()?;
                self.scene.begin(edit, generation)?;
                self.scene.draft = layout;
                if !running {
                    SceneResult::Draft
                } else {
                    self.scene.live = None;
                    let mut obs = self.studio_obs()?;
                    obs.apply_layout(canvas, &self.scene.draft)?;
                    self.scene.live = Some(obs.read_layout(&self.cfg)?);
                    if !self.scene.matches_draft() {
                        self.scene.live = None;
                        return Err(err("owned OBS layout did not match the requested draft"));
                    }
                    SceneResult::Applied
                }
            }
            Command::SceneReconcile(edit) => {
                if !owned {
                    return Err(rejected("scene reconciliation requires running owned OBS"));
                }
                self.checkpoint()?;
                self.scene.begin(edit, generation)?;
                self.scene.live = None;
                self.scene.live = Some(self.studio_obs()?.read_layout(&self.cfg)?);
                SceneResult::Reconciled
            }
            Command::SceneSave(edit) => {
                if !owned || !self.scene.matches_draft() {
                    return Err(rejected(
                        "scene save requires current matching OBS readback",
                    ));
                }
                self.checkpoint()?;
                self.scene.begin(edit, generation)?;
                self.scene.live = None;
                self.scene.live = Some(self.studio_obs()?.read_layout(&self.cfg)?);
                if !self.scene.matches_draft() {
                    self.scene.result(SceneResult::Failed);
                    return Err(rejected("OBS layout changed; read scene before saving"));
                }
                let operation = self.operation.clone();
                if let Err(error) = self.scene.save(&self.cfg.state_dir, || {
                    operation.as_ref().map_or(Ok(()), |op| op.check())
                }) {
                    return Err(error);
                }
                SceneResult::Saved
            }
            Command::SceneReload(edit) => {
                self.checkpoint()?;
                self.scene.begin(edit, generation)?;
                if let Err(error) = self.scene.reload(&self.cfg) {
                    self.scene.result(SceneResult::Failed);
                    return Err(error);
                }
                SceneResult::Reloaded
            }
            _ => return Err(rejected("unknown Studio scene command")),
        };
        self.scene.result(state);
        Ok(self.scene.snapshot(&self.cfg, generation, running))
    }

    fn execute_work(
        &mut self,
        work: crate::server::Work,
        shutdown: &Arc<AtomicBool>,
        limits: crate::server::Limits,
    ) {
        if shutdown.load(Ordering::Relaxed) {
            work.ticket.abandon();
            return;
        }
        if !work.ticket.begin() {
            return;
        }
        let control = self.session.control();
        let activation_owner = if self.cfg.role == "production"
            && matches!(
                work.invocation.command,
                Command::Prepare { .. } | Command::Start(_)
            ) {
            work.invocation.owner.clone()
        } else {
            None
        };
        if !control.begin(work.generation, activation_owner) {
            work.ticket.finish(crate::request::error_response(
                crate::request::Failure::NotExecuted,
                "request superseded by Stop",
            ));
            return;
        }
        self.operation = Some(
            crate::request::Operation::new(
                Arc::clone(shutdown),
                Some(Arc::clone(&work.ticket)),
                limits.operation,
            )
            .with_control(control.clone(), work.generation),
        );
        self.expire_lease();
        let response = self
            .execute(work.invocation)
            .map(|body| (200, body))
            .unwrap_or_else(crate::request::command_failure);
        control.end();
        work.ticket.finish(response);
    }
    fn prepare_remote(&mut self, owner: OwnerId, media: bool) -> io::Result<String> {
        let cap = Duration::from_secs(self.cfg.timeout as u64);
        let operation = self
            .operation
            .clone()
            .unwrap_or_else(|| {
                crate::request::Operation::new(Arc::new(AtomicBool::new(false)), None, cap)
            })
            .with_remote_prepare(self.session.control(), owner.clone());
        let timeout = operation
            .deadline(cap)?
            .saturating_duration_since(Instant::now());
        control::request_operation(
            self.cfg.forward_control_port,
            self.cfg.remote_control_port,
            &self.cfg.token(true)?,
            "prepare",
            &json!({"_owner":owner.as_str(),"media":media}).to_string(),
            timeout,
            Some(&operation),
        )
    }
    fn remote_request(&self, command: &Command) -> io::Result<String> {
        self.checkpoint()?;
        let owner = self
            .session
            .remote_owner()
            .ok_or_else(|| err("no owned remote preparation"))?;
        let (name, mut args) = command.request();
        args["_owner"] = json!(owner.as_str());
        self.request_peer(name, &args.to_string())
    }
    fn request_peer(&self, name: &str, args: &str) -> io::Result<String> {
        let cap = Duration::from_secs(self.cfg.timeout as u64);
        let timeout = self.operation.as_ref().map_or(Ok(cap), |op| {
            op.deadline(cap)
                .map(|end| end.saturating_duration_since(Instant::now()))
        })?;
        control::request_operation(
            self.cfg.forward_control_port,
            self.cfg.remote_control_port,
            &self.cfg.token(true)?,
            name,
            args,
            timeout,
            self.operation.as_ref(),
        )
    }
    fn peer_command(&mut self, command: &Command) -> io::Result<String> {
        let response = self.remote_request(command)?;
        if matches!(command, Command::Mic(_)) {
            let value: Value = serde_json::from_str(&response)
                .map_err(|_| err("invalid peer microphone response"))?;
            self.mic_muted = value["muted"]
                .as_bool()
                .ok_or_else(|| err("invalid peer microphone observation"))?;
        }
        Ok(response)
    }
    fn devices(&mut self, remote: bool) -> io::Result<String> {
        self.checkpoint()?;
        let local = media::devices_operation(&self.cfg.role, self.operation.as_ref())?;
        self.checkpoint()?;
        let mut result = json!({"local":local});
        if remote && self.cfg.role == "capture" {
            let mut temporary = if !self.children.contains(Worker::Tunnel) {
                Some(crate::OwnedProcess::spawn(&media::tunnel(
                    &self.cfg, false,
                ))?)
            } else {
                None
            };
            let response = (|| {
                self.wait_ready(|| self.rust_peer_ready())?;
                self.request_peer("devices", "{\"remote\":false}")
            })();
            if let Some(tunnel) = &mut temporary {
                tunnel.stop(Duration::from_millis(100))?;
            }
            let remote: Value =
                serde_json::from_str(&response?).map_err(|_| err("invalid remote discovery"))?;
            result["remote"] = remote["local"].clone();
        }
        Ok(result.to_string())
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.stop(None);
    }
}
pub fn serve(cfg: config::Config, shutdown: Arc<AtomicBool>) -> io::Result<()> {
    let token = cfg.token(false)?;
    let listener = control::listener(&cfg)?;
    eprintln!(
        "SplitCast Rust candidate: loopback control on 127.0.0.1:{}; no automatic streaming",
        cfg.control_port
    );
    serve_bound(
        cfg,
        listener,
        token,
        shutdown,
        crate::server::Limits::default(),
    )
}
fn serve_bound(
    cfg: config::Config,
    listener: std::net::TcpListener,
    token: String,
    shutdown: Arc<AtomicBool>,
    limits: crate::server::Limits,
) -> io::Result<()> {
    serve_agent(Agent::new(cfg)?, listener, token, shutdown, limits)
}

fn serve_agent(
    mut agent: Agent,
    listener: std::net::TcpListener,
    token: String,
    shutdown: Arc<AtomicBool>,
    limits: crate::server::Limits,
) -> io::Result<()> {
    let (listener, pending) = crate::server::ListenerOwner::start(
        listener,
        token,
        agent.cfg.tts_max_text as usize,
        Arc::clone(&shutdown),
        crate::server::Limits {
            lease: agent.lease_duration(),
            ..limits
        },
        agent.session.control(),
    )?;
    while !shutdown.load(Ordering::Relaxed) {
        let control = agent.session.control();
        agent.operation = Some(
            crate::request::Operation::new(Arc::clone(&shutdown), None, limits.operation)
                .with_control(control.clone(), control.generation()),
        );
        agent.tick();
        match pending.recv_timeout(Duration::from_millis(20)) {
            Ok(work) => {
                agent.execute_work(work, &shutdown, limits);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    shutdown.store(true, Ordering::Relaxed);
    for work in pending.try_iter() {
        work.ticket.abandon();
    }
    let joined = listener.join();
    agent.operation = None;
    let cleaned = agent.stop(None);
    match (joined, cleaned) {
        (Err(join), Err(cleanup)) => Err(err(&format!("{join}; cleanup incomplete: {cleanup}"))),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
#[cfg(test)]
#[path = "agent_lifecycle_tests.rs"]
mod lifecycle_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn a_peer_status_is_not_proof_of_rust_ownership() {
        assert!(
            require_rust_production(r#"{"version":1,"role":"production","state":"ready"}"#)
                .is_err()
        );
        assert!(require_rust_production(
            r#"{"version":1,"role":"capture","engine":"rust-candidate"}"#
        )
        .is_err());
        require_rust_production(r#"{"version":1,"role":"production","engine":"rust-candidate"}"#)
            .unwrap();
    }

    fn peer_session(owner: &str, now: Instant, ttl: Duration) -> Session {
        let mut session = Session::new(Role::Production);
        let owner = OwnerId::parse(owner).unwrap();
        session
            .prepare(Some(owner.clone()), Instant::now(), ttl)
            .unwrap();
        session.prepared().unwrap();
        session.renew(&owner, now, ttl).unwrap();
        session
    }

    fn prepared_agent() -> Agent {
        let cfg = config::Config::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/agent-example.json"
        )))
        .unwrap();
        let mut cfg = cfg;
        cfg.role = "production".into();
        cfg.state_dir = std::env::temp_dir().join(format!(
            "splitcast-agent-test-{}-{}",
            std::process::id(),
            rand_hex().unwrap()
        ));
        let mut agent = Agent::new(cfg).unwrap();
        agent
            .session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        agent.session.prepared().unwrap();
        agent
    }

    #[test]
    fn heartbeat_cannot_claim_a_locally_prepared_session() {
        let mut agent = prepared_agent();
        assert!(
            agent
                .command("heartbeat", "{\"_owner\":\"intruder\"}")
                .is_err(),
            "heartbeat claimed a locally owned session"
        );
    }

    #[test]
    fn matching_remote_owner_renews_without_starting_an_output() {
        let mut agent = prepared_agent();
        let deadline = Instant::now() + Duration::from_secs(1);
        agent.session = peer_session("session_123", Instant::now(), Duration::from_secs(1));
        agent
            .command("heartbeat", "{\"_owner\":\"session_123\"}")
            .unwrap();
        assert!(!agent
            .session
            .lease_expired(deadline + Duration::from_secs(1)));
        assert_eq!(agent.session.state(), Lifecycle::Ready);
        assert_eq!(agent.session.output(), None);
    }

    #[test]
    fn expired_owner_cannot_revive_its_lease() {
        let mut agent = prepared_agent();
        agent.session = peer_session(
            "expired",
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        );
        assert!(
            agent
                .command("heartbeat", "{\"_owner\":\"expired\"}")
                .is_err(),
            "expired owner revived its lease"
        );
        assert_eq!(agent.session.state(), Lifecycle::Idle);
    }

    #[test]
    fn owner_driven_expiry_fences_queued_prepare_before_the_watchdog_runs() {
        let mut agent = prepared_agent();
        agent.session = peer_session(
            "expired",
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        );
        let generation = agent.session.control().generation();
        let shutdown = Arc::new(AtomicBool::new(false));
        let queued: Vec<_> = [
            Command::Prepare { media: true },
            Invocation::parse("start", &json!({}), 100).unwrap().command,
        ]
        .into_iter()
        .map(|command| crate::server::Work {
            invocation: Invocation {
                command,
                owner: Some(OwnerId::parse("expired").unwrap()),
            },
            ticket: Arc::new(crate::request::RequestTicket::new(
                Instant::now() + Duration::from_secs(5),
            )),
            generation,
        })
        .collect();
        // There is deliberately no listener/watchdog. This is the other expiry
        // ordering, which must revoke before stop changes Phase to Stopping.
        agent.expire_lease();
        for work in queued {
            let ticket = Arc::clone(&work.ticket);
            agent.execute_work(work, &shutdown, crate::server::Limits::default());
            let reply: Value = serde_json::from_str(&ticket.response().unwrap().1).unwrap();
            assert_eq!(reply["outcome"], "not_executed");
        }
        assert_eq!(agent.session.state(), Lifecycle::Idle);
        assert!(agent.children.names().is_empty());
        assert!(agent.session.output().is_none());
    }

    #[test]
    fn local_operator_can_emergency_stop_a_remote_session() {
        let mut agent = prepared_agent();
        agent.session = peer_session("remote", Instant::now(), Duration::from_secs(30));
        assert!(
            agent.command("stop", "{}").is_ok(),
            "local emergency stop was refused"
        );
        assert_eq!(agent.session.state(), Lifecycle::Idle);
    }

    #[test]
    fn repeated_start_is_idempotent_without_switching_to_twitch() {
        let mut agent = prepared_agent();
        agent.session.start(Output::Recording).unwrap();
        assert!(
            agent.command("start", "{}").is_ok(),
            "repeated recording start was refused"
        );
        assert!(agent
            .command(
                "start",
                r#"{"output":"twitch-live","duration_seconds":60,"confirm_public":true}"#
            )
            .is_err());
        assert_eq!(agent.session.output(), Some(Output::Recording));
    }

    #[test]
    fn command_rejects_unknown_arguments_and_non_string_owners() {
        let mut agent = prepared_agent();
        for (cmd, args) in [
            ("status", "{\"unexpected\":true}"),
            ("status", "{\"_owner\":[]}"),
            ("status", "null"),
            ("status", "{} {}"),
            ("start", "{\"output\":false}"),
        ] {
            assert!(agent.command(cmd, args).is_err(), "accepted {cmd} {args}");
        }
    }
}
