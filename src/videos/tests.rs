use super::*;
use std::process::{Command, Stdio};
use std::time::Instant;

fn temp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// The located FFmpeg, or `None` (a SKIPPED line) when it is missing and
/// `SELA_REQUIRE_FFMPEG` is not `1`.
pub(crate) fn tools(test: &str) -> Option<Tools> {
    match crate::media::ffmpeg::locate() {
        Ok(located) => Some(Tools::from(&located)),
        Err(error) => {
            let message = format!("SKIPPED {test}: no usable FFmpeg ({error:?})");
            if std::env::var_os("SELA_REQUIRE_FFMPEG").is_some_and(|v| v == "1") {
                panic!("{message}");
            }
            eprintln!("{message}");
            None
        }
    }
}

/// An original synthetic clip made by the located FFmpeg (lavfi testsrc).
pub(crate) fn clip(tools: &Tools, out: &Path, size: &str, seconds: &str) {
    let source = format!("testsrc=size={size}:rate=10:duration={seconds}");
    let status = Command::new(&tools.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i", &source, "-c:v", "mpeg4", "-q:v", "5"])
        .arg(out)
        .stdin(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn names_are_bare_files_with_a_video_extension() {
    for good in [
        "a.mp4",
        "Waves Loop.MOV",
        "x.webm",
        "y.mkv",
        "z.avi",
        "m.m4v",
        "o.mpg",
    ] {
        assert!(valid_name(good), "{good}");
    }
    for bad in [
        "",
        ".a.mp4",
        "a.png",
        "a.mp4.exe",
        "../a.mp4",
        "a/b.mp4",
        "a\\b.mp4",
        "C:a.mp4",
        " a.mp4",
        "a.mp4 ",
        "mp4",
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
    assert!(!valid_name(&format!("{}.mp4", "a".repeat(252))));
    assert!(valid_name(&format!("{}.mp4", "a".repeat(251))));
    assert_eq!(stem("Waves (2).mp4"), "Waves (2)");
}

#[test]
fn the_list_is_sorted_skips_other_files_and_a_missing_folder_is_empty() {
    let profile = temp();
    assert_eq!(list(profile.path()), Ok(Vec::new()));
    let dir = videos_dir(profile.path());
    fs::create_dir_all(dir.join("folder.mp4")).unwrap();
    for name in [
        "b.mp4",
        "A.webm",
        "notes.txt",
        ".sela-video-1-1.tmp",
        "c.png",
    ] {
        fs::write(dir.join(name), b"x").unwrap();
    }
    assert_eq!(
        list(profile.path()).unwrap(),
        vec!["A.webm".to_owned(), "b.mp4".to_owned()]
    );
}

#[test]
fn the_fingerprint_reads_both_ends_and_the_length_only() {
    let dir = temp();
    let path = dir.path().join("v.mp4");
    let mut bytes = vec![7u8; 3 * FINGERPRINT_SPAN as usize];
    fs::write(&path, &bytes).unwrap();
    let base = fingerprint(&path).unwrap();
    assert_eq!(base.0, bytes.len() as u64);
    // A change in the middle is outside both spans: not noticed (documented
    // limit); a change at either end or in the length is.
    bytes[FINGERPRINT_SPAN as usize + 5] = 1;
    fs::write(&path, &bytes).unwrap();
    assert_eq!(fingerprint(&path).unwrap(), base);
    for at in [0, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[at] ^= 0xff;
        fs::write(&path, &changed).unwrap();
        assert_ne!(fingerprint(&path).unwrap().1, base.1, "{at}");
    }
    bytes.push(7);
    fs::write(&path, &bytes).unwrap();
    assert_ne!(fingerprint(&path).unwrap().1, base.1);
    // Short files hash once, with no overlap from the tail read.
    for len in [
        0usize,
        1,
        FINGERPRINT_SPAN as usize,
        FINGERPRINT_SPAN as usize + 1,
    ] {
        fs::write(&path, vec![3u8; len]).unwrap();
        assert_eq!(fingerprint(&path).unwrap().0, len as u64);
    }
}

#[test]
fn verify_reports_missing_changed_and_invalid_and_gives_an_absolute_path() {
    let profile = temp();
    let dir = videos_dir(profile.path());
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.mp4"), b"first").unwrap();
    let pinned = video_ref(profile.path(), "a.mp4").unwrap();
    let path = verify(profile.path(), &pinned).unwrap();
    assert!(path.is_absolute() && path.ends_with("Resources/Videos/a.mp4"));
    fs::write(dir.join("a.mp4"), b"other").unwrap();
    assert_eq!(verify(profile.path(), &pinned), Err(Substitute::Changed));
    fs::remove_file(dir.join("a.mp4")).unwrap();
    assert_eq!(verify(profile.path(), &pinned), Err(Substitute::Missing));
    let bad = VideoRef {
        name: "../a.mp4".into(),
        ..pinned
    };
    assert_eq!(verify(profile.path(), &bad), Err(Substitute::Invalid));
    assert_eq!(video_ref(profile.path(), "a.png"), Err(Error::InvalidName));
    assert_eq!(video_ref(profile.path(), "gone.mp4"), Err(Error::Missing));
    assert!(Substitute::Missing.warning("Waves.mp4").contains("‘Waves’"));
}

#[test]
fn the_lru_evicts_oldest_by_bytes_and_count_but_keeps_the_newest() {
    let mut lru = Lru::new(10, 3);
    lru.insert("a", 1, 4);
    lru.insert("b", 2, 4);
    assert_eq!(lru.get(&"a"), Some(&1)); // a is now the most recent
    lru.insert("c", 3, 4); // 12 bytes: b goes first
    assert!(!lru.contains(&"b") && lru.contains(&"a") && lru.contains(&"c"));
    assert_eq!(lru.bytes(), 8);
    lru.insert("d", 4, 0);
    lru.insert("e", 5, 0); // four entries: the oldest (a) goes
    assert!(!lru.contains(&"a"));
    assert_eq!(lru.len(), 3);
    lru.insert("huge", 9, 100); // over budget alone: kept, others gone
    assert_eq!(lru.len(), 1);
    assert!(lru.contains(&"huge"));
    lru.insert("huge", 8, 1); // replacing a key keeps one entry
    assert_eq!((lru.len(), lru.get(&"huge")), (1, Some(&8)));
    lru.retain(|k| *k != "huge");
    assert!(lru.is_empty());
}

fn key(name: &str) -> ThumbKey {
    ThumbKey::Pinned(
        VideoRef {
            name: name.into(),
            bytes: 1,
            fingerprint: [0; 32],
        },
        Aspect::Zoom,
        THUMB,
    )
}

fn name_of(key: &ThumbKey) -> &str {
    match key {
        ThumbKey::Pinned(video, ..) => &video.name,
        ThumbKey::File(listed) => &listed.name,
    }
}

fn replies(thumbs: &Thumbnailer, count: usize) -> Vec<(ThumbKey, Thumb)> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.len() < count && Instant::now() < deadline {
        match thumbs.poll() {
            Some(reply) => got.push(reply),
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    got
}

#[test]
fn the_thumbnailer_serves_the_latest_wanted_list_in_order_off_the_caller() {
    let caller = std::thread::current().id();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let open = gate.clone();
    let made = Arc::new(Mutex::new(Vec::new()));
    let log = made.clone();
    let thumbs = Thumbnailer::start_with("test-thumbs", move |key: &ThumbKey| {
        assert_ne!(std::thread::current().id(), caller);
        let (lock, ready) = &*open;
        let mut opened = lock.lock().unwrap();
        while !*opened {
            opened = ready.wait(opened).unwrap();
        }
        log.lock().unwrap().push(name_of(key).to_owned());
        if name_of(key) == "bad.mp4" {
            Err(Substitute::Invalid)
        } else {
            Ok(Arc::from(vec![1u8; 4]))
        }
    })
    .unwrap();
    thumbs.want([key("first.mp4"), key("stale.mp4")]);
    // The worker took "first" and waits on the gate; the newer list
    // replaces "stale", which is never decoded.
    std::thread::sleep(Duration::from_millis(50));
    thumbs.want([key("bad.mp4"), key("next.mp4")]);
    {
        let (lock, ready) = &*gate;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
    let got = replies(&thumbs, 3);
    let names: Vec<_> = got.iter().map(|(k, _)| name_of(k)).collect();
    assert_eq!(names, ["first.mp4", "bad.mp4", "next.mp4"]);
    assert_eq!(got[1].1, Err(Substitute::Invalid));
    assert!(got[2].1.is_ok());
    assert_eq!(made.lock().unwrap().len(), 3);
    assert!(thumbs.poll().is_none());
}

#[test]
fn a_key_being_worked_is_never_queued_again_by_a_newer_list() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let open = gate.clone();
    let made = Arc::new(Mutex::new(Vec::new()));
    let log = made.clone();
    let lookups = Lookups::start_with("test-dedup", move |key: &u32| {
        let (lock, ready) = &*open;
        let mut opened = lock.lock().unwrap();
        while !*opened {
            opened = ready.wait(opened).unwrap();
        }
        log.lock().unwrap().push(*key);
        *key * 10
    })
    .unwrap();
    lookups.want([1, 2]);
    std::thread::sleep(Duration::from_millis(50));
    // 1 is being worked: a newer list that still wants it (a poll before
    // its reply landed) must not decode it twice; 2 was dropped and is
    // wanted again later, so it is queued again.
    lookups.want([1, 3]);
    lookups.want([1, 3, 2]);
    {
        let (lock, ready) = &*gate;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.len() < 3 && Instant::now() < deadline {
        match lookups.poll() {
            Some(reply) => got.push(reply),
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    assert_eq!(got, [(1, 10), (3, 30), (2, 20)]);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(*made.lock().unwrap(), [1, 3, 2], "nothing worked twice");
    assert!(lookups.poll().is_none());
}

#[test]
fn the_worker_lists_with_metadata_reports_import_errors_and_never_blocks() {
    let profile = temp();
    fs::create_dir_all(videos_dir(profile.path())).unwrap();
    fs::write(videos_dir(profile.path()).join("a.mp4"), b"12345").unwrap();
    let worker = Worker::start(profile.path().to_owned()).unwrap();
    worker.submit(Job::Scan).unwrap();
    let tools = Tools {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ffprobe: "/nonexistent/ffprobe".into(),
    };
    worker
        .submit(Job::Import(profile.path().join("x.mp4"), tools))
        .unwrap();
    // Two queued jobs fill the queue; a third is Busy, never blocking.
    let mut busy = false;
    for _ in 0..10 {
        busy |= worker.submit(Job::Scan) == Err(Error::Busy);
    }
    assert!(busy);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = (None, None);
    while Instant::now() < deadline && (got.0.is_none() || got.1.is_none()) {
        match worker.poll() {
            Some(Ok(Reply::Videos(list))) => got.0 = Some(list),
            Some(Ok(Reply::Imported(result))) => got.1 = Some(result),
            Some(Err(error)) => panic!("{error:?}"),
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    let listed = got.0.unwrap().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].name.as_str(), listed[0].bytes), ("a.mp4", 5));
    assert!(listed[0].modified.is_some());
    // The chosen file does not exist: refused before FFmpeg is needed.
    assert_eq!(
        got.1.map(|r| r.map(|i| i.name)),
        Some(Err(Error::SourceMissing))
    );
}

#[test]
fn import_copies_a_playable_video_and_reuses_an_identical_one() {
    let Some(tools) = tools("import_copies_a_playable_video_and_reuses_an_identical_one") else {
        return;
    };
    let source = temp();
    let profile = temp();
    let path = source.path().join("Waves.mp4");
    clip(&tools, &path, "64x36", "1");
    let original = fs::read(&path).unwrap();
    let first = import(profile.path(), &path, &tools, &mut || false).unwrap();
    assert_eq!(first.name, "Waves.mp4");
    assert!(!first.reused);
    assert_eq!(
        first.extent,
        Extent {
            width: 64,
            height: 36
        }
    );
    assert!(
        first
            .duration
            .is_some_and(|d| d >= Duration::from_millis(800))
    );
    let stored = videos_dir(profile.path()).join("Waves.mp4");
    assert_eq!(fs::read(&stored).unwrap(), original);
    assert_eq!(
        fs::read(&path).unwrap(),
        original,
        "the source is untouched"
    );
    assert_eq!(
        verify(profile.path(), &first.video),
        Ok(path_of(&profile, "Waves.mp4"))
    );
    // Same bytes again: reused, nothing copied.
    let again = import(profile.path(), &path, &tools, &mut || false).unwrap();
    assert!(again.reused);
    assert_eq!(again.video, first.video);
    // Other bytes under the same name: the next free name, the first kept.
    clip(&tools, &path, "32x18", "1");
    let other = import(profile.path(), &path, &tools, &mut || false).unwrap();
    assert_eq!(other.name, "Waves (2).mp4");
    assert_eq!(fs::read(&stored).unwrap(), original);
    assert!(verify(profile.path(), &first.video).is_ok());
    // No temporary is left behind.
    assert_eq!(
        list(profile.path()).unwrap(),
        vec!["Waves (2).mp4".to_owned(), "Waves.mp4".to_owned()]
    );
    assert_eq!(fs::read_dir(videos_dir(profile.path())).unwrap().count(), 2);
}

fn path_of(profile: &tempfile::TempDir, name: &str) -> PathBuf {
    path(profile.path(), name)
}

#[test]
fn import_refuses_non_video_and_cancelled_imports_leave_no_file() {
    let Some(tools) = tools("import_refuses_non_video_and_cancelled_imports_leave_no_file") else {
        return;
    };
    let source = temp();
    let profile = temp();
    let text = source.path().join("notes.mp4");
    fs::write(&text, b"not a video at all").unwrap();
    assert!(matches!(
        import(profile.path(), &text, &tools, &mut || false),
        Err(Error::NotVideo(_))
    ));
    let wrong = source.path().join("photo.png");
    fs::write(&wrong, b"x").unwrap();
    assert_eq!(
        import(profile.path(), &wrong, &tools, &mut || false),
        Err(Error::Unsupported)
    );
    assert_eq!(
        import(
            profile.path(),
            &source.path().join("gone.mp4"),
            &tools,
            &mut || false
        ),
        Err(Error::SourceMissing)
    );
    // Audio only: probed, refused as no video.
    let audio = source.path().join("tone.mkv");
    let status = Command::new(&tools.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=duration=0.5",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(&audio)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        import(profile.path(), &audio, &tools, &mut || false),
        Err(Error::NotVideo(MediaError::NoVideo))
    );
    let clip_path = source.path().join("a.mkv");
    clip(&tools, &clip_path, "64x36", "1");
    let mut polls = 0;
    let mut cancel_late = || {
        polls += 1;
        polls > 2
    };
    assert_eq!(
        import(profile.path(), &clip_path, &tools, &mut cancel_late),
        Err(Error::Cancelled)
    );
    assert_eq!(list(profile.path()), Ok(Vec::new()));
    assert_eq!(
        fs::read_dir(videos_dir(profile.path())).unwrap().count(),
        0,
        "no temporary"
    );
}

#[test]
fn a_real_thumbnail_is_the_first_frame_at_the_thumbnail_size() {
    let Some(tools) = tools("a_real_thumbnail_is_the_first_frame_at_the_thumbnail_size") else {
        return;
    };
    let profile = temp();
    fs::create_dir_all(videos_dir(profile.path())).unwrap();
    clip(
        &tools,
        &videos_dir(profile.path()).join("a.mp4"),
        "320x180",
        "1",
    );
    let pinned = video_ref(profile.path(), "a.mp4").unwrap();
    let thumbs = Thumbnailer::thumbnails(profile.path().to_owned(), tools).unwrap();
    let missing = VideoRef {
        name: "gone.mp4".into(),
        ..pinned.clone()
    };
    let tile = Extent {
        width: 64,
        height: 36,
    };
    let listed = list_entries(profile.path()).unwrap().remove(0);
    thumbs.want([
        ThumbKey::Pinned(pinned.clone(), Aspect::Zoom, tile),
        ThumbKey::Pinned(missing, Aspect::Zoom, THUMB),
        ThumbKey::File(listed),
    ]);
    let got = replies(&thumbs, 3);
    assert_eq!(got.len(), 3);
    let rgba = got[0].1.as_ref().unwrap();
    assert_eq!(rgba.len(), (tile.width * tile.height * 4) as usize);
    // testsrc is colorful, never a uniform frame.
    assert!(rgba.chunks(4).any(|p| p != &rgba[..4]));
    assert_eq!(got[1].1, Err(Substitute::Missing));
    let listed = got[2].1.as_ref().unwrap();
    assert_eq!(listed.len(), (THUMB.width * THUMB.height * 4) as usize);
    // Pin checks share the worker shape.
    let checks = Checker::checks(profile.path().to_owned()).unwrap();
    checks.want([pinned.clone()]);
    let deadline = Instant::now() + Duration::from_secs(10);
    let checked = loop {
        if let Some(reply) = checks.poll() {
            break reply;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(checked.0, pinned);
    assert_eq!(checked.1, Ok(path(profile.path(), "a.mp4")));
}
