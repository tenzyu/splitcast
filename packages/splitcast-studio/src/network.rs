use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
};

use crate::wire::{Call, Endpoint, Failure, Reply};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Lane {
    Stop,
    Status,
    Control,
    Chat,
    Preview,
}

impl Lane {
    pub(crate) const ALL: [Self; 5] = [
        Self::Stop,
        Self::Status,
        Self::Control,
        Self::Chat,
        Self::Preview,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

struct Job {
    call: Call,
    epoch: u64,
    revision: u64,
}

pub(crate) struct ResultEvent {
    pub lane: Lane,
    pub epoch: u64,
    pub revision: u64,
    pub result: Result<Reply, Failure>,
}

pub(crate) struct Network {
    lanes: Vec<flume::Sender<Job>>,
    epoch: Arc<AtomicU64>,
}

impl Network {
    pub(crate) fn supersede(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub(crate) fn send(&self, lane: Lane, call: Call, epoch: u64, revision: u64) -> bool {
        self.lanes[lane.index()]
            .try_send(Job {
                call,
                epoch,
                revision,
            })
            .is_ok()
    }
}

pub(crate) struct Workers(Vec<thread::JoinHandle<()>>);

impl Workers {
    pub(crate) fn start(
        endpoint: Endpoint,
    ) -> io::Result<(Self, Network, flume::Receiver<ResultEvent>)> {
        let epoch = Arc::new(AtomicU64::new(0));
        let (results, events) = flume::bounded(Lane::ALL.len());
        let mut network = Network {
            lanes: Vec::new(),
            epoch,
        };
        let mut workers = Self(Vec::new());
        for lane in Lane::ALL {
            let (sender, receiver) = flume::bounded::<Job>(1);
            let results = results.clone();
            let endpoint = endpoint.clone();
            let epoch = network.epoch.clone();
            let handle = thread::Builder::new()
                .name(format!("studio-{lane:?}"))
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        let result = if job.epoch == epoch.load(Ordering::Acquire) {
                            endpoint.request(&job.call)
                        } else {
                            Err(Failure::Superseded)
                        };
                        if results
                            .send(ResultEvent {
                                lane,
                                epoch: job.epoch,
                                revision: job.revision,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            match handle {
                Ok(handle) => {
                    workers.0.push(handle);
                    network.lanes.push(sender);
                }
                Err(error) => {
                    drop(network);
                    return Err(error);
                }
            }
        }
        Ok((workers, network, events))
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        for thread in self.0.drain(..) {
            if thread.join().is_err() {
                eprintln!("SplitCast Studio control worker exited unexpectedly");
            }
        }
    }
}
