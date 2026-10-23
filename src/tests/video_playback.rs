//! video-playback: video slide backgrounds go live through the pin check
//! and the renderer's first-frame gate, the Live transport follows the
//! renderer's reports, missing videos substitute black like images, and the
//! Media tab lists, imports and thumbnails videos off the UI thread
//! (M2-02, M2-03, M2-05 basic). The fake renderer stands in for the real
//! one here (it refuses a video whose file is gone, like the first-frame
//! gate); `scripts/video-x11.py` drives the real audience and FFmpeg.
use super::*;
use crate::operator::{MediaLocator, SettingsConfig};
use sela::background::{Background, VideoRef};
use sela::media::backend::Availability;
use sela::media::ffmpeg::{Located, NotFound, Source};
use sela::playback::{MediaAction, PlayState};
use std::path::PathBuf;
use std::sync::mpsc;

const MOTION: &str = "Motion Hymn";
const STILL: &str = "Still Hymn";

/// A located FFmpeg whose executables are never run by these tests (the
/// fake renderer does not decode).
fn located() -> Located {
    Located {
        source: Source::Dev,
        dir: PathBuf::from("/sela-test/ffmpeg/bin"),
        ffmpeg: PathBuf::from("/sela-test/ffmpeg/bin/ffmpeg"),
        ffprobe: PathBuf::from("/sela-test/ffmpeg/bin/ffprobe"),
        version: "n8.1.3-test".into(),
        refused: Vec::new(),
    }
}

/// "Motion Hymn": Verse 1, Chorus, Verse 2 over the profile video `motion.mp4`
/// as the song master, then a Bridge whose own background is a missing
/// video; "Still Hymn": one slide on a color.
fn library_with_video_song(dir: &tempfile::TempDir) -> (PathBuf, VideoRef) {
    let videos = sela::videos::videos_dir(dir.path());
    std::fs::create_dir_all(&videos).unwrap();
    // The operator only pins and checks the bytes; nothing decodes them here.
    std::fs::write(videos.join("motion.mp4"), b"original motion clip bytes").unwrap();
    let video = sela::videos::video_ref(dir.path(), "motion.mp4").unwrap();
    let section = |label: &str, background| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: format!("{label} original line"),
        format: Default::default(),
        background,
    };
    let song = |title: &str, sections, master| Song {
        title: title.into(),
        authors: String::new(),
        copyright: String::new(),
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: Vec::new(),
        sections,
        master,
    };
    let path = dir.path().join("library.sqlite");
    let mut repository = Repository::open(&path).unwrap();
    let missing = VideoRef {
        name: "gone.mp4".into(),
        ..video.clone()
    };
    repository
        .save_song(
            None,
            song(
                MOTION,
                vec![
                    section("Verse 1", None),
                    section("Chorus", None),
                    section("Verse 2", None),
                    section("Bridge", Some(Background::video(missing))),
                ],
                Some(Background::video(video.clone())),
            ),
        )
        .unwrap();
    repository
        .save_song(
            None,
            song(
                STILL,
                vec![section("Verse 1", None)],
                Some(Background::color([20, 30, 90])),
            ),
        )
        .unwrap();
    (path, video)
}

fn open_with(
    cx: &mut TestAppContext,
    library: PathBuf,
    locator: MediaLocator,
) -> (VisualTestContext, Entity<Operator>) {
    let config = SettingsConfig {
        media_locator: Some(locator),
        ..Default::default()
    };
    let (mut cx, operator, _) = with_settings(cx, Some(library), "apply", config);
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    (cx, operator)
}

fn ready(o: &Operator) -> bool {
    matches!(o.media_backend.availability, Availability::Ready(_))
}

fn live_version(o: &Operator) -> Option<sela::scene::ContentVersion> {
    match o.output.as_ref()?.live() {
        LiveState::Confirmed(version) => Some(version),
        LiveState::Unknown => None,
    }
}

fn media_state(o: &Operator) -> Option<PlayState> {
    o.output
        .as_ref()
        .and_then(|output| output.media_status())
        .map(|status| status.state)
}

/// Live output on, `title` previewed.
fn preview(cx: &mut VisualTestContext, operator: &Entity<Operator>, title: &'static str) {
    if operator.read_with(cx, |o, _| o.output.is_none()) {
        click(cx, "live-output");
        settle(cx, operator, "output connection", connected);
    }
    let row = song_row(cx, operator, title);
    click(cx, row);
    settle(cx, operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|item| item.title == title)
    });
}

#[gpui::test]
fn a_video_slide_goes_live_after_its_pin_check_and_the_transport_follows_the_renderer(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (library, video) = library_with_video_song(&dir);
    let (mut cx, operator) = open_with(&mut *cx, library, Arc::new(|| Ok(located())));
    settle(&mut cx, &operator, "located", ready);
    preview(&mut cx, &operator, MOTION);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "video playing", |o| {
        o.live_slide == Some(0) && media_state(o) == Some(PlayState::Playing)
    });
    // The cue carried the checked absolute path of the pinned file.
    let checked = operator.read_with(&cx, |o, _| {
        o.video
            .checked
            .iter()
            .find(|(k, _)| **k == video)
            .map(|(_, v)| v.clone())
    });
    let path = checked.expect("checked").expect("present");
    assert!(path.is_absolute() && path.ends_with("Resources/Videos/motion.mp4"));
    cx.run_until_parked();
    assert!(cx.debug_bounds("video-transport").is_some());
    assert!(cx.debug_bounds("video-position").is_some());
    // The report's sound note reaches the transport row (M2-04).
    let line = operator.read_with(&cx, |o, _| {
        crate::operator::video::transport_line(o.output.as_ref().unwrap().media_status())
    });
    assert_eq!(line, "Playing · 0:00 / 0:10 · No sound output");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.live_video().map(|(v, _, repeat)| (v, repeat))),
        Some((video.clone(), true))
    );
    // Pause: the button sends Pause, the report says Paused.
    click(&mut cx, "video-play-pause");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.output.as_ref().unwrap().media_pending()),
        Some(MediaAction::Pause)
    );
    settle(&mut cx, &operator, "paused", |o| {
        media_state(o) == Some(PlayState::Paused)
    });
    // Play, then Restart, each acknowledged by the renderer's report.
    click(&mut cx, "video-play-pause");
    settle(&mut cx, &operator, "playing", |o| {
        media_state(o) == Some(PlayState::Playing)
            && o.output.as_ref().unwrap().media_pending().is_none()
    });
    click(&mut cx, "video-restart");
    settle(&mut cx, &operator, "restart applied", |o| {
        o.output.as_ref().unwrap().media_pending().is_none()
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.output.as_ref().unwrap().media_rejected()),
        None
    );
    // The actions work from the keyboard-free action route as well.
    operator.update(&mut cx, |o, _| o.toggle_video_playback());
    settle(&mut cx, &operator, "paused by action", |o| {
        media_state(o) == Some(PlayState::Paused)
    });
}

#[gpui::test]
fn a_missing_video_substitutes_black_with_a_warning_and_no_transport(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = library_with_video_song(&dir);
    let (mut cx, operator) = open_with(&mut *cx, library, Arc::new(|| Ok(located())));
    settle(&mut cx, &operator, "located", ready);
    preview(&mut cx, &operator, MOTION);
    click(&mut cx, "preview-slide-3");
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "bridge live", |o| {
        o.live_slide == Some(3) && live_version(o).is_some()
    });
    let message = operator.read_with(&cx, |o, _| o.live_message.clone());
    assert_eq!(
        message.as_deref(),
        Some("Background video ‘gone’ is missing; showing black")
    );
    cx.run_until_parked();
    assert!(cx.debug_bounds("video-transport").is_none());
    assert_eq!(operator.read_with(&cx, |o, _| o.live_video()), None);
    // Transport commands are refused locally: nothing reaches the renderer.
    operator.update(&mut cx, |o, _| o.toggle_video_playback());
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.live_message.as_deref(),
            Some("The live slide has no video")
        );
        assert_eq!(o.output.as_ref().unwrap().media_pending(), None);
    });
}

#[gpui::test]
fn without_ffmpeg_a_video_slide_goes_live_black_with_the_unavailable_warning(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = library_with_video_song(&dir);
    let (mut cx, operator) = open_with(
        &mut *cx,
        library,
        Arc::new(|| Err(NotFound { tried: Vec::new() })),
    );
    settle(&mut cx, &operator, "refused", |o| {
        matches!(o.media_backend.availability, Availability::Unavailable(_))
    });
    preview(&mut cx, &operator, MOTION);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "slide live", |o| {
        o.live_slide == Some(0) && live_version(o).is_some()
    });
    assert_eq!(
        operator
            .read_with(&cx, |o, _| o.live_message.clone())
            .as_deref(),
        Some("Background video ‘motion’ cannot play because video is unavailable; showing black")
    );
    assert_eq!(operator.read_with(&cx, |o, _| o.live_video()), None);
    // Preview names the same problem before anything goes live.
    assert!(
        operator
            .read_with(&cx, |o, _| o.preview_warning())
            .is_some_and(|w| w.contains("video is unavailable"))
    );
}

#[gpui::test]
fn a_video_cue_waits_for_the_ffmpeg_lookup_and_never_replaces_the_scene_meanwhile(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = library_with_video_song(&dir);
    let (sender, receiver) = mpsc::channel::<Result<Located, NotFound>>();
    let receiver = std::sync::Mutex::new(receiver);
    let locator: MediaLocator = Arc::new(move || {
        receiver
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NotFound { tried: Vec::new() }))
    });
    let (mut cx, operator) = open_with(&mut *cx, library, locator);
    preview(&mut cx, &operator, STILL);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "still live", |o| {
        live_version(o).is_some()
    });
    let still = operator.read_with(&cx, |o, _| live_version(o));
    preview(&mut cx, &operator, MOTION);
    click(&mut cx, "go-live");
    // FFmpeg is still being located: the cue waits, the scene stays.
    for _ in 0..20 {
        operator.update(&mut cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
    }
    operator.read_with(&cx, |o, _| {
        assert_eq!(live_version(o), still, "nothing replaced the live scene");
        assert!(o.deferred_send.is_some(), "the video cue is deferred");
        assert_eq!(
            o.live_message.as_deref(),
            Some(sela::t!(sela::i18n::Msg::PreparingBackground))
        );
    });
    sender.send(Ok(located())).unwrap();
    settle(&mut cx, &operator, "video live", |o| {
        media_state(o) == Some(PlayState::Playing)
    });
    assert_ne!(operator.read_with(&cx, |o, _| live_version(o)), still);
}

#[gpui::test]
fn a_video_the_renderer_cannot_open_keeps_the_previous_scene(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (library, video) = library_with_video_song(&dir);
    let (mut cx, operator) = open_with(&mut *cx, library, Arc::new(|| Ok(located())));
    settle(&mut cx, &operator, "located", ready);
    preview(&mut cx, &operator, STILL);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "still live", |o| {
        live_version(o).is_some()
    });
    let still = operator.read_with(&cx, |o, _| live_version(o));
    // The pin is checked while Motion Hymn is previewed...
    preview(&mut cx, &operator, MOTION);
    settle(&mut cx, &operator, "pin checked", |o| {
        o.video.checked.contains(&video)
    });
    // ...then the file goes away before Go Live: the renderer cannot open
    // it and refuses the cue (the fake mirrors the first-frame gate).
    std::fs::remove_file(sela::videos::videos_dir(dir.path()).join("motion.mp4")).unwrap();
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "renderer refusal", |o| {
        o.output.as_ref().unwrap().rejected().is_some()
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(live_version(o), still, "the previous scene stays live");
        assert_eq!(o.live_video(), None);
    });
}

#[gpui::test]
fn the_media_tab_lists_videos_by_name_and_imports_off_the_ui_thread(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = library_with_video_song(&dir);
    let (mut cx, operator) = open_with(&mut *cx, library, Arc::new(|| Ok(located())));
    settle(&mut cx, &operator, "located", ready);
    click(&mut cx, "Media");
    // Images stays the tab's first view; Videos is one click away.
    assert!(cx.debug_bounds("import-image").is_some());
    click(&mut cx, "media-category-videos");
    settle(&mut cx, &operator, "listed", |o| o.video.listed.len() == 1);
    cx.run_until_parked();
    assert!(cx.debug_bounds("video-0").is_some());
    assert!(cx.debug_bounds("import-video").is_some());
    assert!(cx.debug_bounds("import-image").is_none());
    click(&mut cx, "video-0");
    assert_eq!(
        operator.read_with(&cx, |o, _| o.video.selected.clone()),
        Some("motion.mp4".into())
    );
    // The fake FFmpeg cannot probe: the import is refused with the reason
    // and the folder is unchanged; the UI never waited for it.
    let source = dir.path().join("outside.mp4");
    std::fs::write(&source, b"not really a video").unwrap();
    operator.update(&mut cx, |o, _| o.start_video_import(source.clone()));
    assert!(operator.read_with(&cx, |o, _| o.video.importing.is_some()));
    settle(&mut cx, &operator, "import reply", |o| {
        o.video.importing.is_none()
    });
    let message = operator.read_with(&cx, |o, _| o.video.message.clone().unwrap());
    assert!(message.starts_with("Video import failed:"), "{message}");
    assert_eq!(
        sela::videos::list(dir.path()).unwrap(),
        vec!["motion.mp4".to_owned()]
    );
}

/// With the pinned development FFmpeg: a real import probes and copies the
/// clip, and its row thumbnail lands from the thumbnail worker.
#[gpui::test]
fn a_real_import_lists_the_video_and_its_thumbnail_lands(cx: &mut TestAppContext) {
    let Ok(found) = sela::media::ffmpeg::locate() else {
        let message = "SKIPPED a_real_import_lists_the_video_and_its_thumbnail_lands: no FFmpeg";
        assert!(
            std::env::var_os("SELA_REQUIRE_FFMPEG").is_none_or(|v| v != "1"),
            "{message}"
        );
        eprintln!("{message}");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = library_with_video_song(&dir);
    let clip = dir.path().join("Sunrise Loop.mkv");
    let status = std::process::Command::new(&found.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x180:rate=10:duration=1",
        ])
        .args(["-c:v", "ffv1"])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(status.success());
    let (mut cx, operator) = open_with(&mut *cx, library, Arc::new(move || Ok(found.clone())));
    settle(&mut cx, &operator, "located", ready);
    click(&mut cx, "Media");
    click(&mut cx, "media-category-videos");
    operator.update(&mut cx, |o, _| o.start_video_import(clip.clone()));
    settle(&mut cx, &operator, "imported and listed", |o| {
        o.video.importing.is_none() && o.video.listed.len() == 2
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.video.message.clone()),
        Some("Imported video ‘Sunrise Loop’".into())
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| o.video.selected.clone()),
        Some("Sunrise Loop.mkv".into())
    );
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        operator.update(&mut cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        let landed = operator.read_with(&cx, |o, _| {
            o.video
                .listed
                .iter()
                .find(|l| l.name == "Sunrise Loop.mkv")
                .and_then(|l| o.video.list_thumb(l))
                .is_some()
        });
        if landed {
            break;
        }
        assert!(Instant::now() < end, "the thumbnail landed");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Bounded: never more posters than the cache's entry cap.
    assert!(
        operator.read_with(&cx, |o, _| o.video.thumb_cache.len()) <= sela::videos::THUMB_ENTRIES
    );
}

#[test]
fn the_transport_line_names_state_position_and_sound() {
    use crate::operator::video::transport_line;
    use sela::playback::{AudioState, MediaStatus};
    assert_eq!(transport_line(None), "Starting video…");
    let status = MediaStatus {
        scene: sela::scene::ContentVersion { id: 1, revision: 1 },
        state: PlayState::Paused,
        position: Duration::from_millis(65_400),
        duration: None,
        audio: AudioState::None,
    };
    assert_eq!(transport_line(Some(status)), "Paused · 1:05");
    let failed = MediaStatus {
        audio: AudioState::Failed,
        duration: Some(Duration::from_secs(125)),
        ..status
    };
    assert_eq!(
        transport_line(Some(failed)),
        "Paused · 1:05 / 2:05 · Sound failed; the video plays on silently"
    );
}
