use crate::obs::ProgressSnapshot;
use crate::protocol::{OwnerId, RemoteOwner, StopProof};
use crate::request::{error_response, rejected, Failure, RequestTicket, Response};
use crate::supervision::{Fault, MAX_SAMPLE_AGE};
use crate::Output;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};
use std::{fmt, io};

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
    RemoteAttempt(RemoteOwner),
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

    fn peer_owner(&self) -> Option<&OwnerId> {
        match self {
            Self::Peer { owner, .. } => Some(owner),
            Self::Capture(_) | Self::Local => None,
        }
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
        observation: Option<RunningObservation>,
    },
    Degraded {
        authority: Authority,
        output: Option<Output>,
    },
    Stopping {
        authority: Authority,
        output: Option<Output>,
    },
    Failed {
        authority: Authority,
        compensating: Option<Fault>,
    },
}

#[derive(Clone, Debug)]
struct RunningObservation {
    progress: ProgressSnapshot,
    sampled_at: Instant,
    terminal: Option<Fault>,
}

impl RunningObservation {
    fn fault(&self, now: Instant) -> Option<Fault> {
        self.terminal.or_else(|| {
            (now.saturating_duration_since(self.sampled_at) >= MAX_SAMPLE_AGE)
                .then_some(Fault::SampleOverdue)
        })
    }
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
            Self::Failed { .. } => Lifecycle::Failed,
        }
    }
    fn authority(&self) -> Option<&Authority> {
        match self {
            Self::Idle => None,
            Self::Preparing(a) | Self::Ready(a) => Some(a),
            Self::Running { authority, .. }
            | Self::Degraded { authority, .. }
            | Self::Failed { authority, .. }
            | Self::Stopping { authority, .. } => Some(authority),
        }
    }
    fn authority_mut(&mut self) -> Option<&mut Authority> {
        match self {
            Self::Idle => None,
            Self::Preparing(a) | Self::Ready(a) => Some(a),
            Self::Running { authority, .. }
            | Self::Degraded { authority, .. }
            | Self::Failed { authority, .. }
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
    fn observation_fault(&self, now: Instant) -> Option<Fault> {
        match self {
            Self::Running {
                observation: Some(observation),
                ..
            } => observation.fault(now),
            Self::Failed { compensating, .. } => *compensating,
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

// Only the effect owner changes Phase. The listener authorizes intent and
// renews leases under the same lock. I/O under it is limited to nonblocking
// dispatch syscalls.
#[derive(Debug)]
struct ControlState {
    phase: Phase,
    generation: u64,
    output_until: Option<Instant>,
    instance: String,
    stop: Option<StopRecord>,
}
impl ControlState {
    fn pending_stop(&self) -> Option<&StopIntent> {
        match &self.stop {
            Some(StopRecord::Pending(intent)) => Some(intent),
            _ => None,
        }
    }

    fn expired(&self, now: Instant) -> bool {
        self.output_until.is_some_and(|end| now >= end)
            || self.phase.authority().is_some_and(|a| a.expired(now))
    }

    fn revoked(&self, generation: u64) -> bool {
        let now = Instant::now();
        self.generation != generation
            || self.pending_stop().is_some()
            || self.expired(now)
            || self.phase.observation_fault(now).is_some()
    }

    fn advance_generation(&mut self) -> io::Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("control generation exhausted"))?;
        Ok(())
    }

    fn start_id(&self, now: Instant) -> Option<String> {
        if self.pending_stop().is_some()
            || self.expired(now)
            || self.phase.observation_fault(now).is_some()
        {
            return None;
        }
        let phase = match self.phase.state() {
            Lifecycle::Idle => "idle",
            Lifecycle::Ready => "ready",
            Lifecycle::Running => "running",
            _ => return None,
        };
        Some(format!("{}-{}-{phase}", self.instance, self.generation))
    }

    fn idle_id(&self, now: Instant) -> Option<String> {
        if matches!(self.phase, Phase::Idle) {
            self.start_id(now)
        } else {
            None
        }
    }

    fn prepare_authority(
        &self,
        role: Role,
        owner: Option<&OwnerId>,
        now: Instant,
        ttl: Duration,
    ) -> Result<Authority, TransitionError> {
        let id = self
            .idle_id(now)
            .ok_or_else(|| Session::error(&self.phase, "prepare"))?;
        match (role, owner) {
            (Role::Production, Some(owner)) if owner.matches_prepare_id(&id) => {
                Ok(Authority::Peer {
                    owner: owner.clone(),
                    expires: now + ttl,
                })
            }
            (Role::Production, Some(_)) => Err(Session::error(
                &self.phase,
                "production Idle prepare ID or nonce",
            )),
            (Role::Production, None) => Ok(Authority::Local),
            (Role::Capture, Some(_)) => Err(Session::error(&self.phase, "capture peer ownership")),
            (Role::Capture, None) => Ok(Authority::Capture(CaptureAuthority::LocalOnly)),
        }
    }

    fn commit_prepare(&mut self, authority: Authority) {
        self.phase = Phase::Preparing(authority);
        self.stop = None;
    }
}

pub(crate) enum PrepareAdmission<'a> {
    Dequeued {
        generation: u64,
        ticket: &'a RequestTicket,
    },
    Inline {
        generation: Option<u64>,
    },
}

impl PrepareAdmission<'_> {
    fn generation(&self) -> Option<u64> {
        match self {
            Self::Dequeued { generation, .. } => Some(*generation),
            Self::Inline { generation } => *generation,
        }
    }
}

pub(crate) enum PrepareDisposition {
    Acquired,
    AlreadyPrepared,
}

#[derive(Debug)]
pub(crate) struct StopAdmission {
    id: String,
    ticket: Arc<RequestTicket>,
}
impl StopAdmission {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn response(&self) -> Option<Response> {
        self.ticket.response()
    }
}
#[derive(Debug)]
struct StopIntent {
    id: String,
    owner: Option<OwnerId>,
    ticket: Arc<RequestTicket>,
    since: Instant,
}
impl StopIntent {
    fn admission(&self) -> StopAdmission {
        StopAdmission {
            id: self.id.clone(),
            ticket: Arc::clone(&self.ticket),
        }
    }
}

#[derive(Debug)]
enum StopRecord {
    Pending(StopIntent),
    Incomplete(String),
    Completed { id: String, owner: Option<OwnerId> },
}
impl StopRecord {
    fn id(&self) -> &str {
        match self {
            Self::Pending(intent) => &intent.id,
            Self::Incomplete(id) | Self::Completed { id, .. } => id,
        }
    }
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
        if !observation {
            if state.pending_stop().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Stop cleanup pending; submit a fresh command after completion",
                ));
            }
            if let Some(fault) = state.phase.observation_fault(Instant::now()) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    fault.to_string(),
                ));
            }
        }
        Ok(state.generation)
    }
    pub(crate) fn output_progress(&self) -> Option<ProgressSnapshot> {
        if let Phase::Running {
            observation: Some(observation),
            ..
        } = &self.lock().phase
        {
            Some(observation.progress)
        } else {
            None
        }
    }
    pub(crate) fn observation_fault(&self) -> Option<Fault> {
        self.lock().phase.observation_fault(Instant::now())
    }
    pub(crate) fn publish_progress(&self, generation: u64, progress: ProgressSnapshot) -> bool {
        let mut state = self.lock();
        if state.revoked(generation) {
            return false;
        }
        let Phase::Running {
            observation: Some(observation),
            ..
        } = &mut state.phase
        else {
            return false;
        };
        observation.progress = progress;
        observation.sampled_at = Instant::now();
        true
    }
    pub(crate) fn latch_observer_fault(&self, generation: u64, fault: Fault) {
        let mut state = self.lock();
        let now = Instant::now();
        if state.generation != generation || state.pending_stop().is_some() || state.expired(now) {
            return;
        }
        if let Phase::Running {
            observation: Some(observation),
            ..
        } = &mut state.phase
        {
            observation.terminal = observation.fault(now).or(Some(fault));
        }
    }
    pub(crate) fn finish_ordinary(
        &self,
        generation: u64,
        ticket: &RequestTicket,
        response: Response,
    ) {
        let state = self.lock();
        let response = if response.0 == 200 && state.revoked(generation) {
            error_response(
                Failure::Unknown,
                "session revoked before command completion",
            )
        } else {
            response
        };
        ticket.finish(response);
    }
    pub(crate) fn prepare_id(&self, owner: Option<&OwnerId>) -> io::Result<Option<String>> {
        let state = self.lock();
        if owner.is_some()
            && (self.role != Role::Production
                || state.phase.authority().is_some_and(|a| !a.matches(owner)))
        {
            return Err(rejected("no matching production owner"));
        }
        Ok(if self.role == Role::Production {
            state.idle_id(Instant::now())
        } else {
            None
        })
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
                "session revoked effect dispatch; outcome may be unknown",
            ));
        }
        // Callers perform one nonblocking socket syscall, never a capacity or
        // acknowledgment wait, while the admission lock is held.
        write()
    }
    pub(crate) fn dispatch_prepare(
        &self,
        generation: u64,
        owner: &RemoteOwner,
        write: impl FnOnce() -> std::io::Result<usize>,
    ) -> std::io::Result<usize> {
        let mut state = self.lock();
        let revoked = || {
            io::Error::new(
                io::ErrorKind::Interrupted,
                "remote Prepare dispatch revoked",
            )
        };
        if state.revoked(generation) {
            return Err(revoked());
        }
        let Phase::Preparing(Authority::Capture(authority)) = &mut state.phase else {
            return Err(revoked());
        };
        if matches!(authority, CaptureAuthority::RemoteAttempt(held) if held != owner) {
            return Err(revoked());
        }
        // The socket is nonblocking. A positive write and its obligation become
        // visible under the same lock as Stop. Err/zero prove no bytes from this
        // write; even an HTTP prefix is conservatively a remote attempt.
        let written = write()?;
        if written > 0 && matches!(authority, CaptureAuthority::LocalOnly) {
            *authority = CaptureAuthority::RemoteAttempt(owner.clone());
        }
        Ok(written)
    }
    fn request_locked(state: &mut ControlState) -> io::Result<StopAdmission> {
        if let Some(intent) = state.pending_stop() {
            return Ok(intent.admission());
        }
        state.advance_generation()?;
        let intent = StopIntent {
            id: format!("{}-{}", state.instance, state.generation),
            owner: state
                .phase
                .authority()
                .and_then(Authority::peer_owner)
                .cloned(),
            ticket: Arc::new(RequestTicket::accepted()),
            since: Instant::now(),
        };
        let admission = intent.admission();
        state.stop = Some(StopRecord::Pending(intent));
        Ok(admission)
    }
    pub(crate) fn request_stop(&self, owner: Option<&OwnerId>) -> std::io::Result<StopAdmission> {
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
                match &state.stop {
                    Some(StopRecord::Pending(intent)) if intent.owner.as_ref() == Some(owner) => {
                        // The effect owner has released the phase and is about
                        // to publish the receipt for this already-admitted Stop.
                        return Ok(intent.admission());
                    }
                    Some(StopRecord::Completed {
                        id,
                        owner: Some(released),
                    }) if released == owner => {
                        let ticket = Arc::new(RequestTicket::accepted());
                        ticket.finish((
                            200,
                            serde_json::json!({"state":"idle","stop":"completed","outcome":"completed","owner":owner.as_str(),"stop_id":id})
                                .to_string(),
                        ));
                        return Ok(StopAdmission {
                            id: id.clone(),
                            ticket,
                        });
                    }
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "no completed Stop for this owner; in-transit effects remain unknown",
                        ))
                    }
                }
            }
        }
        Self::request_locked(&mut state)
    }
    pub(crate) fn stop_id(&self) -> Option<String> {
        self.lock().stop.as_ref().map(|stop| stop.id().to_owned())
    }
    pub(crate) fn completed_owner(&self) -> Option<OwnerId> {
        match &self.lock().stop {
            Some(StopRecord::Completed { owner, .. }) => owner.clone(),
            _ => None,
        }
    }
    pub(crate) fn stop_completed(&self) -> bool {
        matches!(self.lock().stop, Some(StopRecord::Completed { .. }))
    }
    pub(crate) fn status_during_cleanup(
        &self,
        owner: Option<&OwnerId>,
    ) -> io::Result<Option<String>> {
        self.cleanup_status(&self.lock(), owner)
    }

    pub(crate) fn try_status_during_cleanup(
        &self,
        owner: Option<&OwnerId>,
    ) -> std::io::Result<Option<String>> {
        // A queued Status may already have a completed response. Probing
        // cleanup metadata must never wait for the control lock and delay it.
        let state = match self.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return Ok(None),
        };
        self.cleanup_status(&state, owner)
    }

    fn cleanup_status(
        &self,
        state: &ControlState,
        owner: Option<&OwnerId>,
    ) -> io::Result<Option<String>> {
        let Some(intent) = state.pending_stop() else {
            let Phase::Failed {
                authority,
                compensating: Some(fault),
            } = &state.phase
            else {
                return Ok(None);
            };
            if owner.is_some() && !authority.matches(owner) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "no matching production owner",
                ));
            }
            return Ok(Some(
                serde_json::json!({
                    "version": 1, "engine": "rust-candidate", "role": "production",
                    "state": "failed", "start_id": null, "output": null, "output_progress": null,
                    "error": fault.to_string(), "children": null, "resource_observation": "unknown",
                    "remote_release_pending": false,
                })
                .to_string(),
            ));
        };
        if owner.is_some() && (self.role != Role::Production || intent.owner.as_ref() != owner) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "no matching production owner",
            ));
        }
        let peer = intent.owner.as_ref().map(OwnerId::as_str);
        let remote = matches!(
            state.phase.authority(),
            Some(Authority::Capture(CaptureAuthority::RemoteAttempt(_)))
        );
        let overdue = intent.since.elapsed() >= STOP_OBSERVATION_BUDGET;
        Ok(Some(serde_json::json!({"version":1,"engine":"rust-candidate","role":match self.role {Role::Capture=>"capture",Role::Production=>"production"},"state":"stopping","stop":"requested","outcome":if overdue {"unknown"} else {"accepted"},"cleanup":if overdue {"unknown"} else {"pending"},"resource_observation":"unknown","children":null,"stop_id":intent.id,"output":state.phase.output().map(Output::as_str),"remote_release_pending":remote,"owner":peer}).to_string()))
    }
    pub(crate) fn heartbeat(
        &self,
        owner: Option<&OwnerId>,
        ttl: Duration,
    ) -> std::io::Result<String> {
        let mut state = self.lock();
        if self.role != Role::Production
            || state.pending_stop().is_some()
            || state.phase.observation_fault(Instant::now()).is_some()
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
        if !matches!(state.phase, Phase::Stopping { .. }) && state.expired(Instant::now()) {
            // Generation exhaustion is fail-closed: Operation also observes the
            // expired authority, and the effect owner still checks lease expiry.
            let _ = Self::request_locked(&mut state);
        }
    }
    pub(crate) fn stop_pending(&self) -> bool {
        self.lock().pending_stop().is_some()
    }
    pub(crate) fn finish_stop(&self, mut response: Response) {
        let mut state = self.lock();
        if let Some(intent) = state.pending_stop() {
            let mut body: serde_json::Value =
                serde_json::from_str(&response.1).expect("Stop response is JSON");
            body["stop_id"] = serde_json::json!(intent.id);
            let record = if response.0 == 200 && matches!(state.phase, Phase::Idle) {
                body["owner"] = serde_json::json!(intent.owner.as_ref().map(OwnerId::as_str));
                StopRecord::Completed {
                    id: intent.id.clone(),
                    owner: intent.owner.clone(),
                }
            } else {
                StopRecord::Incomplete(intent.id.clone())
            };
            response.1 = body.to_string();
            intent.ticket.finish(response);
            state.stop = Some(record);
        }
    }
}
impl Session {
    pub fn new(role: Role) -> Self {
        Self::for_agent(role, String::new())
    }

    pub(crate) fn for_agent(role: Role, instance: String) -> Self {
        Self {
            role,
            control: SessionControl {
                role,
                state: Arc::new(Mutex::new(ControlState {
                    phase: Phase::Idle,
                    generation: 0,
                    output_until: None,
                    instance,
                    stop: None,
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
        !matches!(state.phase, Phase::Stopping { .. }) && state.expired(now)
    }

    pub(crate) fn arm_output_limit(
        &mut self,
        limit: Option<Duration>,
    ) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        let now = Instant::now();
        if self.role != Role::Production
            || state.phase.state() != Lifecycle::Ready
            || state.pending_stop().is_some()
            || state.expired(now)
        {
            return Err(Self::error(&state.phase, "arm output"));
        }
        state.output_until = limit.map(|duration| now + duration);
        Ok(())
    }

    pub(crate) fn start_id(&self) -> Option<String> {
        self.control.lock().start_id(Instant::now())
    }

    pub(crate) fn output_seconds_remaining(&self) -> Option<u64> {
        self.control
            .lock()
            .output_until
            .map(|end| end.saturating_duration_since(Instant::now()).as_secs())
    }
    pub fn prepare(
        &mut self,
        owner: Option<OwnerId>,
        now: Instant,
        ttl: Duration,
    ) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        let authority = state.prepare_authority(self.role, owner.as_ref(), now, ttl)?;
        state.commit_prepare(authority);
        Ok(())
    }

    pub(crate) fn begin_prepare(
        &mut self,
        owner: Option<&OwnerId>,
        ttl: Duration,
        admission: PrepareAdmission<'_>,
    ) -> io::Result<PrepareDisposition> {
        let mut state = self.control.lock();
        let now = Instant::now();
        if state.pending_stop().is_some()
            || state.expired(now)
            || state.phase.observation_fault(now).is_some()
            || admission
                .generation()
                .is_some_and(|generation| generation != state.generation)
        {
            return Err(rejected("Prepare superseded by Stop or expiry"));
        }
        let authority = match &state.phase {
            Phase::Idle => Some(state.prepare_authority(self.role, owner, now, ttl).map_err(|_| {
                rejected("owned Prepare requires the current production Idle prepare ID and nonce")
            })?),
            Phase::Ready(authority) | Phase::Running { authority, .. }
                if authority.matches(owner) => None,
            _ => return Err(rejected("stop before preparing another session")),
        };
        if let PrepareAdmission::Dequeued { ticket, .. } = admission {
            // Queue expiry, ownership and Stop are ordered at dequeue. After
            // this ticket begins, interruption requires normal compensation.
            if !ticket.begin() {
                return Err(rejected("Prepare expired or was cancelled before dequeue"));
            }
        }
        if let Some(authority) = authority {
            state.commit_prepare(authority);
            Ok(PrepareDisposition::Acquired)
        } else {
            Ok(PrepareDisposition::AlreadyPrepared)
        }
    }

    pub fn prepared(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if state.pending_stop().is_some()
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
    pub(crate) fn remote_owner(&self) -> Option<RemoteOwner> {
        match self.control.lock().phase.authority() {
            Some(Authority::Capture(CaptureAuthority::RemoteAttempt(owner))) => Some(owner.clone()),
            _ => None,
        }
    }
    pub(crate) fn remote_released(&mut self, proof: StopProof) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if !matches!(
            state.phase.state(),
            Lifecycle::Preparing | Lifecycle::Failed | Lifecycle::Stopping
        ) {
            return Err(Self::error(&state.phase, "remote release"));
        }
        match state.phase.authority_mut() {
            Some(Authority::Capture(authority)) if matches!(&*authority, CaptureAuthority::RemoteAttempt(held) if proof.proves(held)) =>
            {
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
        ) || state.pending_stop().is_some()
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
        self.start_running(output, None).map(|_| ())
    }
    pub(crate) fn start_observed(
        &mut self,
        output: Output,
        progress: ProgressSnapshot,
    ) -> Result<u64, TransitionError> {
        self.start_running(output, Some(progress))
    }
    fn start_running(
        &mut self,
        output: Output,
        progress: Option<ProgressSnapshot>,
    ) -> Result<u64, TransitionError> {
        let mut state = self.control.lock();
        let now = Instant::now();
        if state.pending_stop().is_some()
            || state.expired(now)
            || progress.is_some() && self.role != Role::Production
        {
            return Err(Self::error(&state.phase, "cancelled start"));
        }
        match &state.phase {
            Phase::Ready(authority) => {
                state.phase = Phase::Running {
                    authority: authority.clone(),
                    output,
                    observation: progress.map(|progress| RunningObservation {
                        progress,
                        sampled_at: now,
                        terminal: None,
                    }),
                };
                Ok(state.generation)
            }
            Phase::Running { output: active, .. } if *active == output && progress.is_none() => {
                Ok(state.generation)
            }
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
        if state.pending_stop().is_some()
            || !matches!(
                state.phase.state(),
                Lifecycle::Preparing | Lifecycle::Ready | Lifecycle::Running | Lifecycle::Degraded
            )
        {
            return Err(Self::error(&state.phase, "fail"));
        }
        let compensating = state.phase.observation_fault(Instant::now());
        if compensating.is_some() {
            state
                .advance_generation()
                .map_err(|_| Self::error(&state.phase, "control generation exhausted"))?;
        }
        state.phase = Phase::Failed {
            authority: state.phase.authority().unwrap().clone(),
            compensating,
        };
        Ok(())
    }
    pub(crate) fn failure_reconciled(&mut self) {
        if let Phase::Failed { compensating, .. } = &mut self.control.lock().phase {
            *compensating = None;
        }
    }
    pub fn stop(&mut self) -> Result<(), TransitionError> {
        let mut state = self.control.lock();
        if matches!(state.phase.state(), Lifecycle::Idle | Lifecycle::Stopping) {
            return Ok(());
        }
        // A listener-admitted Stop has already advanced the generation.
        if state.pending_stop().is_none() {
            state
                .advance_generation()
                .map_err(|_| Self::error(&state.phase, "control generation exhausted"))?;
            state.stop = None;
        }
        state.phase = Phase::Stopping {
            authority: state.phase.authority().unwrap().clone(),
            output: state.phase.output(),
        };
        state.output_until = None;
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
        state.phase = Phase::Idle;
        Ok(())
    }
}

#[cfg(test)]
impl Session {
    pub(crate) fn fixture_owner(&self, nonce: u128) -> OwnerId {
        let id = self.control.prepare_id(None).unwrap().unwrap();
        OwnerId::parse(&format!("{id}-{nonce:032x}")).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_fault_revokes_commands_but_stop_wins_and_fences_old_samples() {
        let mut session = Session::for_agent(Role::Production, "fixture".into());
        let control = session.control();
        let prepare = |session: &mut Session, bytes| {
            session
                .prepare(None, Instant::now(), Duration::from_secs(30))
                .unwrap();
            session.prepared().unwrap();
            session
                .start_observed(Output::Recording, ProgressSnapshot::test_recording(bytes))
                .unwrap()
        };
        let generation = prepare(&mut session, 1);
        assert!(control.publish_progress(generation, ProgressSnapshot::test_recording(2)));
        control.latch_observer_fault(generation, Fault::OutputStalled);
        control.latch_observer_fault(generation, Fault::ObserverExited);
        assert_eq!(control.observation_fault(), Some(Fault::OutputStalled));
        assert!(control.admit(false).is_err());
        assert!(control.admit(true).is_ok());
        assert!(control
            .dispatch(generation, || -> io::Result<()> {
                panic!("revoked dispatch")
            })
            .is_err());
        let work = RequestTicket::accepted();
        control.finish_ordinary(generation, &work, (200, "{}".into()));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&work.response().unwrap().1).unwrap()
                ["outcome"],
            "unknown"
        );

        let admitted = control.request_stop(None).unwrap();
        assert!(session.fail().is_err(), "pending Stop owns compensation");
        session.stop().unwrap();
        session.stopped().unwrap();
        control.finish_stop((200, "{}".into()));
        assert_eq!(admitted.response().unwrap().0, 200);
        assert!(control.output_progress().is_none());

        let next = prepare(&mut session, 3);
        assert_ne!(next, generation);
        assert!(!control.publish_progress(generation, ProgressSnapshot::test_recording(9)));
        control.latch_observer_fault(generation, Fault::ObserverExited);
        assert_eq!(control.observation_fault(), None);
        assert_eq!(control.output_progress().unwrap().status()["bytes"], 3);
    }

    #[test]
    fn overdue_observation_cannot_be_made_healthy_by_a_late_sample() {
        let mut session = Session::new(Role::Production);
        session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        session.prepared().unwrap();
        let generation = session
            .start_observed(Output::Recording, ProgressSnapshot::test_recording(1))
            .unwrap();
        let control = session.control();
        if let Phase::Running {
            observation: Some(observation),
            ..
        } = &mut control.lock().phase
        {
            observation.sampled_at -= MAX_SAMPLE_AGE;
        }
        assert!(control.cancelled(generation));
        assert!(!control.publish_progress(generation, ProgressSnapshot::test_recording(2)));
        assert_eq!(control.observation_fault(), Some(Fault::SampleOverdue));
    }

    #[test]
    fn output_watchdog_revokes_start_and_requests_one_correlated_stop() {
        let mut session = Session::for_agent(Role::Production, "fixture".into());
        session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        session.prepared().unwrap();
        let old_id = session.start_id().unwrap();
        let control = session.control();
        let generation = control.generation();
        session.arm_output_limit(Some(Duration::ZERO)).unwrap();
        assert!(session.start(Output::TwitchLive).is_err());
        assert!(control.cancelled(generation));
        control.expire();
        let admitted = control.request_stop(None).unwrap();
        assert_eq!(admitted.id(), "fixture-1");
        session.stop().unwrap();
        session.stopped().unwrap();
        control.finish_stop((200, r#"{"state":"idle","stop":"completed"}"#.into()));
        let reply: serde_json::Value =
            serde_json::from_str(&admitted.response().unwrap().1).unwrap();
        assert_eq!(reply["stop_id"], admitted.id());
        assert_ne!(session.start_id().unwrap(), old_id);
    }
    #[test]
    fn overdue_stop_observation_is_unknown_without_erasing_owned_state() {
        let mut session = ready(None, Instant::now());
        session.start(Output::Recording).unwrap();
        let control = session.control();
        control.request_stop(None).unwrap();
        let mut state = control.lock();
        let Some(StopRecord::Pending(intent)) = state.stop.as_mut() else {
            panic!("expected pending Stop");
        };
        intent.since = Instant::now() - STOP_OBSERVATION_BUDGET;
        drop(state);
        let snapshot: serde_json::Value =
            serde_json::from_str(&control.try_status_during_cleanup(None).unwrap().unwrap())
                .unwrap();
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
    fn queued_peer_prepare_cannot_begin_after_emergency_stop() {
        let mut session = Session::new(Role::Production);
        let control = session.control();
        let owner = session.fixture_owner(1);
        let admitted = control.admit(false).unwrap();
        let queued = RequestTicket::new(Instant::now() + Duration::from_secs(30));
        let admission = || PrepareAdmission::Dequeued {
            generation: admitted,
            ticket: &queued,
        };

        // Admission has only queued work. No matching owner can yet claim
        // completion, but the local emergency Stop can revoke the work.
        assert!(control.request_stop(Some(&owner)).is_err());
        let stop = control.request_stop(None).unwrap();
        assert!(session
            .begin_prepare(Some(&owner), Duration::from_secs(30), admission())
            .is_err());
        assert!(control.prepare_id(None).unwrap().is_none());
        control.finish_stop((200, r#"{"state":"idle","stop":"completed"}"#.into()));
        assert_eq!(stop.response().unwrap().0, 200);
        assert!(session
            .begin_prepare(Some(&owner), Duration::from_secs(30), admission())
            .is_err());
        assert!(control.request_stop(Some(&owner)).is_err());
        assert_eq!(session.state(), Lifecycle::Idle);
        assert_ne!(
            control.prepare_id(None).unwrap().as_deref(),
            Some(owner.as_str().rsplit_once('-').unwrap().0)
        );
        assert!(control.admit(false).is_ok());
    }
    #[test]
    fn admission_and_prepare_begin_are_ordered_with_stop() {
        let mut session = Session::new(Role::Production);
        let control = session.control();
        let owner = session.fixture_owner(1);
        let intruder = session.fixture_owner(2);
        let generation = control.admit(false).unwrap();
        let ticket = RequestTicket::new(Instant::now() + Duration::from_secs(30));
        assert!(matches!(
            session.begin_prepare(
                Some(&owner),
                Duration::from_secs(30),
                PrepareAdmission::Dequeued {
                    generation,
                    ticket: &ticket
                }
            ),
            Ok(PrepareDisposition::Acquired)
        ));
        assert_eq!(session.state(), Lifecycle::Preparing);
        assert!(control.prepare_id(None).unwrap().is_none());
        assert_eq!(control.generation(), generation);
        assert!(control.request_stop(Some(&intruder)).is_err());
        let stop = control.request_stop(Some(&owner)).unwrap();
        assert!(
            control.admit(false).is_err(),
            "activation was parked during cleanup"
        );
        assert!(session.prepared().is_err());
        session.stop().unwrap();
        session.stopped().unwrap();
        let pending: serde_json::Value = serde_json::from_str(
            &control
                .try_status_during_cleanup(Some(&owner))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(pending["owner"], owner.as_str());
        assert_eq!(pending["stop_id"], stop.id());
        assert!(control
            .request_stop(Some(&owner))
            .unwrap()
            .response()
            .is_none());
        assert!(control.request_stop(Some(&intruder)).is_err());
        control.finish_stop((200, "{}".into()));
        assert_eq!(stop.response().unwrap().0, 200);
        let proof: serde_json::Value = serde_json::from_str(&stop.response().unwrap().1).unwrap();
        assert_eq!(proof["owner"], owner.as_str());
        assert_eq!(proof["stop_id"], stop.id());
        assert!(control.cancelled(generation));
        assert_eq!(session.state(), Lifecycle::Idle);
    }
    #[test]
    fn stop_authorization_precedes_revocation_and_final_commit() {
        let (mut session, owner) = ready_peer(1, Instant::now());
        let intruder = OwnerId::parse("intruder").unwrap();
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
            control.cancelled(generation),
            "old queued activation escaped Stop"
        );
        assert!(control.request_stop(Some(&intruder)).is_err());
        let completed = control.request_stop(Some(&owner)).unwrap();
        let proof: serde_json::Value =
            serde_json::from_str(&completed.response().unwrap().1).unwrap();
        assert_eq!(proof["owner"], owner.as_str());
        let newer = session.fixture_owner(2);
        session
            .prepare(Some(newer), Instant::now(), Duration::from_secs(30))
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
        let mut session = Session::new(Role::Production);
        let owner = session.fixture_owner(1);
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
        let owner = RemoteOwner::fixture(1);
        let other = RemoteOwner::fixture(2);
        let proof = |peer: &RemoteOwner| {
            peer.observe_stop(&serde_json::json!({
                "state":"idle","stop":"completed","outcome":"completed",
                "owner":peer.owner().as_str(),"stop_id":format!("{}-1", RemoteOwner::FIXTURE_INSTANCE)
            }), None).unwrap().unwrap()
        };
        let mut session = Session::new(Role::Capture);
        session.prepare(None, now, Duration::from_secs(30)).unwrap();
        assert!(session.prepared().is_err());
        assert!(session.remote_owner().is_none());
        let control = session.control();
        control
            .dispatch_prepare(control.generation(), &owner, || Ok(1))
            .unwrap();
        session.prepared().unwrap();
        session.start(Output::Recording).unwrap();
        assert!(session.remote_released(proof(&owner)).is_err());
        session.fail().unwrap();
        session.stop().unwrap();
        assert_eq!(session.remote_owner(), Some(owner.clone()));
        assert!(session.stopped().is_err());
        assert!(session.remote_released(proof(&other)).is_err());
        session.remote_released(proof(&owner)).unwrap();
        session.stopped().unwrap();
        assert!(session.remote_owner().is_none());
        let production = ready(None, now).control();
        assert!(production
            .dispatch_prepare(production.generation(), &owner, || unreachable!())
            .is_err());
    }
    #[test]
    fn lease_expiry_does_not_retrigger_teardown_while_stopping() {
        let now = Instant::now();
        let (mut session, _) = ready_peer(1, now);
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
    fn ready_peer(nonce: u128, now: Instant) -> (Session, OwnerId) {
        let mut session = Session::new(Role::Production);
        let owner = session.fixture_owner(nonce);
        session
            .prepare(Some(owner.clone()), now, Duration::from_secs(30))
            .unwrap();
        session.prepared().unwrap();
        (session, owner)
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
        let (mut peer, owner) = ready_peer(1, now);
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
        assert!(peer
            .prepare(Some(owner), now, Duration::from_secs(30))
            .is_err());
    }
}
