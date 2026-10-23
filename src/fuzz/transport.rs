//! The operator-to-renderer pipes (`transport::Frame`, audience and stage
//! processes). Both ends are Sela, but a crashed, stalled or mismatched peer
//! can send anything, and a renderer must turn every byte sequence into a
//! typed error, never a panic or an unbounded allocation.
use super::{config, mutated};
use crate::delivery::{Acknowledgment, Command, Epoch, Lane, Outcome, Payload, Stamp};
use crate::masks::Layer;
use crate::scene::{ContentVersion, Extent, PreparedCue, RendererCapabilities};
use crate::transport::{Frame, MAX_RESOURCE_BODY};
use proptest::prelude::*;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 4096,
};

fn stamp() -> Stamp {
    Stamp {
        epoch: Epoch(7),
        sequence: 3,
    }
}

fn encoded(frame: &Frame) -> Vec<u8> {
    let mut bytes = Vec::new();
    frame.write(&mut bytes).unwrap();
    bytes
}

fn seeds() -> Vec<Vec<u8>> {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(1);
    let mask = Command::from_wire(stamp(), Lane::Safety, Payload::Mask(Layer::Black), deadline);
    let version = ContentVersion { id: 9, revision: 2 };
    let extent = Extent {
        width: 64,
        height: 36,
    };
    let opaque = PreparedCue::diagnostic_color(version, extent, [1, 2, 3, 255], CAPS).unwrap();
    let mut seeds = vec![
        encoded(&Frame::ready(Epoch(7), 4096)),
        encoded(&Frame::surface(Epoch(7), extent)),
        encoded(&Frame::acknowledgment(Acknowledgment {
            stamp: stamp(),
            outcome: Outcome::Accepted,
        })),
        encoded(&Frame::command(&mask, now).unwrap()),
        encoded(
            &Frame::command(
                &Command::from_wire(
                    stamp(),
                    Lane::Cue,
                    Payload::Scene(Arc::new(opaque)),
                    deadline,
                ),
                now,
            )
            .unwrap(),
        ),
    ];
    // A logo travels as an owned-resource (version 2) frame.
    let logo = PreparedCue::diagnostic_color(version, extent, [9, 8, 7, 255], CAPS).unwrap();
    seeds.push(encoded(
        &Frame::command(
            &Command::from_wire(stamp(), Lane::Cue, Payload::Logo(Arc::new(logo)), deadline),
            now,
        )
        .unwrap(),
    ));
    // A stage panel (kind 7, stage-display) carries its text and font.
    let panel = crate::stage::StagePanel {
        current_label: "Song - Verse 1".into(),
        current: "Line one\nLine two".into(),
        next_label: "Next: Chorus".into(),
        next: "Chorus line".into(),
        message: "Two minutes".into(),
        show_clock: true,
        show_next: true,
    };
    let stage = PreparedCue::for_stage(
        version,
        Extent {
            width: 640,
            height: 360,
        },
        crate::stage::BACKGROUND,
        panel,
        (
            ContentVersion { id: 8, revision: 1 },
            Arc::from(crate::fonts::BUNDLED),
            0,
        ),
        CAPS,
    )
    .unwrap();
    seeds.push(encoded(
        &Frame::command(
            &Command::from_wire(
                stamp(),
                Lane::Cue,
                Payload::Scene(Arc::new(stage)),
                deadline,
            ),
            now,
        )
        .unwrap(),
    ));
    seeds
}

/// Every reader of a frame either succeeds or returns an I/O error.
fn check(bytes: &[u8]) {
    let started = Instant::now();
    let mut reader = Cursor::new(bytes);
    // A stream may hold several frames; read until the first error.
    for _ in 0..8 {
        let Ok(frame) = Frame::read(&mut reader) else {
            break;
        };
        let _ = frame.command_stamp();
        let _ = frame.command_lane();
        let _ = frame.is_mask();
        let (ready, surface, ack) = (
            frame.is_ready(),
            frame.is_surface(),
            frame.is_acknowledgment(),
        );
        let written = encoded(&frame);
        assert!(written.len() <= 8 + 4 + MAX_RESOURCE_BODY);
        if ready {
            let _ = Frame::read(Cursor::new(&written)).unwrap().into_ready();
        }
        if surface {
            let _ = Frame::read(Cursor::new(&written)).unwrap().into_surface();
        }
        if ack {
            let _ = Frame::read(Cursor::new(&written))
                .unwrap()
                .into_acknowledgment();
        }
        if let Ok(command) = frame.into_command(Instant::now(), CAPS) {
            if let Payload::Scene(cue) | Payload::Logo(cue) = command.payload() {
                assert!(cue.extent().width <= CAPS.max_texture_dimension);
                assert!(cue.extent().height <= CAPS.max_texture_dimension);
                if let Some(stage) = cue.stage() {
                    assert!(stage.panel().is_valid(), "a stage panel within its bounds");
                }
            }
            // What was accepted is sent on unchanged by the same encoder.
            let _ = Frame::command(&command, Instant::now());
        }
    }
    assert!(started.elapsed() < Duration::from_secs(5));
}

proptest! {
    #![proptest_config(config(0x5e1a_0801, 512, "tests/fixtures/fuzz/transport.proptest-regressions"))]

    #[test]
    fn mutated_frames_are_typed_errors_or_valid_commands(
        bytes in mutated(seeds(), &["SCUE", "\u{1}", "\u{2}", "\u{5}", "\u{6}", "\u{ff}\u{ff}\u{ff}\u{ff}", "\u{0}\u{0}\u{0}\u{0}"]),
    ) {
        check(&bytes);
    }

    #[test]
    fn arbitrary_streams_are_typed_errors(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        check(&bytes);
        let mut framed = b"SCUE".to_vec();
        framed.extend(&bytes);
        check(&framed);
    }
}

#[test]
fn a_declared_resource_body_past_the_limit_is_refused_before_allocation() {
    use std::io::{ErrorKind, Read};
    // The stream holds enough bytes after the header that reading the body
    // would succeed, so only the size check can refuse it: the refusal must
    // be `InvalidData` from the bound, not `UnexpectedEof` from a body read
    // after allocating (review finding: `is_err()` alone could not tell).
    for size in [MAX_RESOURCE_BODY as u32 + 1, u32::MAX] {
        let mut bytes = b"SCUE\x02\x01\x00\x00".to_vec();
        bytes.extend_from_slice(&size.to_le_bytes());
        let stream =
            Cursor::new(bytes).chain(std::io::repeat(0).take(MAX_RESOURCE_BODY as u64 + 2));
        let started = Instant::now();
        let error = Frame::read(stream).err().expect("refused");
        assert_eq!(error.kind(), ErrorKind::InvalidData, "size {size}");
        assert!(started.elapsed() < Duration::from_millis(100));
    }
    // At the limit the same stream is read as a frame (it is then refused
    // later as a resource payload, not here).
    let mut bytes = b"SCUE\x02\x01\x00\x00".to_vec();
    bytes.extend_from_slice(&(MAX_RESOURCE_BODY as u32).to_le_bytes());
    let stream = Cursor::new(bytes).chain(std::io::repeat(0).take(MAX_RESOURCE_BODY as u64));
    assert!(Frame::read(stream).is_ok());
}

#[test]
fn every_seed_is_a_frame_the_renderer_accepts() {
    let mut stages = 0;
    for seed in seeds() {
        let frame = Frame::read(Cursor::new(&seed)).expect("a seed is a frame");
        if frame.command_stamp().is_ok() {
            let command = frame
                .into_command(Instant::now(), CAPS)
                .expect("a seed command is accepted");
            if let Payload::Scene(cue) = command.payload() {
                stages += usize::from(cue.stage().is_some());
            }
        }
    }
    assert_eq!(stages, 1, "the stage panel seed reaches the stage decoder");
}
