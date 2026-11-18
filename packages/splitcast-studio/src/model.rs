use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use splitcast_rs_core::Output;

use crate::wire::{
    ChatPage, Failure, Layout, Message, Mode, OperationState, Persistence, Phase, Scene, Status,
    StopReply,
};

pub(crate) const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(1);
const STOP_OBSERVATION: Duration = Duration::from_secs(17);
const CHAT_MIN: Duration = Duration::from_millis(300);
const CHAT_MAX: Duration = Duration::from_secs(4);
const HISTORY: usize = 128;

#[derive(Default)]
pub(crate) enum Stop {
    #[default]
    None,
    Sending,
    Admitted {
        id: String,
        until: Instant,
    },
    Complete,
    Unknown(&'static str),
    Rejected(Failure),
}

impl Stop {
    pub(crate) fn label(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Sending => Some("Sending Stop…"),
            Self::Admitted { .. } => Some("Stop admitted · awaiting cleanup proof"),
            Self::Complete => Some("Stop completed · Agent confirmed cleanup"),
            Self::Unknown(reason) => Some(reason),
            Self::Rejected(failure) => Some(failure.label()),
        }
    }

    pub(crate) fn pending(&self) -> bool {
        matches!(self, Self::Sending | Self::Admitted { .. })
    }

    pub(crate) fn accept(&mut self, result: Result<StopReply, Failure>, at: Instant) {
        *self = match result {
            Ok(StopReply::Admitted(id)) => Self::Admitted {
                id,
                until: at + STOP_OBSERVATION,
            },
            Ok(StopReply::Completed) => Self::Complete,
            Ok(StopReply::Incomplete) => Self::Unknown("Stop cleanup incomplete · inspect Agent"),
            Err(Failure::Unknown) => Self::Unknown("Stop admission unknown · check status"),
            Err(failure) => Self::Rejected(failure),
        };
    }

    pub(crate) fn observe(&mut self, status: Option<&Status>, at: Instant) {
        if let Self::Admitted { id, until } = self {
            match status {
                Some(status) if status.completed_stop(id) => *self = Self::Complete,
                Some(status) if status.stop_id.as_ref() != Some(id) => {
                    *self = Self::Unknown("Stop cleanup unknown · observation ID changed");
                }
                Some(status)
                    if status.outcome.as_deref() == Some("unknown")
                        || status.stop.as_deref() == Some("incomplete") =>
                {
                    *self = Self::Unknown("Stop cleanup unknown · Agent has retained the intent");
                }
                _ if at >= *until => {
                    *self = Self::Unknown("Stop admitted · cleanup observation expired");
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn start_args(
    status: &Status,
    output: Output,
    armed_id: Option<&str>,
    seconds: &str,
    confirm: &str,
) -> Option<Value> {
    let id = status.start_id()?;
    if armed_id != Some(id) {
        return None;
    }
    let mut args = json!({"output": output.as_str(), "session_id": id});
    if !seconds.is_empty() || output == Output::TwitchLive {
        let seconds = seconds
            .parse::<u64>()
            .ok()
            .filter(|n| (1..=28_800).contains(n))?;
        args["duration_seconds"] = json!(seconds);
    }
    if output == Output::TwitchLive {
        if confirm != "LIVE" {
            return None;
        }
        args["confirm_public"] = json!(true);
    }
    Some(args)
}

// Each use owns one visibility context. In-flight results from a closed context
// are discarded even if the panel is reopened before they arrive.
pub(crate) struct Reader {
    pub visible: bool,
    pub revision: u64,
    pub next: Instant,
}

impl Reader {
    pub(crate) fn new(at: Instant) -> Self {
        Self {
            visible: false,
            revision: 0,
            next: at,
        }
    }

    pub(crate) fn visibility(&mut self, visible: bool) -> bool {
        if visible == self.visible {
            return false;
        }
        self.visible = visible;
        self.revision += 1;
        true
    }

    pub(crate) fn due(&self, at: Instant) -> bool {
        self.visible && at >= self.next
    }

    pub(crate) fn accepts(&self, revision: u64) -> bool {
        self.visible && self.revision == revision
    }
}

#[derive(Default)]
pub(crate) struct Chat {
    pub messages: VecDeque<Message>,
    pub epoch: Option<String>,
    pub cursor: u64,
    label: Option<String>,
    gap: bool,
    delay: Option<Duration>,
}

impl Chat {
    pub(crate) fn label(&self) -> &str {
        self.label.as_deref().unwrap_or("Awaiting chat history")
    }

    pub(crate) fn gap(&self) -> bool {
        self.gap
    }

    pub(crate) fn update(&mut self, result: Result<ChatPage, Failure>) -> (bool, Duration) {
        let old_label = self.label.take();
        let old_gap = self.gap;
        let mut changed = false;
        let (label, more) = match result {
            Err(error) => (error.label().to_owned(), false),
            Ok(page) => {
                if self.epoch != page.epoch || page.reset || page.overrun {
                    changed = !self.messages.is_empty();
                    self.messages.clear();
                    self.gap = page.overrun || (page.reset && page.epoch.is_some());
                }
                self.epoch = page.epoch;
                self.cursor = page.cursor;
                changed |= !page.messages.is_empty();
                for message in page.messages {
                    if self.messages.len() == HISTORY {
                        self.messages.pop_front();
                    }
                    self.messages.push_back(message);
                }
                let label = match page.subscription.as_ref().map(|sub| sub.state.as_str()) {
                    Some("subscribed") => "Subscribed",
                    Some("connecting") => "Connecting",
                    Some("reconnecting") => "Reconnecting",
                    Some("revoked") => "Subscription revoked",
                    Some("failed") => "Subscription failed",
                    Some(_) => "Subscription pending",
                    None if !page.enabled => "Chat disabled on production",
                    None => "No active chat session",
                };
                (label.to_owned(), page.more)
            }
        };
        changed |= old_label.as_ref() != Some(&label) || old_gap != self.gap;
        self.label = Some(label);
        let delay = if more {
            Duration::ZERO
        } else if changed {
            CHAT_MIN
        } else {
            (self.delay.unwrap_or(CHAT_MIN) * 2).clamp(CHAT_MIN, CHAT_MAX)
        };
        self.delay = Some(delay);
        (changed, delay)
    }
}

#[derive(Default)]
pub(crate) struct Editor {
    pub scene: Option<Box<Scene>>,
    pub draft: Layout,
    pub dirty: bool,
    pub conflict: bool,
    pub uncertain: Option<String>,
    pub notice: Option<&'static str>,
}

impl Editor {
    pub(crate) fn edited(&mut self, draft: Layout) {
        if self.draft != draft {
            self.draft = draft;
            self.dirty = self
                .scene
                .as_ref()
                .is_none_or(|scene| scene.draft != self.draft);
        }
    }

    pub(crate) fn observe(&mut self, scene: Box<Scene>, submitted: Option<&str>) {
        let observed = scene.operation.as_ref();
        let matched = submitted.is_some_and(|id| observed.is_some_and(|op| op.id == id));
        let expected = self.uncertain.as_deref();
        let recovered = expected.is_some_and(|id| observed.is_some_and(|op| op.id == id));
        let successful = observed.is_some_and(|op| {
            matches!(
                op.state,
                OperationState::Draft
                    | OperationState::Applied
                    | OperationState::Reconciled
                    | OperationState::Saved
                    | OperationState::Reloaded
            )
        });
        if (matched || recovered) && successful {
            self.dirty = false;
            self.conflict = false;
        } else if self.dirty {
            if let Some(old) = &self.scene {
                self.conflict |= old.generation != scene.generation && !matched && !recovered;
            } else {
                self.draft.camera = scene.draft.camera.clone();
            }
        }
        if !self.dirty {
            self.draft = scene.draft.clone();
            self.conflict = false;
        }
        self.notice = match (submitted, expected, observed.map(|op| op.state)) {
            (Some(_), _, _) if !matched => Some("Operation result unknown · response ID changed"),
            (None, Some(_), _) if !recovered => {
                Some("Earlier operation unconfirmed · current Agent scene observed")
            }
            (_, _, Some(OperationState::Unknown | OperationState::Failed)) => {
                Some("Scene outcome uncertain · observe OBS or saved layout")
            }
            _ => None,
        };
        self.uncertain = if submitted.is_some() && !matched {
            submitted.map(str::to_owned)
        } else {
            None
        };
        self.scene = Some(scene);
    }

    pub(crate) fn replace_with_agent(&mut self) {
        if let Some(scene) = &self.scene {
            self.draft = scene.draft.clone();
            self.dirty = false;
            self.conflict = false;
        }
    }

    pub(crate) fn valid(&self) -> bool {
        self.scene.as_ref().is_some_and(|scene| {
            scene.draft.camera.is_some() == self.draft.camera.is_some()
                && self.draft.within(&scene.canvas)
        })
    }

    pub(crate) fn can_save(&self) -> bool {
        !self.dirty
            && self.scene.as_ref().is_some_and(|scene| {
                scene.mode == Mode::Live
                    && scene.applied.as_ref() == Some(&scene.draft)
                    && scene.persistence != Persistence::Saved
            })
    }

    pub(crate) fn label(&self) -> &'static str {
        if self.uncertain.is_some() {
            return "Outcome unknown · observe Agent scene";
        }
        if self.conflict {
            return "Conflict · compare local draft with Agent";
        }
        if self.dirty {
            return "Local draft";
        }
        let Some(scene) = &self.scene else {
            return "Offline draft";
        };
        match (scene.mode, scene.persistence, scene.applied.as_ref()) {
            (_, Persistence::Unknown, _) => "Saved state unknown · reload to observe",
            (Mode::Live, Persistence::Saved, Some(applied)) if *applied == scene.draft => {
                "Saved · OBS verified"
            }
            (Mode::Live, _, Some(applied)) if *applied == scene.draft => "Applied · unsaved",
            (Mode::Live, _, Some(_)) => "OBS differs from draft",
            (Mode::Live, _, None) => "Awaiting OBS readback",
            (Mode::Draft, Persistence::Saved, _) => "Draft matches saved layout",
            (Mode::Draft, Persistence::Unsaved, _) => "Agent draft · unsaved",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        serde_json::from_value(json!({
            "role":"capture","state":"ready","start_id":"start-1","remote_release_pending":false,
        }))
        .unwrap()
    }

    #[test]
    fn public_start_requires_ready_matching_id_duration_and_explicit_confirmation() {
        let status = status();
        let start =
            |id, duration, confirm| start_args(&status, Output::TwitchLive, id, duration, confirm);
        assert!(start(Some("start-0"), "60", "LIVE").is_none());
        assert!(start(Some("start-1"), "", "LIVE").is_none());
        assert!(start(Some("start-1"), "28801", "LIVE").is_none());
        assert!(start(Some("start-1"), "60", "live").is_none());
        assert_eq!(
            start(Some("start-1"), "60", "LIVE").unwrap(),
            json!({
                "output":"twitch-live","session_id":"start-1","duration_seconds":60,"confirm_public":true,
            })
        );
    }

    #[test]
    fn stop_requires_current_completed_idle_release() {
        let now = Instant::now();
        let mut stop = Stop::Sending;
        stop.accept(Ok(StopReply::Admitted("stop-1".into())), now);
        let mut status = status();
        status.state = Phase::Stopping;
        status.stop_id = Some("stop-1".into());
        status.outcome = Some("accepted".into());
        stop.observe(Some(&status), now);
        assert!(matches!(stop, Stop::Admitted { .. }));
        status.state = Phase::Idle;
        status.stop = Some("completed".into());
        status.outcome = Some("completed".into());
        status.remote_release_pending = true;
        stop.observe(Some(&status), now);
        assert!(matches!(stop, Stop::Admitted { .. }));
        status.remote_release_pending = false;
        stop.observe(Some(&status), now);
        assert!(matches!(stop, Stop::Complete));
    }

    #[test]
    fn hidden_or_superseded_read_is_discarded_without_resetting_cadence() {
        let now = Instant::now();
        let mut reader = Reader::new(now);
        reader.visibility(true);
        let revision = reader.revision;
        reader.next = now + SNAPSHOT_INTERVAL;
        reader.visibility(false);
        assert!(!reader.due(now + SNAPSHOT_INTERVAL));
        reader.visibility(true);
        assert!(!reader.accepts(revision));
        assert!(!reader.due(now));
    }

    #[test]
    fn chat_is_bounded_and_empty_pages_do_not_change_the_display() {
        let mut chat = Chat::default();
        let page = |scalar: u64, count, overrun| ChatPage {
            enabled: true,
            epoch: Some("epoch".into()),
            cursor: scalar + count,
            more: false,
            overrun,
            reset: false,
            subscription: None,
            messages: (1..=count)
                .map(|i| Message {
                    cursor: scalar + i,
                    id: format!("id-{}", scalar + i),
                    display_name: "fixture".into(),
                    text: "".into(),
                })
                .collect(),
        };
        for i in 0..8 {
            chat.update(Ok(page(i * 32, 32, false)));
        }
        assert_eq!(chat.messages.len(), 128);
        assert_eq!(chat.messages.front().unwrap().cursor, 129);
        let (changed, delay) = chat.update(Ok(page(256, 0, false)));
        assert!(!changed && delay > CHAT_MIN);
        assert_eq!(chat.cursor, 256);
        chat.update(Ok(page(300, 1, true)));
        assert_eq!(chat.messages.len(), 1);
        assert!(chat.gap());
    }
}
