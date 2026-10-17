//! Cue construction off the UI thread: text fitting, the normalized size cap
//! and `PreparedCue` assembly run on one `sela-prepare-cue` worker.
//!
//! Lanes: `Live` is an ordered FIFO bounded at `LIVE_DEPTH` outstanding cues
//! (queued, running or not yet polled); a full lane refuses with
//! `PrepareError::Busy` so the caller can say so instead of growing memory.
//! `Preview` is one slot, newest wins. The worker serves Live before Preview.
//!
//! Black/Clear/Logo never enter this worker: masks go straight to the output
//! supervisor and the logo has its own `Preparer`, so a heavy cue cannot delay
//! a safety control.
//!
//! Staleness is by generation. `cancel(lane)` raises that lane's fence: queued
//! jobs are dropped, a running job is told to stop, and any result at or below
//! the fence is discarded even if it completes later.
use crate::{
    fonts::Resolved,
    scene::RendererCapabilities,
    scene::{ContentVersion, Extent, PrepareError, PreparedBackground, PreparedCue},
    slides::{self, Sizing, Slide},
    storage::Version,
};
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

/// Live cues outstanding at once; the next is refused with `Busy`.
pub const LIVE_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Live,
    Preview,
}

impl Lane {
    fn slot(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub lane: Lane,
    pub generation: u64,
}

/// Everything a cue needs, owned, so nothing is read from UI state.
pub struct CueJob {
    pub item: Version,
    pub index: usize,
    pub version: ContentVersion,
    pub slides: Arc<[Slide]>,
    pub resolved: Arc<[Resolved]>,
    pub extent: Extent,
    pub caps: RendererCapabilities,
    pub sizing: Sizing,
    /// Already fitted to `extent` by the image worker, or a plain color.
    pub background: PreparedBackground,
    /// The song's copyright strip (`slides::copyright_strip`), drawn on every
    /// slide of the item; `None` for a song without copyright text.
    pub footer: Option<Arc<str>>,
}

pub struct Built {
    pub cue: PreparedCue,
    pub size_cap: Option<u16>,
}

pub enum CueEvent {
    Ready {
        ticket: Ticket,
        cue: Arc<PreparedCue>,
        size_cap: Option<u16>,
    },
    Failed {
        ticket: Ticket,
        error: PrepareError,
    },
}

impl CueEvent {
    pub fn ticket(&self) -> Ticket {
        match self {
            Self::Ready { ticket, .. } | Self::Failed { ticket, .. } => *ticket,
        }
    }
}

type Builder = Box<dyn FnMut(CueJob, &AtomicBool) -> Result<Built, PrepareError> + Send>;

struct Queued {
    ticket: Ticket,
    job: CueJob,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    next: u64,
    live: VecDeque<Queued>,
    preview: Option<Queued>,
    running: Option<(Ticket, Arc<AtomicBool>)>,
    done: VecDeque<CueEvent>,
    /// Generations at or below the fence are stale, per lane.
    fence: [u64; 2],
    closed: bool,
    coalesced: u64,
}

impl State {
    fn outstanding(&self, lane: Lane) -> usize {
        let queued = match lane {
            Lane::Live => self.live.len(),
            Lane::Preview => usize::from(self.preview.is_some()),
        };
        let running = self.running.as_ref().is_some_and(|(t, _)| t.lane == lane);
        queued + usize::from(running) + self.done.iter().filter(|e| e.ticket().lane == lane).count()
    }
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // The worker catches builder panics, so poisoning would mean a bug in
        // this module; the queue state is still structurally valid.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

pub struct CuePreparer {
    shared: Arc<Shared>,
}

impl CuePreparer {
    pub fn new() -> std::io::Result<Self> {
        let mut cache = CapCache::default();
        Self::with_builder(move |job, cancel| job.build(&mut cache, cancel))
    }

    /// Fault-injection seam: tests substitute the build step and keep the
    /// real queue, fence and worker thread.
    pub fn with_builder(
        builder: impl FnMut(CueJob, &AtomicBool) -> Result<Built, PrepareError> + Send + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("sela-prepare-cue".into())
            .spawn(move || run(worker, Box::new(builder)))?;
        Ok(Self { shared })
    }

    /// Queues `job`. Live: `Busy` when `LIVE_DEPTH` cues are outstanding.
    /// Preview: replaces any older preview job, which is cancelled.
    pub fn submit(&self, lane: Lane, job: CueJob) -> Result<Ticket, PrepareError> {
        let mut state = self.shared.lock();
        if state.closed {
            return Err(PrepareError::Disconnected);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        match lane {
            Lane::Live if state.outstanding(Lane::Live) >= LIVE_DEPTH => {
                return Err(PrepareError::Busy);
            }
            Lane::Live => {}
            Lane::Preview => {
                // Newest wins: the previous preview is superseded wherever it is.
                let superseded = state.preview.take().is_some()
                    || state.running.as_ref().is_some_and(|(t, _)| t.lane == lane)
                    || state.done.iter().any(|e| e.ticket().lane == lane);
                if let Some((t, flag)) = &state.running
                    && t.lane == lane
                {
                    flag.store(true, Ordering::Release);
                }
                state.done.retain(|e| e.ticket().lane != lane);
                state.coalesced += u64::from(superseded);
                state.fence[lane.slot()] = state.next;
            }
        }
        state.next += 1;
        let ticket = Ticket {
            lane,
            generation: state.next,
        };
        let queued = Queued {
            ticket,
            job,
            cancel,
        };
        match lane {
            Lane::Live => state.live.push_back(queued),
            Lane::Preview => state.preview = Some(queued),
        }
        drop(state);
        self.shared.wake.notify_one();
        Ok(ticket)
    }

    /// Everything submitted so far on `lane` becomes stale.
    pub fn cancel(&self, lane: Lane) {
        let mut state = self.shared.lock();
        state.fence[lane.slot()] = state.next;
        match lane {
            Lane::Live => state.live.clear(),
            Lane::Preview => state.preview = None,
        }
        if let Some((t, flag)) = &state.running
            && t.lane == lane
        {
            flag.store(true, Ordering::Release);
        }
        state.done.retain(|e| e.ticket().lane != lane);
    }

    /// Completed cues, oldest first. Never blocks.
    pub fn poll(&self) -> Option<CueEvent> {
        let mut state = self.shared.lock();
        while let Some(event) = state.done.pop_front() {
            let ticket = event.ticket();
            if ticket.generation > state.fence[ticket.lane.slot()] {
                return Some(event);
            }
        }
        None
    }

    pub fn outstanding(&self, lane: Lane) -> usize {
        self.shared.lock().outstanding(lane)
    }

    /// Preview jobs superseded before their result was used.
    pub fn coalesced(&self) -> u64 {
        self.shared.lock().coalesced
    }
}

impl Drop for CuePreparer {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.closed = true;
        state.live.clear();
        state.preview = None;
        if let Some((_, flag)) = &state.running {
            flag.store(true, Ordering::Release);
        }
        drop(state);
        // No join: a build in flight ends on its own and the thread exits.
        self.shared.wake.notify_all();
    }
}

fn run(shared: Arc<Shared>, mut builder: Builder) {
    loop {
        let queued = {
            let mut state = shared.lock();
            loop {
                if state.closed {
                    return;
                }
                if let Some(next) = state.live.pop_front().or_else(|| state.preview.take()) {
                    state.running = Some((next.ticket, next.cancel.clone()));
                    break next;
                }
                state = shared.wake.wait(state).unwrap_or_else(|p| p.into_inner());
            }
        };
        let Queued {
            ticket,
            job,
            cancel,
        } = queued;
        let result = if cancel.load(Ordering::Acquire) {
            Err(PrepareError::Cancelled)
        } else {
            catch_unwind(AssertUnwindSafe(|| builder(job, &cancel)))
                .unwrap_or(Err(PrepareError::InvalidScene))
        };
        let mut state = shared.lock();
        state.running = None;
        if cancel.load(Ordering::Acquire) || ticket.generation <= state.fence[ticket.lane.slot()] {
            continue;
        }
        state.done.push_back(match result {
            Ok(Built { cue, size_cap }) => CueEvent::Ready {
                ticket,
                cue: Arc::new(cue),
                size_cap,
            },
            Err(error) => CueEvent::Failed { ticket, error },
        });
    }
}

/// The normalized cap depends on every slide of an item; recompute it only
/// when the item, surface, sizing or face resolution changes. Holding the
/// resolution `Arc` keeps its address from being reused while cached.
#[derive(Default)]
struct CapCache(Option<CapEntry>);

type CapEntry = (Version, Extent, Sizing, Arc<[Resolved]>, Option<u16>);

impl CapCache {
    fn cap(&mut self, job: &CueJob) -> Option<u16> {
        if let Some((item, extent, sizing, resolved, cap)) = &self.0
            && (*item, *extent, *sizing) == (job.item, job.extent, job.sizing)
            && Arc::ptr_eq(resolved, &job.resolved)
        {
            return *cap;
        }
        let cap = slides::size_cap_with_footer(
            &job.slides,
            &job.resolved,
            job.extent,
            job.sizing,
            job.footer.as_deref(),
        );
        self.0 = Some((job.item, job.extent, job.sizing, job.resolved.clone(), cap));
        cap
    }
}

impl CueJob {
    fn build(self, cache: &mut CapCache, cancel: &AtomicBool) -> Result<Built, PrepareError> {
        let size_cap = cache.cap(&self);
        if cancel.load(Ordering::Acquire) {
            return Err(PrepareError::Cancelled);
        }
        let (Some(slide), Some(face)) =
            (self.slides.get(self.index), self.resolved.get(self.index))
        else {
            return Err(PrepareError::InvalidScene);
        };
        let cue = slides::cue_with_footer(
            self.version,
            slide,
            self.footer.as_deref(),
            face,
            self.extent,
            self.caps,
            size_cap,
            self.background,
        )?;
        Ok(Built { cue, size_cap })
    }
}

#[cfg(test)]
mod tests;
