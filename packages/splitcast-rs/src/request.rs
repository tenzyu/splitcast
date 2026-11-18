use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

pub(crate) type Response = (u16, String);
#[derive(Debug)]
enum Phase {
    Queued,
    Executing { abandoned: bool },
    Cancelled,
    Finished(Response),
}
#[derive(Debug)]
pub(crate) struct RequestTicket {
    phase: Mutex<Phase>,
    expires: Instant,
}
impl RequestTicket {
    pub(crate) fn accepted() -> Self {
        Self {
            phase: Mutex::new(Phase::Executing { abandoned: false }),
            expires: Instant::now(),
        }
    }
    pub(crate) fn new(expires: Instant) -> Self {
        Self {
            phase: Mutex::new(Phase::Queued),
            expires,
        }
    }
    pub(crate) fn begin(&self) -> bool {
        let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*phase, Phase::Queued) {
            if Instant::now() < self.expires {
                *phase = Phase::Executing { abandoned: false };
                return true;
            }
            *phase = Phase::Cancelled;
        }
        false
    }
    pub(crate) fn cancel_queued(&self) -> bool {
        let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*phase, Phase::Queued | Phase::Cancelled) {
            *phase = Phase::Cancelled;
            true
        } else {
            false
        }
    }
    pub(crate) fn abandon(&self) -> Response {
        let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *phase {
            Phase::Queued | Phase::Cancelled => {
                *phase = Phase::Cancelled;
                error_response(Failure::NotExecuted, "request cancelled before execution")
            }
            Phase::Executing { abandoned } => {
                *abandoned = true;
                error_response(
                    Failure::Unknown,
                    "execution started; inspect status and use explicit Stop",
                )
            }
            Phase::Finished(response) => response.clone(),
        }
    }
    pub(crate) fn finish(&self, response: Response) {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Finished(response);
    }
    pub(crate) fn response(&self) -> Option<Response> {
        let phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        match &*phase {
            Phase::Finished(response) => Some(response.clone()),
            _ => None,
        }
    }
    fn cancelled(&self) -> bool {
        let phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        matches!(
            *phase,
            Phase::Cancelled | Phase::Executing { abandoned: true }
        )
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Failure {
    NotExecuted,
    Unknown,
}

impl Failure {
    pub(crate) fn outcome(self) -> &'static str {
        match self {
            Self::NotExecuted => "not_executed",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn error_response(failure: Failure, message: &str) -> Response {
    let code = match failure {
        Failure::NotExecuted => "not_executed",
        Failure::Unknown => "outcome_unknown",
    };
    (
        503,
        serde_json::json!({"error":message,"code":code,"outcome":failure.outcome()}).to_string(),
    )
}

#[derive(Debug)]
pub(crate) struct Rejected(pub(crate) &'static str);

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Rejected {}

pub(crate) fn rejected(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, Rejected(message))
}

pub(crate) fn command_failure(error: io::Error) -> Response {
    let rejected = error.get_ref().is_some_and(|cause| {
        cause.is::<Rejected>()
            || matches!(
                cause.downcast_ref::<crate::control::RemoteFailure>(),
                Some(crate::control::RemoteFailure::NotExecuted)
            )
    });
    let (failure, code) = if rejected {
        (Failure::NotExecuted, "command_rejected")
    } else {
        (Failure::Unknown, "command_failed")
    };
    (
        409,
        serde_json::json!({"error":error.to_string(),"code":code,"outcome":failure.outcome()})
            .to_string(),
    )
}

#[derive(Clone)]
pub(crate) struct Operation {
    shutdown: Arc<AtomicBool>,
    ticket: Option<Arc<RequestTicket>>,
    deadline: Instant,
    control: Option<(crate::lifecycle::SessionControl, u64)>,
    remote_prepare: Option<crate::protocol::OwnerId>,
}
impl Operation {
    pub(crate) fn new(
        shutdown: Arc<AtomicBool>,
        ticket: Option<Arc<RequestTicket>>,
        budget: Duration,
    ) -> Self {
        Self {
            shutdown,
            ticket,
            deadline: Instant::now() + budget,
            control: None,
            remote_prepare: None,
        }
    }
    pub(crate) fn until(shutdown: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            shutdown,
            ticket: None,
            deadline,
            control: None,
            remote_prepare: None,
        }
    }
    pub(crate) fn with_control(
        mut self,
        control: crate::lifecycle::SessionControl,
        generation: u64,
    ) -> Self {
        self.control = Some((control, generation));
        self
    }
    pub(crate) fn check(&self) -> io::Result<()> {
        if self.shutdown.load(Ordering::Relaxed)
            || self.ticket.as_ref().is_some_and(|t| t.cancelled())
            || self
                .control
                .as_ref()
                .is_some_and(|(control, generation)| control.cancelled(*generation))
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "operation cancelled; outcome may be unknown",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "operation deadline exceeded; outcome may be unknown",
            ));
        }
        Ok(())
    }
    pub(crate) fn with_remote_prepare(
        mut self,
        control: crate::lifecycle::SessionControl,
        owner: crate::protocol::OwnerId,
    ) -> Self {
        // Never refresh the generation of an already executing operation.
        let generation = self
            .control
            .as_ref()
            .map_or_else(|| control.generation(), |(_, generation)| *generation);
        self.control = Some((control, generation));
        self.remote_prepare = Some(owner);
        self
    }
    pub(crate) fn capped(mut self, cap: Duration) -> Self {
        self.deadline = self.deadline.min(Instant::now() + cap);
        self
    }
    pub(crate) fn deadline(&self, cap: Duration) -> io::Result<Instant> {
        self.check()?;
        Ok(self.deadline.min(Instant::now() + cap))
    }
    pub(crate) fn read(
        &self,
        stream: &mut TcpStream,
        bytes: &mut [u8],
        deadline: Instant,
    ) -> io::Result<usize> {
        stream.set_nonblocking(true)?;
        let result = self.poll_io(deadline, || stream.read(bytes));
        let restored = stream.set_nonblocking(false);
        result.and_then(|n| restored.map(|()| n))
    }
    pub(crate) fn write(
        &self,
        stream: &mut TcpStream,
        bytes: &[u8],
        deadline: Instant,
    ) -> io::Result<usize> {
        stream.set_nonblocking(true)?;
        let result = self.poll_io(deadline, || match &self.control {
            Some((control, generation)) => match &self.remote_prepare {
                Some(owner) => control.dispatch_prepare(*generation, owner, || stream.write(bytes)),
                None => control.dispatch(*generation, || stream.write(bytes)),
            },
            None => stream.write(bytes),
        });
        let restored = stream.set_nonblocking(false);
        result.and_then(|n| restored.map(|()| n))
    }
    fn poll_io<T>(
        &self,
        deadline: Instant,
        mut io: impl FnMut() -> io::Result<T>,
    ) -> io::Result<T> {
        loop {
            self.check()?;
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "operation I/O deadline exceeded; outcome may be unknown",
                ));
            }
            match io() {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                result => return result,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_prepare_obligation_tracks_socket_bytes_not_bookkeeping() {
        for sent_prefix in [false, true] {
            let mut session = crate::Session::new(crate::Role::Capture);
            session
                .prepare(None, Instant::now(), Duration::from_secs(30))
                .unwrap();
            let owner = crate::protocol::OwnerId::parse("wire-owner").unwrap();
            let control = session.control();
            let operation = Operation::new(
                Arc::new(AtomicBool::new(false)),
                None,
                Duration::from_secs(2),
            )
            .with_control(control.clone(), control.generation())
            .with_remote_prepare(control.clone(), owner.clone());
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (mut peer, _) = listener.accept().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let deadline = Instant::now() + Duration::from_secs(1);
            if sent_prefix {
                assert_eq!(operation.write(&mut stream, b"POST", deadline).unwrap(), 4);
                let mut prefix = [0; 4];
                peer.read_exact(&mut prefix).unwrap();
                assert_eq!(&prefix, b"POST");
                assert_eq!(session.remote_owner(), Some(owner.clone()));
            }
            control.request_stop(None).unwrap();
            assert_eq!(
                operation
                    .write(&mut stream, b" /api/command", deadline)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Interrupted
            );
            peer.set_nonblocking(true).unwrap();
            assert_eq!(
                peer.read(&mut [0; 1]).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            assert_eq!(session.remote_owner(), sent_prefix.then_some(owner));
            session.stop().unwrap();
            if sent_prefix {
                assert!(
                    session.stopped().is_err(),
                    "transmitted prefix lost its unresolved obligation"
                );
                assert_eq!(session.state(), crate::Lifecycle::Stopping);
            } else {
                session.stopped().unwrap();
                assert_eq!(session.state(), crate::Lifecycle::Idle);
            }
        }
    }
    #[test]
    fn stop_orders_actual_socket_writes_and_rejects_late_commit() {
        let mut session = crate::Session::new(crate::Role::Production);
        session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        session.prepared().unwrap();
        let control = session.control();
        let operation = Operation::new(
            Arc::new(AtomicBool::new(false)),
            None,
            Duration::from_secs(2),
        )
        .with_control(control.clone(), control.generation());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let end = Instant::now() + Duration::from_secs(1);
        assert_eq!(operation.write(&mut stream, b"before", end).unwrap(), 6);
        let mut bytes = [0; 6];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"before");
        control.request_stop(None).unwrap();
        assert_eq!(
            operation
                .write(&mut stream, b"after", end)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(session.start(crate::Output::Recording).is_err());
        assert_eq!(session.state(), crate::Lifecycle::Ready);
        peer.set_nonblocking(true).unwrap();
        assert_eq!(
            peer.read(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}
