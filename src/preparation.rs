//! One worker, one queued input, one queued completion. No blocking UI calls.
use crate::scene::{self, PrepareError, PreparedCue, RendererCapabilities, SceneSpec};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(pub u64);

pub enum PreparationEvent {
    Ready {
        request: RequestId,
        cue: Arc<PreparedCue>,
    },
    Failed {
        request: RequestId,
        error: PrepareError,
    },
}

struct Pending {
    id: RequestId,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
}
struct Job {
    id: RequestId,
    spec: SceneSpec,
    cancel: Arc<AtomicBool>,
}
type Completion = (RequestId, Result<PreparedCue, PrepareError>);

pub struct Preparer {
    jobs: SyncSender<Job>,
    results: Receiver<Completion>,
    pending: Option<Pending>,
    next: u64,
    caps: RendererCapabilities,
}

impl Preparer {
    pub fn new(caps: RendererCapabilities) -> std::io::Result<Self> {
        Self::with_preparer(caps, move |spec, cancel| scene::prepare(spec, caps, cancel))
    }

    // Private fault-injection seam exercises the real queue/state machine in tests.
    fn with_preparer(
        caps: RendererCapabilities,
        prepare: impl Fn(SceneSpec, &AtomicBool) -> Result<PreparedCue, PrepareError> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (jobs, incoming) = mpsc::sync_channel::<Job>(1);
        let (completed, results) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("sela-prepare".into())
            .spawn(move || {
                while let Ok(job) = incoming.recv() {
                    if job.cancel.load(Ordering::Acquire) {
                        continue;
                    }
                    let result = prepare(job.spec, &job.cancel);
                    if !job.cancel.load(Ordering::Acquire)
                        && completed.send((job.id, result)).is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs,
            results,
            pending: None,
            next: 0,
            caps,
        })
    }

    /// Accepted input supersedes the previous request. Full/invalid input leaves
    /// that request intact. Timeout starts at acceptance, including queue wait.
    pub fn request(
        &mut self,
        spec: SceneSpec,
        now: Instant,
        timeout: Duration,
    ) -> Result<RequestId, PrepareError> {
        scene::validate(&spec, self.caps)?;
        if timeout.is_zero() {
            return Err(PrepareError::TimedOut);
        }
        let deadline = now.checked_add(timeout).ok_or(PrepareError::InvalidScene)?;
        let id = RequestId(
            self.next
                .checked_add(1)
                .expect("preparation request counter exhausted"),
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job {
            id,
            spec,
            cancel: cancel.clone(),
        };
        self.jobs.try_send(job).map_err(|e| match e {
            TrySendError::Full(_) => PrepareError::Busy,
            TrySendError::Disconnected(_) => PrepareError::Disconnected,
        })?;
        self.cancel();
        self.next = id.0;
        self.pending = Some(Pending {
            id,
            deadline,
            cancel,
        });
        Ok(id)
    }

    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.cancel.store(true, Ordering::Release);
        }
    }

    /// Caller supplies a monotonic instant, enabling exact deadline tests.
    /// Expiry wins at the boundary, even if a completed result is already queued.
    pub fn poll(&mut self, now: Instant) -> Option<PreparationEvent> {
        if let Some(pending) = &self.pending
            && now >= pending.deadline
        {
            let request = pending.id;
            self.cancel();
            return Some(PreparationEvent::Failed {
                request,
                error: PrepareError::TimedOut,
            });
        }
        // At most one running and one queued job can finish without new input.
        for _ in 0..2 {
            match self.results.try_recv() {
                Ok((request, result)) => {
                    if self.pending.as_ref().is_none_or(|p| p.id != request) {
                        continue;
                    }
                    self.pending = None;
                    return Some(match result {
                        Ok(cue) => PreparationEvent::Ready {
                            request,
                            cue: Arc::new(cue),
                        },
                        Err(error) => PreparationEvent::Failed { request, error },
                    });
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let pending = self.pending.take()?;
                    return Some(PreparationEvent::Failed {
                        request: pending.id,
                        error: PrepareError::Disconnected,
                    });
                }
            }
        }
        None
    }
}

impl Drop for Preparer {
    fn drop(&mut self) {
        self.cancel();
        // Deliberately no join on UI teardown. A blocked OS call is not forcibly
        // interruptible; the worker exits when it returns and channels close.
    }
}

#[cfg(test)]
mod tests;
