use crate::{
    config::Config,
    lifecycle::SessionControl,
    media::{self, ReceiveMonitor, ReceivePathError},
    obs::{self, OutputProgress, OutputStatus, ProgressSnapshot},
    request::Operation,
    Output,
};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::{fmt, io};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const SAMPLE_BUDGET: Duration = media::RECEIVE_TIMEOUT.saturating_add(obs::TIMEOUT);
pub(crate) const MAX_SAMPLE_AGE: Duration = SAMPLE_INTERVAL.saturating_add(SAMPLE_BUDGET);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Fault {
    IngressUnavailable,
    InvalidIngress(ReceivePathError),
    IngressStalled,
    OutputUnavailable,
    OutputStalled,
    ObserverExited,
    SampleOverdue,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::IngressUnavailable => "incoming media observation unavailable",
            Self::InvalidIngress(error) => return write!(f, "unusable incoming media: {error}"),
            Self::IngressStalled => "incoming media stopped progressing",
            Self::OutputUnavailable => "OBS output observation unavailable",
            Self::OutputStalled => "OBS output stopped progressing",
            Self::ObserverExited => "production media observer exited unexpectedly",
            Self::SampleOverdue => "production media observation overdue",
        })
    }
}

/// Takes the two accepted Start baselines and only the inputs needed to read.
pub(crate) struct RunningMedia {
    receiver_port: u16,
    require_camera: bool,
    obs_port: u16,
    obs_password: PathBuf,
    output: Output,
    ingress: ReceiveMonitor,
    progress: OutputProgress,
}

impl RunningMedia {
    pub(crate) fn new(
        cfg: &Config,
        output: Output,
        ingress: ReceiveMonitor,
        output_status: OutputStatus,
    ) -> io::Result<Self> {
        let progress = OutputProgress::start(output_status).ok_or_else(|| {
            io::Error::other("OBS output counters were not ready before confirming Start")
        })?;
        Ok(Self {
            receiver_port: cfg.api,
            require_camera: cfg.camera.is_some(),
            obs_port: cfg.obs_port,
            obs_password: cfg.obs_password.clone(),
            output,
            ingress,
            progress,
        })
    }

    pub(crate) fn baseline(&self) -> ProgressSnapshot {
        self.progress.snapshot()
    }

    pub(crate) fn supervise(
        mut self,
        control: SessionControl,
        generation: u64,
    ) -> io::Result<Observer> {
        Observer::launch(control, generation, move |operation| self.sample(operation))
    }

    fn sample(&mut self, operation: &Operation) -> Result<ProgressSnapshot, Fault> {
        let snapshot =
            media::receive_snapshot(self.receiver_port, self.require_camera, Some(operation))
                .map_err(|_| Fault::IngressUnavailable)?;
        if !self
            .ingress
            .observe(&snapshot, Instant::now())
            .map_err(Fault::InvalidIngress)?
        {
            return Err(Fault::IngressStalled);
        }

        // read_output_status checks cancellation before connecting and during OBS I/O.
        let output = obs::read_output_status(
            self.obs_port,
            &self.obs_password,
            self.output,
            Some(operation),
        )
        .map_err(|_| Fault::OutputUnavailable)?;
        self.progress.observe(output).ok_or(Fault::OutputStalled)
    }
}

pub(crate) struct Observer {
    generation: u64,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Observer {
    fn launch(
        control: SessionControl,
        generation: u64,
        mut sample: impl FnMut(&Operation) -> Result<ProgressSnapshot, Fault> + Send + 'static,
    ) -> io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&cancel);
        let thread = thread::Builder::new()
            .name("splitcast-watch".into())
            .spawn(move || {
                let exit = ObserveExit {
                    control,
                    generation,
                };
                let mut next = Instant::now() + SAMPLE_INTERVAL;
                while !running.load(Ordering::Relaxed) {
                    if let Some(delay) = next.checked_duration_since(Instant::now()) {
                        thread::park_timeout(delay);
                        continue;
                    }
                    let operation = Operation::new(Arc::clone(&running), None, SAMPLE_BUDGET)
                        .with_control(exit.control.clone(), generation);
                    if operation.check().is_err() {
                        break;
                    }
                    match sample(&operation) {
                        Ok(progress) => {
                            if !exit.control.publish_progress(generation, progress) {
                                break;
                            }
                        }
                        Err(fault) => {
                            exit.control.latch_observer_fault(generation, fault);
                            break;
                        }
                    }
                    next = Instant::now() + SAMPLE_INTERVAL;
                }
            })?;
        Ok(Self {
            generation,
            cancel,
            thread: Some(thread),
        })
    }

    pub(crate) fn exited_generation(&self) -> Option<u64> {
        self.thread
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
            .then_some(self.generation)
    }

    pub(crate) fn shutdown(&mut self) -> io::Result<()> {
        if let Some(thread) = self.thread.take() {
            self.cancel.store(true, Ordering::Relaxed);
            thread.thread().unpark();
            thread
                .join()
                .map_err(|_| io::Error::other("production media observer panicked"))?;
        }
        Ok(())
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

// On unwind or unexpected return, the generation and phase check distinguishes
// an active observer's loss from a normal Stop, expiry, or failure teardown.
struct ObserveExit {
    control: SessionControl,
    generation: u64,
}

impl Drop for ObserveExit {
    fn drop(&mut self) {
        self.control
            .latch_observer_fault(self.generation, Fault::ObserverExited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Role, Session};

    #[test]
    fn unexpected_observer_exit_latches_and_is_joined() {
        let mut session = Session::new(Role::Production);
        session
            .prepare(None, Instant::now(), Duration::ZERO)
            .unwrap();
        session.prepared().unwrap();
        let generation = session
            .start_observed(Output::Recording, ProgressSnapshot::test_recording(1))
            .unwrap();
        let control = session.control();
        let mut observer = Observer::launch(control.clone(), generation, |_| {
            panic!("observer exit fixture");
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(4);
        while observer.exited_generation().is_none() {
            assert!(Instant::now() < deadline, "observer did not exit");
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(control.observation_fault(), Some(Fault::ObserverExited));
        assert!(control.cancelled(generation));
        assert!(observer.shutdown().is_err());
        assert!(observer.shutdown().is_ok());
    }
}
