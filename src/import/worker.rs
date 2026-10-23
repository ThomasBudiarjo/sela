//! The `sela-import` thread (M1-14): finds, reads and stages the files of
//! one import batch off the UI thread and reports each file as it is done.
//! Both channels are bounded: one job waits at most, and the worker blocks
//! once `EVENT_DEPTH` results are unread, so a slow UI holds the worker
//! back instead of growing a queue. Nothing here touches the library; the
//! operator commits accepted songs through the storage worker.
use super::batch::{self, FileError, MAX_FILES, MAX_STAGED_BYTES, Staged};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

/// Unread events the worker may get ahead of the UI.
pub const EVENT_DEPTH: usize = 16;

/// Called with each file's index before it is read. Tests use it to hold a
/// batch at a known point; production passes none.
pub type Pace = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(Debug)]
pub enum Event {
    /// The chosen paths are expanded: `total` file results follow, in order.
    Found {
        job: u64,
        total: usize,
        ignored: usize,
        over_limit: usize,
        unreadable: Vec<(PathBuf, String)>,
        /// Chosen folders cut at `batch::MAX_FOLDER_ENTRIES`.
        truncated: Vec<PathBuf>,
    },
    File {
        job: u64,
        index: usize,
        path: PathBuf,
        result: Result<Box<Staged>, FileError>,
    },
    /// Last event of a job. `canceled`: it stopped early on request.
    Done { job: u64, canceled: bool },
    /// Last event of a job whose thread ended without finishing it (a
    /// panic). The importer cannot take another job; start a new one.
    Failed { job: u64 },
}

struct Job {
    id: u64,
    paths: Vec<PathBuf>,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// A job is still running; cancel it and wait for its `Done`.
    Busy,
    /// The thread is gone.
    Closed,
}

pub struct Importer {
    jobs: mpsc::SyncSender<Job>,
    events: mpsc::Receiver<Event>,
    /// Cancel flag of the job not yet `Done`.
    running: Option<(u64, Arc<AtomicBool>)>,
}

impl Drop for Importer {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Importer {
    pub fn start() -> std::io::Result<Self> {
        Self::start_paced(None)
    }

    pub fn start_paced(pace: Option<Pace>) -> std::io::Result<Self> {
        let (jobs, job_rx) = mpsc::sync_channel::<Job>(1);
        let (event_tx, events) = mpsc::sync_channel(EVENT_DEPTH);
        std::thread::Builder::new()
            .name("sela-import".into())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    if run(&job, &event_tx, pace.as_deref()).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs,
            events,
            running: None,
        })
    }

    /// Starts reading `paths` as job `id`. One job runs at a time.
    pub fn submit(&mut self, id: u64, paths: Vec<PathBuf>) -> Result<(), SubmitError> {
        if self.running.is_some() {
            return Err(SubmitError::Busy);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs
            .try_send(Job {
                id,
                paths,
                cancel: cancel.clone(),
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => SubmitError::Busy,
                mpsc::TrySendError::Disconnected(_) => SubmitError::Closed,
            })?;
        self.running = Some((id, cancel));
        Ok(())
    }

    /// Asks the running job to stop before its next file. Its `Done` still
    /// arrives, with `canceled` set.
    pub fn cancel(&self) {
        if let Some((_, cancel)) = &self.running {
            cancel.store(true, Ordering::Release);
        }
    }

    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// The next event, without blocking.
    pub fn poll(&mut self) -> Option<Event> {
        let event = match self.events.try_recv() {
            Ok(event) => event,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The thread ended (it panicked): finish the job as failed
                // so the operator is never left waiting and is told.
                let (job, _) = self.running.take()?;
                return Some(Event::Failed { job });
            }
        };
        if let Event::Done { job, .. } = &event
            && self.running.as_ref().is_some_and(|(id, _)| id == job)
        {
            self.running = None;
        }
        Some(event)
    }
}

/// Runs one job. `Err` only when the receiving side is gone.
fn run(
    job: &Job,
    events: &mpsc::SyncSender<Event>,
    pace: Option<&(dyn Fn(usize) + Send + Sync)>,
) -> Result<(), ()> {
    let canceled = || job.cancel.load(Ordering::Acquire);
    let send = |event: Event| events.send(event).map_err(|_| ());
    let found = batch::find(&job.paths, &job.cancel);
    send(Event::Found {
        job: job.id,
        total: found.files.len(),
        ignored: found.ignored,
        over_limit: found.over_limit,
        unreadable: found.unreadable,
        truncated: found.truncated,
    })?;
    let mut held = 0usize;
    for (index, path) in found.files.into_iter().enumerate() {
        if let Some(pace) = pace {
            pace(index);
        }
        if canceled() {
            return send(Event::Done {
                job: job.id,
                canceled: true,
            });
        }
        let result = match over_budget(index, held, None) {
            Some(error) => Err(error),
            None => batch::read(&path)
                .and_then(|bytes| batch::stage(&path, &bytes))
                .and_then(
                    |staged| match over_budget(index, held, Some(staged.bytes)) {
                        Some(error) => Err(error),
                        None => {
                            held += staged.bytes;
                            Ok(Box::new(staged))
                        }
                    },
                ),
        };
        send(Event::File {
            job: job.id,
            index,
            path,
            result,
        })?;
    }
    send(Event::Done {
        job: job.id,
        canceled: canceled(),
    })
}

/// Which batch limit file `index` hits with `held` bytes already staged
/// and, once read, `next` bytes of its own: the file count (`BatchFull`)
/// or the staged text (`BatchTooLarge`), so the preview names the right one.
fn over_budget(index: usize, held: usize, next: Option<usize>) -> Option<FileError> {
    if index >= MAX_FILES {
        Some(FileError::BatchFull)
    } else if held.saturating_add(next.unwrap_or(0)) > MAX_STAGED_BYTES
        || (next.is_none() && held >= MAX_STAGED_BYTES)
    {
        Some(FileError::BatchTooLarge)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn drain(importer: &mut Importer, job: u64) -> Vec<Event> {
        let end = Instant::now() + Duration::from_secs(10);
        let mut out = Vec::new();
        loop {
            match importer.poll() {
                Some(event) => {
                    let done = matches!(event, Event::Done { job: j, .. } if j == job);
                    out.push(event);
                    if done {
                        return out;
                    }
                }
                None => {
                    assert!(Instant::now() < end, "bounded wait for job {job}");
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        }
    }

    #[test]
    fn a_batch_reports_every_file_in_order_with_partial_failures() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.cho"), "{title: A}\nOne line\n").unwrap();
        std::fs::write(dir.path().join("b.xml"), "<song><broken>").unwrap();
        std::fs::write(dir.path().join("c.cho"), "{title: C}\nAnother line\n").unwrap();
        std::fs::write(dir.path().join("d.png"), "not a song").unwrap();
        let mut importer = Importer::start().unwrap();
        importer.submit(7, vec![dir.path().to_path_buf()]).unwrap();
        assert_eq!(importer.submit(8, vec![]), Err(SubmitError::Busy));
        let events = drain(&mut importer, 7);
        assert!(matches!(
            events[0],
            Event::Found {
                job: 7,
                total: 3,
                ignored: 1,
                over_limit: 0,
                ..
            }
        ));
        let results: Vec<(usize, bool)> = events
            .iter()
            .filter_map(|e| match e {
                Event::File { index, result, .. } => Some((*index, result.is_ok())),
                _ => None,
            })
            .collect();
        assert_eq!(results, [(0, true), (1, false), (2, true)]);
        assert!(matches!(
            events.last(),
            Some(Event::Done {
                job: 7,
                canceled: false
            })
        ));
        assert!(!importer.busy());
        // The thread takes the next job.
        importer.submit(8, vec![dir.path().join("a.cho")]).unwrap();
        assert_eq!(drain(&mut importer, 8).len(), 3);
    }

    #[test]
    fn the_byte_limit_and_the_file_limit_are_told_apart() {
        assert_eq!(over_budget(MAX_FILES, 0, None), Some(FileError::BatchFull));
        assert_eq!(
            over_budget(200, MAX_STAGED_BYTES, None),
            Some(FileError::BatchTooLarge)
        );
        assert_eq!(
            over_budget(200, MAX_STAGED_BYTES - 10, Some(11)),
            Some(FileError::BatchTooLarge)
        );
        assert_eq!(over_budget(200, MAX_STAGED_BYTES - 10, Some(10)), None);
        assert_eq!(over_budget(0, 0, None), None);
    }

    #[test]
    fn cancel_stops_before_the_next_file() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..5 {
            std::fs::write(dir.path().join(format!("{n}.cho")), "Line\n").unwrap();
        }
        let (reached_tx, reached) = mpsc::sync_channel::<usize>(8);
        let (go, go_rx) = mpsc::sync_channel::<()>(8);
        let go_rx = std::sync::Mutex::new(go_rx);
        let pace: Pace = Arc::new(move |index| {
            let _ = reached_tx.send(index);
            if index == 2 {
                let _ = go_rx.lock().unwrap().recv();
            }
        });
        let mut importer = Importer::start_paced(Some(pace)).unwrap();
        importer.submit(1, vec![dir.path().to_path_buf()]).unwrap();
        // Hold the worker before file 2, cancel, then let it go.
        while reached.recv_timeout(Duration::from_secs(10)).unwrap() != 2 {}
        importer.cancel();
        go.send(()).unwrap();
        let events = drain(&mut importer, 1);
        let files = events
            .iter()
            .filter(|e| matches!(e, Event::File { .. }))
            .count();
        assert_eq!(files, 2, "files 0 and 1 only");
        assert!(matches!(
            events.last(),
            Some(Event::Done {
                job: 1,
                canceled: true
            })
        ));
    }

    #[test]
    fn a_panicked_worker_reports_the_job_failed_not_canceled() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.cho"), "Line\n").unwrap();
        let pace: Pace = Arc::new(|_| panic!("a parser bug"));
        let mut importer = Importer::start_paced(Some(pace)).unwrap();
        importer.submit(3, vec![dir.path().to_path_buf()]).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let last = loop {
            match importer.poll() {
                Some(Event::Found { .. }) => {}
                Some(event) => break event,
                None => {
                    assert!(Instant::now() < end, "bounded wait");
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        };
        assert!(matches!(last, Event::Failed { job: 3 }), "{last:?}");
        assert!(!importer.busy());
        assert_eq!(importer.submit(4, vec![]), Err(SubmitError::Closed));
    }

    #[test]
    fn the_worker_waits_for_a_slow_reader() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..EVENT_DEPTH * 3 {
            std::fs::write(dir.path().join(format!("{n:03}.cho")), "Line\n").unwrap();
        }
        let (reached_tx, reached) = mpsc::channel::<usize>();
        let pace: Pace = Arc::new(move |index| {
            let _ = reached_tx.send(index);
        });
        let mut importer = Importer::start_paced(Some(pace)).unwrap();
        importer.submit(3, vec![dir.path().to_path_buf()]).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        // Nothing was read yet: the worker got at most one event past the
        // channel's capacity ahead (Found plus EVENT_DEPTH files).
        let ahead = reached.try_iter().count();
        assert!(ahead <= EVENT_DEPTH + 1, "worker ran {ahead} files ahead");
        let events = drain(&mut importer, 3);
        assert_eq!(events.len(), EVENT_DEPTH * 3 + 2);
    }
}
