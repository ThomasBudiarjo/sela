//! Bounded, single-owner command/acknowledgment model; not an IPC transport.
use crate::scene::{ContentVersion, PreparedCue};
use std::{collections::VecDeque, sync::Arc, time::Instant};

/// Supervisor supplies a fresh, never-reused epoch for each renderer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epoch(pub u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub epoch: Epoch,
    pub sequence: u64,
}

/// Reserved capacity only; this does not define operator mask semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Cue,
    Safety,
}

pub struct Command {
    stamp: Stamp,
    lane: Lane,
    cue: Arc<PreparedCue>,
    deadline: Instant,
}

impl Command {
    pub(crate) fn from_wire(
        stamp: Stamp,
        lane: Lane,
        cue: Arc<PreparedCue>,
        deadline: Instant,
    ) -> Self {
        Self {
            stamp,
            lane,
            cue,
            deadline,
        }
    }
    pub(crate) fn lane(&self) -> Lane {
        self.lane
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn stamp(&self) -> Stamp {
        self.stamp
    }
    pub fn cue(&self) -> &Arc<PreparedCue> {
        &self.cue
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryError {
    Busy,
    Disconnected,
    TimedOut,
    WrongEpoch,
    Stale,
    RenderFailed,
    SequenceExhausted,
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "Delivery capacity occupied; wait for acknowledgment before retrying",
            Self::Disconnected => "Output state unknown; establish a fresh renderer session",
            Self::TimedOut => {
                "Presentation acknowledgment expired; verify output before reconnecting"
            }
            Self::WrongEpoch => "Command belongs to a different renderer session; do not replay it",
            Self::Stale => "Command was already consumed or superseded; do not replay it",
            Self::RenderFailed => "Renderer rejected presentation; prior scene remains applied",
            Self::SequenceExhausted => "Command sequence exhausted; establish a fresh session",
        })
    }
}
impl std::error::Error for DeliveryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Applied,
    Rejected(DeliveryError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acknowledgment {
    pub stamp: Stamp,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveState {
    Unknown,
    Confirmed(ContentVersion),
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Counters {
    pub submitted: u64,
    pub overloaded: u64,
    pub applied: u64,
    pub rejected: u64,
    pub ignored_acknowledgments: u64,
    pub timed_out: u64,
}

struct Pending {
    stamp: Stamp,
    lane: Lane,
    version: ContentVersion,
    deadline: Instant,
    command: Option<Command>,
    accepted: bool,
}

/// One outstanding cue plus one outstanding safety command, including in-flight
/// commands awaiting terminal acknowledgment. Submission cannot grow a side queue.
pub struct Delivery {
    epoch: Epoch,
    next: u64,
    connected: bool,
    pending: VecDeque<Pending>,
    last_confirmed: Option<(Stamp, ContentVersion)>,
    counters: Counters,
}

impl Delivery {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            epoch,
            next: 0,
            connected: true,
            pending: VecDeque::with_capacity(2),
            last_confirmed: None,
            counters: Counters::default(),
        }
    }

    pub fn submit(
        &mut self,
        cue: Arc<PreparedCue>,
        lane: Lane,
        now: Instant,
        deadline: Instant,
    ) -> Result<Stamp, DeliveryError> {
        if !self.connected {
            return Err(DeliveryError::Disconnected);
        }
        if deadline <= now {
            return Err(DeliveryError::TimedOut);
        }
        if self.pending.iter().any(|p| p.lane == lane) {
            self.counters.overloaded = self.counters.overloaded.saturating_add(1);
            return Err(DeliveryError::Busy);
        }
        let sequence = self
            .next
            .checked_add(1)
            .ok_or(DeliveryError::SequenceExhausted)?;
        let stamp = Stamp {
            epoch: self.epoch,
            sequence,
        };
        self.pending.push_back(Pending {
            stamp,
            lane,
            version: cue.version(),
            deadline,
            command: Some(Command {
                stamp,
                lane,
                cue,
                deadline,
            }),
            accepted: false,
        });
        self.next = sequence;
        self.counters.submitted = self.counters.submitted.saturating_add(1);
        Ok(stamp)
    }

    /// FIFO across both lanes. Reserved capacity does not silently reorder intent.
    pub fn take_next(&mut self) -> Option<Command> {
        self.pending.iter_mut().find_map(|p| p.command.take())
    }

    pub fn is_accepted(&self, stamp: Stamp) -> bool {
        self.pending.iter().any(|p| p.stamp == stamp && p.accepted)
    }

    pub fn live(&self) -> LiveState {
        if self.connected
            && let Some((_, version)) = self.last_confirmed
        {
            LiveState::Confirmed(version)
        } else {
            LiveState::Unknown
        }
    }

    /// Diagnostic history only, never a claim about a disconnected output.
    pub fn last_confirmed(&self) -> Option<ContentVersion> {
        self.last_confirmed.map(|(_, version)| version)
    }
    pub fn counters(&self) -> Counters {
        self.counters
    }

    pub fn acknowledge(&mut self, ack: Acknowledgment, now: Instant) -> bool {
        self.expire(now);
        let index = self
            .pending
            .iter()
            .position(|p| p.stamp == ack.stamp && p.command.is_none());
        let Some(index) = index.filter(|_| self.connected && ack.stamp.epoch == self.epoch) else {
            self.counters.ignored_acknowledgments =
                self.counters.ignored_acknowledgments.saturating_add(1);
            return false;
        };
        if ack.outcome == Outcome::Accepted {
            self.pending[index].accepted = true;
            return true;
        }
        // A stale receipt may describe a duplicate of an earlier applied command,
        // not proof that the original failed. Without a newer confirmed state,
        // conservatively invalidate the session instead of claiming old output.
        if matches!(ack.outcome, Outcome::Rejected(DeliveryError::WrongEpoch))
            || (ack.outcome == Outcome::Rejected(DeliveryError::Stale)
                && self
                    .last_confirmed
                    .is_none_or(|(stamp, _)| stamp.sequence < ack.stamp.sequence))
        {
            self.counters.rejected = self.counters.rejected.saturating_add(1);
            self.disconnect();
            return true;
        }
        let pending = self.pending.remove(index).unwrap();
        match ack.outcome {
            Outcome::Applied => {
                if self
                    .last_confirmed
                    .is_none_or(|(stamp, _)| stamp.sequence < ack.stamp.sequence)
                {
                    self.last_confirmed = Some((ack.stamp, pending.version));
                }
                self.counters.applied = self.counters.applied.saturating_add(1);
            }
            Outcome::Rejected(_) => {
                self.counters.rejected = self.counters.rejected.saturating_add(1)
            }
            Outcome::Accepted => unreachable!(),
        }
        true
    }

    /// A missing terminal acknowledgment may mean presentation happened. Invalidate
    /// the session rather than asserting the prior scene is still live or retrying.
    pub fn expire(&mut self, now: Instant) -> Option<Stamp> {
        let expired = self.pending.iter().find(|p| now >= p.deadline)?.stamp;
        self.counters.timed_out = self.counters.timed_out.saturating_add(1);
        self.disconnect();
        Some(expired)
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.pending.clear();
    }
}

/// Renderer-side ordering and apply boundary. The compositor must finish resource
/// preparation before `present`; its callback must submit without blocking I/O
/// and must not replace the live scene on failure. This model does no rendering.
pub struct RendererSession {
    epoch: Epoch,
    consumed: u64,
    pending: VecDeque<Command>,
    applied: Option<Arc<PreparedCue>>,
    applied_stamp: Option<Stamp>,
}

impl RendererSession {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            epoch,
            consumed: 0,
            pending: VecDeque::with_capacity(2),
            applied: None,
            applied_stamp: None,
        }
    }

    /// A structurally framed command failed renderer-owned resource validation.
    /// Consume its ordering identity without touching pending/applied resources.
    pub fn reject_preparation(&mut self, stamp: Stamp) -> Acknowledgment {
        let error = if stamp.epoch != self.epoch {
            DeliveryError::WrongEpoch
        } else if stamp.sequence <= self.consumed {
            DeliveryError::Stale
        } else {
            self.consumed = stamp.sequence;
            DeliveryError::RenderFailed
        };
        Acknowledgment {
            stamp,
            outcome: Outcome::Rejected(error),
        }
    }

    pub fn accept(&mut self, command: Command, now: Instant) -> Acknowledgment {
        let stamp = command.stamp;
        if self.applied_stamp == Some(stamp) {
            return Acknowledgment {
                stamp,
                outcome: Outcome::Applied,
            };
        }
        if self.pending.iter().any(|p| p.stamp == stamp) {
            return Acknowledgment {
                stamp,
                outcome: Outcome::Accepted,
            };
        }
        let error = if stamp.epoch != self.epoch {
            Some(DeliveryError::WrongEpoch)
        } else if stamp.sequence <= self.consumed {
            Some(DeliveryError::Stale)
        } else {
            self.consumed = stamp.sequence;
            if now >= command.deadline {
                Some(DeliveryError::TimedOut)
            } else if self.pending.iter().any(|p| p.lane == command.lane) {
                Some(DeliveryError::Busy)
            } else {
                None
            }
        };
        let outcome = if let Some(error) = error {
            Outcome::Rejected(error)
        } else {
            self.pending.push_back(command);
            Outcome::Accepted
        };
        Acknowledgment { stamp, outcome }
    }

    pub fn pending(&self) -> Option<&Command> {
        self.pending.front()
    }
    pub fn applied(&self) -> Option<&Arc<PreparedCue>> {
        self.applied.as_ref()
    }

    /// Check expiry BEFORE touching output. A successful callback means frame
    /// submission, not measured scanout. `now` is the start of this commit attempt.
    pub fn present(
        &mut self,
        now: Instant,
        submit: impl FnOnce(&PreparedCue) -> Result<(), DeliveryError>,
    ) -> Option<Acknowledgment> {
        let command = self.pending.pop_front()?;
        let outcome = if now >= command.deadline {
            Outcome::Rejected(DeliveryError::TimedOut)
        } else if submit(&command.cue).is_err() {
            Outcome::Rejected(DeliveryError::RenderFailed)
        } else {
            self.applied = Some(command.cue);
            self.applied_stamp = Some(command.stamp);
            Outcome::Applied
        };
        Some(Acknowledgment {
            stamp: command.stamp,
            outcome,
        })
    }
}

#[cfg(test)]
mod tests;
