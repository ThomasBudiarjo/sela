//! Bounded local pipe diagnostic. Version 1 colors and version 2 owned resources.
//! Blocking reads/writes belong to two workers, not the native frame thread.
use crate::{
    delivery::{Acknowledgment, Command, DeliveryError, Epoch, Lane, Outcome, Payload, Stamp},
    masks::Layer,
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
const SURFACE: u8 = 4;
/// Version 1 only: stamp, lane, budget, layer. Never waits for preparation.
const MASK: u8 = 5;
/// Version 2 only: same body as an owned-resource COMMAND, aimed at the logo.
const LOGO: u8 = 6;
const MAX_BODY: usize = 65;
pub const MAX_RESOURCE_BODY: usize = crate::scene::MAX_SCENE_BYTES + 160;

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid cue protocol frame")
}
fn length(kind: u8) -> io::Result<usize> {
    match kind {
        COMMAND => Ok(65),
        ACK => Ok(25),
        READY => Ok(20),
        SURFACE => Ok(24),
        MASK => Ok(30),
        _ => Err(invalid()),
    }
}

/// Bounded owned wire value; no native Rust layouts cross the pipe. Resource
/// allocation follows a checked declaration. Read/construct on workers only.
pub struct Frame {
    bytes: Vec<u8>,
    len: usize,
    // Receiver-local metadata, NEVER serialized. Includes inbound channel delay
    // in the renderer's relative budget without transferring Instant layouts.
    received_at: Option<Instant>,
}
impl Frame {
    fn new(kind: u8) -> Self {
        let body = length(kind).expect("internal frame kind");
        let mut bytes = vec![0; HEADER + MAX_BODY];
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
        if &bytes[..4] != b"SCUE" {
            return Err(invalid());
        }
        if bytes[4] == 2 {
            if !matches!(bytes[5], COMMAND | LOGO) || bytes[6..8] != [0, 0] {
                return Err(invalid());
            }
            let received_at = Instant::now();
            let mut size = [0; 4];
            reader.read_exact(&mut size)?;
            let size = u32::from_le_bytes(size) as usize;
            if !(67..=MAX_RESOURCE_BODY).contains(&size) {
                return Err(invalid());
            }
            let mut owned = vec![0; HEADER + size];
            owned[..HEADER].copy_from_slice(&bytes[..HEADER]);
            reader.read_exact(&mut owned[HEADER..])?;
            return Ok(Self {
                bytes: owned,
                len: HEADER + size,
                received_at: Some(received_at),
            });
        }
        if bytes[4] != 1 {
            return Err(invalid());
        }
        let body = length(bytes[5])?;
        if usize::from(u16::from_le_bytes(bytes[6..8].try_into().unwrap())) != body {
            return Err(invalid());
        }
        reader.read_exact(&mut bytes[HEADER..HEADER + body])?;
        Ok(Self {
            bytes: bytes.to_vec(),
            len: HEADER + body,
            received_at: Some(Instant::now()),
        })
    }
    pub fn write(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(&self.bytes[..HEADER])?;
        if self.bytes[4] == 2 {
            writer.write_all(&((self.len - HEADER) as u32).to_le_bytes())?;
        }
        writer.write_all(&self.bytes[HEADER..self.len])?;
        writer.flush()
    }
    pub fn command(command: &Command, now: Instant) -> io::Result<Self> {
        let cue = match command.payload() {
            Payload::Mask(layer) => return Self::mask(command, *layer, now),
            Payload::Logo(cue) => return Self::resource_command(command, cue, LOGO, now),
            Payload::Scene(cue) => cue,
        };
        if cue.text().is_some() || matches!(cue.background(), PreparedBackground::Image { .. }) {
            return Self::resource_command(command, cue, COMMAND, now);
        }
        let PreparedBackground::Color(color) = cue.background() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Image transport not implemented",
            ));
        };
        if color[3] != 255 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Alpha transport not implemented",
            ));
        }
        let budget = budget(command, now)?;
        let mut f = Self::new(COMMAND);
        let b = &mut f.bytes[HEADER..];
        put_stamp(b, command.stamp());
        b[24] = lane_byte(command.lane());
        b[25..29].copy_from_slice(&budget.to_le_bytes());
        let v = cue.version();
        b[29..45].copy_from_slice(&v.id.to_le_bytes());
        b[45..53].copy_from_slice(&v.revision.to_le_bytes());
        let extent = cue.extent();
        b[53..57].copy_from_slice(&extent.width.to_le_bytes());
        b[57..61].copy_from_slice(&extent.height.to_le_bytes());
        b[61..65].copy_from_slice(color);
        Ok(f)
    }
    fn mask(command: &Command, layer: Layer, now: Instant) -> io::Result<Self> {
        let budget = budget(command, now)?;
        let mut f = Self::new(MASK);
        let b = &mut f.bytes[HEADER..];
        put_stamp(b, command.stamp());
        b[24] = lane_byte(command.lane());
        b[25..29].copy_from_slice(&budget.to_le_bytes());
        b[29] = match layer {
            Layer::None => 0,
            Layer::Clear => 1,
            Layer::Black => 2,
            Layer::Logo => 3,
        };
        Ok(f)
    }
    fn resource_command(
        command: &Command,
        cue: &PreparedCue,
        kind: u8,
        now: Instant,
    ) -> io::Result<Self> {
        let budget = budget(command, now)?;
        let mut f = Self::new(COMMAND);
        f.bytes.reserve_exact(cue.resource_bytes() + 160);
        f.bytes[4] = 2;
        f.bytes[5] = kind;
        f.bytes[6..8].fill(0);
        f.bytes.truncate(HEADER + 61);
        let b = &mut f.bytes[HEADER..];
        put_stamp(b, command.stamp());
        b[24] = lane_byte(command.lane());
        b[25..29].copy_from_slice(&budget.to_le_bytes());
        b[29..45].copy_from_slice(&cue.version().id.to_le_bytes());
        b[45..53].copy_from_slice(&cue.version().revision.to_le_bytes());
        b[53..57].copy_from_slice(&cue.extent().width.to_le_bytes());
        b[57..61].copy_from_slice(&cue.extent().height.to_le_bytes());
        match cue.background() {
            PreparedBackground::Color(color) => {
                f.bytes.push(0);
                f.bytes.extend_from_slice(color);
            }
            PreparedBackground::Image {
                version,
                extent,
                rgba,
            } => {
                f.bytes.push(1);
                put_version(&mut f.bytes, *version);
                f.bytes.extend_from_slice(&extent.width.to_le_bytes());
                f.bytes.extend_from_slice(&extent.height.to_le_bytes());
                put_blob(&mut f.bytes, rgba);
            }
        }
        if let Some(text) = cue.text() {
            let style = text.style();
            f.bytes.push(2); // Styled owned text (tag 1 was removed in M1-05g2).
            put_version(&mut f.bytes, text.font_version());
            f.bytes.extend_from_slice(&text.face_index().to_le_bytes());
            f.bytes.extend_from_slice(&text.font_size().to_le_bytes());
            f.bytes.extend_from_slice(&style.color);
            f.bytes.push(style.align as u8);
            f.bytes.push(style.valign as u8);
            let mut flags = 0u8;
            if style.underline {
                flags |= 1;
            }
            if style.italic {
                flags |= 2;
            }
            if style.synth_bold {
                flags |= 4;
            }
            if style.synth_italic {
                flags |= 8;
            }
            f.bytes.push(flags);
            match style.outline {
                Some(outline) => {
                    f.bytes.push(1);
                    f.bytes.extend_from_slice(&outline.color);
                    f.bytes.extend_from_slice(&[outline.size, outline.opacity]);
                }
                None => f.bytes.push(0),
            }
            match style.shadow {
                Some(shadow) => {
                    f.bytes.push(1);
                    f.bytes.extend_from_slice(&shadow.color);
                    f.bytes.extend_from_slice(&shadow.angle.to_le_bytes());
                    f.bytes
                        .extend_from_slice(&[shadow.offset, shadow.blur, shadow.opacity]);
                }
                None => f.bytes.push(0),
            }
            put_blob(&mut f.bytes, text.content().as_bytes());
            put_blob(&mut f.bytes, text.font());
        } else {
            f.bytes.push(0);
        }
        f.len = f.bytes.len();
        if f.len - HEADER > MAX_RESOURCE_BODY {
            return Err(invalid());
        }
        Ok(f)
    }
    fn is_command_kind(&self) -> bool {
        matches!(self.bytes[5], COMMAND | MASK | LOGO)
    }
    pub fn command_stamp(&self) -> io::Result<Stamp> {
        if !self.is_command_kind() {
            return Err(invalid());
        }
        let stamp = get_stamp(&self.bytes[HEADER..]);
        if stamp.sequence == 0 {
            return Err(invalid());
        }
        Ok(stamp)
    }
    pub fn command_lane(&self) -> io::Result<Lane> {
        if !self.is_command_kind() {
            return Err(invalid());
        }
        match self.bytes[HEADER + 24] {
            0 => Ok(Lane::Cue),
            1 => Ok(Lane::Safety),
            _ => Err(invalid()),
        }
    }
    /// Masks carry no resources and bypass the renderer's preparation worker.
    pub fn is_mask(&self) -> bool {
        self.bytes[4] == 1 && self.bytes[5] == MASK
    }
    /// Relative budget starts after v1 read, or before v2 length/body transfer.
    /// Queue/preparation delay never restarts it; pre-header transit is excluded.
    /// Controller's original acknowledgment deadline bounds end-to-end uncertainty.
    pub fn into_command(self, now: Instant, caps: RendererCapabilities) -> io::Result<Command> {
        if !self.is_command_kind() {
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
        let deadline = self.received_at.unwrap_or(now) + Duration::from_millis(u64::from(budget));
        if self.bytes[5] == MASK {
            let layer = match b[29] {
                0 => Layer::None,
                1 => Layer::Clear,
                2 => Layer::Black,
                3 => Layer::Logo,
                _ => return Err(invalid()),
            };
            return Ok(Command::from_wire(
                stamp,
                lane,
                Payload::Mask(layer),
                deadline,
            ));
        }
        let version = ContentVersion {
            id: u128::from_le_bytes(b[29..45].try_into().unwrap()),
            revision: u64::from_le_bytes(b[45..53].try_into().unwrap()),
        };
        let extent = Extent {
            width: u32::from_le_bytes(b[53..57].try_into().unwrap()),
            height: u32::from_le_bytes(b[57..61].try_into().unwrap()),
        };
        let cue = if self.bytes[4] == 2 {
            let mut payload = Reader(&b[61..self.len - HEADER]);
            let background = match payload.byte()? {
                0 => PreparedBackground::Color(payload.take(4)?.try_into().unwrap()),
                1 => PreparedBackground::Image {
                    version: payload.version()?,
                    extent: Extent {
                        width: payload.u32()?,
                        height: payload.u32()?,
                    },
                    rgba: payload.blob(crate::scene::MAX_SCENE_BYTES)?.into(),
                },
                _ => return Err(invalid()),
            };
            let text = match payload.byte()? {
                0 => None,
                // Tag 1 (unstyled text) was removed with M1-05g2; both ends of
                // the pipe build from the same revision, so reject it outright.
                1 => return Err(invalid()),
                2 => {
                    let version = payload.version()?;
                    let face_index = payload.u32()?;
                    let size = u16::from_le_bytes(payload.take(2)?.try_into().unwrap());
                    let color: [u8; 3] = payload.take(3)?.try_into().unwrap();
                    let align = match payload.byte()? {
                        0 => crate::format::Align::Left,
                        1 => crate::format::Align::Center,
                        2 => crate::format::Align::Right,
                        _ => return Err(invalid()),
                    };
                    let valign = match payload.byte()? {
                        0 => crate::format::VAlign::Top,
                        1 => crate::format::VAlign::Middle,
                        2 => crate::format::VAlign::Bottom,
                        _ => return Err(invalid()),
                    };
                    let flags = payload.byte()?;
                    if flags > 0b1111 {
                        return Err(invalid());
                    }
                    let outline = match payload.byte()? {
                        0 => None,
                        1 => Some(crate::scene::OutlineStyle {
                            color: payload.take(3)?.try_into().unwrap(),
                            size: payload.byte()?,
                            opacity: payload.byte()?,
                        }),
                        _ => return Err(invalid()),
                    };
                    let shadow = match payload.byte()? {
                        0 => None,
                        1 => Some(crate::scene::ShadowStyle {
                            color: payload.take(3)?.try_into().unwrap(),
                            angle: u16::from_le_bytes(payload.take(2)?.try_into().unwrap()),
                            offset: payload.byte()?,
                            blur: payload.byte()?,
                            opacity: payload.byte()?,
                        }),
                        _ => return Err(invalid()),
                    };
                    let content = std::str::from_utf8(payload.blob(crate::scene::MAX_TEXT_BYTES)?)
                        .map_err(|_| invalid())?
                        .to_owned();
                    let font = payload.blob(crate::scene::MAX_SOURCE_BYTES)?.into();
                    Some(crate::scene::OwnedText {
                        content,
                        font_version: version,
                        font,
                        face_index,
                        font_size: size,
                        style: crate::scene::TextStyle {
                            color,
                            align,
                            valign,
                            underline: flags & 1 != 0,
                            italic: flags & 2 != 0,
                            synth_bold: flags & 4 != 0,
                            synth_italic: flags & 8 != 0,
                            outline,
                            shadow,
                        },
                    })
                }
                _ => return Err(invalid()),
            };
            if !payload.0.is_empty() {
                return Err(invalid());
            }
            PreparedCue::from_owned(version, extent, background, text, caps)
        } else {
            PreparedCue::diagnostic_color(version, extent, b[61..65].try_into().unwrap(), caps)
        }
        .map_err(|_| invalid())?;
        let cue = Arc::new(cue);
        let payload = if self.bytes[5] == LOGO {
            Payload::Logo(cue)
        } else {
            Payload::Scene(cue)
        };
        Ok(Command::from_wire(stamp, lane, payload, deadline))
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
    /// Renderer's current surface extent, after Ready and on every change.
    /// Cues must match it exactly; this is not a presented-scene receipt.
    pub fn surface(epoch: Epoch, extent: Extent) -> Self {
        let mut f = Self::new(SURFACE);
        let b = &mut f.bytes[HEADER..];
        b[..16].copy_from_slice(&epoch.0.to_le_bytes());
        b[16..20].copy_from_slice(&extent.width.to_le_bytes());
        b[20..24].copy_from_slice(&extent.height.to_le_bytes());
        f
    }
    pub fn into_surface(self) -> io::Result<(Epoch, Extent)> {
        if self.bytes[5] != SURFACE {
            return Err(invalid());
        }
        let b = &self.bytes[HEADER..];
        let extent = Extent {
            width: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            height: u32::from_le_bytes(b[20..24].try_into().unwrap()),
        };
        if extent.width == 0 || extent.height == 0 {
            return Err(invalid());
        }
        Ok((
            Epoch(u128::from_le_bytes(b[..16].try_into().unwrap())),
            extent,
        ))
    }
    pub fn is_ready(&self) -> bool {
        self.bytes[4] == 1 && self.bytes[5] == READY
    }
    pub fn is_surface(&self) -> bool {
        self.bytes[4] == 1 && self.bytes[5] == SURFACE
    }
    pub fn is_acknowledgment(&self) -> bool {
        self.bytes[4] == 1 && self.bytes[5] == ACK
    }
}
fn budget(command: &Command, now: Instant) -> io::Result<u32> {
    let budget = command
        .deadline()
        .saturating_duration_since(now)
        .as_millis();
    if budget == 0 || budget > u128::from(MAX_BUDGET_MS) {
        return Err(invalid());
    }
    Ok(budget as u32)
}
fn lane_byte(lane: Lane) -> u8 {
    match lane {
        Lane::Cue => 0,
        Lane::Safety => 1,
    }
}
fn put_version(bytes: &mut Vec<u8>, version: ContentVersion) {
    bytes.extend_from_slice(&version.id.to_le_bytes());
    bytes.extend_from_slice(&version.revision.to_le_bytes());
}
fn put_blob(bytes: &mut Vec<u8>, blob: &[u8]) {
    bytes.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    bytes.extend_from_slice(blob);
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        if count > self.0.len() {
            return Err(invalid());
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn version(&mut self) -> io::Result<ContentVersion> {
        Ok(ContentVersion {
            id: u128::from_le_bytes(self.take(16)?.try_into().unwrap()),
            revision: u64::from_le_bytes(self.take(8)?.try_into().unwrap()),
        })
    }
    fn blob(&mut self, limit: usize) -> io::Result<&'a [u8]> {
        let size = self.u32()? as usize;
        if size > limit {
            return Err(invalid());
        }
        self.take(size)
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

/// Two bounded workers. At most two inbound and four outbound owned frames,
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
    fn surface_report_round_trip_and_rejection() {
        let extent = Extent {
            width: 2560,
            height: 1600,
        };
        let mut wire = Vec::new();
        Frame::surface(Epoch(7), extent).write(&mut wire).unwrap();
        assert_eq!(wire.len(), HEADER + 24);
        let frame = Frame::read(wire.as_slice()).unwrap();
        assert!(frame.is_surface() && !frame.is_ready() && !frame.is_acknowledgment());
        assert_eq!(frame.into_surface().unwrap(), (Epoch(7), extent));
        for len in 0..wire.len() {
            assert!(Frame::read(&wire[..len]).is_err());
        }
        let mut length = wire.clone();
        length[6] = 20;
        assert!(Frame::read(length.as_slice()).is_err());
        let mut zero = wire.clone();
        zero[HEADER + 16..HEADER + 20].fill(0);
        assert!(
            Frame::read(zero.as_slice())
                .unwrap()
                .into_surface()
                .is_err()
        );
        // A surface report is never a receipt, Ready or command.
        let frame = || Frame::read(wire.as_slice()).unwrap();
        assert!(frame().into_ready().is_err());
        assert!(frame().into_acknowledgment().is_err());
        assert!(frame().command_stamp().is_err());
        assert!(Frame::ready(Epoch(7), 2048).into_surface().is_err());
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
    fn owned_resources_round_trip_without_paths() {
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
                Payload::Scene(Arc::new(cue)),
                now + Duration::from_secs(2),
            );
            let mut wire = Vec::new();
            Frame::command(&command, now)
                .unwrap()
                .write(&mut wire)
                .unwrap();
            let decoded = Frame::read(wire.as_slice())
                .unwrap()
                .into_command(now, caps)
                .unwrap();
            let (decoded_cue, command_cue) = (decoded.cue().unwrap(), command.cue().unwrap());
            assert_eq!(decoded_cue.version(), version);
            assert_eq!(decoded_cue.resource_bytes(), command_cue.resource_bytes());
            let queued = Frame::read(wire.as_slice()).unwrap();
            let later = queued.received_at.unwrap() + Duration::from_secs(6);
            let mut renderer = RendererSession::new(Epoch(42));
            assert_eq!(
                renderer
                    .accept(queued.into_command(later, caps).unwrap(), later)
                    .outcome,
                Outcome::Rejected(DeliveryError::TimedOut)
            );
            if let Some(text) = decoded_cue.text() {
                assert_eq!(text.content(), "Original diagnostic text");
                assert_eq!(text.font(), command_cue.text().unwrap().font());
            } else {
                let PreparedBackground::Image { rgba, .. } = decoded_cue.background() else {
                    panic!("lost image")
                };
                assert_eq!(rgba.as_ref(), &[17, 53, 99, 255]);
            }
            for cut in [8, 11, 12, wire.len() - 1] {
                assert!(Frame::read(&wire[..cut]).is_err());
            }
            let mut oversize = wire.clone();
            oversize[8..12].copy_from_slice(&((MAX_RESOURCE_BODY + 1) as u32).to_le_bytes());
            assert!(Frame::read(oversize.as_slice()).is_err());
            let mut trailing = wire;
            let size = u32::from_le_bytes(trailing[8..12].try_into().unwrap()) + 1;
            trailing[8..12].copy_from_slice(&size.to_le_bytes());
            trailing.push(0);
            assert!(
                Frame::read(trailing.as_slice())
                    .unwrap()
                    .into_command(now, caps)
                    .is_err()
            );
        }
    }

    #[test]
    fn owned_resource_validation_and_aggregate_budget() {
        use crate::scene::{
            MAX_SCENE_BYTES, MAX_SOURCE_BYTES, MAX_TEXT_BYTES, OwnedText, PrepareError, TextStyle,
        };
        let version = ContentVersion {
            id: 71,
            revision: 13,
        };
        let extent = Extent {
            width: 641,
            height: 360,
        };
        let caps = RendererCapabilities {
            max_texture_dimension: 8192,
        };
        let text = |content: String, font: Vec<u8>, style: TextStyle| {
            Some(OwnedText {
                content,
                font_version: version,
                font: font.into(),
                face_index: 0,
                font_size: 32,
                style,
            })
        };
        let construct =
            |background, text| PreparedCue::from_owned(version, extent, background, text, caps);
        for (font, content, expected) in [
            (
                b"OTTOgarbage".to_vec(),
                "Signal".to_owned(),
                PrepareError::InvalidFont,
            ),
            (
                vec![0; MAX_SOURCE_BYTES + 1],
                "Signal".to_owned(),
                PrepareError::TooLarge,
            ),
            (
                vec![],
                "A".repeat(MAX_TEXT_BYTES + 1),
                PrepareError::TooLarge,
            ),
        ] {
            assert_eq!(
                construct(
                    PreparedBackground::Color([17, 53, 99, 255]),
                    text(content, font, TextStyle::default())
                )
                .err(),
                Some(expected)
            );
        }
        let font = include_bytes!("../tests/fixtures/DejaVuSans.ttf");
        // A face index the font does not have is refused, not wrapped around.
        let bad_index = OwnedText {
            content: "Signal".into(),
            font_version: version,
            font: font.as_slice().into(),
            face_index: 1,
            font_size: 32,
            style: TextStyle::default(),
        };
        assert_eq!(
            construct(
                PreparedBackground::Color([17, 53, 99, 255]),
                Some(bad_index)
            )
            .err(),
            Some(PrepareError::InvalidFont)
        );
        // Out-of-range style values are refused even though the face is fine.
        for style in [
            TextStyle {
                outline: Some(crate::scene::OutlineStyle {
                    color: [0, 0, 0],
                    size: 51,
                    opacity: 100,
                }),
                ..TextStyle::default()
            },
            TextStyle {
                outline: Some(crate::scene::OutlineStyle {
                    color: [0, 0, 0],
                    size: 7,
                    opacity: 101,
                }),
                ..TextStyle::default()
            },
            TextStyle {
                shadow: Some(crate::scene::ShadowStyle {
                    color: [0, 0, 0],
                    angle: 360,
                    offset: 18,
                    blur: 9,
                    opacity: 90,
                }),
                ..TextStyle::default()
            },
            TextStyle {
                shadow: Some(crate::scene::ShadowStyle {
                    color: [0, 0, 0],
                    angle: 315,
                    offset: 18,
                    blur: 51,
                    opacity: 90,
                }),
                ..TextStyle::default()
            },
        ] {
            assert_eq!(
                construct(
                    PreparedBackground::Color([17, 53, 99, 255]),
                    text("Signal".to_owned(), font.to_vec(), style)
                )
                .err(),
                Some(PrepareError::InvalidScene)
            );
        }
        assert_eq!(
            construct(
                PreparedBackground::Image {
                    version,
                    extent: Extent {
                        width: 3,
                        height: 2
                    },
                    rgba: vec![0; 23].into(),
                },
                None
            )
            .err(),
            Some(PrepareError::InvalidImage)
        );
        assert_eq!(
            construct(
                PreparedBackground::Image {
                    version,
                    extent: Extent {
                        width: 4096,
                        height: 4096
                    },
                    rgba: vec![0; MAX_SCENE_BYTES].into(),
                },
                text("Signal".to_owned(), font.to_vec(), TextStyle::default())
            )
            .err(),
            Some(PrepareError::TooLarge)
        );
    }

    #[test]
    fn styled_text_round_trips_and_rejects_bad_wire_values() {
        let now = Instant::now();
        let caps = RendererCapabilities {
            max_texture_dimension: 8192,
        };
        let version = ContentVersion {
            id: 19,
            revision: 23,
        };
        let extent = Extent {
            width: 641,
            height: 360,
        };
        let format = crate::format::tests::full();
        let resolved = crate::fonts::Resolved::bundled(&format);
        let slide = crate::slides::Slide {
            label: String::new(),
            text: "Styled wire refrain".into(),
            format,
        };
        let cue =
            Arc::new(crate::slides::cue(version, &slide, &resolved, extent, caps, None).unwrap());
        assert!(cue.text().is_some());
        let command = Command::from_wire(
            Stamp {
                epoch: Epoch(42),
                sequence: 1,
            },
            Lane::Cue,
            Payload::Scene(cue.clone()),
            now + Duration::from_secs(2),
        );
        let mut wire = Vec::new();
        Frame::command(&command, now)
            .unwrap()
            .write(&mut wire)
            .unwrap();
        let decoded = Frame::read(wire.as_slice())
            .unwrap()
            .into_command(now, caps)
            .unwrap();
        let (decoded_text, cue_text) =
            (decoded.cue().unwrap().text().unwrap(), cue.text().unwrap());
        assert_eq!(decoded_text.content(), cue_text.content());
        assert_eq!(decoded_text.font(), cue_text.font());
        assert_eq!(decoded_text.font_version(), cue_text.font_version());
        assert_eq!(decoded_text.face_index(), cue_text.face_index());
        assert_eq!(decoded_text.font_size(), cue_text.font_size());
        assert_eq!(decoded_text.style(), cue_text.style());
        // Stream wire: 8-byte header, 4-byte resource length, the fixed
        // 61-byte body (stamp, lane, budget, version, extent), a color
        // background, then the text section: tag, version, face index, size,
        // color, align, valign, flags, outline, shadow, then the blobs.
        let text = HEADER + 4 + 61 + 5;
        let [align, valign, flags, outline_size, shadow_blur] =
            [text + 34, text + 35, text + 36, text + 41, text + 50];
        for (at, value, what) in [
            (text, 1u8, "removed unstyled tag"),
            (text, 3, "unknown text tag"),
            (align, 3, "align"),
            (valign, 3, "valign"),
            (flags, 16, "flag bits"),
            (outline_size, 51, "outline size"),
            (shadow_blur, 51, "shadow blur"),
        ] {
            let mut bad = wire.clone();
            bad[at] = value;
            assert!(
                Frame::read(bad.as_slice())
                    .unwrap()
                    .into_command(now, caps)
                    .is_err(),
                "{what} must be rejected"
            );
        }
        for cut in [HEADER, text, text + 40, wire.len() - 1] {
            assert!(Frame::read(&wire[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn preparation_overload_consumes_identity_without_replacing_live() {
        let now = Instant::now();
        let mut renderer = RendererSession::new(Epoch(42));
        renderer.accept(command(), now);
        renderer.present(now, |_| Ok(())).unwrap();
        for sequence in 2..1002 {
            let stamp = Stamp {
                epoch: Epoch(42),
                sequence,
            };
            assert_eq!(
                renderer.reject_overload(stamp, Lane::Cue).outcome,
                Outcome::Rejected(DeliveryError::Busy)
            );
            assert_eq!(renderer.applied().unwrap().version().revision, 23);
            assert!(renderer.pending().is_none());
        }
        assert_eq!(
            renderer
                .reject_overload(
                    Stamp {
                        epoch: Epoch(42),
                        sequence: 1001
                    },
                    Lane::Cue
                )
                .outcome,
            Outcome::Rejected(DeliveryError::Stale)
        );
        assert_eq!(
            renderer
                .reject_overload(
                    Stamp {
                        epoch: Epoch(99),
                        sequence: 1002
                    },
                    Lane::Cue
                )
                .outcome,
            Outcome::Rejected(DeliveryError::WrongEpoch)
        );
    }
}
