//! Bounded, single-owner command/acknowledgment model; not an IPC transport.
use crate::{
    masks::Layer,
    scene::{ContentVersion, PreparedCue},
};
use std::{collections::VecDeque, sync::Arc, time::Instant};

/// Supervisor supplies a fresh, never-reused epoch for each renderer session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epoch(pub u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub epoch: Epoch,
    pub sequence: u64,
}

/// Reserved capacity: one outstanding command per lane. Masks use Safety so a
/// slide still being prepared never delays them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Cue,
    Safety,
}

impl Lane {
    fn index(self) -> usize {
        match self {
            Self::Cue => 0,
            Self::Safety => 1,
        }
    }
}

/// Renderer state a command replaces. Each is ordered independently.
pub enum Payload {
    /// The applied slide.
    Scene(Arc<PreparedCue>),
    /// The image drawn for `Layer::Logo`; never shown by itself.
    Logo(Arc<PreparedCue>),
    Mask(Layer),
}

impl Payload {
    pub fn cue(&self) -> Option<&Arc<PreparedCue>> {
        match self {
            Self::Scene(cue) | Self::Logo(cue) => Some(cue),
            Self::Mask(_) => None,
        }
    }
}

pub struct Command {
    stamp: Stamp,
    lane: Lane,
    payload: Payload,
    deadline: Instant,
}

impl Command {
    pub(crate) fn from_wire(stamp: Stamp, lane: Lane, payload: Payload, deadline: Instant) -> Self {
        Self {
            stamp,
            lane,
            payload,
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
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    /// Scene or logo image; None for a mask.
    pub fn cue(&self) -> Option<&Arc<PreparedCue>> {
        self.payload.cue()
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

#[derive(Clone, Copy)]
enum Sent {
    Scene(ContentVersion),
    Logo(ContentVersion),
    Mask(Layer),
}

struct Pending {
    stamp: Stamp,
    lane: Lane,
    sent: Sent,
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
    confirmed_logo: Option<(Stamp, ContentVersion)>,
    /// A fresh renderer session starts unmasked by protocol.
    confirmed_mask: (u64, Layer),
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
            confirmed_logo: None,
            confirmed_mask: (0, Layer::None),
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
        self.submit_payload(Payload::Scene(cue), lane, now, deadline)
    }

    /// Replaces the renderer's logo image. Uses the cue lane: it needs the
    /// same preparation as a slide and must not occupy mask capacity.
    pub fn submit_logo(
        &mut self,
        cue: Arc<PreparedCue>,
        now: Instant,
        deadline: Instant,
    ) -> Result<Stamp, DeliveryError> {
        self.submit_payload(Payload::Logo(cue), Lane::Cue, now, deadline)
    }

    pub fn submit_mask(
        &mut self,
        layer: Layer,
        now: Instant,
        deadline: Instant,
    ) -> Result<Stamp, DeliveryError> {
        self.submit_payload(Payload::Mask(layer), Lane::Safety, now, deadline)
    }

    fn submit_payload(
        &mut self,
        payload: Payload,
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
        let sent = match &payload {
            Payload::Scene(cue) => Sent::Scene(cue.version()),
            Payload::Logo(cue) => Sent::Logo(cue.version()),
            Payload::Mask(layer) => Sent::Mask(*layer),
        };
        self.pending.push_back(Pending {
            stamp,
            lane,
            sent,
            deadline,
            command: Some(Command {
                stamp,
                lane,
                payload,
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

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Renderer-acknowledged mask layer; None once the session is not intact.
    pub fn mask(&self) -> Option<Layer> {
        self.connected.then_some(self.confirmed_mask.1)
    }

    /// Renderer-acknowledged logo image; None once the session is not intact.
    pub fn logo(&self) -> Option<ContentVersion> {
        self.confirmed_logo
            .filter(|_| self.connected)
            .map(|(_, version)| version)
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
        if matches!(
            ack.outcome,
            Outcome::Rejected(DeliveryError::WrongEpoch | DeliveryError::Disconnected)
        ) || (ack.outcome == Outcome::Rejected(DeliveryError::Stale)
            && self
                .last_confirmed
                .is_none_or(|(stamp, _)| stamp.sequence < ack.stamp.sequence))
        {
            self.counters.rejected = self.counters.rejected.saturating_add(1);
            self.disconnect();
            return true;
        }
        let pending = self.pending.remove(index).unwrap();
        let newer =
            |current: Option<Stamp>| current.is_none_or(|s| s.sequence < ack.stamp.sequence);
        match ack.outcome {
            Outcome::Applied => {
                match pending.sent {
                    Sent::Scene(version) => {
                        if newer(self.last_confirmed.map(|(s, _)| s)) {
                            self.last_confirmed = Some((ack.stamp, version));
                        }
                    }
                    Sent::Logo(version) => {
                        if newer(self.confirmed_logo.map(|(s, _)| s)) {
                            self.confirmed_logo = Some((ack.stamp, version));
                        }
                    }
                    Sent::Mask(layer) => {
                        if self.confirmed_mask.0 < ack.stamp.sequence {
                            self.confirmed_mask = (ack.stamp.sequence, layer);
                        }
                    }
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
///
/// Sequences are ordered per lane, so a mask accepted while an earlier slide is
/// still being prepared does not make that slide stale.
pub struct RendererSession {
    epoch: Epoch,
    consumed: [u64; 2],
    pending: VecDeque<Command>,
    applied: Option<Arc<PreparedCue>>,
    logo: Option<Arc<PreparedCue>>,
    layer: Layer,
    applied_stamps: [Option<Stamp>; 2],
}

impl RendererSession {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            epoch,
            consumed: [0; 2],
            pending: VecDeque::with_capacity(2),
            applied: None,
            logo: None,
            layer: Layer::None,
            applied_stamps: [None; 2],
        }
    }

    /// A structurally framed command failed renderer-owned resource validation.
    /// Consume its ordering identity without touching pending/applied resources.
    pub fn reject_preparation(&mut self, stamp: Stamp, lane: Lane) -> Acknowledgment {
        self.reject_before_admission(stamp, lane, DeliveryError::RenderFailed)
    }

    /// Bounded preparation queue was saturated. No eviction or retry queue.
    pub fn reject_overload(&mut self, stamp: Stamp, lane: Lane) -> Acknowledgment {
        self.reject_before_admission(stamp, lane, DeliveryError::Busy)
    }

    fn reject_before_admission(
        &mut self,
        stamp: Stamp,
        lane: Lane,
        reason: DeliveryError,
    ) -> Acknowledgment {
        let consumed = &mut self.consumed[lane.index()];
        let error = if stamp.epoch != self.epoch {
            DeliveryError::WrongEpoch
        } else if stamp.sequence <= *consumed {
            DeliveryError::Stale
        } else {
            *consumed = stamp.sequence;
            reason
        };
        Acknowledgment {
            stamp,
            outcome: Outcome::Rejected(error),
        }
    }

    pub fn accept(&mut self, command: Command, now: Instant) -> Acknowledgment {
        let stamp = command.stamp;
        let lane = command.lane.index();
        if self.applied_stamps[lane] == Some(stamp) {
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
        } else if stamp.sequence <= self.consumed[lane] {
            Some(DeliveryError::Stale)
        } else {
            self.consumed[lane] = stamp.sequence;
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
    pub fn logo(&self) -> Option<&Arc<PreparedCue>> {
        self.logo.as_ref()
    }
    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// Check expiry BEFORE touching output. A successful callback means frame
    /// submission, not measured scanout. `now` is the start of this commit attempt.
    pub fn present(
        &mut self,
        now: Instant,
        submit: impl FnOnce(&Payload) -> Result<(), DeliveryError>,
    ) -> Option<Acknowledgment> {
        let command = self.pending.pop_front()?;
        let outcome = if now >= command.deadline {
            Outcome::Rejected(DeliveryError::TimedOut)
        } else if submit(&command.payload).is_err() {
            Outcome::Rejected(DeliveryError::RenderFailed)
        } else {
            self.applied_stamps[command.lane.index()] = Some(command.stamp);
            match command.payload {
                Payload::Scene(cue) => self.applied = Some(cue),
                Payload::Logo(cue) => self.logo = Some(cue),
                Payload::Mask(layer) => self.layer = layer,
            }
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
