//! M0-06b version-1 local pipe diagnostic. Fixed-size owned opaque-color cues
//! only: text/images are explicitly unsupported, never silently stripped.
//! Blocking reads/writes belong to two workers, not the native frame thread.
use crate::{
    delivery::{Acknowledgment, Command, DeliveryError, Epoch, Lane, Outcome, Stamp},
    scene::{ContentVersion, Extent, PreparedBackground, PreparedCue, RendererCapabilities},
};
use std::{
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    time::{Duration, Instant},
};

pub const MAX_BUDGET_MS: u32 = 5_000;
const HEADER: usize = 8;
const COMMAND: u8 = 1;
const ACK: u8 = 2;
const READY: u8 = 3;
const MAX_BODY: usize = 65;

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid cue protocol frame")
}
fn length(kind: u8) -> io::Result<usize> {
    match kind {
        COMMAND => Ok(65),
        ACK => Ok(25),
        READY => Ok(20),
        _ => Err(invalid()),
    }
}

/// Entire wire value is inline, bounded and owned; no native Rust layouts cross
/// the pipe. All integers are little-endian. No payload allocation on read.
pub struct Frame {
    bytes: [u8; HEADER + MAX_BODY],
    len: usize,
    // Receiver-local metadata, NEVER serialized. Includes inbound channel delay
    // in the renderer's relative budget without transferring Instant layouts.
    received_at: Option<Instant>,
}
impl Frame {
    fn new(kind: u8) -> Self {
        let body = length(kind).expect("internal frame kind");
        let mut bytes = [0; HEADER + MAX_BODY];
        bytes[..4].copy_from_slice(b"SCUE");
        bytes[4] = 1;
        bytes[5] = kind;
        bytes[6..8].copy_from_slice(&(body as u16).to_le_bytes());
        Self {
            bytes,
            len: HEADER + body,
            received_at: None,
        }
    }
    pub fn read(mut reader: impl Read) -> io::Result<Self> {
        let mut bytes = [0; HEADER + MAX_BODY];
        reader.read_exact(&mut bytes[..HEADER])?;
        if &bytes[..4] != b"SCUE" || bytes[4] != 1 {
            return Err(invalid());
        }
        let body = length(bytes[5])?;
        if usize::from(u16::from_le_bytes(bytes[6..8].try_into().unwrap())) != body {
            return Err(invalid());
        }
        reader.read_exact(&mut bytes[HEADER..HEADER + body])?;
        Ok(Self {
            bytes,
            len: HEADER + body,
            received_at: Some(Instant::now()),
        })
    }
    pub fn write(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(&self.bytes[..self.len])?;
        writer.flush()
    }
    pub fn command(command: &Command, now: Instant) -> io::Result<Self> {
        let PreparedBackground::Color(color) = command.cue().background() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Image transport not implemented",
            ));
        };
        if command.cue().text().is_some() || color[3] != 255 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Text/alpha transport not implemented",
            ));
        }
        let budget = command
            .deadline()
            .saturating_duration_since(now)
            .as_millis();
        if budget == 0 || budget > u128::from(MAX_BUDGET_MS) {
            return Err(invalid());
        }
        let mut f = Self::new(COMMAND);
        let b = &mut f.bytes[HEADER..];
        put_stamp(b, command.stamp());
        b[24] = match command.lane() {
            Lane::Cue => 0,
            Lane::Safety => 1,
        };
        b[25..29].copy_from_slice(&(budget as u32).to_le_bytes());
        let v = command.cue().version();
        b[29..45].copy_from_slice(&v.id.to_le_bytes());
        b[45..53].copy_from_slice(&v.revision.to_le_bytes());
        let extent = command.cue().extent();
        b[53..57].copy_from_slice(&extent.width.to_le_bytes());
        b[57..61].copy_from_slice(&extent.height.to_le_bytes());
        b[61..65].copy_from_slice(color);
        Ok(f)
    }
    pub fn command_stamp(&self) -> io::Result<Stamp> {
        if self.bytes[5] != COMMAND {
            return Err(invalid());
        }
        let stamp = get_stamp(&self.bytes[HEADER..]);
        if stamp.sequence == 0 {
            return Err(invalid());
        }
        Ok(stamp)
    }
    /// Relative renderer-local budget starts when the worker finishes reading
    /// the frame (including inbound channel delay, but not OS pipe transit).
    /// Controller's original acknowledgment deadline bounds end-to-end uncertainty.
    pub fn into_command(self, now: Instant, caps: RendererCapabilities) -> io::Result<Command> {
        if self.bytes[5] != COMMAND {
            return Err(invalid());
        }
        let b = &self.bytes[HEADER..];
        let stamp = get_stamp(b);
        if stamp.sequence == 0 {
            return Err(invalid());
        }
        let lane = match b[24] {
            0 => Lane::Cue,
            1 => Lane::Safety,
            _ => return Err(invalid()),
        };
        let budget = u32::from_le_bytes(b[25..29].try_into().unwrap());
        if budget == 0 || budget > MAX_BUDGET_MS {
            return Err(invalid());
        }
        let cue = PreparedCue::diagnostic_color(
            ContentVersion {
                id: u128::from_le_bytes(b[29..45].try_into().unwrap()),
                revision: u64::from_le_bytes(b[45..53].try_into().unwrap()),
            },
            Extent {
                width: u32::from_le_bytes(b[53..57].try_into().unwrap()),
                height: u32::from_le_bytes(b[57..61].try_into().unwrap()),
            },
            b[61..65].try_into().unwrap(),
            caps,
        )
        .map_err(|_| invalid())?;
        Ok(Command::from_wire(
            stamp,
            lane,
            Arc::new(cue),
            self.received_at.unwrap_or(now) + Duration::from_millis(u64::from(budget)),
        ))
    }
    pub fn acknowledgment(ack: Acknowledgment) -> Self {
        let mut f = Self::new(ACK);
        put_stamp(&mut f.bytes[HEADER..], ack.stamp);
        f.bytes[HEADER + 24] = match ack.outcome {
            Outcome::Accepted => 0,
            Outcome::Applied => 1,
            Outcome::Rejected(DeliveryError::Busy) => 2,
            Outcome::Rejected(DeliveryError::Disconnected) => 3,
            Outcome::Rejected(DeliveryError::TimedOut) => 4,
            Outcome::Rejected(DeliveryError::WrongEpoch) => 5,
            Outcome::Rejected(DeliveryError::Stale) => 6,
            Outcome::Rejected(DeliveryError::RenderFailed) => 7,
            Outcome::Rejected(DeliveryError::SequenceExhausted) => 8,
        };
        f
    }
    pub fn into_acknowledgment(self) -> io::Result<Acknowledgment> {
        if self.bytes[5] != ACK {
            return Err(invalid());
        }
        let b = &self.bytes[HEADER..];
        let outcome = match b[24] {
            0 => Outcome::Accepted,
            1 => Outcome::Applied,
            2 => Outcome::Rejected(DeliveryError::Busy),
            3 => Outcome::Rejected(DeliveryError::Disconnected),
            4 => Outcome::Rejected(DeliveryError::TimedOut),
            5 => Outcome::Rejected(DeliveryError::WrongEpoch),
            6 => Outcome::Rejected(DeliveryError::Stale),
            7 => Outcome::Rejected(DeliveryError::RenderFailed),
            8 => Outcome::Rejected(DeliveryError::SequenceExhausted),
            _ => return Err(invalid()),
        };
        Ok(Acknowledgment {
            stamp: get_stamp(b),
            outcome,
        })
    }
    /// Renderer advertises its session only after native device/surface setup.
    pub fn ready(epoch: Epoch, max_dimension: u32) -> Self {
        let mut f = Self::new(READY);
        f.bytes[HEADER..HEADER + 16].copy_from_slice(&epoch.0.to_le_bytes());
        f.bytes[HEADER + 16..HEADER + 20].copy_from_slice(&max_dimension.to_le_bytes());
        f
    }
    pub fn into_ready(self) -> io::Result<(Epoch, u32)> {
        if self.bytes[5] != READY {
            return Err(invalid());
        }
        let b = &self.bytes[HEADER..];
        Ok((
            Epoch(u128::from_le_bytes(b[..16].try_into().unwrap())),
            u32::from_le_bytes(b[16..20].try_into().unwrap()),
        ))
    }
}
fn put_stamp(b: &mut [u8], stamp: Stamp) {
    b[..16].copy_from_slice(&stamp.epoch.0.to_le_bytes());
    b[16..24].copy_from_slice(&stamp.sequence.to_le_bytes());
}
fn get_stamp(b: &[u8]) -> Stamp {
    Stamp {
        epoch: Epoch(u128::from_le_bytes(b[..16].try_into().unwrap())),
        sequence: u64::from_le_bytes(b[16..24].try_into().unwrap()),
    }
}

/// Two bounded workers. At most two inbound and four outbound inline frames,
/// plus one executing per worker. No unbounded event/log queue. Dropping this
/// handle does not join potentially stuck OS I/O: owner must terminate/reap the
/// child and retire its pipes before creating another session.
pub struct PipeWorkers {
    outgoing: SyncSender<Frame>,
    incoming: Receiver<Frame>,
    failed: Arc<AtomicBool>,
}
impl PipeWorkers {
    pub fn new(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
    ) -> io::Result<Self> {
        let (outgoing, writes) = mpsc::sync_channel::<Frame>(4);
        let (reads, incoming) = mpsc::sync_channel(2);
        let failed = Arc::new(AtomicBool::new(false));
        let failure = failed.clone();
        std::thread::Builder::new()
            .name("cue-pipe-write".into())
            .spawn(move || {
                let mut writer = writer;
                while let Ok(frame) = writes.recv() {
                    if frame.write(&mut writer).is_err() {
                        failure.store(true, Ordering::Release);
                        break;
                    }
                }
            })?;
        std::thread::Builder::new()
            .name("cue-pipe-read".into())
            .spawn(move || {
                let mut reader = reader;
                while let Ok(frame) = Frame::read(&mut reader) {
                    if reads.send(frame).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            outgoing,
            incoming,
            failed,
        })
    }
    /// Full receipt queue is fatal to a session, not a reason to block rendering
    /// or discard Applied and continue claiming a healthy connection.
    pub fn try_send(&self, frame: Frame) -> io::Result<()> {
        if self.failed.load(Ordering::Acquire) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.outgoing.try_send(frame).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => io::ErrorKind::WouldBlock.into(),
            mpsc::TrySendError::Disconnected(_) => io::ErrorKind::BrokenPipe.into(),
        })
    }
    pub fn poll(&self) -> io::Result<Option<Frame>> {
        if self.failed.load(Ordering::Acquire) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        match self.incoming.try_recv() {
            Ok(frame) => Ok(Some(frame)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::{Delivery, RendererSession};
    fn command() -> Command {
        let now = Instant::now();
        let cue = PreparedCue::diagnostic_color(
            ContentVersion {
                id: 19,
                revision: 23,
            },
            Extent {
                width: 641,
                height: 360,
            },
            [18, 53, 109, 255],
            RendererCapabilities {
                max_texture_dimension: 2048,
            },
        )
        .unwrap();
        let mut d = Delivery::new(Epoch(42));
        d.submit(Arc::new(cue), Lane::Cue, now, now + Duration::from_secs(2))
            .unwrap();
        d.take_next().unwrap()
    }
    #[test]
    fn round_trip_and_failed_resource_preserves_prior() {
        let now = Instant::now();
        let frame = Frame::command(&command(), now).unwrap();
        let mut wire = Vec::new();
        frame.write(&mut wire).unwrap();
        let mut renderer = RendererSession::new(Epoch(42));
        let cmd = Frame::read(wire.as_slice())
            .unwrap()
            .into_command(
                now,
                RendererCapabilities {
                    max_texture_dimension: 2048,
                },
            )
            .unwrap();
        assert_eq!(renderer.accept(cmd, now).outcome, Outcome::Accepted);
        assert_eq!(
            renderer.present(now, |_| Ok(())).unwrap().outcome,
            Outcome::Applied
        );
        assert_eq!(renderer.applied().unwrap().version().revision, 23);
        wire[HEADER + 61 + 3] = 0;
        assert!(
            Frame::read(wire.as_slice())
                .unwrap()
                .into_command(
                    now,
                    RendererCapabilities {
                        max_texture_dimension: 2048
                    }
                )
                .is_err()
        );
        assert_eq!(renderer.applied().unwrap().version().revision, 23);
    }
    #[test]
    fn malformed_short_oversize_and_version() {
        let mut bytes = Vec::new();
        Frame::command(&command(), Instant::now())
            .unwrap()
            .write(&mut bytes)
            .unwrap();
        for len in 0..bytes.len() {
            assert!(Frame::read(&bytes[..len]).is_err());
        }
        for index in [0, 4, 5, 6, 7] {
            let mut bad = bytes.clone();
            bad[index] = 255;
            assert!(Frame::read(bad.as_slice()).is_err());
        }
        for index in [HEADER + 24, HEADER + 28, HEADER + 56] {
            let mut bad = bytes.clone();
            bad[index] = 255;
            assert!(
                Frame::read(bad.as_slice())
                    .unwrap()
                    .into_command(
                        Instant::now(),
                        RendererCapabilities {
                            max_texture_dimension: 2048
                        }
                    )
                    .is_err()
            );
        }
        let mut transparent = bytes.clone();
        transparent[HEADER + 64] = 0;
        assert!(
            Frame::read(transparent.as_slice())
                .unwrap()
                .into_command(
                    Instant::now(),
                    RendererCapabilities {
                        max_texture_dimension: 2048
                    }
                )
                .is_err()
        );
        let mut zero = bytes;
        zero[HEADER + 16..HEADER + 24].fill(0);
        assert!(
            Frame::read(zero.as_slice())
                .unwrap()
                .into_command(
                    Instant::now(),
                    RendererCapabilities {
                        max_texture_dimension: 2048
                    }
                )
                .is_err()
        );
    }
    #[test]
    fn receipt_codes_round_trip() {
        for outcome in [
            Outcome::Accepted,
            Outcome::Applied,
            Outcome::Rejected(DeliveryError::Busy),
            Outcome::Rejected(DeliveryError::TimedOut),
            Outcome::Rejected(DeliveryError::WrongEpoch),
            Outcome::Rejected(DeliveryError::Stale),
            Outcome::Rejected(DeliveryError::RenderFailed),
            Outcome::Rejected(DeliveryError::Disconnected),
            Outcome::Rejected(DeliveryError::SequenceExhausted),
        ] {
            let ack = Acknowledgment {
                stamp: Stamp {
                    epoch: Epoch(42),
                    sequence: 17,
                },
                outcome,
            };
            assert_eq!(
                Frame::acknowledgment(ack).into_acknowledgment().unwrap(),
                ack
            );
        }
    }

    #[test]
    fn inbound_queue_delay_does_not_restart_renderer_budget() {
        let now = Instant::now();
        let mut bytes = Vec::new();
        Frame::command(&command(), now)
            .unwrap()
            .write(&mut bytes)
            .unwrap();
        let frame = Frame::read(bytes.as_slice()).unwrap();
        let received = frame.received_at.unwrap();
        let later = received + Duration::from_secs(6);
        let cmd = frame
            .into_command(
                later,
                RendererCapabilities {
                    max_texture_dimension: 2048,
                },
            )
            .unwrap();
        let mut session = RendererSession::new(Epoch(42));
        assert_eq!(
            session.accept(cmd, later).outcome,
            Outcome::Rejected(DeliveryError::TimedOut)
        );
        assert!(session.pending().is_none());
    }

    #[test]
    fn pipe_writer_saturation_is_bounded_and_nonblocking() {
        struct GatedWriter {
            entered: Option<SyncSender<()>>,
            release: Receiver<()>,
        }
        impl Write for GatedWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if let Some(entered) = self.entered.take() {
                    entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        struct GatedReader(Receiver<()>);
        impl Read for GatedReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                self.0.recv().unwrap();
                Ok(0)
            }
        }
        let (entered, waiting) = mpsc::sync_channel(1);
        let (release, gated) = mpsc::sync_channel(1);
        let (read_release, read_gate) = mpsc::sync_channel(1);
        let workers = PipeWorkers::new(
            GatedReader(read_gate),
            GatedWriter {
                entered: Some(entered),
                release: gated,
            },
        )
        .unwrap();
        workers.try_send(Frame::ready(Epoch(42), 2048)).unwrap();
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..4 {
            workers.try_send(Frame::ready(Epoch(42), 2048)).unwrap();
        }
        for _ in 0..1000 {
            assert_eq!(
                workers
                    .try_send(Frame::ready(Epoch(42), 2048))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
        }
        release.send(()).unwrap();
        read_release.send(()).unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        while workers.poll().is_ok() {
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
    }

    #[test]
    fn unsupported_owned_resources_are_not_silently_dropped() {
        use crate::scene::{self, BackgroundSpec, ResourceRef, SceneSpec, TextSpec};
        use sha2::{Digest, Sha256};
        let now = Instant::now();
        let caps = RendererCapabilities {
            max_texture_dimension: 2048,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([17, 53, 99, 255]))
            .save(&path)
            .unwrap();
        let version = ContentVersion {
            id: 19,
            revision: 23,
        };
        let image = ResourceRef {
            version,
            sha256: Sha256::digest(std::fs::read(&path).unwrap()).into(),
            path,
        };
        let font_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DejaVuSans.ttf");
        let font = ResourceRef {
            version,
            sha256: Sha256::digest(std::fs::read(&font_path).unwrap()).into(),
            path: font_path,
        };
        for (background, text) in [
            (BackgroundSpec::Image(image), None),
            (
                BackgroundSpec::Color([17, 53, 99, 255]),
                Some(TextSpec {
                    content: "Original diagnostic text".into(),
                    font,
                    font_size: 40,
                }),
            ),
        ] {
            let cue = scene::prepare(
                SceneSpec {
                    version,
                    extent: Extent {
                        width: 641,
                        height: 360,
                    },
                    background,
                    text,
                },
                caps,
                &AtomicBool::new(false),
            )
            .unwrap();
            assert!(cue.resource_bytes() > 0);
            let command = Command::from_wire(
                Stamp {
                    epoch: Epoch(42),
                    sequence: 1,
                },
                Lane::Cue,
                Arc::new(cue),
                now + Duration::from_secs(2),
            );
            assert_eq!(
                Frame::command(&command, now).err().unwrap().kind(),
                io::ErrorKind::Unsupported
            );
        }
    }
}
