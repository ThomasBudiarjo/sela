//! M1-12 autosave and recovery driven through the real Operator: cadence on
//! the injected GPUI clock, the recovery prompt after an unclean run, the
//! close flush, failure reporting and autosave while Live output runs.
use super::*;
use crate::operator::{AutosaveSetup, Dialog, Then};
use sela::autosave::{
    self as save, Budget, Cadence, Config, Entry, RECOVERY_DIR, SESSION_FILE, Snapshot, Store,
};
use sela::schedule::Pin;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const NOW_MS: u64 = 1_700_000_000_000;

/// Real files with switchable faults: revision writes can fail with a full
/// disk or wait at a gate; the recovery folder can be read-only.
#[derive(Default)]
struct TestFs {
    disk_full: AtomicBool,
    read_only: AtomicBool,
    /// `session.json` writes fail with a full disk.
    session_full: AtomicBool,
    hold: Mutex<Option<Arc<AtomicBool>>>,
    /// Holds the start-up scan (the folder listing) until released.
    hold_scan: Mutex<Option<Arc<AtomicBool>>>,
}

fn wait_for(gate: Option<Arc<AtomicBool>>) {
    if let Some(gate) = gate {
        let end = Instant::now() + Duration::from_secs(30);
        while !gate.load(Ordering::SeqCst) {
            assert!(Instant::now() < end, "the test never opened the gate");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn is_revision(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("autosave-"))
}

impl save::Fs for TestFs {
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        if self.read_only.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        save::RealFs.create_dir_all(dir)
    }
    fn list(&self, dir: &Path) -> io::Result<Vec<(String, u64)>> {
        wait_for(self.hold_scan.lock().unwrap().clone());
        save::RealFs.list(dir)
    }
    fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>> {
        save::RealFs.read(path, max)
    }
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let session = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(SESSION_FILE));
        if session && self.session_full.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::StorageFull.into());
        }
        if is_revision(path) {
            wait_for(self.hold.lock().unwrap().clone());
            if self.disk_full.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::StorageFull.into());
            }
        }
        save::RealFs.write_new(path, bytes)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        save::RealFs.rename(from, to)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        save::RealFs.remove(path)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        save::RealFs.sync_dir(dir)
    }
}

fn config(fs: Arc<TestFs>) -> Config {
    Config {
        fs,
        clock: Arc::new(|| NOW_MS + 7 * 60_000),
        budget: Budget::default(),
        cadence: Cadence::default(),
    }
}

/// An operator whose autosave uses `fs` and the GPUI test clock.
fn start(
    cx: &mut TestAppContext,
    library: PathBuf,
    fs: Arc<TestFs>,
    mode: &'static str,
) -> (VisualTestContext, Entity<Operator>) {
    start_with(cx, library, fs, mode, 2)
}

fn start_with(
    cx: &mut TestAppContext,
    library: PathBuf,
    fs: Arc<TestFs>,
    mode: &'static str,
    songs: usize,
) -> (VisualTestContext, Entity<Operator>) {
    cx.update(|cx| cx.set_global(AutosaveSetup(config(fs))));
    let (mut vcx, operator, _) = with_library(cx, Some(library), mode);
    vcx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut vcx, &operator, "catalog", |o| o.catalog.len() == songs);
    (vcx, operator)
}

/// Asks the window to close (the real guard) and waits, bounded, until the
/// close flush lets it go.
fn close_window(cx: &mut VisualTestContext, operator: &Entity<Operator>) {
    let closed = cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
    assert!(!closed, "the close waits for the autosave worker");
    let end = Instant::now() + Duration::from_secs(10);
    while cx.read(|cx| cx.windows().len()) > 0 {
        assert!(Instant::now() < end, "the window closes after the flush");
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn ready(o: &Operator) -> bool {
    o.autosave_ready()
}

fn recovery(profile: &Path) -> PathBuf {
    profile.join(RECOVERY_DIR)
}

fn revision_files(profile: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(recovery(profile))
        .map(|d| {
            d.map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|x| x == "selarecovery"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn newest_revision(profile: &Path) -> Option<save::Revision> {
    let path = revision_files(profile).pop()?;
    save::decode_revision(&std::fs::read(path).ok()?)
}

fn session(profile: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(recovery(profile).join(SESSION_FILE)).unwrap()).unwrap()
}

/// Advances the GPUI clock (the autosave cadence's `now`) and lets the poll
/// loop and the worker's replies run.
fn advance(cx: &mut VisualTestContext, operator: &Entity<Operator>, by: Duration) {
    cx.executor().advance_clock(by);
    operator.update(cx, |o, cx| o.poll(cx));
    cx.run_until_parked();
}

/// Waits (bounded, real time) for a condition on disk while polling.
fn settle_disk(
    cx: &mut VisualTestContext,
    operator: &Entity<Operator>,
    what: &str,
    done: impl Fn() -> bool,
) {
    let end = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < end, "bounded wait for {what}");
        operator.update(cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    operator.update(cx, |o, cx| o.poll(cx));
    cx.run_until_parked();
}

fn note(o: &Operator) -> Option<(String, bool)> {
    o.autosave_note()
}

fn versions(library: &Path) -> Vec<(sela::storage::Version, String)> {
    let repository = Repository::open(library).unwrap();
    let mut songs: Vec<_> = repository
        .heads(false, None)
        .unwrap()
        .into_iter()
        .map(|v| (v, repository.song(v).unwrap().title))
        .collect();
    songs.sort_by(|a, b| a.1.cmp(&b.1));
    songs
}

fn entry_of(version: sela::storage::Version, title: &str) -> Entry {
    Entry {
        pin: Pin {
            version,
            arrangement: None,
        },
        title: title.into(),
    }
}

/// A profile whose last run committed `snapshot`, then made a newer edit
/// (marker for `newer_items` entries) and was killed before committing it.
fn crashed_profile(library: &Path, snapshot: &Snapshot, newer_items: usize) {
    let profile = library.parent().unwrap();
    let config = Config {
        fs: Arc::new(save::RealFs),
        clock: Arc::new(|| NOW_MS),
        budget: Budget::default(),
        cadence: Cadence::default(),
    };
    let (mut store, _) = Store::open(profile, &config).unwrap();
    store.commit(1, snapshot).unwrap();
    store.mark_uncommitted(2, newer_items).unwrap();
}

#[gpui::test]
fn an_edit_is_marked_at_once_and_committed_only_after_the_quiet_period(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "autosave ready", ready);
    assert!(
        revision_files(dir.path()).is_empty(),
        "an empty start writes no revision"
    );
    add_song(&mut cx, &operator, "Signal Hymn");
    settle_disk(&mut cx, &operator, "the uncommitted marker", || {
        !session(dir.path())["uncommitted"].is_null()
    });
    assert_eq!(session(dir.path())["uncommitted"]["items"], 1);
    advance(&mut cx, &operator, Duration::from_millis(1_900));
    std::thread::sleep(Duration::from_millis(50));
    advance(&mut cx, &operator, Duration::ZERO);
    assert!(
        revision_files(dir.path()).is_empty(),
        "no commit inside the quiet period"
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| note(o)),
        None,
        "not protected yet"
    );
    advance(&mut cx, &operator, Duration::from_millis(200));
    settle_disk(&mut cx, &operator, "the first revision", || {
        newest_revision(dir.path()).is_some()
    });
    let revision = newest_revision(dir.path()).unwrap();
    assert_eq!(revision.snapshot.path, None);
    assert_eq!(
        revision
            .snapshot
            .entries
            .iter()
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>(),
        ["Signal Hymn"]
    );
    settle(&mut cx, &operator, "the footer says protected", |o| {
        note(o) == Some(("Unsaved changes are autosaved".into(), false))
    });
    let shown = operator.read_with(&cx, |o, _| o.autosave_note());
    assert!(cx.debug_bounds("autosave-status").is_some(), "{shown:?}");
}

#[gpui::test]
fn after_a_crash_restore_brings_back_the_last_commit_and_discloses_lost_work(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![
            entry_of(songs[0].0, &songs[0].1),
            entry_of(songs[1].0, &songs[1].1),
        ],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 3);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    cx.run_until_parked();
    let lines = operator.read_with(&cx, |o, _| o.recovery_lines());
    assert_eq!(
        lines,
        [
            "Sela did not close normally last time.",
            "The last autosave (7 min ago) holds “Untitled schedule” with 2 item(s).",
            "Those changes were never saved to the schedule file.",
            "Newer changes (made 7 min ago) were not autosaved and cannot be recovered; the schedule then had 3 item(s).",
            "If you don't restore, the copies stay in the profile's Recovery folder until newer autosaves replace them.",
        ]
    );
    assert!(cx.debug_bounds("recovery-body").is_some());
    // Restore is the default: Enter restores.
    assert_control(&mut cx, &operator, Control::DialogConfirm);
    cx.simulate_keystrokes("enter");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert_eq!(entry_titles(o), [songs[0].1.clone(), songs[1].1.clone()]);
        assert!(o.schedule.is_dirty(), "restored edits stay unsaved");
        assert_eq!(o.schedule.path(), None);
        assert!(o.live.is_none(), "recovery never touches Live");
        assert_eq!(
            o.schedule_message.as_deref(),
            Some("Restored the autosaved schedule (2 item(s))")
        );
    });
    // Committed again at once as this run's revision; the old run is
    // acknowledged so it is never offered twice.
    // The revision lands first, then the session that acknowledges it.
    settle_disk(&mut cx, &operator, "the restore commit", || {
        revision_files(dir.path()).len() == 2 && session(dir.path())["acknowledged"] == 1
    });
    assert_eq!(session(dir.path())["acknowledged"], 1);
    assert!(session(dir.path())["uncommitted"].is_null());
    assert_eq!(newest_revision(dir.path()).unwrap().snapshot, committed);
    cx.update(|window, cx| {
        operator.update(cx, |o, _| o.sync_window_title(window));
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| o.window_title.clone()),
        "Untitled schedule* — Sela"
    );
}

#[gpui::test]
fn a_saved_schedule_restores_with_its_file_and_only_the_unsaved_part_dirty(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let file = dir.path().join("Sunday.selaschedule");
    let committed = Snapshot {
        path: Some(file.clone()),
        entries: vec![
            entry_of(songs[0].0, &songs[0].1),
            entry_of(songs[1].0, &songs[1].1),
        ],
        baseline: vec![Pin {
            version: songs[0].0,
            arrangement: None,
        }],
    };
    crashed_profile(&library, &committed, 2);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    click(&mut cx, "dialog-confirm");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.schedule.path(), Some(file.as_path()));
        assert!(o.schedule.is_dirty());
        assert_eq!(o.schedule.baseline().len(), 1);
    });
}

#[gpui::test]
fn esc_declines_keeps_the_copies_and_the_next_start_asks_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 2);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert!(o.schedule.entries().is_empty(), "nothing restored");
    });
    settle_disk(&mut cx, &operator, "the answer is recorded", || {
        session(dir.path())["acknowledged"] == 1
    });
    assert_eq!(revision_files(dir.path()).len(), 1, "the copy is kept");
    // The next start (the same profile, as if relaunched) offers nothing.
    let (_, startup) = Store::open(dir.path(), &config(Arc::default())).unwrap();
    assert!(startup.unclean, "this test never closed its operator");
    assert_eq!(startup.offer, None);
}

#[gpui::test]
fn closing_with_the_prompt_unanswered_offers_it_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 2);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    close_window(&mut cx, &operator);
    assert_eq!(session(dir.path())["state"], "offered", "not marked clean");
    assert_eq!(session(dir.path())["acknowledged"], 0);
    let (_, startup) = Store::open(dir.path(), &config(Arc::default())).unwrap();
    assert!(!startup.unclean, "the operator closed it");
    let offer = startup.offer.expect("offered again on the next start");
    assert!(offer.unanswered);
    assert_eq!(offer.revision.unwrap().snapshot, committed);
    assert!(offer.lost.is_some());
}

#[gpui::test]
fn closing_while_the_start_up_scan_runs_keeps_the_previous_offer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 2);
    let fs = Arc::new(TestFs::default());
    let scan = Arc::new(AtomicBool::new(false));
    *fs.hold_scan.lock().unwrap() = Some(scan.clone());
    let (mut cx, operator) = start(cx, library, fs, "apply");
    settle(&mut cx, &operator, "settings loaded", |o| {
        o.settings.loaded_for_test()
    });
    operator.read_with(&cx, |o, _| {
        assert!(!o.autosave_ready(), "the scan is still running");
        assert!(o.autosave.offer.is_none(), "no offer has arrived yet");
    });
    // Ctrl+Q / the close box in the gap between the first frame and the
    // scan's reply: the close must not acknowledge the unseen offer.
    let closed = cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
    assert!(!closed);
    scan.store(true, Ordering::SeqCst);
    let end = Instant::now() + Duration::from_secs(10);
    while cx.read(|cx| cx.windows().len()) > 0 {
        assert!(Instant::now() < end, "the window closes");
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session(dir.path())["state"], "offered");
    assert_eq!(session(dir.path())["acknowledged"], 0);
    assert!(!session(dir.path())["uncommitted"].is_null());
    assert_eq!(revision_files(dir.path()).len(), 1, "nothing committed");
    let (_, startup) = Store::open(dir.path(), &config(Arc::default())).unwrap();
    let offer = startup.offer.expect("still offered on the next start");
    assert!(offer.unanswered);
    assert_eq!(offer.revision.unwrap().snapshot, committed);
}

#[gpui::test]
fn a_prompt_closed_unanswered_says_so_the_next_time(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 2);
    {
        // The run after the crash closed with the prompt unanswered.
        let (mut store, startup) = Store::open(dir.path(), &config(Arc::default())).unwrap();
        assert!(startup.offer.is_some());
        store.close(None).unwrap();
    }
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    let lines = operator.read_with(&cx, |o, _| o.recovery_lines());
    assert_eq!(
        lines[0],
        "Sela was closed last time before this recovery question was answered."
    );
    assert_eq!(
        lines[1],
        "The last autosave (7 min ago) holds “Untitled schedule” with 1 item(s)."
    );
}

#[gpui::test]
fn an_unwritable_profile_after_a_crash_still_offers_a_read_only_restore(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![
            entry_of(songs[0].0, &songs[0].1),
            entry_of(songs[1].0, &songs[1].1),
        ],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 3);
    let before = std::fs::read(recovery(dir.path()).join(SESSION_FILE)).unwrap();
    let fs = Arc::new(TestFs::default());
    fs.session_full.store(true, Ordering::SeqCst);
    let (mut cx, operator) = start(cx, library, fs, "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    cx.run_until_parked();
    let lines = operator.read_with(&cx, |o, _| o.recovery_lines());
    assert_eq!(
        lines.last().map(String::as_str),
        Some(
            "Autosave is off (the disk is full), so this answer cannot be recorded: the question returns at the next start, and a restored schedule is not autosaved."
        )
    );
    assert_eq!(
        operator.read_with(&cx, |o, _| note(o)),
        Some(("Autosave is off: the disk is full.".into(), true))
    );
    cx.simulate_keystrokes("enter");
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.dialog, None);
        assert_eq!(entry_titles(o), [songs[0].1.clone(), songs[1].1.clone()]);
        assert!(o.schedule.is_dirty());
    });
    advance(&mut cx, &operator, Duration::from_secs(3));
    assert_eq!(
        std::fs::read(recovery(dir.path()).join(SESSION_FILE)).unwrap(),
        before,
        "nothing could be recorded"
    );
    assert_eq!(revision_files(dir.path()).len(), 1);
    let closes =
        cx.update(|window, cx| operator.update(cx, |o, cx| o.autosave_allow_close(window, cx)));
    assert!(closes, "autosave is off: nothing to flush");
}

#[gpui::test]
fn a_restored_schedule_gets_the_open_time_missing_file_check(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // One song whose Chorus uses `gone.png`, which is not in the profile.
    let library = library_with_background_song(&dir);
    let songs = versions(&library);
    let committed = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    crashed_profile(&library, &committed, 1);
    let (mut cx, operator) = start_with(cx, library, Arc::default(), "apply", 1);
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    settle(&mut cx, &operator, "the missing-file report", |o| {
        o.dialog == Some(Dialog::Preflight)
            && o.assets
                .report
                .as_ref()
                .is_some_and(|r| !r.items.is_empty())
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        [songs[0].1.clone()]
    );
}

#[gpui::test]
fn a_corrupt_newest_revision_restores_the_previous_one_and_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let songs = versions(&library);
    let older = Snapshot {
        path: None,
        entries: vec![entry_of(songs[0].0, &songs[0].1)],
        baseline: Vec::new(),
    };
    let profile = dir.path();
    {
        let config = config(Arc::default());
        let (mut store, _) = Store::open(profile, &config).unwrap();
        store.commit(1, &older).unwrap();
        let mut newer = older.clone();
        newer.entries.push(entry_of(songs[1].0, &songs[1].1));
        store.commit(2, &newer).unwrap();
    }
    let newest = revision_files(profile).pop().unwrap();
    let mut bytes = std::fs::read(&newest).unwrap();
    let at = bytes.len() / 2;
    bytes[at] ^= 0x04;
    std::fs::write(&newest, bytes).unwrap();
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the recovery prompt", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    let lines = operator.read_with(&cx, |o, _| o.recovery_lines());
    assert!(
        lines.contains(&"1 newer autosave copies were damaged and skipped.".to_string()),
        "{lines:?}"
    );
    click(&mut cx, "dialog-confirm");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        [songs[0].1.clone()],
        "never a partial or damaged schedule"
    );
}

#[gpui::test]
fn lost_work_without_any_copy_is_disclosed_with_ok_only(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    {
        let (mut store, _) = Store::open(dir.path(), &config(Arc::default())).unwrap();
        store.mark_uncommitted(5, 1).unwrap();
    }
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "the loss notice", |o| {
        o.dialog == Some(Dialog::Recovery)
    });
    cx.run_until_parked();
    let lines = operator.read_with(&cx, |o, _| o.recovery_lines());
    assert_eq!(lines[1], "No autosaved copy of the schedule exists.");
    assert!(
        cx.debug_bounds("dialog-confirm").is_none(),
        "nothing to restore"
    );
    assert_control(&mut cx, &operator, Control::DialogCancel);
    click(&mut cx, "dialog-cancel");
    assert_eq!(operator.read_with(&cx, |o, _| o.dialog.clone()), None);
}

#[gpui::test]
fn quit_with_dont_save_flushes_the_last_state_and_marks_the_run_clean(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let (mut cx, operator) = start(cx, library, Arc::default(), "apply");
    settle(&mut cx, &operator, "autosave ready", ready);
    settle(&mut cx, &operator, "settings loaded", |o| {
        o.settings.loaded_for_test()
    });
    add_song(&mut cx, &operator, "Signal Hymn");
    // Quit at once: the quiet period has not passed, nothing is committed.
    let closed = cx.update(|window, cx| operator.update(cx, |o, cx| o.may_close(window, cx)));
    assert!(!closed);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dialog.clone()),
        Some(Dialog::Unsaved(Then::Quit))
    );
    click(&mut cx, "dialog-confirm"); // Don't Save
    let end = Instant::now() + Duration::from_secs(10);
    while cx.read(|cx| cx.windows().len()) > 0 {
        assert!(Instant::now() < end, "the window closes after the flush");
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session(dir.path())["state"], "clean");
    let flushed = newest_revision(dir.path()).expect("the close committed the last state");
    assert_eq!(flushed.snapshot.entries[0].title, "Signal Hymn");
    let (_, startup) = Store::open(dir.path(), &config(Arc::default())).unwrap();
    assert!(!startup.unclean);
    assert_eq!(
        startup.offer, None,
        "a deliberate Don't Save is not offered"
    );
}

#[gpui::test]
fn a_full_disk_is_reported_and_retried_until_space_returns(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let fs = Arc::new(TestFs::default());
    fs.disk_full.store(true, Ordering::SeqCst);
    let (mut cx, operator) = start(cx, library, fs.clone(), "apply");
    settle(&mut cx, &operator, "autosave ready", ready);
    add_song(&mut cx, &operator, "Signal Hymn");
    advance(&mut cx, &operator, Duration::ZERO);
    advance(&mut cx, &operator, Duration::from_secs(3));
    settle(&mut cx, &operator, "the failure note", |o| {
        note(o).is_some_and(|(_, problem)| problem)
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| note(o)).unwrap().0,
        "Autosave failed: the disk is full. Unsaved changes are not protected; Sela retries every 30 seconds."
    );
    assert!(revision_files(dir.path()).is_empty());
    fs.disk_full.store(false, Ordering::SeqCst);
    advance(&mut cx, &operator, Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(50));
    advance(&mut cx, &operator, Duration::ZERO);
    assert!(revision_files(dir.path()).is_empty(), "waits for the retry");
    advance(&mut cx, &operator, Duration::from_secs(21));
    settle(&mut cx, &operator, "the retried commit", |o| {
        note(o) == Some(("Unsaved changes are autosaved".into(), false))
    });
    assert_eq!(revision_files(dir.path()).len(), 1);
}

#[gpui::test]
fn a_read_only_profile_turns_autosave_off_with_the_reason(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let fs = Arc::new(TestFs::default());
    fs.read_only.store(true, Ordering::SeqCst);
    let (mut cx, operator) = start(cx, library, fs, "apply");
    settle(&mut cx, &operator, "the off note", |o| note(o).is_some());
    assert_eq!(
        operator.read_with(&cx, |o, _| note(o)),
        Some((
            "Autosave is off: the profile folder is read-only.".into(),
            true
        ))
    );
    add_song(&mut cx, &operator, "Signal Hymn");
    assert_eq!(
        operator.read_with(&cx, |o, _| entry_titles(o)),
        ["Signal Hymn"],
        "editing goes on"
    );
    // Nothing to flush: the window may close at once.
    settle(&mut cx, &operator, "settings loaded", |o| {
        o.settings.loaded_for_test()
    });
    cx.update(|window, cx| operator.update(cx, |o, _| o.save_layout(window)));
    settle(&mut cx, &operator, "layout stored", |o| o.settings_idle());
    let closes =
        cx.update(|window, cx| operator.update(cx, |o, cx| o.autosave_allow_close(window, cx)));
    assert!(closes);
}

#[gpui::test]
fn autosave_never_holds_up_live_controls(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let library = library_with_songs(&dir);
    let fs = Arc::new(TestFs::default());
    let gate = Arc::new(AtomicBool::new(false));
    *fs.hold.lock().unwrap() = Some(gate.clone());
    let (mut cx, operator) = start(cx, library, fs, "apply");
    settle(&mut cx, &operator, "autosave ready", ready);
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    add_song(&mut cx, &operator, "Signal Hymn");
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("down");
    settle(&mut cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    advance(&mut cx, &operator, Duration::from_secs(3));
    settle(&mut cx, &operator, "a commit held at the disk", |o| {
        o.autosave.policy.in_flight()
    });
    // The autosave write is stuck; Go Live, Next and Black still reach the
    // renderer and come back acknowledged.
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    cx.simulate_keystrokes("b");
    settle(&mut cx, &operator, "Black acknowledged", |o| {
        confirmed_mask(o) == Some(Layer::Black)
    });
    assert!(operator.read_with(&cx, |o, _| o.autosave.policy.in_flight()));
    assert!(revision_files(dir.path()).is_empty(), "still held");
    gate.store(true, Ordering::SeqCst);
    settle(&mut cx, &operator, "the commit lands", |o| {
        !o.autosave.policy.in_flight() && o.autosave.last.is_some()
    });
    assert_eq!(revision_files(dir.path()).len(), 1);
    assert_eq!(
        operator.read_with(&cx, |o, _| o.on_screen().map(str::to_owned)),
        Some("Signal Hymn · Chorus".into()),
        "the commit changes nothing on screen"
    );
}

#[gpui::test]
fn without_a_setup_or_profile_there_is_no_autosave(cx: &mut TestAppContext) {
    let (cx, operator, _) = fixture(cx);
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.autosave_note(), None);
        assert!(!o.autosave.policy.in_flight());
    });
}
