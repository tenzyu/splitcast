use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

pub(crate) type Response = (u16, String);
enum Phase {
    Queued,
    Executing { abandoned: bool },
    Cancelled,
    Finished(Response),
}
pub(crate) struct RequestTicket {
    phase: Mutex<Phase>,
    expires: Instant,
}
impl RequestTicket {
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
                error_response("not_executed", "request cancelled before execution")
            }
            Phase::Executing { abandoned } => {
                *abandoned = true;
                error_response(
                    "unknown",
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
pub(crate) fn error_response(outcome: &str, message: &str) -> Response {
    let code = if outcome == "unknown" {
        "outcome_unknown"
    } else {
        "not_executed"
    };
    (
        503,
        serde_json::json!({"error":message,"code":code,"outcome":outcome}).to_string(),
    )
}
#[derive(Clone)]
pub(crate) struct Operation {
    shutdown: Arc<AtomicBool>,
    ticket: Option<Arc<RequestTicket>>,
    deadline: Instant,
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
        }
    }
    pub(crate) fn check(&self) -> io::Result<()> {
        if self.shutdown.load(Ordering::Relaxed)
            || self.ticket.as_ref().is_some_and(|t| t.cancelled())
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
    pub(crate) fn deadline(&self, cap: Duration) -> io::Result<Instant> {
        self.check()?;
        Ok(self.deadline.min(Instant::now() + cap))
    }
}
