use crate::{protocol::OwnerId, Output};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Capture,
    Production,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Idle,
    Preparing,
    Ready,
    Running,
    Degraded,
    Stopping,
    Failed,
}

#[derive(Clone, Debug)]
enum CaptureAuthority {
    LocalOnly,
    RemoteAttempt(OwnerId),
}
#[derive(Clone, Debug)]
enum Authority {
    Capture(CaptureAuthority),
    Local,
    Peer { owner: OwnerId, expires: Instant },
}
impl Authority {
    fn matches(&self, peer: Option<&OwnerId>) -> bool {
        match self {
            Self::Local | Self::Capture(_) => peer.is_none(),
            Self::Peer { owner, .. } => peer == Some(owner),
        }
    }
    fn expired(&self, now: Instant) -> bool {
        matches!(self,Self::Peer{expires,..}if now>=*expires)
    }
}

#[derive(Clone, Debug)]
enum Phase {
    Idle,
    Preparing(Authority),
    Ready(Authority),
    Running {
        authority: Authority,
        output: Output,
    },
    Degraded {
        authority: Authority,
        output: Option<Output>,
    },
    Stopping {
        authority: Authority,
        output: Option<Output>,
    },
    Failed(Authority),
}
impl Phase {
    fn state(&self) -> Lifecycle {
        match self {
            Self::Idle => Lifecycle::Idle,
            Self::Preparing(_) => Lifecycle::Preparing,
            Self::Ready(_) => Lifecycle::Ready,
            Self::Running { .. } => Lifecycle::Running,
            Self::Degraded { .. } => Lifecycle::Degraded,
            Self::Stopping { .. } => Lifecycle::Stopping,
            Self::Failed(_) => Lifecycle::Failed,
        }
    }
    fn authority(&self) -> Option<&Authority> {
        match self {
            Self::Idle => None,
            Self::Preparing(a) | Self::Ready(a) | Self::Failed(a) => Some(a),
            Self::Running { authority, .. }
            | Self::Degraded { authority, .. }
            | Self::Stopping { authority, .. } => Some(authority),
        }
    }
    fn authority_mut(&mut self) -> Option<&mut Authority> {
        match self {
            Self::Idle => None,
            Self::Preparing(a) | Self::Ready(a) | Self::Failed(a) => Some(a),
            Self::Running { authority, .. }
            | Self::Degraded { authority, .. }
            | Self::Stopping { authority, .. } => Some(authority),
        }
    }
    fn output(&self) -> Option<Output> {
        match self {
            Self::Running { output, .. } => Some(*output),
            Self::Degraded { output, .. } | Self::Stopping { output, .. } => *output,
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TransitionError {
    pub from: Lifecycle,
    pub event: &'static str,
}
impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid lifecycle transition: {:?} via {}",
            self.from, self.event
        )
    }
}
impl std::error::Error for TransitionError {}

#[derive(Debug)]
pub struct Session {
    role: Role,
    control: SessionControl,
}

// Only Session changes Phase. The listener can authorize intent and renew a
// lease, but never owns effects or performs a lifecycle transition. Only a
// nonblocking dispatch syscall may perform I/O under this metadata lock.
#[derive(Debug)]
struct ControlState {
    phase: Phase,
    generation: u64,
    stop: Option<StopIntent>,
    active_owner: Option<OwnerId>,
    last_stop_owner: Option<OwnerId>,
    stop_completed: bool,
}
impl ControlState {
    fn revoked(&self, generation: u64) -> bool {
        self.generation != generation
            || self.stop.is_some()
            || self
                .phase
                .authority()
                .is_some_and(|a| a.expired(Instant::now()))
    }
}
#[derive(Debug)]
struct StopIntent {
    ticket: Arc<crate::request::RequestTicket>,
    since: Instant,
}
const STOP_OBSERVATION_BUDGET: Duration = Duration::from_secs(15);
#[derive(Clone, Debug)]
pub(crate) struct SessionControl {
    role: Role,
    state: Arc<Mutex<ControlState>>,
}
impl SessionControl {
    fn lock(&self) -> MutexGuard<'_, ControlState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub(crate) fn generation(&self) -> u64 {
        self.lock().generation
    }
    pub(crate) fn admit(&self, observation: bool) -> std::io::Result<u64> {
        let state = self.lock();
        if state.stop.is_some() && !observation {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Stop cleanup pending; submit a fresh command after completion",
            ));
        }
        Ok(state.generation)
    }
    pub(crate) fn begin(&self, generation: u64, activation_owner: Option<OwnerId>) -> bool {
        let mut state = self.lock();
        if state.generation != generation || state.stop.is_some() {
            return false;
        }
        state.active_owner = activation_owner;
        true
    }
    pub(crate) fn end(&self) {
        let mut state = self.lock();
        state.active_owner = None;
    }
    pub(crate) fn cancelled(&self, generation: u64) -> bool {
        self.lock().revoked(generation)
    }
    pub(crate) fn dispatch<T>(
        &self,
        generation: u64,
        write: impl FnOnce() -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        let state = self.lock();
        if state.revoked(generation) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Stop revoked effect dispatch; outcome may be unknown",
            ));
        }
        // Callers perform one nonblocking socket syscall, never a capacity or
        // acknowledgment wait, while the admission lock is held.
        write()
    }
    pub(crate) fn dispatch_prepare(
        &self,
        generation: u64,
        owner: &OwnerId,
        write: impl FnOnce() -> std::io::Result<usize>,
    ) -> std::io::Result<usize> {
        let mut state = self.lock();
        let preparing = match &state.phase {
            Phase::Preparing(Authority::Capture(CaptureAuthority::LocalOnly)) => true,
            Phase::Preparing(Authority::Capture(CaptureAuthority::RemoteAttempt(held))) => {
                held == owner
            }
            _ => false,
        };
        if state.revoked(generation) || !preparing {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "remote Prepare dispatch revoked",
            ));
        }
        // The socket is nonblocking. A positive write and its obligation become
        // visible under the same lock as Stop. Err/zero prove no bytes from this
        // write; even an HTTP prefix is conservatively a remote attempt.
        let written = write()?;
        if written > 0 {
            state.phase = Phase::Preparing(Authority::Capture(CaptureAuthority::RemoteAttempt(
                owner.clone(),
            )));
        }
        Ok(written)
    }
    fn request_locked(
        state: &mut ControlState,
    ) -> std::io::Result<Arc<crate::request::RequestTicket>> {
        if let Some(intent) = &state.stop {
            return Ok(Arc::clone(&intent.ticket));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("control generation exhausted"))?;
        if matches!(state.phase, Phase::Idle) && state.active_owner.is_some() {
            // Stop can win after begin but before Preparing. That reservation
            // has no effects and cannot subsequently prepare, yet its owner
            // still needs a matching completion proof after unwinding.
            state.last_stop_owner = state.active_owner.clone();
        }
        let ticket = Arc::new(crate::request::RequestTicket::accepted());
        state.stop_completed = false;
        state.stop = Some(StopIntent {
            ticket: Arc::clone(&ticket),
            since: Instant::now(),
        });
        Ok(ticket)
    }
    pub(crate) fn request_stop(
        &self,
        owner: Option<&OwnerId>,
    ) -> std::io::Result<Arc<crate::request::RequestTicket>> {
        let mut state = self.lock();
        if let Some(owner) = owner {
            // Expiry revokes creation/renewal, not the matching owner's duty
            // to release held effects. Phase ownership still rejects old peers
            // after a different owner acquires the session.
            if self.role != Role::Production
                || state
                    .phase
                    .authority()
                    .is_some_and(|a| !a.matches(Some(owner)))
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "no matching production owner for release",
                ));
            }
            // Idle has no effect to cancel. A stale remote Stop must not fence
            // another owner's queued Prepare. It can confirm already-idle.
            if matches!(state.phase, Phase::Idle) {
                if let Some(active) = &state.active_owner {
                    if active != owner {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "another owner is beginning preparation",
                        ));
                    }
                    let ticket = Self::request_locked(&mut state)?;
                    return Ok(ticket);
                }
                if state.last_stop_owner.as_ref() != Some(owner) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "no completed Stop for this owner; in-transit effects remain unknown",
                    ));
                }
                let ticket = Arc::new(crate::request::RequestTicket::new(Instant::now()));
                ticket.finish((
                    200,
                    serde_json::json!({"state":"idle","stop":"completed","outcome":"completed","owner":owner.as_str()})
                        .to_string(),
                ));
                return Ok(ticket);
            }
        }
        let ticket = Self::request_locked(&mut state)?;
        Ok(ticket)
    }
    pub(crate) fn completed_owner(&self) -> Option<OwnerId> {
        let state = self.lock();
        if matches!(state.phase, Phase::Idle) && state.stop.is_none() {
            state.last_stop_owner.clone()
        } else {
            None
        }
    }
    pub(crate) fn stop_completed(&self) -> bool {
        let state = self.lock();
        state.stop_completed && state.stop.is_none() && matches!(state.phase, Phase::Idle)
    }
    pub(crate) fn stop_observation(
        &self,
        owner: Option<&OwnerId>,
    ) -> std::io::Result<Option<String>> {
        let state = self.lock();
        let Some(intent) = &state.stop else {
            return Ok(None);
        };
        if owner.is_some()
            && (self.role != Role::Production
                || state.phase.authority().is_some_and(|a| !a.matches(owner)))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "no matching production owner",
            ));
        }
        let peer = match state.phase.authority() {
            Some(Authority::Peer { owner, .. }) => Some(owner.as_str()),
            _ => state.active_owner.as_ref().map(OwnerId::as_str),
        };
        let remote = matches!(
            state.phase.authority(),
            Some(Authority::Capture(CaptureAuthority::RemoteAttempt(_)))
        );
        let overdue = intent.since.elapsed() >= STOP_OBSERVATION_BUDGET;
        Ok(Some(serde_json::json!({"version":1,"engine":"rust-candidate","role":match self.role {Role::Capture=>"capture",Role::Production=>"production"},"state":"stopping","stop":"requested","outcome":if overdue {"unknown"} else {"accepted"},"cleanup":if overdue {"unknown"} else {"pending"},"resource_observation":"unknown","children":null,"output":state.phase.output().map(Output::as_str),"remote_release_pending":remote,"owner":peer}).to_string()))
    }
    pub(crate) fn heartbeat(
        &self,
        owner: Option<&OwnerId>,
        ttl: Duration,
    ) -> std::io::Result<String> {
        let mut state = self.lock();
        if self.role != Role::Production
            || state.stop.is_some()
            || !matches!(
                state.phase.state(),
                Lifecycle::Ready | Lifecycle::Running | Lifecycle::Degraded
            )
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "no renewable production lease",
            ));
        }
        let now = Instant::now();
        match state.phase.authority_mut() {
            Some(Authority::Peer {
                owner: held,
                expires,
            }) if Some(&*held) == owner && now < *expires => *expires = now + ttl,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "no matching live production owner",
                ))
            }
        }
        Ok(serde_json::json!({"renewed":true,"state":match state.phase.state() { Lifecycle::Ready=>"ready", Lifecycle::Running=>"running", Lifecycle::Degraded=>"degraded", _=>unreachable!("renewable state checked under lock") }}).to_string())
    }
    pub(crate) fn expire(&self) {
        let mut state = self.lock();
        if !matches!(state.phase, Phase::Stopping { .. })
            && state
                .phase
                .authority()
                .is_some_and(|a| a.expired(Instant::now()))
        {
            // Generation exhaustion is fail-closed: Operation also observes the
            // expired authority, and the effect owner still checks lease expiry.
            let _ = Self::request_locked(&mut state);
        }
    }
    pub(crate) fn stop_pending(&self) -> bool {
        self.lock().stop.is_some()
    }
    pub(crate) fn finish_stop(&self, mut response: crate::request::Response) {
        let mut state = self.lock();
        if let Some(intent) = state.stop.take() {
            state.stop_completed = response.0 == 200;
            if response.0 == 200 {
                if let Some(owner) = &state.last_stop_owner {
                    let mut body: serde_json::Value =
                        serde_json::from_str(&response.1).expect("owner Stop completion is JSON");
                    body["owner"] = serde_json::json!(owner.as_str());
                    response.1 = body.to_string();
                }
            }
            intent.ticket.finish(response);
        }
    }
}
impl Session {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            control: SessionControl {
                role,
                state: Arc::new(Mutex::new(ControlState {
                    phase: Phase::Idle,
                    generation: 0,
                    stop: None,
                    active_owner: None,
                    last_stop_owner: None,
                    stop_completed: false,
                })),
            },
        }
    }
    pub fn role(&self) -> Role {
        self.role
    }
    pub fn state(&self) -> Lifecycle {
        self.control.lock().phase.state()
    }
    pub fn output(&self) -> Option<Output> {
        self.control.lock().phase.output()
    }
    pub(crate) fn control(&self) -> SessionControl {
        self.control.clone()
    }
    fn error(phase: &Phase, event: &'static str) -> TransitionError {
        TransitionError {
            from: phase.state(),
            event,
        }
    }
    pub fn authorize(&self, owner: Option<&OwnerId>) -> Result<(), TransitionError> {
        let state = self.control.lock();
        if state.phase.authority().is_none_or(|a| a.matches(owner)) {
            Ok(())
        } else {
            Err(Self::error(&state.phase, "ownership"))
        }
    }
    pub fn lease_expired(&self, now: Instant) -> bool {
        let state = self.control.lock();
        !matches!(state.phase, Phase::Stopping { .. })
            && state.phase.authority().is_some_and(|a| a.expired(now))
    }
    pub fn prepare(
        &mut self,
        owner: Option<OwnerId>,
        now: Instant,
        ttl: Duration,
    ) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.phase.state() != Lifecycle::Idle || state.stop.is_some() {
            return Err(Self::error(&state.phase, "prepare"));
        }
        let authority = match owner {
            Some(owner) if self.role == Role::Production => Authority::Peer {
                owner,
                expires: now + ttl,
            },
            Some(_) => return Err(Self::error(&state.phase, "capture peer ownership")),
            None if self.role == Role::Capture => Authority::Capture(CaptureAuthority::LocalOnly),
            None => Authority::Local,
        };
        state.phase = Phase::Preparing(authority);
        state.last_stop_owner = None;
        state.stop_completed = false;
        Ok(())
    }
    pub fn prepared(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.stop.is_some()
            || state
                .phase
                .authority()
                .is_some_and(|a| a.expired(Instant::now()))
        {
            return Err(Self::error(&state.phase, "cancelled preparation"));
        }
        let Phase::Preparing(authority) = &state.phase else {
            return Err(Self::error(&state.phase, "prepared"));
        };
        if matches!(authority, Authority::Capture(CaptureAuthority::LocalOnly)) {
            return Err(Self::error(
                &state.phase,
                "capture requires a remote preparation",
            ));
        }
        state.phase = Phase::Ready(authority.clone());
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn record_remote_attempt(&mut self, owner: OwnerId) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.stop.is_some() {
            return Err(Self::error(&state.phase, "cancelled remote preparation"));
        }
        let Phase::Preparing(Authority::Capture(authority @ CaptureAuthority::LocalOnly)) =
            &mut state.phase
        else {
            return Err(Self::error(&state.phase, "remote preparation"));
        };
        *authority = CaptureAuthority::RemoteAttempt(owner);
        Ok(())
    }
    pub(crate) fn remote_owner(&self) -> Option<OwnerId> {
        match self.control.lock().phase.authority() {
            Some(Authority::Capture(CaptureAuthority::RemoteAttempt(owner))) => Some(owner.clone()),
            _ => None,
        }
    }
    pub(crate) fn remote_released(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if !matches!(
            state.phase.state(),
            Lifecycle::Preparing | Lifecycle::Failed | Lifecycle::Stopping
        ) {
            return Err(Self::error(&state.phase, "remote release"));
        }
        match state.phase.authority_mut() {
            Some(Authority::Capture(authority @ CaptureAuthority::RemoteAttempt(_))) => {
                *authority = CaptureAuthority::LocalOnly;
                Ok(())
            }
            _ => Err(Self::error(&state.phase, "remote release")),
        }
    }
    pub fn renew(
        &mut self,
        owner: &OwnerId,
        now: Instant,
        ttl: Duration,
    ) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if !matches!(
            state.phase.state(),
            Lifecycle::Ready | Lifecycle::Running | Lifecycle::Degraded
        ) || state.stop.is_some()
            || state.phase.authority().is_some_and(|a| a.expired(now))
        {
            return Err(Self::error(&state.phase, "heartbeat"));
        }
        match state.phase.authority_mut() {
            Some(Authority::Peer {
                owner: held,
                expires,
            }) if held == owner => {
                *expires = now + ttl;
                Ok(())
            }
            _ => Err(Self::error(&state.phase, "heartbeat owner")),
        }
    }
    pub fn start(&mut self, output: Output) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.stop.is_some()
            || state
                .phase
                .authority()
                .is_some_and(|a| a.expired(Instant::now()))
        {
            return Err(Self::error(&state.phase, "cancelled start"));
        }
        match &state.phase {
            Phase::Ready(authority) => {
                state.phase = Phase::Running {
                    authority: authority.clone(),
                    output,
                };
                Ok(())
            }
            Phase::Running { output: active, .. } if *active == output => Ok(()),
            _ => Err(Self::error(&state.phase, "start")),
        }
    }
    pub fn degrade(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if !matches!(state.phase.state(), Lifecycle::Ready | Lifecycle::Running) {
            return Err(Self::error(&state.phase, "degrade"));
        }
        state.phase = Phase::Degraded {
            authority: state.phase.authority().unwrap().clone(),
            output: state.phase.output(),
        };
        Ok(())
    }
    pub fn fail(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if !matches!(
            state.phase.state(),
            Lifecycle::Preparing | Lifecycle::Ready | Lifecycle::Running | Lifecycle::Degraded
        ) {
            return Err(Self::error(&state.phase, "fail"));
        }
        state.phase = Phase::Failed(state.phase.authority().unwrap().clone());
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if matches!(state.phase.state(), Lifecycle::Idle | Lifecycle::Stopping) {
            return Ok(());
        }
        state.phase = Phase::Stopping {
            authority: state.phase.authority().unwrap().clone(),
            output: state.phase.output(),
        };
        Ok(())
    }
    pub fn stopped(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.phase.state() != Lifecycle::Stopping
            || matches!(
                state.phase.authority(),
                Some(Authority::Capture(CaptureAuthority::RemoteAttempt(_)))
            )
        {
            return Err(Self::error(&state.phase, "stopped"));
        }
        state.last_stop_owner = match state.phase.authority() {
            Some(Authority::Peer { owner, .. }) => Some(owner.clone()),
            _ => None,
        };
        state.phase = Phase::Idle;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overdue_stop_observation_is_unknown_without_erasing_owned_state() {
        let mut session = ready(None, Instant::now());
        session.start(Output::Recording).unwrap();
        let control = session.control();
        control.request_stop(None).unwrap();
        control.lock().stop.as_mut().unwrap().since = Instant::now() - STOP_OBSERVATION_BUDGET;
        let snapshot: serde_json::Value =
            serde_json::from_str(&control.stop_observation(None).unwrap().unwrap()).unwrap();
        assert_eq!(snapshot["state"], "stopping");
        assert_eq!(snapshot["cleanup"], "unknown");
        assert_eq!(snapshot["outcome"], "unknown");
        assert!(snapshot["children"].is_null());
        assert_eq!(session.output(), Some(Output::Recording));
        assert!(session.start(Output::Recording).is_err());
        assert!(session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .is_err());
    }
    #[test]
    fn admission_and_prepare_begin_are_ordered_with_stop() {
        let mut session = Session::new(Role::Production);
        let control = session.control();
        let owner = OwnerId::parse("beginning-owner").unwrap();
        let intruder = OwnerId::parse("intruder").unwrap();
        let generation = control.admit(false).unwrap();
        assert!(control.begin(generation, Some(owner.clone())));
        assert!(control.request_stop(Some(&intruder)).is_err());
        let ticket = control.request_stop(Some(&owner)).unwrap();
        assert!(
            control.admit(false).is_err(),
            "activation was parked during cleanup"
        );
        assert!(session
            .prepare(Some(owner), Instant::now(), Duration::from_secs(30))
            .is_err());
        control.end();
        control.finish_stop((200, "{}".into()));
        assert_eq!(ticket.response().unwrap().0, 200);
        let proof: serde_json::Value = serde_json::from_str(&ticket.response().unwrap().1).unwrap();
        assert_eq!(
            proof["owner"], "beginning-owner",
            "cancelled begin lost its remote completion proof"
        );
        assert!(!control.begin(generation, None));
        assert!(control.begin(control.admit(false).unwrap(), None));
        assert_eq!(session.state(), Lifecycle::Idle);
    }
    #[test]
    fn stop_authorization_precedes_revocation_and_final_commit() {
        let owner = OwnerId::parse("owner").unwrap();
        let intruder = OwnerId::parse("intruder").unwrap();
        let mut session = ready(Some(owner.clone()), Instant::now());
        let control = session.control();
        let generation = control.generation();
        assert!(control.request_stop(Some(&intruder)).is_err());
        assert!(
            session.start(Output::Recording).is_ok(),
            "unauthorized Stop revoked Start"
        );
        let ticket = control.request_stop(Some(&owner)).unwrap();
        assert!(
            session.start(Output::Recording).is_err(),
            "Stop lost the final commit race"
        );
        assert!(session
            .renew(&owner, Instant::now(), Duration::from_secs(30))
            .is_err());
        session.stop().unwrap();
        session.stopped().unwrap();
        control.finish_stop((
            200,
            serde_json::json!({"stop":"completed","state":"idle"}).to_string(),
        ));
        assert_eq!(ticket.response().unwrap().0, 200);
        assert_eq!(session.state(), Lifecycle::Idle);
        assert!(
            !control.begin(generation, None),
            "old queued activation escaped Stop"
        );
        assert!(control.request_stop(Some(&intruder)).is_err());
        let completed = control.request_stop(Some(&owner)).unwrap();
        let proof: serde_json::Value =
            serde_json::from_str(&completed.response().unwrap().1).unwrap();
        assert_eq!(proof["owner"], owner.as_str());
        session
            .prepare(
                Some(intruder.clone()),
                Instant::now(),
                Duration::from_secs(30),
            )
            .unwrap();
        assert!(
            control.request_stop(Some(&owner)).is_err(),
            "old owner cancelled newer authority"
        );
        session.prepared().unwrap();
        session.start(Output::Recording).unwrap();
        assert_eq!(session.output(), Some(Output::Recording));
    }

    #[test]
    fn expired_preparation_cannot_commit_or_be_revived_by_urgent_control() {
        let owner = OwnerId::parse("expired").unwrap();
        let mut session = Session::new(Role::Production);
        session
            .prepare(
                Some(owner.clone()),
                Instant::now() - Duration::from_secs(2),
                Duration::from_secs(1),
            )
            .unwrap();
        let control = session.control();
        assert!(control.request_stop(Some(&owner)).is_ok());
        assert!(control
            .heartbeat(Some(&owner), Duration::from_secs(30))
            .is_err());
        assert!(session.prepared().is_err());
        assert_eq!(session.state(), Lifecycle::Preparing);
        control.expire();
        session.stop().unwrap();
        session.stopped().unwrap();
        control.finish_stop((200, "{}".into()));
        assert_eq!(session.state(), Lifecycle::Idle);
    }

    #[test]
    fn accepted_stop_blocks_prepared_commit_and_does_not_cancel_cleanup() {
        let mut session = Session::new(Role::Production);
        session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        let control = session.control();
        let first = control.request_stop(None).unwrap();
        let repeat = control.request_stop(None).unwrap();
        assert!(session.prepared().is_err());
        session.stop().unwrap();
        // Failed cleanup cannot permit prepare or discard the current stop.
        assert!(session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .is_err());
        control.finish_stop((409, r#"{"stop":"incomplete","outcome":"unknown"}"#.into()));
        assert_eq!(first.response(), repeat.response());
        assert_eq!(session.state(), Lifecycle::Stopping);
        let retry = control.request_stop(None).unwrap();
        session.stopped().unwrap();
        control.finish_stop((200, "{}".into()));
        assert_eq!(retry.response().unwrap().0, 200);
        assert_eq!(session.state(), Lifecycle::Idle);
    }
    #[test]
    fn capture_remote_obligation_survives_failure_until_confirmed_release() {
        let now = Instant::now();
        let owner = OwnerId::parse("attempt").unwrap();
        let mut session = Session::new(Role::Capture);
        session.prepare(None, now, Duration::from_secs(30)).unwrap();
        assert!(session.prepared().is_err());
        assert!(session.remote_owner().is_none());
        session.record_remote_attempt(owner.clone()).unwrap();
        session.prepared().unwrap();
        session.start(Output::Recording).unwrap();
        assert!(session.remote_released().is_err());
        session.fail().unwrap();
        session.stop().unwrap();
        assert_eq!(session.remote_owner(), Some(owner.clone()));
        assert!(session.stopped().is_err());
        session.remote_released().unwrap();
        session.stopped().unwrap();
        assert!(session.remote_owner().is_none());
        let mut production = ready(None, now);
        assert!(production.record_remote_attempt(owner).is_err());
    }
    #[test]
    fn lease_expiry_does_not_retrigger_teardown_while_stopping() {
        let now = Instant::now();
        let mut session = ready(Some(OwnerId::parse("owner").unwrap()), now);
        assert!(session.lease_expired(now + Duration::from_secs(31)));
        session.stop().unwrap();
        assert!(
            !session.lease_expired(now + Duration::from_secs(31)),
            "stopping retriggers automatic expired-lease cleanup"
        );
    }
    fn ready(owner: Option<OwnerId>, now: Instant) -> Session {
        let mut s = Session::new(Role::Production);
        s.prepare(owner, now, Duration::from_secs(30)).unwrap();
        s.prepared().unwrap();
        s
    }
    #[test]
    fn no_output_before_explicit_start() {
        let mut s = ready(None, Instant::now());
        assert_eq!(s.output(), None);
        s.start(Output::Recording).unwrap();
        s.start(Output::Recording).unwrap();
        s.degrade().unwrap();
        s.fail().unwrap();
        assert_eq!(s.output(), None);
        s.stop().unwrap();
        s.stopped().unwrap();
        assert_eq!(s.state(), Lifecycle::Idle);
    }
    #[test]
    fn ownership_cannot_be_invented_or_revived() {
        let now = Instant::now();
        let owner = OwnerId::parse("owner").unwrap();
        let mut local = ready(None, now);
        assert!(local.renew(&owner, now, Duration::from_secs(30)).is_err());
        let mut peer = ready(Some(owner.clone()), now);
        peer.renew(&owner, now, Duration::from_secs(30)).unwrap();
        assert!(peer
            .renew(
                &owner,
                now + Duration::from_secs(31),
                Duration::from_secs(30)
            )
            .is_err());
        assert!(peer.authorize(None).is_err());
        peer.stop().unwrap();
        peer.stopped().unwrap();
        assert!(!peer.lease_expired(now + Duration::from_secs(90)));
    }
}
