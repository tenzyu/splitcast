//! One-way, consume-on-read speech on the prepared Agent control connection.
use crate::config::Config;
use crate::lifecycle::SessionControl;
use crate::poc_speech;
use crate::request::Operation;
use crate::tts::{Speech, AUDIO_LIMIT};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const ENCODED_LIMIT: usize = AUDIO_LIMIT.div_ceil(3) * 4;
pub(crate) const REPLY_LIMIT: usize = ENCODED_LIMIT + 512;

pub(crate) struct Source {
    speech: Speech,
    epoch: String,
    issued: u64,
    admission: Option<(String, Instant)>,
    expiry: Duration,
}

impl Source {
    pub(crate) fn new(cfg: &Config, epoch: String) -> io::Result<Self> {
        Ok(Self {
            speech: Speech::in_memory(cfg)?,
            epoch,
            issued: 0,
            admission: None,
            expiry: Duration::from_secs(cfg.tts_expiry_seconds.into()),
        })
    }

    pub(crate) fn status(&mut self) -> io::Result<String> {
        let now = Instant::now();
        if self.admission.as_ref().is_none_or(|(_, end)| *end <= now) {
            self.issued = self
                .issued
                .checked_add(1)
                .ok_or_else(|| io::Error::other("speech sequence exhausted"))?;
            self.admission = Some((format!("{}-{}", self.epoch, self.issued), now + self.expiry));
        }
        let mut status = self.speech.command("status", &[])?;
        status["epoch"] = json!(self.epoch);
        status["next_id"] = json!(self.admission.as_ref().map(|(id, _)| id));
        Ok(status.to_string())
    }

    pub(crate) fn say(&mut self, text: &str, id: Option<&str>) -> io::Result<String> {
        let id =
            id.ok_or_else(|| io::Error::other("say requires the current session speech ID"))?;
        let (_, expires) = self
            .admission
            .as_ref()
            .filter(|(issued, end)| issued == id && *end > Instant::now())
            .ok_or_else(|| io::Error::other("speech ID is stale or out of sequence"))?;
        let result = self.speech.enqueue_until(text, id, *expires)?;
        self.admission = None;
        Ok(result.to_string())
    }

    pub(crate) fn tick(&mut self) {
        self.speech.tick();
    }

    pub(crate) fn take(&mut self) -> String {
        let Some((id, expires, wav)) = self.speech.take_return() else {
            return "null".into();
        };
        let wav = STANDARD.encode(wav);
        json!({"id":id,"remaining_ms":expires.saturating_duration_since(Instant::now()).as_millis() as u64,"wav":wav}).to_string()
    }
}

struct Player {
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<Result<(), poc_speech::PlaybackError>>,
}

pub(crate) struct Receiver {
    epoch: String,
    consumed: u64,
    next_poll: Instant,
    player: Option<Player>,
    cleanup_unknown: bool,
}

impl Receiver {
    pub(crate) fn from_status(status: &str) -> io::Result<Self> {
        let body: Value = serde_json::from_str(status)?;
        let epoch = body["epoch"]
            .as_str()
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| io::Error::other("production speech session missing"))?;
        Ok(Self {
            epoch: epoch.into(),
            consumed: 0,
            next_poll: Instant::now(),
            player: None,
            cleanup_unknown: false,
        })
    }

    pub(crate) fn poll_due(&mut self) -> io::Result<bool> {
        if self.cleanup_unknown {
            return Ok(false);
        }
        if let Some(player) = &self.player {
            if !player.handle.is_finished() {
                return Ok(false);
            }
            self.join()?;
        }
        let now = Instant::now();
        if now < self.next_poll {
            return Ok(false);
        }
        self.next_poll = now + Duration::from_millis(200);
        Ok(true)
    }

    pub(crate) fn receive(
        &mut self,
        response: &str,
        started: Instant,
        cfg: &Config,
        control: SessionControl,
        generation: u64,
    ) -> io::Result<()> {
        let doc: Value = serde_json::from_str(response)?;
        if doc.is_null() {
            return Ok(());
        }
        let invalid = || io::Error::other("invalid, stale or misrouted speech reply");
        if doc.as_object().is_none_or(|o| o.len() != 3) || self.player.is_some() {
            return Err(invalid());
        }
        let (epoch, seq) = doc["id"]
            .as_str()
            .and_then(|s| s.split_once('-'))
            .ok_or_else(invalid)?;
        let sequence = seq.parse::<u64>().map_err(|_| invalid())?;
        if epoch != self.epoch || sequence <= self.consumed || sequence.to_string() != seq {
            return Err(invalid());
        }
        let ms = doc["remaining_ms"]
            .as_u64()
            .filter(|ms| *ms > 0 && *ms <= u64::from(cfg.tts_expiry_seconds) * 1000)
            .ok_or_else(invalid)?;
        let encoded = doc["wav"]
            .as_str()
            .filter(|s| s.len() <= ENCODED_LIMIT)
            .ok_or_else(invalid)?;
        let wav = STANDARD.decode(encoded).map_err(|_| invalid())?;
        // Anchor at local request start. Transit and validation can only shorten the grant.
        let deadline = started + Duration::from_millis(ms);
        let cancel = Arc::new(AtomicBool::new(false));
        let operation =
            Operation::until(Arc::clone(&cancel), deadline).with_control(control, generation);
        operation.check()?;
        let target = cfg.tts_sink.clone();
        self.consumed = sequence;
        let handle = thread::Builder::new()
            .name("splitcast-return-player".into())
            .spawn(move || poc_speech::play_return(&wav, &target, &operation))?;
        self.player = Some(Player { cancel, handle });
        Ok(())
    }

    fn join(&mut self) -> io::Result<()> {
        if let Some(player) = self.player.take() {
            match player.handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    self.cleanup_unknown = matches!(&error, poc_speech::PlaybackError::Operation(e)
                        if e.get_ref().is_some_and(|cause| cause.is::<crate::process::HelperCleanupError>()));
                    return Err(io::Error::other(error));
                }
                Err(_) => {
                    self.cleanup_unknown = true;
                    return Err(io::Error::other("return player thread panicked"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn stop(&mut self) -> io::Result<()> {
        if let Some(player) = &self.player {
            player.cancel.store(true, Ordering::Release);
        }
        let result = self.join();
        if self.cleanup_unknown {
            return Err(io::Error::other(
                "return player cleanup unconfirmed; inspect the enclosing cgroup",
            ));
        }
        // An ordinary playback error (including requested cancellation) follows group teardown.
        if let Err(error) = result {
            eprintln!("return playback ended: {error}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_rejects_wrong_session_replay_bad_framing_and_expired_transfer() {
        let cfg = Config::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/agent-example.json"
        )))
        .unwrap();
        let epoch = "abcdeabcdeabcdeabcdeabcdeabcdeab";
        let mut receiver = Receiver::from_status(&json!({"epoch":epoch}).to_string()).unwrap();
        receiver.consumed = 7;
        let control = crate::Session::new(crate::Role::Capture).control();
        let grant = json!({"id":format!("{epoch}-8"),"remaining_ms":1000,"wav":"UklGRg=="});
        for (field, invalid) in [
            ("id", json!("old-8")),
            ("id", json!(format!("{epoch}-7"))),
            ("id", json!(format!("{epoch}-08"))),
            ("wav", json!("%%%")),
            ("remaining_ms", json!(u64::MAX)),
            ("remaining_ms", json!(0)),
            ("target", json!("unselected")),
        ] {
            let mut body = grant.clone();
            body[field] = invalid;
            assert!(receiver
                .receive(
                    &body.to_string(),
                    Instant::now(),
                    &cfg,
                    control.clone(),
                    control.generation()
                )
                .is_err());
            assert!(receiver.player.is_none());
        }
        assert!(receiver
            .receive(
                &grant.to_string(),
                Instant::now() - Duration::from_secs(2),
                &cfg,
                control.clone(),
                control.generation()
            )
            .is_err());
        assert_eq!(receiver.consumed, 7);
        assert!(receiver.player.is_none());
    }
}
