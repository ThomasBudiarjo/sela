//! Operator-side owner of one audience renderer process session. No GPUI types.
//! Spawning, pipe I/O and reaping run on threads; `poll` never blocks.
use crate::{
    delivery::{
        Acknowledgment, Counters, Delivery, DeliveryError, Epoch, Lane, LiveState, Outcome, Stamp,
    },
    scene::{ContentVersion, Extent, PreparedCue, RendererCapabilities},
    transport::{Frame, PipeWorkers},
};
use std::{
    io,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

/// Native device and surface setup must finish within this bound.
pub const STARTUP: Duration = Duration::from_secs(15);
/// Each cue must reach a terminal acknowledgment within this bound.
pub const ACKNOWLEDGMENT: Duration = Duration::from_secs(3);
/// Inbound frames handled per poll, so a flood cannot monopolize the UI.
const FRAMES_PER_POLL: usize = 8;

/// Which child stream carries renderer frames; the other is left inherited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

pub struct Launch {
    pub command: Command,
    pub frames: Stream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loss {
    Spawn,
    StartupTimedOut,
    Exited,
    Protocol,
    Delivery(DeliveryError),
}

impl std::fmt::Display for Loss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn => f.write_str("Audience output could not be started"),
            Self::StartupTimedOut => f.write_str("Audience output did not become ready in time"),
            Self::Exited => f.write_str("Audience output process ended"),
            Self::Protocol => f.write_str("Audience output sent an invalid message"),
            Self::Delivery(error) => error.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Connected {
        extent: Extent,
        caps: RendererCapabilities,
    },
    /// Output state is unknown. A fresh session (new epoch) is required.
    Lost(Loss),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotConnected,
    WrongExtent,
}

type Spawned = io::Result<(Child, PipeWorkers)>;

/// One renderer session. Holds at most one cue awaiting acknowledgment and
/// one newer wanted cue; a later request replaces the unsent one, so rapid
/// input cannot apply an older scene after a newer one.
pub struct Supervisor {
    epoch: Epoch,
    status: Status,
    started: Instant,
    spawning: Option<Receiver<Spawned>>,
    child: Option<Child>,
    pipes: Option<PipeWorkers>,
    delivery: Delivery,
    max_dimension: Option<u32>,
    wanted: Option<Arc<PreparedCue>>,
    in_flight: Option<(Stamp, ContentVersion)>,
    rejected: Option<(ContentVersion, DeliveryError)>,
}

impl Supervisor {
    /// `launch` runs on a spawner thread with this session's fresh epoch.
    pub fn start(launch: impl FnOnce(Epoch) -> Launch + Send + 'static, now: Instant) -> Self {
        let epoch = fresh_epoch();
        let (sender, spawning) = mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("sela-output-spawn".into())
            .spawn(move || {
                let result = spawn(launch(epoch));
                if let Err(mpsc::TrySendError::Disconnected(Ok((child, _)))) =
                    sender.try_send(result)
                {
                    reap(child);
                }
            });
        let mut this = Self {
            epoch,
            status: Status::Starting,
            started: now,
            spawning: Some(spawning),
            child: None,
            pipes: None,
            delivery: Delivery::new(epoch),
            max_dimension: None,
            wanted: None,
            in_flight: None,
            rejected: None,
        };
        if spawned.is_err() {
            this.lose(Loss::Spawn);
        }
        this
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }
    pub fn status(&self) -> Status {
        self.status
    }
    /// Renderer-acknowledged scene. Unknown whenever the session is not intact.
    pub fn live(&self) -> LiveState {
        if matches!(self.status, Status::Connected { .. }) {
            self.delivery.live()
        } else {
            LiveState::Unknown
        }
    }
    /// Sent and awaiting a terminal acknowledgment.
    pub fn in_flight(&self) -> Option<ContentVersion> {
        self.in_flight.map(|(_, version)| version)
    }
    /// Requested but not yet sent because another cue is in flight.
    pub fn wanted(&self) -> Option<ContentVersion> {
        self.wanted.as_ref().map(|cue| cue.version())
    }
    pub fn counters(&self) -> Counters {
        self.delivery.counters()
    }
    /// Most recent renderer rejection; the prior scene remains applied.
    pub fn rejected(&self) -> Option<(ContentVersion, DeliveryError)> {
        self.rejected
    }

    /// Cue extent must equal the reported surface extent.
    pub fn present(&mut self, cue: Arc<PreparedCue>, now: Instant) -> Result<(), Refusal> {
        let Status::Connected { extent, .. } = self.status else {
            return Err(Refusal::NotConnected);
        };
        if cue.extent() != extent {
            return Err(Refusal::WrongExtent);
        }
        self.wanted = Some(cue);
        self.pump(now);
        Ok(())
    }

    /// Returns true when observable state changed.
    pub fn poll(&mut self, now: Instant) -> bool {
        let before = self.snapshot();
        self.poll_inner(now);
        before != self.snapshot()
    }

    fn snapshot(&self) -> impl PartialEq + use<> {
        (
            self.status,
            self.live(),
            self.in_flight(),
            self.wanted(),
            self.rejected,
        )
    }

    fn poll_inner(&mut self, now: Instant) {
        if matches!(self.status, Status::Lost(_)) {
            return;
        }
        if let Some(spawning) = &self.spawning {
            match spawning.try_recv() {
                Ok(Ok((child, pipes))) => {
                    self.child = Some(child);
                    self.pipes = Some(pipes);
                    self.spawning = None;
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => return self.lose(Loss::Spawn),
                Err(TryRecvError::Empty) => {}
            }
        }
        if self.status == Status::Starting && now.saturating_duration_since(self.started) >= STARTUP
        {
            return self.lose(Loss::StartupTimedOut);
        }
        for _ in 0..FRAMES_PER_POLL {
            let Some(pipes) = &self.pipes else { break };
            match pipes.poll() {
                Ok(Some(frame)) => {
                    if let Err(loss) = self.receive(frame, now) {
                        return self.lose(loss);
                    }
                }
                Ok(None) => break,
                Err(_) => return self.lose(Loss::Exited),
            }
        }
        if let Some(child) = &mut self.child
            && !matches!(child.try_wait(), Ok(None))
        {
            return self.lose(Loss::Exited);
        }
        if self.delivery.expire(now).is_some() {
            return self.lose(Loss::Delivery(DeliveryError::TimedOut));
        }
        self.pump(now);
    }

    fn receive(&mut self, frame: Frame, now: Instant) -> Result<(), Loss> {
        if frame.is_ready() {
            let (epoch, max) = frame.into_ready().map_err(|_| Loss::Protocol)?;
            if epoch != self.epoch || self.max_dimension.is_some() || max == 0 {
                return Err(Loss::Protocol);
            }
            self.max_dimension = Some(max);
        } else if frame.is_surface() {
            let (epoch, extent) = frame.into_surface().map_err(|_| Loss::Protocol)?;
            let max_texture_dimension = self.max_dimension.ok_or(Loss::Protocol)?;
            if epoch != self.epoch {
                return Err(Loss::Protocol);
            }
            self.status = Status::Connected {
                extent,
                caps: RendererCapabilities {
                    max_texture_dimension,
                },
            };
            if self.wanted.as_ref().is_some_and(|w| w.extent() != extent) {
                self.wanted = None;
            }
        } else if frame.is_acknowledgment() {
            let ack = frame.into_acknowledgment().map_err(|_| Loss::Protocol)?;
            self.acknowledge(ack, now)?;
        } else {
            return Err(Loss::Protocol);
        }
        Ok(())
    }

    fn acknowledge(&mut self, ack: Acknowledgment, now: Instant) -> Result<(), Loss> {
        if !self.delivery.acknowledge(ack, now) {
            return Err(Loss::Protocol);
        }
        if !self.delivery.is_connected() {
            let Outcome::Rejected(error) = ack.outcome else {
                return Err(Loss::Protocol);
            };
            return Err(Loss::Delivery(error));
        }
        if let Some((stamp, version)) = self.in_flight
            && stamp == ack.stamp
            && ack.outcome != Outcome::Accepted
        {
            self.in_flight = None;
            if let Outcome::Rejected(error) = ack.outcome {
                self.rejected = Some((version, error));
            } else if self.rejected.is_some_and(|(v, _)| v == version) {
                self.rejected = None;
            }
        }
        Ok(())
    }

    fn pump(&mut self, now: Instant) {
        if self.in_flight.is_some() || !matches!(self.status, Status::Connected { .. }) {
            return;
        }
        let Some(cue) = self.wanted.take() else {
            return;
        };
        let version = cue.version();
        let stamp = match self
            .delivery
            .submit(cue, Lane::Cue, now, now + ACKNOWLEDGMENT)
        {
            Ok(stamp) => stamp,
            Err(error) => return self.lose(Loss::Delivery(error)),
        };
        let command = self
            .delivery
            .take_next()
            .expect("submitted cue is next in an otherwise idle session");
        let sent = Frame::command(&command, now).and_then(|frame| {
            self.pipes
                .as_ref()
                .ok_or_else(|| io::ErrorKind::NotConnected.into())
                .and_then(|pipes| pipes.try_send(frame))
        });
        match sent {
            Ok(()) => self.in_flight = Some((stamp, version)),
            // A full outbound queue with nothing in flight means a stuck writer.
            Err(_) => self.lose(Loss::Exited),
        }
    }

    fn lose(&mut self, loss: Loss) {
        self.status = Status::Lost(loss);
        self.delivery.disconnect();
        self.wanted = None;
        self.in_flight = None;
        self.spawning = None;
        self.pipes = None;
        if let Some(child) = self.child.take() {
            reap(child);
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            reap(child);
        }
    }
}

fn spawn(launch: Launch) -> Spawned {
    let Launch {
        mut command,
        frames,
    } = launch;
    command.stdin(Stdio::piped());
    match frames {
        Stream::Stdout => command.stdout(Stdio::piped()),
        Stream::Stderr => command.stderr(Stdio::piped()).stdout(Stdio::null()),
    };
    let mut child = command.spawn()?;
    let writer = child.stdin.take();
    let reader: Option<Box<dyn io::Read + Send>> = match frames {
        Stream::Stdout => child.stdout.take().map(|r| Box::new(r) as _),
        Stream::Stderr => child.stderr.take().map(|r| Box::new(r) as _),
    };
    let (Some(reader), Some(writer)) = (reader, writer) else {
        reap(child);
        return Err(io::ErrorKind::BrokenPipe.into());
    };
    match PipeWorkers::new(reader, writer) {
        Ok(pipes) => Ok((child, pipes)),
        Err(error) => {
            reap(child);
            Err(error)
        }
    }
}

/// Kill and wait off the caller's thread; a wedged child cannot stall the UI.
fn reap(mut child: Child) {
    let _ = child.kill();
    let reaper = std::thread::Builder::new()
        .name("sela-output-reap".into())
        .spawn(move || {
            let _ = child.wait();
        });
    drop(reaper);
}

/// CPU-only, never zero; unique per process start and call.
fn fresh_epoch() -> Epoch {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hash = Sha256::new();
    hash.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hash.update(std::process::id().to_le_bytes());
    hash.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let value = u128::from_le_bytes(hash.finalize()[..16].try_into().unwrap());
    Epoch(value.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_are_fresh_and_nonzero() {
        let a = fresh_epoch();
        let b = fresh_epoch();
        assert_ne!(a, b);
        assert_ne!(a.0, 0);
    }

    #[test]
    fn failed_spawn_is_lost_and_refuses_cues() {
        let now = Instant::now();
        let mut output = Supervisor::start(
            |_| Launch {
                command: Command::new("sela-definitely-missing-audience-binary"),
                frames: Stream::Stdout,
            },
            now,
        );
        let end = Instant::now() + Duration::from_secs(5);
        while output.status() == Status::Starting {
            assert!(Instant::now() < end, "spawn result must arrive");
            output.poll(Instant::now());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(output.status(), Status::Lost(Loss::Spawn));
        assert_eq!(output.live(), LiveState::Unknown);
        let cue = PreparedCue::diagnostic_color(
            ContentVersion { id: 1, revision: 1 },
            Extent {
                width: 64,
                height: 64,
            },
            [0, 0, 0, 255],
            RendererCapabilities {
                max_texture_dimension: 64,
            },
        )
        .unwrap();
        assert_eq!(
            output.present(Arc::new(cue), Instant::now()),
            Err(Refusal::NotConnected)
        );
    }
}
