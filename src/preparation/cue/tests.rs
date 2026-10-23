use super::*;
use crate::{format::SlideFormat, storage::Id};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 4096,
};
const EXTENT: Extent = Extent {
    width: 640,
    height: 360,
};

fn job(tag: u64) -> CueJob {
    let format = SlideFormat::default();
    let slides: Arc<[Slide]> = vec![Slide {
        label: "Verse 1".into(),
        text: "First original line\nSecond original line".into(),
        format: format.clone(),
        background: None,
    }]
    .into();
    CueJob {
        item: Version {
            id: Id([9; 16]),
            revision: 1,
        },
        index: 0,
        version: ContentVersion {
            id: 7,
            revision: tag,
        },
        slides,
        resolved: vec![Resolved::bundled(&format)].into(),
        extent: EXTENT,
        caps: CAPS,
        sizing: Sizing::PerSlide,
        background: slides::color_background([0, 0, 0]),
        footer: None,
        footer_shown: true,
    }
}

#[test]
fn a_hidden_strip_still_shapes_the_normalized_size_and_the_cap_follows_the_strip() {
    let format = SlideFormat::default();
    let slides: Arc<[Slide]> = (0..2)
        .map(|_| Slide {
            label: "Verse".into(),
            text: (1..=9)
                .map(|n| format!("Original line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
            format: format.clone(),
            background: None,
        })
        .collect::<Vec<_>>()
        .into();
    let resolved: Arc<[Resolved]> = vec![Resolved::bundled(&format); 2].into();
    let at = |index: usize, footer: Option<&str>, shown: bool| CueJob {
        index,
        slides: slides.clone(),
        resolved: resolved.clone(),
        sizing: Sizing::Normalized,
        extent: Extent {
            width: 1280,
            height: 720,
        },
        footer: footer.map(Into::into),
        footer_shown: shown,
        ..job(index as u64 + 1)
    };
    let strip = "Signal Hymn - A. Writer\n(c) 2020 Some Owner";
    let mut cache = CapCache::default();
    let cancel = AtomicBool::new(false);
    let first = at(0, Some(strip), true).build(&mut cache, &cancel).unwrap();
    let second = at(1, Some(strip), false)
        .build(&mut cache, &cancel)
        .unwrap();
    assert!(first.cue.footer().is_some());
    assert!(second.cue.footer().is_none(), "not drawn on this slide");
    assert_eq!(first.size_cap, second.size_cap, "one size for the item");
    assert_eq!(
        first.cue.text().unwrap().font_size(),
        second.cue.text().unwrap().font_size()
    );
    // A licensing line is a taller strip: the cached cap is recomputed.
    let taller = "Signal Hymn - A. Writer\n(c) 2020 Some Owner\nCCLI License # 1";
    let third = at(1, Some(taller), true)
        .build(&mut cache, &cancel)
        .unwrap();
    let fresh = at(1, Some(taller), true)
        .build(&mut CapCache::default(), &cancel)
        .unwrap();
    assert_eq!(third.size_cap, fresh.size_cap);
    assert!(third.size_cap < first.size_cap);
    let none = at(1, None, true).build(&mut cache, &cancel).unwrap();
    assert!(none.size_cap > first.size_cap);
}

fn build(job: CueJob) -> Result<Built, PrepareError> {
    job.build(&mut CapCache::default(), &AtomicBool::new(false))
}

fn wait(preparer: &CuePreparer) -> CueEvent {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(event) = preparer.poll() {
            return event;
        }
        assert!(Instant::now() < end, "bounded wait for a cue");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn revision(event: &CueEvent) -> u64 {
    match event {
        CueEvent::Ready { cue, .. } => cue.version().revision,
        CueEvent::Failed { .. } => panic!("expected a prepared cue"),
    }
}

/// A builder that waits for one token per job.
fn gated() -> (mpsc::Sender<()>, CuePreparer) {
    let (open, gate) = mpsc::channel::<()>();
    let preparer = CuePreparer::with_builder(move |job, _| {
        gate.recv_timeout(Duration::from_secs(10)).unwrap();
        build(job)
    })
    .unwrap();
    (open, preparer)
}

#[test]
fn cues_are_built_on_the_prepare_worker_not_the_caller() {
    let caller = std::thread::current().id();
    let (seen, names) = mpsc::channel();
    let preparer = CuePreparer::with_builder(move |job, _| {
        let thread = std::thread::current();
        seen.send((thread.id(), thread.name().map(str::to_owned)))
            .unwrap();
        build(job)
    })
    .unwrap();
    preparer.submit(Lane::Live, job(1)).unwrap();
    assert_eq!(revision(&wait(&preparer)), 1);
    let (id, name) = names.recv().unwrap();
    assert_ne!(id, caller);
    assert_eq!(name.as_deref(), Some("sela-prepare-cue"));
}

#[test]
fn the_default_build_runs_on_the_worker_and_caps_normalized_sizes() {
    let preparer = CuePreparer::new().unwrap();
    let mut normalized = job(2);
    normalized.sizing = Sizing::Normalized;
    let expected = slides::size_cap(
        &normalized.slides,
        &normalized.resolved,
        EXTENT,
        Sizing::Normalized,
    );
    assert!(expected.is_some());
    preparer.submit(Lane::Live, normalized).unwrap();
    match wait(&preparer) {
        CueEvent::Ready { cue, size_cap, .. } => {
            assert_eq!(cue.version().revision, 2);
            assert_eq!(size_cap, expected);
        }
        CueEvent::Failed { .. } => panic!("default build fails"),
    }
}

#[test]
fn a_two_second_build_never_blocks_the_caller_and_lands_only_when_done() {
    let preparer = CuePreparer::with_builder(|job, _| {
        std::thread::sleep(Duration::from_secs(2));
        build(job)
    })
    .unwrap();
    let start = Instant::now();
    preparer.submit(Lane::Live, job(1)).unwrap();
    let mut polls = 0;
    while start.elapsed() < Duration::from_millis(1500) {
        assert!(
            preparer.poll().is_none(),
            "nothing applies before the build ends"
        );
        polls += 1;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(polls > 100, "the caller kept running: {polls} polls");
    assert_eq!(revision(&wait(&preparer)), 1);
    assert!(start.elapsed() >= Duration::from_secs(2));
}

#[test]
fn live_cues_complete_in_submission_order() {
    let (open, preparer) = gated();
    for tag in 1..=3 {
        preparer.submit(Lane::Live, job(tag)).unwrap();
    }
    for _ in 0..3 {
        open.send(()).unwrap();
    }
    let order: Vec<_> = (0..3).map(|_| revision(&wait(&preparer))).collect();
    assert_eq!(order, [1, 2, 3]);
}

#[test]
fn a_full_live_lane_refuses_with_busy_until_results_are_taken() {
    let (open, preparer) = gated();
    for tag in 0..LIVE_DEPTH as u64 {
        preparer.submit(Lane::Live, job(tag)).unwrap();
    }
    assert_eq!(preparer.outstanding(Lane::Live), LIVE_DEPTH);
    assert!(matches!(
        preparer.submit(Lane::Live, job(99)),
        Err(PrepareError::Busy)
    ));
    // A finished but unpolled result still counts: memory stays bounded.
    open.send(()).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while preparer.shared.lock().done.is_empty() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        preparer.submit(Lane::Live, job(99)),
        Err(PrepareError::Busy)
    ));
    assert_eq!(revision(&wait(&preparer)), 0);
    preparer.submit(Lane::Live, job(100)).unwrap();
}

#[test]
fn a_cancelled_build_is_discarded_even_when_it_completes_later() {
    let (open, preparer) = gated();
    preparer.submit(Lane::Live, job(1)).unwrap();
    // Let the worker pick it up, then queue a second behind it.
    let end = Instant::now() + Duration::from_secs(10);
    while preparer.outstanding(Lane::Live) != 1 || preparer.shared.lock().running.is_none() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    preparer.submit(Lane::Live, job(2)).unwrap();
    preparer.cancel(Lane::Live);
    assert_eq!(
        preparer.outstanding(Lane::Live),
        1,
        "only the running build remains"
    );
    open.send(()).unwrap();
    let t = preparer.submit(Lane::Live, job(3)).unwrap();
    open.send(()).unwrap();
    let event = wait(&preparer);
    assert_eq!(event.ticket(), t);
    assert_eq!(revision(&event), 3);
    assert!(preparer.poll().is_none(), "the stale results never surface");
}

#[test]
fn a_failed_build_reports_failed_and_does_not_poison_the_next() {
    let mut first = true;
    let preparer = CuePreparer::with_builder(move |job, _| {
        if std::mem::take(&mut first) {
            Err(PrepareError::InvalidScene)
        } else {
            build(job)
        }
    })
    .unwrap();
    let bad = preparer.submit(Lane::Live, job(1)).unwrap();
    match wait(&preparer) {
        CueEvent::Failed { ticket, error } => {
            assert_eq!(ticket, bad);
            assert_eq!(error, PrepareError::InvalidScene);
        }
        CueEvent::Ready { .. } => panic!("failed builds never produce a cue"),
    }
    preparer.submit(Lane::Live, job(2)).unwrap();
    assert_eq!(revision(&wait(&preparer)), 2);
}

#[test]
fn a_panicking_build_fails_that_cue_and_keeps_the_worker() {
    let mut first = true;
    let preparer = CuePreparer::with_builder(move |job, _| {
        if std::mem::take(&mut first) {
            panic!("injected");
        }
        build(job)
    })
    .unwrap();
    preparer.submit(Lane::Live, job(1)).unwrap();
    assert!(matches!(wait(&preparer), CueEvent::Failed { .. }));
    preparer.submit(Lane::Live, job(2)).unwrap();
    assert_eq!(revision(&wait(&preparer)), 2);
}

#[test]
fn rapid_preview_requests_coalesce_and_the_last_completes() {
    let (open, preparer) = gated();
    // Occupy the worker so the burst piles up behind it.
    preparer.submit(Lane::Live, job(1000)).unwrap();
    for tag in 0..200 {
        preparer.submit(Lane::Preview, job(tag)).unwrap();
        assert!(preparer.outstanding(Lane::Preview) <= 1);
    }
    assert_eq!(preparer.coalesced(), 199);
    open.send(()).unwrap();
    open.send(()).unwrap();
    let revisions: Vec<_> = (0..2).map(|_| revision(&wait(&preparer))).collect();
    assert_eq!(revisions, [1000, 199]);
    assert!(preparer.poll().is_none());
}

#[test]
fn live_is_served_before_a_waiting_preview() {
    let (open, preparer) = gated();
    preparer.submit(Lane::Live, job(1)).unwrap();
    preparer.submit(Lane::Preview, job(2)).unwrap();
    preparer.submit(Lane::Live, job(3)).unwrap();
    for _ in 0..3 {
        open.send(()).unwrap();
    }
    let order: Vec<_> = (0..3).map(|_| revision(&wait(&preparer))).collect();
    assert_eq!(order, [1, 3, 2]);
}
