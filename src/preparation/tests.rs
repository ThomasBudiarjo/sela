use super::*;
use crate::scene::*;
use image::ImageEncoder;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, sync::mpsc};

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 8192,
};
const FONT: &[u8] = include_bytes!("../../tests/fixtures/DejaVuSans.ttf");

fn scene(revision: u64) -> SceneSpec {
    SceneSpec {
        version: ContentVersion { id: 47, revision },
        extent: Extent {
            width: 1280,
            height: 720,
        },
        background: BackgroundSpec::Color([18, 53, 109, 255]),
        text: None,
    }
}
fn resource(path: &Path, data: &[u8]) -> ResourceRef {
    fs::write(path, data).unwrap();
    ResourceRef {
        version: ContentVersion {
            id: 63,
            revision: 5,
        },
        path: path.into(),
        sha256: Sha256::digest(data).into(),
    }
}
fn png() -> (Vec<u8>, Vec<u8>) {
    let pixels = vec![
        255, 0, 0, 255, 0, 90, 0, 128, 0, 0, 200, 0, 17, 23, 41, 255, 51, 61, 71, 32, 81, 91, 101,
        210,
    ];
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(&pixels, 3, 2, image::ExtendedColorType::Rgba8)
        .unwrap();
    (encoded, pixels)
}
fn wait(preparer: &mut Preparer, clock: Instant) -> PreparationEvent {
    let watchdog = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(event) = preparer.poll(clock) {
            return event;
        }
        assert!(
            Instant::now() < watchdog,
            "worker completion watchdog elapsed"
        );
        std::thread::yield_now();
    }
}
fn ready(event: PreparationEvent) -> (RequestId, Arc<PreparedCue>) {
    match event {
        PreparationEvent::Ready { request, cue } => (request, cue),
        PreparationEvent::Failed { error, .. } => panic!("unexpected preparation failure: {error}"),
    }
}
fn fail(spec: SceneSpec, error: PrepareError) {
    assert_eq!(
        scene::prepare(spec, CAPS, &AtomicBool::new(false)).err(),
        Some(error)
    );
}

#[test]
fn real_worker_decodes_asymmetric_rgba_and_keeps_snapshot_after_file_removal() {
    let directory = tempfile::tempdir().unwrap();
    let (encoded, pixels) = png();
    let image = resource(&directory.path().join("background.png"), &encoded);
    let mut draft = scene(3);
    draft.background = BackgroundSpec::Image(image.clone());
    let mut preparer = Preparer::new(CAPS).unwrap();
    let now = Instant::now();
    let request = preparer
        .request(draft.clone(), now, Duration::from_secs(5))
        .unwrap();
    draft.version.revision = 4;
    draft.background = BackgroundSpec::Color([0; 4]);
    let (actual, cue) = ready(wait(&mut preparer, now));
    assert_eq!(actual, request);
    fs::remove_file(image.path).unwrap();
    assert_eq!(
        cue.version(),
        ContentVersion {
            id: 47,
            revision: 3
        }
    );
    assert_eq!(cue.resource_bytes(), 24);
    match cue.background() {
        PreparedBackground::Image {
            version,
            extent,
            rgba,
        } => {
            assert_eq!(*version, image.version);
            assert_eq!(
                *extent,
                Extent {
                    width: 3,
                    height: 2
                }
            );
            assert_eq!(rgba.as_ref(), pixels);
        }
        _ => panic!("image snapshot was changed by editing"),
    }
}

#[test]
fn font_and_unicode_line_breaks_are_owned_without_system_font_lookup() {
    let directory = tempfile::tempdir().unwrap();
    let font = resource(&directory.path().join("font.ttf"), FONT);
    let content = "Original fixture\né e\u{301}\nمرحبا";
    let mut draft = scene(8);
    draft.text = Some(TextSpec {
        content: content.into(),
        font: font.clone(),
        font_size: 57,
    });
    let cue = scene::prepare(draft.clone(), CAPS, &AtomicBool::new(false)).unwrap();
    draft.text.as_mut().unwrap().content.clear();
    fs::remove_file(font.path).unwrap();
    let text = cue.text().unwrap();
    assert_eq!(text.content(), content);
    assert_eq!(text.font_size(), 57);
    assert_eq!(text.font_version(), font.version);
    assert_eq!(text.font(), FONT);
    assert_eq!(cue.resource_bytes(), FONT.len() + content.len());
}

#[test]
fn jpeg_is_normalized_to_opaque_rgba() {
    let directory = tempfile::tempdir().unwrap();
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 100)
        .encode(
            &[31, 87, 149].repeat(6),
            3,
            2,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    let mut spec = scene(3);
    spec.background = BackgroundSpec::Image(resource(&directory.path().join("image"), &encoded));
    let cue = scene::prepare(spec, CAPS, &AtomicBool::new(false)).unwrap();
    let PreparedBackground::Image { extent, rgba, .. } = cue.background() else {
        panic!("missing image")
    };
    assert_eq!(
        *extent,
        Extent {
            width: 3,
            height: 2
        }
    );
    for pixel in rgba.as_chunks::<4>().0 {
        for (value, expected) in pixel[..3].iter().zip([31, 87, 149]) {
            assert!(
                value.abs_diff(expected) <= 2,
                "JPEG quantization exceeded fixture tolerance"
            );
        }
        assert_eq!(pixel[3], 255);
    }
}

#[test]
fn queued_completion_is_filtered_by_identity_and_exact_deadline() {
    // Inject only the transport to force a completed result to be queued before
    // polling; real worker races are covered separately by the gated tests.
    let (jobs, _incoming) = mpsc::sync_channel(1);
    let (completed, results) = mpsc::sync_channel(1);
    let now = Instant::now();
    let mut preparer = Preparer {
        jobs,
        results,
        pending: None,
        next: 0,
        caps: CAPS,
    };
    let cue = || scene::prepare(scene(3), CAPS, &AtomicBool::new(false));
    preparer.pending = Some(Pending {
        id: RequestId(2),
        deadline: now + Duration::from_secs(1),
        cancel: Arc::new(AtomicBool::new(false)),
    });
    completed.send((RequestId(1), cue())).ok().unwrap();
    assert!(preparer.poll(now).is_none());
    assert_eq!(preparer.pending.as_ref().unwrap().id, RequestId(2));
    completed.send((RequestId(2), cue())).ok().unwrap();
    assert!(matches!(
        preparer.poll(now + Duration::from_secs(1)),
        Some(PreparationEvent::Failed {
            request: RequestId(2),
            error: PrepareError::TimedOut
        })
    ));
    assert!(preparer.poll(now + Duration::from_secs(1)).is_none());
}

#[test]
fn missing_changed_corrupt_and_unsupported_assets_fail_without_replacing_valid_cue() {
    let live = scene::prepare(scene(2), CAPS, &AtomicBool::new(false)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (encoded, _) = png();
    let reference = resource(&directory.path().join("asset"), &encoded);
    let mut candidate = scene(7);
    candidate.background = BackgroundSpec::Image(reference.clone());
    fs::remove_file(&reference.path).unwrap();
    fail(
        candidate.clone(),
        PrepareError::MissingResource(reference.version),
    );
    fs::write(&reference.path, b"changed bytes at same path").unwrap();
    fail(
        candidate.clone(),
        PrepareError::ChangedResource(reference.version),
    );
    candidate.background = BackgroundSpec::Image(resource(&reference.path, &encoded[..16]));
    fail(candidate.clone(), PrepareError::InvalidImage);
    candidate.background = BackgroundSpec::Image(resource(&reference.path, b"GIF89aunsupported"));
    fail(candidate.clone(), PrepareError::UnsupportedImage);
    candidate.background = BackgroundSpec::Color([255; 4]);
    candidate.text = Some(TextSpec {
        content: "fixture".into(),
        font: resource(&reference.path, b"not a font"),
        font_size: 30,
    });
    fail(candidate, PrepareError::InvalidFont);
    assert_eq!(live.version().revision, 2);
    assert!(matches!(
        live.background(),
        PreparedBackground::Color([18, 53, 109, 255])
    ));
}

#[test]
fn source_pixel_text_and_capability_budgets_reject_boundaries_without_overflow() {
    let mut spec = scene(1);
    spec.extent = Extent {
        width: u32::MAX,
        height: u32::MAX,
    };
    assert_eq!(
        scene::validate(
            &spec,
            RendererCapabilities {
                max_texture_dimension: u32::MAX
            }
        ),
        Err(PrepareError::TooLarge)
    );
    spec.extent = Extent {
        width: 4096,
        height: 4096,
    };
    assert!(scene::validate(&spec, CAPS).is_ok());
    spec.extent.height += 1;
    fail(spec.clone(), PrepareError::TooLarge);
    spec.extent = Extent {
        width: 0,
        height: 7,
    };
    fail(spec, PrepareError::InvalidScene);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large");
    let reference = resource(&path, &[]);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_SOURCE_BYTES as u64 + 1)
        .unwrap();
    let mut spec = scene(2);
    spec.background = BackgroundSpec::Image(reference.clone());
    fail(spec.clone(), PrepareError::TooLarge);
    spec.background = BackgroundSpec::Color([0; 4]);
    spec.text = Some(TextSpec {
        content: "a".repeat(MAX_TEXT_BYTES),
        font: reference,
        font_size: 50,
    });
    assert!(scene::validate(&spec, CAPS).is_ok());
    spec.text.as_mut().unwrap().content.push('b');
    fail(spec, PrepareError::TooLarge);

    let (encoded, _) = png();
    let mut spec = scene(3);
    spec.extent = Extent {
        width: 2,
        height: 2,
    };
    spec.background = BackgroundSpec::Image(resource(&path, &encoded));
    assert_eq!(
        scene::prepare(
            spec,
            RendererCapabilities {
                max_texture_dimension: 2
            },
            &AtomicBool::new(false)
        )
        .err(),
        Some(PrepareError::TooLarge)
    );
}

#[test]
fn saturation_does_not_cancel_last_accepted_request_and_late_old_work_cannot_win() {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let caller = std::thread::current().id();
    let mut preparer = Preparer::with_preparer(CAPS, move |spec, cancel| {
        assert_ne!(
            std::thread::current().id(),
            caller,
            "preparation ran on caller thread"
        );
        if spec.version.revision == 1 {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            // Deliberately ignore cancellation: receiver must still filter stale work.
            return scene::prepare(spec, CAPS, &AtomicBool::new(false));
        }
        scene::prepare(spec, CAPS, cancel)
    })
    .unwrap();
    let now = Instant::now();
    preparer
        .request(scene(1), now, Duration::from_secs(1))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let latest = preparer
        .request(scene(9), now, Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        preparer.request(scene(15), now, Duration::from_secs(1)),
        Err(PrepareError::Busy)
    );
    let mut invalid = scene(17);
    invalid.extent.width = 0;
    assert_eq!(
        preparer.request(invalid, now, Duration::from_secs(1)),
        Err(PrepareError::InvalidScene)
    );
    release.send(()).unwrap();
    let (request, cue) = ready(wait(&mut preparer, now));
    assert_eq!(request, latest);
    assert_eq!(cue.version().revision, 9);
    assert!(preparer.poll(now).is_none());
}

#[test]
fn deadline_boundary_rejects_late_result_and_next_request_recovers() {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let mut preparer = Preparer::with_preparer(CAPS, move |spec, _| {
        let result = scene::prepare(spec.clone(), CAPS, &AtomicBool::new(false));
        if spec.version.revision == 1 {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        result
    })
    .unwrap();
    let now = Instant::now();
    let timeout = Duration::from_secs(2);
    let request = preparer.request(scene(1), now, timeout).unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        preparer
            .poll(now + timeout - Duration::from_nanos(1))
            .is_none()
    );
    assert!(
        matches!(preparer.poll(now + timeout), Some(PreparationEvent::Failed { request: id, error: PrepareError::TimedOut }) if id == request)
    );
    release.send(()).unwrap();
    let next = preparer.request(scene(11), now + timeout, timeout).unwrap();
    let (id, cue) = ready(wait(&mut preparer, now + timeout));
    assert_eq!(id, next);
    assert_eq!(cue.version().revision, 11);
    assert_eq!(
        preparer.request(scene(12), now, Duration::ZERO),
        Err(PrepareError::TimedOut)
    );
}

#[test]
fn cancellation_and_drop_do_not_wait_for_blocked_worker() {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let (finished, done) = mpsc::sync_channel(1);
    let mut preparer = Preparer::with_preparer(CAPS, move |spec, _| {
        started.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(5)).unwrap();
        finished.send(()).unwrap();
        scene::prepare(spec, CAPS, &AtomicBool::new(false))
    })
    .unwrap();
    let now = Instant::now();
    preparer
        .request(scene(4), now, Duration::from_secs(5))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    preparer.cancel();
    assert!(preparer.poll(now).is_none());
    let (dropped, notice) = mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        drop(preparer);
        dropped.send(()).unwrap();
    });
    notice
        .recv_timeout(Duration::from_secs(1))
        .expect("drop waited for blocked worker");
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(5)).unwrap();
    thread.join().unwrap();
}

#[test]
fn worker_disconnect_is_reported_without_payload_or_path_in_diagnostics() {
    let mut preparer =
        Preparer::with_preparer(CAPS, |_, _| panic!("synthetic worker failure")).unwrap();
    let now = Instant::now();
    let request = preparer
        .request(scene(4), now, Duration::from_secs(2))
        .unwrap();
    assert!(
        matches!(wait(&mut preparer, now), PreparationEvent::Failed { request: id, error: PrepareError::Disconnected } if id == request)
    );
    assert_eq!(
        preparer.request(scene(5), now, Duration::from_secs(2)),
        Err(PrepareError::Disconnected)
    );
    let error = PrepareError::MissingResource(ContentVersion {
        id: 63,
        revision: 5,
    });
    assert!(error.to_string().contains("locate"));
    assert!(!format!("{error:?}").contains("Original fixture"));
}
