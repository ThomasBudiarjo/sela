//! media-core: FFmpeg located once off the UI thread at startup, logged,
//! and a non-blocking status-bar warning when it is missing or refused
//! (M2-01; VAL-MEDIA-001/002/003).
use super::*;
use crate::operator::{MediaLocator, SettingsConfig};
use sela::media::backend::{Availability, Unavailable};
use sela::media::ffmpeg::{Candidate, Located, NotFound, Refusal, Source, Tool, Tried};
use std::path::PathBuf;
use std::sync::mpsc;

fn refused(refusal: Refusal) -> NotFound {
    NotFound {
        tried: vec![Tried {
            candidate: Candidate {
                source: Source::Env,
                dir: PathBuf::from("/usr/bin"),
            },
            refusal,
        }],
    }
}

fn located() -> Located {
    Located {
        source: Source::Dev,
        dir: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin"),
        ffmpeg: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin/ffmpeg"),
        ffprobe: PathBuf::from("/sela/.amp/in/ffmpeg/n8.1.3-lgpl/bin/ffprobe"),
        version: "n8.1.3-9-g29e619e767".into(),
        refused: Vec::new(),
    }
}

fn open_with(
    cx: &mut TestAppContext,
    dir: &tempfile::TempDir,
    locator: MediaLocator,
) -> (VisualTestContext, Entity<Operator>) {
    let config = SettingsConfig {
        media_locator: Some(locator),
        ..Default::default()
    };
    let (cx, operator, _) = with_settings(cx, Some(library_with_songs(dir)), "apply", config);
    cx.simulate_resize(size(px(1280.), px(800.)));
    (cx, operator)
}

/// A locator held until the test releases it with the answer.
fn gated() -> (MediaLocator, mpsc::Sender<Result<Located, NotFound>>) {
    let (sender, receiver) = mpsc::channel::<Result<Located, NotFound>>();
    let receiver = std::sync::Mutex::new(receiver);
    let locator: MediaLocator = Arc::new(move || {
        receiver
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NotFound { tried: Vec::new() }))
    });
    (locator, sender)
}

fn availability(cx: &VisualTestContext, operator: &Entity<Operator>) -> Availability {
    operator.read_with(cx, |o, _| o.media_backend.availability.clone())
}

#[gpui::test]
fn a_gpl_ffmpeg_shows_the_warning_while_live_output_keeps_working(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (locator, answer) = gated();
    let (mut cx, operator) = open_with(cx, &dir, locator);
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    // Still locating: no warning, and the show runs without FFmpeg.
    assert_eq!(availability(&cx, &operator), Availability::Checking);
    assert!(cx.debug_bounds("media-unavailable").is_none());
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    let hymn = song_row(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, hymn);
    settle(&mut cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "first slide live", |o| {
        o.live_slide == Some(0)
    });
    // The system FFmpeg is GPL: refused, and the warning appears.
    answer.send(Err(refused(Refusal::Gpl))).unwrap();
    settle(&mut cx, &operator, "locate result", |o| {
        o.media_backend.availability != Availability::Checking
    });
    cx.run_until_parked();
    assert_eq!(
        availability(&cx, &operator),
        Availability::Unavailable(Unavailable::Gpl)
    );
    assert!(
        cx.debug_bounds("media-unavailable").is_some(),
        "the warning is rendered in the status bar"
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| o.media_backend.warning()),
        Some("Video and audio unavailable: the FFmpeg found is a GPL build")
    );
    assert!(operator.read_with(&cx, |o, _| o.media_tools().is_none()));
    // Not modal: keys and the safety controls still reach the renderer.
    assert!(operator.read_with(&cx, |o, _| o.dialog.is_none()));
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "next slide live", |o| {
        o.live_slide == Some(1)
    });
    cx.simulate_keystrokes("ctrl-b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        o.indicator(Mask::Black) == Indicator::On
    });
    assert!(cx.debug_bounds("media-unavailable").is_some());
}

#[gpui::test]
fn locate_runs_on_its_own_thread_and_never_delays_input(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (locator, answer) = gated();
    let (mut cx, operator) = open_with(cx, &dir, locator);
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    // The locate call is blocked right now, yet input is handled at once.
    let tab_before = operator.read_with(&cx, |o, _| o.tab);
    click(&mut cx, "Media");
    assert_ne!(operator.read_with(&cx, |o, _| o.tab), tab_before);
    cx.simulate_keystrokes("ctrl-b");
    assert!(operator.read_with(&cx, |o, _| o.masks.is_on(Mask::Black)));
    let thread = operator.read_with(&cx, |o, _| o.media_backend.located_on.clone());
    let end = Instant::now() + Duration::from_secs(5);
    while thread.lock().unwrap().is_none() {
        assert!(Instant::now() < end, "locate started");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_ne!(
        *thread.lock().unwrap(),
        Some(std::thread::current().id()),
        "locate must not run on the UI thread"
    );
    assert_eq!(availability(&cx, &operator), Availability::Checking);
    answer
        .send(Err(refused(Refusal::Missing(Tool::Ffmpeg))))
        .unwrap();
    settle(&mut cx, &operator, "missing", |o| {
        o.media_backend.availability == Availability::Unavailable(Unavailable::Missing)
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.media_backend.warning()),
        Some("Video and audio unavailable: FFmpeg was not found")
    );
}

#[gpui::test]
fn an_accepted_ffmpeg_shows_no_warning_and_offers_its_tools(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open_with(cx, &dir, Arc::new(|| Ok(located())));
    settle(&mut cx, &operator, "located", |o| {
        matches!(o.media_backend.availability, Availability::Ready(_))
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("media-unavailable").is_none());
    let tools = operator.read_with(&cx, |o, _| o.media_tools()).unwrap();
    assert_eq!(tools.ffmpeg, located().ffmpeg);
    assert_eq!(tools.ffprobe, located().ffprobe);
}

#[gpui::test]
fn the_warning_follows_the_ui_language(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open_with(cx, &dir, Arc::new(|| Err(refused(Refusal::Nonfree))));
    settle(&mut cx, &operator, "refused", |o| {
        o.media_backend.availability == Availability::Unavailable(Unavailable::Nonfree)
    });
    sela::i18n::set_language(sela::i18n::Language::Indonesian);
    let indonesian = operator.read_with(&cx, |o, _| o.media_backend.warning());
    sela::i18n::set_language(sela::i18n::Language::English);
    assert_eq!(
        indonesian,
        Some("Video dan audio tidak tersedia: FFmpeg yang ditemukan bukan perangkat lunak bebas")
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| o.media_backend.warning()),
        Some("Video and audio unavailable: the FFmpeg found is a nonfree build")
    );
}

/// The real locator against this checkout's development copy (fetched by
/// `scripts/fetch-ffmpeg`); skipped with a message when it is absent unless
/// `SELA_REQUIRE_FFMPEG=1`.
#[gpui::test]
fn the_real_locator_accepts_the_pinned_development_build(cx: &mut TestAppContext) {
    let dev = sela::media::ffmpeg::dev_dir().unwrap();
    if !dev.join(Tool::Ffmpeg.file_name()).is_file() {
        let message = format!(
            "SKIPPED the_real_locator_accepts_the_pinned_development_build: no {}",
            dev.display()
        );
        assert!(
            std::env::var_os("SELA_REQUIRE_FFMPEG").is_none_or(|v| v != "1"),
            "{message}"
        );
        eprintln!("{message}");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open_with(cx, &dir, Arc::new(sela::media::ffmpeg::locate));
    settle(&mut cx, &operator, "located", |o| {
        o.media_backend.availability != Availability::Checking
    });
    let Availability::Ready(found) = availability(&cx, &operator) else {
        panic!("{:?}", availability(&cx, &operator));
    };
    assert!(found.version.starts_with("n8.1"), "{}", found.version);
    cx.run_until_parked();
    assert!(cx.debug_bounds("media-unavailable").is_none());
}

/// The longest warning (Indonesian, nonfree) at the minimum window size
/// truncates inside the Resources header instead of pushing the header
/// controls or running past the window edge (review finding).
#[gpui::test]
fn the_longest_warning_fits_the_minimum_window_without_squeezing_controls(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator) = open_with(cx, &dir, Arc::new(|| Err(refused(Refusal::Nonfree))));
    settle(&mut cx, &operator, "refused", |o| {
        o.media_backend.availability == Availability::Unavailable(Unavailable::Nonfree)
    });
    sela::i18n::set_language(sela::i18n::Language::Indonesian);
    operator.update(&mut cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let wide: Vec<_> = ["collapse-resources", "quit", "reset-layout"]
        .iter()
        .map(|id| cx.debug_bounds(id).expect(id).size)
        .collect();
    let (width, height) = sela::pane_layout::MIN_WINDOW;
    cx.simulate_resize(size(px(width as f32), px(height as f32)));
    operator.update(&mut cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let warning = cx.debug_bounds("media-unavailable").expect("warning shown");
    let reset = cx.debug_bounds("reset-layout").expect("reset-layout");
    let narrow: Vec<_> = ["collapse-resources", "quit", "reset-layout"]
        .iter()
        .map(|id| cx.debug_bounds(id).expect(id).size)
        .collect();
    sela::i18n::set_language(sela::i18n::Language::English);
    assert_eq!(narrow, wide, "header controls keep their size");
    assert!(reset.right() <= warning.left(), "{reset:?} {warning:?}");
    assert!(
        warning.right() <= px(width as f32),
        "the warning stays inside the window: {warning:?}"
    );
    assert!(warning.size.width > px(0.));
}
