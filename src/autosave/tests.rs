use super::*;
use crate::schedule::Schedule;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

fn v(n: u8, revision: i64) -> Version {
    Version {
        id: Id([n; 16]),
        revision,
    }
}

fn pin(n: u8) -> Pin {
    Pin {
        version: v(n, 1),
        arrangement: None,
    }
}

fn entry(n: u8, title: &str) -> Entry {
    Entry {
        pin: pin(n),
        title: title.into(),
    }
}

/// Unsaved: two entries, never saved to a file.
fn untitled() -> Snapshot {
    Snapshot {
        path: None,
        entries: vec![entry(1, "Morning Light"), entry(2, "Evening Song")],
        baseline: Vec::new(),
    }
}

/// Saved: equals its file.
fn saved() -> Snapshot {
    Snapshot {
        path: Some(PathBuf::from("/services/Sunday.selaschedule")),
        entries: vec![entry(1, "Morning Light")],
        baseline: vec![pin(1)],
    }
}

/// Fixed wall clock the tests move by hand.
fn clock() -> (WallClock, Arc<AtomicU64>) {
    let now = Arc::new(AtomicU64::new(1_700_000_000_000));
    let read = now.clone();
    (Arc::new(move || read.load(Ordering::SeqCst)), now)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    CreateDir,
    List,
    Read,
    Write,
    Rename,
    Remove,
}

/// Real filesystem with injected failures by operation and file-name
/// fragment, and an optional gate that holds every write until released.
#[derive(Default)]
struct FaultFs {
    faults: Mutex<Vec<(Op, &'static str, io::ErrorKind)>>,
    gate: Option<Arc<AtomicBool>>,
    writes: AtomicU64,
}

impl FaultFs {
    fn fail(&self, op: Op, fragment: &'static str, kind: io::ErrorKind) {
        self.faults.lock().unwrap().push((op, fragment, kind));
    }

    fn heal(&self) {
        self.faults.lock().unwrap().clear();
    }

    fn check(&self, op: Op, path: &Path) -> io::Result<()> {
        let name = path.to_string_lossy();
        for (fault, fragment, kind) in self.faults.lock().unwrap().iter() {
            if *fault == op && name.contains(fragment) {
                return Err(io::Error::from(*kind));
            }
        }
        Ok(())
    }
}

impl Fs for FaultFs {
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        self.check(Op::CreateDir, dir)?;
        RealFs.create_dir_all(dir)
    }
    fn list(&self, dir: &Path) -> io::Result<Vec<(String, u64)>> {
        self.check(Op::List, dir)?;
        RealFs.list(dir)
    }
    fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>> {
        self.check(Op::Read, path)?;
        RealFs.read(path, max)
    }
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(gate) = &self.gate {
            let end = std::time::Instant::now() + Duration::from_secs(20);
            while !gate.load(Ordering::SeqCst) {
                assert!(std::time::Instant::now() < end, "gate never opened");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        if let Err(error) = self.check(Op::Write, path) {
            // A full disk fails after creating the file, like a real one.
            let _ = std::fs::write(path, &bytes[..bytes.len() / 2]);
            return Err(error);
        }
        RealFs.write_new(path, bytes)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.check(Op::Rename, to)?;
        RealFs.rename(from, to)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        self.check(Op::Remove, path)?;
        RealFs.remove(path)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        RealFs.sync_dir(dir)
    }
    fn modified(&self, path: &Path) -> io::Result<u64> {
        RealFs.modified(path)
    }
}

fn config_with(fs: Arc<dyn Fs>, clock: WallClock) -> Config {
    Config {
        fs,
        clock,
        budget: Budget::default(),
        cadence: Cadence::default(),
    }
}

fn config() -> Config {
    config_with(Arc::new(RealFs), clock().0)
}

fn names(profile: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(profile.join(RECOVERY_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn revisions(profile: &Path) -> Vec<String> {
    names(profile)
        .into_iter()
        .filter(|n| n.ends_with(REVISION_SUFFIX))
        .collect()
}

#[test]
fn a_revision_round_trips_and_any_damage_is_rejected() {
    let revision = Revision {
        sequence: 7,
        saved_at: 42,
        snapshot: Snapshot {
            path: Some(PathBuf::from("/s/Évening “late”.selaschedule")),
            entries: vec![
                Entry {
                    pin: Pin {
                        version: v(3, 9),
                        arrangement: Some(VariantId(Id([5; 16]))),
                    },
                    title: "Line\nbreak \"quoted\"".into(),
                },
                entry(3, "Same song twice"),
            ],
            baseline: vec![pin(3)],
        },
    };
    let bytes = encode_revision(&revision);
    assert_eq!(decode_revision(&bytes), Some(revision.clone()));
    // Every truncation and every flipped bit of the body is refused.
    for cut in 0..bytes.len() {
        assert_eq!(decode_revision(&bytes[..cut]), None, "cut at {cut}");
    }
    let body_end = bytes.iter().position(|b| *b == b'\n').unwrap();
    for at in (0..body_end).step_by(7) {
        let mut flipped = bytes.clone();
        flipped[at] ^= 0x01;
        assert_eq!(decode_revision(&flipped), None, "flip at {at}");
    }
    assert!(revision.snapshot.unsaved());
    assert!(untitled().unsaved());
    assert!(!saved().unsaved());
    assert!(!Snapshot::default().unsaved());
}

#[test]
fn snapshot_and_fingerprint_follow_the_schedule() {
    let mut schedule = Schedule::default();
    let empty = fingerprint(&schedule);
    let a = schedule.push(v(1, 1), "Morning Light".into()).unwrap();
    let one = fingerprint(&schedule);
    assert_ne!(one, empty);
    schedule.set_arrangement(a, Some(VariantId(Id([9; 16]))));
    assert_ne!(fingerprint(&schedule), one, "arrangement counts");
    schedule.set_arrangement(a, None);
    assert_eq!(fingerprint(&schedule), one, "same content, same value");
    let snapshot = Snapshot::of(&schedule);
    assert_eq!(snapshot.entries, vec![entry(1, "Morning Light")]);
    assert!(snapshot.unsaved() && snapshot.path.is_none());
    schedule.mark_saved(
        PathBuf::from("/s/A.selaschedule"),
        schedule.pins().collect(),
    );
    let saved = Snapshot::of(&schedule);
    assert!(!saved.unsaved());
    assert_ne!(fingerprint(&schedule), one, "path and baseline count");

    let mut restored = Schedule::default();
    restored
        .restore(
            saved.path.clone(),
            saved
                .entries
                .iter()
                .map(|e| (e.pin, e.title.clone()))
                .collect(),
            saved.baseline.clone(),
        )
        .unwrap();
    assert_eq!(Snapshot::of(&restored), saved);
    assert!(!restored.is_dirty());
    restored
        .restore(None, vec![(pin(2), "B".into())], Vec::new())
        .unwrap();
    assert!(restored.is_dirty(), "an untitled restore is unsaved");
}

#[test]
fn a_first_run_and_a_clean_close_offer_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, startup) = Store::open(dir.path(), &config()).unwrap();
    assert_eq!(
        startup,
        Startup {
            unclean: false,
            offer: None
        }
    );
    assert_eq!(names(dir.path()), [SESSION_FILE]);
    store.commit(1, &untitled()).unwrap();
    store.close(None).unwrap();
    let (_, startup) = Store::open(dir.path(), &config()).unwrap();
    assert_eq!(startup.offer, None);
    assert!(!startup.unclean);
    assert_eq!(revisions(dir.path()).len(), 1, "kept as a backup");
}

#[test]
fn an_unclean_run_offers_its_newest_unsaved_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (c, now) = clock();
    let cfg = config_with(Arc::new(RealFs), c);
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.mark_uncommitted(10, 1).unwrap();
    now.fetch_add(2_000, Ordering::SeqCst);
    let (first, _) = store.commit(10, &saved()).unwrap();
    store.mark_uncommitted(11, 2).unwrap();
    let (second, _) = store.commit(11, &untitled()).unwrap();
    assert_eq!((first.sequence, second.sequence), (1, 2));
    drop(store); // no close: the run ends uncleanly

    let (_, startup) = Store::open(dir.path(), &cfg).unwrap();
    assert!(startup.unclean);
    let offer = startup.offer.unwrap();
    assert_eq!(
        offer.revision,
        Some(Revision {
            sequence: 2,
            saved_at: second.saved_at,
            snapshot: untitled()
        })
    );
    assert_eq!(offer.lost, None, "the marker was covered by commit 2");
    assert_eq!((offer.damaged, offer.interrupted), (0, false));
}

#[test]
fn newer_uncommitted_work_is_disclosed_as_lost() {
    let dir = tempfile::tempdir().unwrap();
    let (c, now) = clock();
    let cfg = config_with(Arc::new(RealFs), c);
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.commit(10, &untitled()).unwrap();
    now.fetch_add(5_000, Ordering::SeqCst);
    let since = now.load(Ordering::SeqCst);
    store.mark_uncommitted(11, 3).unwrap();
    drop(store);
    let offer = Store::open(dir.path(), &cfg).unwrap().1.offer.unwrap();
    assert_eq!(offer.revision.unwrap().snapshot, untitled());
    assert_eq!(
        offer.lost,
        Some(Uncommitted {
            after: 1,
            since,
            items: 3
        })
    );
}

#[test]
fn a_file_saved_after_the_offered_revision_is_disclosed() {
    // Review round 2: commit 1 holds unsaved edits, the operator then saves
    // (the marker is withdrawn) and the process dies before the saved
    // schedule is committed. Only the file's modification time remembers.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Sunday.selaschedule");
    let (c, now) = clock();
    let cfg = config_with(Arc::new(RealFs), c);
    let unsaved = Snapshot {
        path: Some(file.clone()),
        entries: vec![entry(1, "Morning Light"), entry(2, "Evening Song")],
        baseline: vec![pin(1)],
    };
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    let committed_at = now.load(Ordering::SeqCst);
    store.commit(1, &unsaved).unwrap();
    store.mark_uncommitted(2, 2).unwrap();
    store.clear_uncommitted().unwrap();
    drop(store);
    let set_modified = |ms: u64| {
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_millis(ms))
            .unwrap();
    };
    let offer = |cfg: &Config| Store::open(dir.path(), cfg).unwrap().1.offer.unwrap();
    assert_eq!(offer(&cfg).file_saved, None, "no file at all");
    std::fs::write(&file, b"older").unwrap();
    set_modified(committed_at - 60_000);
    assert_eq!(offer(&cfg).file_saved, None, "saved before the commit");
    set_modified(committed_at + 1_500);
    let found = offer(&cfg);
    assert_eq!(found.file_saved, Some(committed_at + 1_500));
    assert_eq!(found.lost, None, "the save withdrew the marker");
    assert_eq!(found.revision.unwrap().snapshot, unsaved);
}

#[test]
fn a_late_marker_for_the_committed_schedule_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.mark_uncommitted(10, 2).unwrap();
    drop(store);
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!(offer.lost, None);
}

#[test]
fn a_saved_schedule_without_newer_work_needs_no_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.commit(11, &saved()).unwrap();
    drop(store);
    let (_, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(startup.unclean);
    assert_eq!(startup.offer, None, "the schedule file has everything");
}

#[test]
fn a_corrupt_latest_revision_falls_back_to_the_previous_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.commit(11, &saved()).unwrap();
    store.commit(12, &untitled()).unwrap();
    drop(store);
    let recovery = dir.path().join(RECOVERY_DIR);
    // Bit rot in 3, a truncated 2.
    let three = recovery.join(revision_name(3));
    let mut bytes = std::fs::read(&three).unwrap();
    bytes[20] ^= 0x20;
    std::fs::write(&three, bytes).unwrap();
    let two = recovery.join(revision_name(2));
    let bytes = std::fs::read(&two).unwrap();
    std::fs::write(&two, &bytes[..bytes.len() - 3]).unwrap();
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    let revision = offer.revision.unwrap();
    assert_eq!((revision.sequence, offer.damaged), (1, 2));
    assert_eq!(revision.snapshot, untitled());
    assert_eq!(revisions(dir.path()).len(), 3, "damaged files are kept");
}

#[test]
fn a_revision_under_another_sequence_name_is_damaged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    drop(store);
    let recovery = dir.path().join(RECOVERY_DIR);
    std::fs::copy(
        recovery.join(revision_name(1)),
        recovery.join(revision_name(2)),
    )
    .unwrap();
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!((offer.revision.unwrap().sequence, offer.damaged), (1, 1));
}

#[test]
fn an_interrupted_commit_is_removed_and_disclosed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.mark_uncommitted(11, 3).unwrap();
    drop(store);
    let recovery = dir.path().join(RECOVERY_DIR);
    let partial = recovery.join(format!("{}{TEMP_SUFFIX}", revision_name(2)));
    std::fs::write(&partial, b"{\"format\":\"sela-rec").unwrap();
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert!(offer.interrupted);
    assert!(offer.lost.is_some());
    assert_eq!(offer.revision.unwrap().sequence, 1);
    assert!(!partial.exists());
}

#[test]
fn a_damaged_session_file_still_offers_unsaved_work() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.close(None).unwrap();
    std::fs::write(dir.path().join(RECOVERY_DIR).join(SESSION_FILE), b"{trunc").unwrap();
    let (_, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(startup.unclean);
    assert_eq!(startup.offer.unwrap().revision.unwrap().sequence, 1);
}

#[test]
fn an_unanswered_offer_survives_a_second_crash() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.mark_uncommitted(11, 3).unwrap();
    drop(store);
    let first = Store::open(dir.path(), &config()).unwrap().1.offer;
    let second = Store::open(dir.path(), &config()).unwrap().1.offer;
    assert!(first.is_some());
    assert_eq!(first, second);
    // A marker attempted while the offer is open keeps the previous run's.
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.mark_uncommitted(12, 1).unwrap();
    drop(store);
    let third = Store::open(dir.path(), &config()).unwrap().1.offer;
    assert_eq!(first, third, "the previous run's marker stays");
}

#[test]
fn restoring_commits_a_new_revision_that_a_later_crash_still_offers() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    drop(store);
    let (mut store, startup) = Store::open(dir.path(), &config()).unwrap();
    let offered = startup.offer.unwrap().revision.unwrap().snapshot;
    let committed = store.resolve(Some((20, &offered))).unwrap().unwrap();
    assert_eq!(committed.sequence, 2);
    drop(store); // crashes again before any edit
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!(offer.revision.unwrap().sequence, 2);
}

#[test]
fn declining_is_final_and_keeps_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(10, &untitled()).unwrap();
    store.mark_uncommitted(11, 3).unwrap();
    drop(store);
    let (mut store, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(startup.offer.is_some());
    assert_eq!(store.resolve(None).unwrap(), None);
    drop(store);
    let (_, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(startup.unclean);
    assert_eq!(startup.offer, None);
    assert_eq!(revisions(dir.path()), [revision_name(1)]);
}

#[test]
fn work_after_a_flushed_close_is_protected_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.close(Some((10, &untitled()))).unwrap();
    // The close was cancelled by an edit: the next commit reopens the run.
    store.commit(11, &untitled()).unwrap();
    drop(store);
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!(offer.revision.unwrap().sequence, 2);
}

#[test]
fn rotation_keeps_the_newest_revisions_within_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.budget = Budget {
        max_revisions: 3,
        max_bytes: 1 << 20,
    };
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    for n in 0..6 {
        let (_, warning) = store.commit(n, &untitled()).unwrap();
        assert_eq!(warning, None);
    }
    assert_eq!(revisions(dir.path()), [4, 5, 6].map(revision_name).to_vec());
    // A byte budget for two revisions keeps two.
    let size = std::fs::metadata(dir.path().join(RECOVERY_DIR).join(revision_name(6)))
        .unwrap()
        .len();
    store.budget = Budget {
        max_revisions: 10,
        max_bytes: 2 * size + 1,
    };
    store.commit(7, &untitled()).unwrap();
    assert_eq!(revisions(dir.path()), [6, 7].map(revision_name).to_vec());
}

#[test]
fn an_exhausted_budget_refuses_the_revision_and_keeps_the_old_ones() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config();
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.commit(1, &saved()).unwrap();
    let big = Snapshot {
        path: None,
        entries: (0..MAX_ITEMS as u8)
            .map(|n| entry(n, &"x".repeat(1000)))
            .collect(),
        baseline: Vec::new(),
    };
    cfg.budget.max_bytes = 4096;
    store.budget = cfg.budget;
    assert_eq!(store.commit(2, &big), Err(Error::Budget));
    assert_eq!(revisions(dir.path()), [revision_name(1)]);
    assert_eq!(store.newest(), 1);
}

#[test]
fn a_full_disk_leaves_no_partial_file_and_the_next_commit_reuses_the_number() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FaultFs::default());
    let cfg = config_with(fs.clone(), clock().0);
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.commit(1, &saved()).unwrap();
    let before = std::fs::read(dir.path().join(RECOVERY_DIR).join(revision_name(1))).unwrap();
    fs.fail(Op::Write, REVISION_PREFIX, io::ErrorKind::StorageFull);
    assert_eq!(store.commit(2, &untitled()), Err(Error::DiskFull));
    assert_eq!(names(dir.path()), [revision_name(1), SESSION_FILE.into()]);
    assert_eq!(
        std::fs::read(dir.path().join(RECOVERY_DIR).join(revision_name(1))).unwrap(),
        before
    );
    fs.heal();
    let (committed, _) = store.commit(2, &untitled()).unwrap();
    assert_eq!(committed.sequence, 2);
}

#[test]
fn a_read_only_or_locked_profile_reports_why() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FaultFs::default());
    fs.fail(Op::CreateDir, RECOVERY_DIR, io::ErrorKind::PermissionDenied);
    let cfg = config_with(fs.clone(), clock().0);
    assert_eq!(Store::open(dir.path(), &cfg).err(), Some(Error::ReadOnly));
    fs.heal();
    fs.fail(Op::Write, SESSION_FILE, io::ErrorKind::ReadOnlyFilesystem);
    assert_eq!(Store::open(dir.path(), &cfg).err(), Some(Error::ReadOnly));
    fs.heal();
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    fs.fail(Op::Rename, REVISION_PREFIX, io::ErrorKind::ResourceBusy);
    assert_eq!(store.commit(1, &untitled()), Err(Error::Locked));
    assert_eq!(names(dir.path()), [SESSION_FILE], "no temporary file left");
    // A session file another process holds open fails the marker, not the run.
    fs.heal();
    fs.fail(Op::Write, SESSION_FILE, io::ErrorKind::PermissionDenied);
    assert_eq!(store.mark_uncommitted(1, 2), Err(Error::ReadOnly));
    fs.heal();
    store.commit(1, &untitled()).unwrap();
}

#[test]
fn a_rotation_failure_is_a_warning_on_a_committed_revision() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FaultFs::default());
    let mut cfg = config_with(fs.clone(), clock().0);
    cfg.budget.max_revisions = 1;
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.commit(1, &untitled()).unwrap();
    fs.fail(
        Op::Remove,
        "autosave-000000000001",
        io::ErrorKind::PermissionDenied,
    );
    let (committed, warning) = store.commit(2, &untitled()).unwrap();
    assert_eq!((committed.sequence, warning), (2, Some(Error::ReadOnly)));
    assert_eq!(revisions(dir.path()).len(), 2);
}

#[test]
fn io_errors_map_to_operator_reasons() {
    let map = |kind| Error::from_io(&io::Error::from(kind));
    assert_eq!(map(io::ErrorKind::StorageFull), Error::DiskFull);
    assert_eq!(map(io::ErrorKind::QuotaExceeded), Error::DiskFull);
    assert_eq!(map(io::ErrorKind::PermissionDenied), Error::ReadOnly);
    assert_eq!(map(io::ErrorKind::ReadOnlyFilesystem), Error::ReadOnly);
    assert_eq!(map(io::ErrorKind::ResourceBusy), Error::Locked);
    assert_eq!(map(io::ErrorKind::Other), Error::Io("Other".into()));
}

/// A previous run that committed an unsaved schedule, made a newer edit and
/// was killed.
fn crashed(dir: &Path, cfg: &Config) {
    let (mut store, _) = Store::open(dir, cfg).unwrap();
    store.commit(1, &untitled()).unwrap();
    store.mark_uncommitted(2, 3).unwrap();
}

#[test]
fn closing_before_the_offer_is_answered_keeps_it_for_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    crashed(dir.path(), &config());
    // The next run closes with its offer unresolved: unshown (the window
    // closed before the scan's reply arrived) or shown and unanswered.
    let (mut store, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(startup.offer.is_some() && store.offer_open());
    assert_eq!(
        store.close(Some((9, &saved()))),
        Ok(None),
        "nothing committed"
    );
    assert_eq!(revisions(dir.path()), [revision_name(1)]);
    let (store, startup) = Store::open(dir.path(), &config()).unwrap();
    assert!(!startup.unclean, "the operator closed that run");
    let offer = startup
        .offer
        .expect("offered again, not acknowledged unseen");
    assert!(offer.unanswered);
    assert_eq!(offer.revision.unwrap().snapshot, untitled());
    assert_eq!(offer.lost.map(|l| (l.after, l.items)), Some((1, 3)));
    // A crash of this run (still unanswered) is an unclean one again.
    drop(store);
    let (mut store, startup) = Store::open(dir.path(), &config()).unwrap();
    let offer = startup.offer.unwrap();
    assert!(startup.unclean && !offer.unanswered);
    // Once answered, a close is a normal clean one.
    store.resolve(None).unwrap();
    assert!(!store.offer_open());
    store.close(None).unwrap();
    let (_, startup) = Store::open(dir.path(), &config()).unwrap();
    assert_eq!((startup.unclean, startup.offer), (false, None));
}

#[test]
fn an_unwritable_session_after_a_crash_still_returns_the_offer() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FaultFs::default());
    let cfg = config_with(fs.clone(), clock().0);
    crashed(dir.path(), &cfg);
    for kind in [
        io::ErrorKind::StorageFull,
        io::ErrorKind::ReadOnlyFilesystem,
    ] {
        fs.fail(Op::Write, SESSION_FILE, kind);
        let failure = Store::open_keeping_offer(dir.path(), &cfg).err().unwrap();
        assert_eq!(failure.error, Error::from_io(&io::Error::from(kind)));
        let offer = failure.offer.expect("the readable copy is still offered");
        assert_eq!(offer.revision.unwrap().snapshot, untitled());
        assert_eq!(offer.lost.map(|l| l.items), Some(3));
        fs.heal();
    }
    // Nothing was acknowledged, so a later start offers it again.
    let (_, startup) = Store::open(dir.path(), &cfg).unwrap();
    assert!(startup.unclean && startup.offer.is_some());
}

#[test]
fn a_withdrawn_marker_discloses_no_loss() {
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::open(dir.path(), &config()).unwrap();
    store.commit(1, &untitled()).unwrap();
    store.mark_uncommitted(2, 3).unwrap();
    store.clear_uncommitted().unwrap();
    drop(store);
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!(offer.lost, None, "the edit was undone or saved to its file");
}

#[test]
fn a_failed_marker_clear_is_written_again_on_retry() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FaultFs::default());
    let cfg = config_with(fs.clone(), clock().0);
    let (mut store, _) = Store::open(dir.path(), &cfg).unwrap();
    store.commit(1, &untitled()).unwrap();
    store.mark_uncommitted(2, 3).unwrap();
    fs.fail(Op::Write, SESSION_FILE, io::ErrorKind::StorageFull);
    assert!(store.clear_uncommitted().is_err());
    fs.heal();
    store.clear_uncommitted().unwrap();
    drop(store);
    let offer = Store::open(dir.path(), &config()).unwrap().1.offer.unwrap();
    assert_eq!(offer.lost, None, "the retried clear reached the disk");
}

#[test]
fn a_failed_marker_write_is_taken_again_after_the_quiet_period() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut policy = Policy::new(cadence(), 0);
    assert_eq!(policy.step(1, true, at(0)), Step::Mark(1));
    policy.marker_failed(Step::Mark(1), at(100));
    assert_eq!(policy.step(1, true, at(1_000)), Step::Idle, "not hammered");
    assert_eq!(policy.step(1, true, at(2_100)), Step::Mark(1), "retried");
    assert_eq!(policy.step(1, true, at(2_200)), Step::Commit(1));
    policy.finished(1, false, at(2_300));
    // Saved to its file while the commit fails: the marker goes, and a
    // failed withdrawal is retried too.
    assert_eq!(policy.step(2, false, at(2_400)), Step::Unmark);
    policy.marker_failed(Step::Unmark, at(2_500));
    assert_eq!(policy.step(2, false, at(3_000)), Step::Idle);
    assert_eq!(policy.step(2, false, at(4_500)), Step::Unmark, "retried");
}

#[test]
fn an_edit_made_while_a_commit_runs_is_marked_without_waiting_for_it() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut policy = Policy::new(cadence(), 0);
    assert_eq!(policy.step(1, true, at(0)), Step::Mark(1));
    assert_eq!(policy.step(1, true, at(2_000)), Step::Commit(1));
    // Commit 1 is running; the schedule moves on.
    assert_eq!(policy.step(2, true, at(2_100)), Step::Mark(2));
    assert_eq!(policy.step(3, true, at(2_200)), Step::Idle, "marked once");
    policy.finished(1, true, at(2_300));
    assert_eq!(policy.step(3, true, at(2_400)), Step::Idle);
    assert_eq!(policy.step(3, true, at(4_200)), Step::Commit(3));
    // An unqueued in-flight marker is tried again.
    assert_eq!(policy.step(4, true, at(4_300)), Step::Mark(4));
    policy.unqueued(Step::Mark(4));
    assert_eq!(policy.step(4, true, at(4_400)), Step::Mark(4));
}

#[test]
fn only_unsaved_work_is_marked_and_a_marker_is_withdrawn_when_untrue() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut policy = Policy::new(cadence(), 0);
    // An opened (saved) schedule is committed without a marker.
    assert_eq!(policy.step(5, false, at(0)), Step::Idle);
    assert_eq!(policy.step(5, false, at(2_000)), Step::Commit(5));
    policy.finished(5, true, at(2_000));
    // An edit marks; undoing it back to the commit withdraws the marker.
    assert_eq!(policy.step(6, true, at(3_000)), Step::Mark(6));
    assert_eq!(policy.step(5, false, at(3_100)), Step::Unmark);
    assert_eq!(policy.step(5, false, at(9_000)), Step::Idle);
    // Saving the edit to its file withdraws it too, and commits later.
    assert_eq!(policy.step(6, true, at(10_000)), Step::Mark(6));
    assert_eq!(policy.step(7, false, at(10_100)), Step::Unmark);
    let unmark = policy.step(8, true, at(10_200));
    assert_eq!(unmark, Step::Mark(8), "unsaved again: marked again");
    assert_eq!(policy.step(7, false, at(10_300)), Step::Unmark);
    policy.unqueued(Step::Unmark);
    assert_eq!(policy.step(7, false, at(10_400)), Step::Unmark, "retried");
    assert_eq!(policy.step(7, false, at(12_400)), Step::Commit(7));
    // Never withdrawn while a commit runs: it may still fail.
    assert_eq!(policy.step(8, true, at(12_500)), Step::Mark(8));
    assert_eq!(policy.step(7, false, at(12_600)), Step::Idle);
    policy.finished(7, false, at(12_700));
    assert_eq!(policy.step(7, false, at(12_800)), Step::Unmark);
}

fn cadence() -> Cadence {
    Cadence {
        quiet: Duration::from_secs(2),
        max_delay: Duration::from_secs(10),
        retry: Duration::from_secs(30),
    }
}

#[test]
fn the_policy_marks_at_once_and_commits_after_a_quiet_period() {
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_millis(s);
    let mut policy = Policy::new(cadence(), 0);
    assert_eq!(policy.step(0, true, at(0)), Step::Idle);
    assert_eq!(
        policy.step(1, true, at(0)),
        Step::Mark(1),
        "first change marks"
    );
    assert_eq!(policy.step(1, true, at(1_999)), Step::Idle);
    assert_eq!(
        policy.step(2, true, at(1_999)),
        Step::Idle,
        "a change restarts quiet"
    );
    assert_eq!(policy.step(2, true, at(3_998)), Step::Idle);
    assert_eq!(policy.step(2, true, at(3_999)), Step::Commit(2));
    assert!(policy.in_flight());
    assert_eq!(
        policy.step(3, true, at(4_000)),
        Step::Mark(3),
        "an edit during the commit is marked at once, behind the commit"
    );
    assert_eq!(
        policy.step(3, true, at(4_050)),
        Step::Idle,
        "one commit at a time"
    );
    policy.finished(2, true, at(4_100));
    assert_eq!(
        policy.step(3, true, at(4_100)),
        Step::Idle,
        "its marker is already queued"
    );
    assert_eq!(policy.step(3, true, at(6_000)), Step::Commit(3));
    policy.finished(3, true, at(6_050));
    assert!(!policy.uncommitted());
    assert_eq!(policy.step(3, true, at(60_000)), Step::Idle);
}

#[test]
fn continuous_editing_still_commits_within_the_maximum_delay() {
    let t0 = Instant::now();
    let mut policy = Policy::new(cadence(), 0);
    let mut commits = Vec::new();
    // An edit every second for 25 seconds: never quiet for 2 s.
    for second in 0..25u64 {
        let now = t0 + Duration::from_secs(second);
        if let Step::Commit(f) = policy.step(second + 1, true, now) {
            commits.push(second);
            policy.finished(f, true, now);
        }
    }
    // 10 s after the first edit, then 10 s after the first edit it missed.
    assert_eq!(commits, [10, 21], "bounded by max_delay");
}

#[test]
fn a_failed_commit_retries_after_the_retry_interval() {
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_secs(s);
    let mut policy = Policy::new(cadence(), 0);
    assert_eq!(policy.step(1, true, at(0)), Step::Mark(1));
    assert_eq!(policy.step(1, true, at(2)), Step::Commit(1));
    policy.finished(1, false, at(2));
    assert!(policy.failing() && policy.uncommitted());
    assert_eq!(policy.step(1, true, at(31)), Step::Idle);
    assert_eq!(
        policy.step(2, true, at(31)),
        Step::Idle,
        "no new marker: it still holds"
    );
    assert_eq!(policy.step(2, true, at(32)), Step::Commit(2));
    policy.finished(2, true, at(32));
    assert!(!policy.failing() && !policy.uncommitted());
}

#[test]
fn reverting_to_the_committed_schedule_needs_no_commit() {
    let t0 = Instant::now();
    let mut policy = Policy::new(cadence(), 5);
    assert_eq!(policy.step(6, true, t0), Step::Mark(6));
    assert_eq!(
        policy.step(5, false, t0 + Duration::from_secs(9)),
        Step::Unmark,
        "the marker is withdrawn, and no commit follows"
    );
    assert_eq!(
        policy.step(5, false, t0 + Duration::from_secs(9)),
        Step::Idle
    );
    assert_eq!(
        policy.step(5, false, t0 + Duration::from_secs(60)),
        Step::Idle
    );
}

#[test]
fn an_unqueued_step_is_tried_again() {
    let t0 = Instant::now();
    let mut policy = Policy::new(cadence(), 0);
    let mark = policy.step(1, true, t0);
    policy.unqueued(mark);
    assert_eq!(policy.step(1, true, t0), Step::Mark(1));
    let later = t0 + Duration::from_secs(3);
    let commit = policy.step(1, true, later);
    assert_eq!(commit, Step::Commit(1));
    policy.unqueued(commit);
    assert_eq!(policy.step(1, true, later), Step::Commit(1));
}

#[test]
fn a_restore_commits_without_a_marker_and_a_flush_settles_everything() {
    let t0 = Instant::now();
    let mut policy = Policy::new(cadence(), 0);
    policy.restoring(7);
    assert_eq!(
        policy.step(7, true, t0),
        Step::Idle,
        "the restore commit is running"
    );
    policy.finished(7, true, t0);
    assert!(!policy.uncommitted());
    assert_eq!(policy.step(8, true, t0), Step::Mark(8));
    policy.flushed(8);
    assert!(!policy.uncommitted() && !policy.in_flight());
}

#[test]
fn the_worker_never_blocks_the_caller_and_bounds_its_queue() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Arc::new(AtomicBool::new(true));
    let fs = Arc::new(FaultFs {
        gate: Some(gate.clone()),
        ..Default::default()
    });
    let worker =
        Worker::start(dir.path().to_path_buf(), config_with(fs.clone(), clock().0)).unwrap();
    let opened = poll_for(&worker);
    assert!(matches!(opened, Reply::Opened(Ok(_))), "{opened:?}");
    gate.store(false, Ordering::SeqCst);
    let started = std::time::Instant::now();
    let mut accepted = 0;
    for n in 0..10 {
        if worker.submit(Job::Commit(n, Box::new(untitled()))) {
            accepted += 1;
        }
    }
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "submit is non-blocking"
    );
    // One job runs (held at the gate), QUEUE more wait, the rest is refused.
    assert!((QUEUE..=QUEUE + 1).contains(&accepted), "{accepted}");
    assert!(worker.poll().is_none(), "the held commit has not landed");
    gate.store(true, Ordering::SeqCst);
    for _ in 0..accepted {
        assert!(matches!(poll_for(&worker), Reply::Committed(_, Ok(_))));
    }
}

fn poll_for(worker: &Worker) -> Reply {
    let end = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(reply) = worker.poll() {
            return reply;
        }
        assert!(std::time::Instant::now() < end, "bounded wait for a reply");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Scenario S03 with real process termination: a child process commits a
/// revision, makes a newer edit and is killed (SIGKILL on Unix,
/// TerminateProcess on Windows) before, during or after committing it. The
/// next open recovers exactly the last committed revision, never a partial
/// one, and discloses the newer work that was lost.
mod s03 {
    use super::*;

    const CHILD: &str = "SELA_S03_CHILD";

    /// Holds a revision write half-way, as a crash in the middle would.
    struct StallFs {
        ready: PathBuf,
        stall: bool,
    }

    impl Fs for StallFs {
        fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
            RealFs.create_dir_all(dir)
        }
        fn list(&self, dir: &Path) -> io::Result<Vec<(String, u64)>> {
            RealFs.list(dir)
        }
        fn read(&self, path: &Path, max: u64) -> io::Result<Vec<u8>> {
            RealFs.read(path, max)
        }
        fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            let name = path.file_name().unwrap().to_string_lossy();
            if self.stall && name.starts_with(&revision_name(2)) {
                let mut file = std::fs::File::create_new(path)?;
                file.write_all(&bytes[..bytes.len() / 2])?;
                file.sync_all()?;
                std::fs::write(&self.ready, b"during")?;
                loop {
                    std::thread::park();
                }
            }
            RealFs.write_new(path, bytes)
        }
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            RealFs.rename(from, to)
        }
        fn remove(&self, path: &Path) -> io::Result<()> {
            RealFs.remove(path)
        }
        fn sync_dir(&self, dir: &Path) -> io::Result<()> {
            RealFs.sync_dir(dir)
        }
        fn modified(&self, path: &Path) -> io::Result<u64> {
            RealFs.modified(path)
        }
    }

    fn newer() -> Snapshot {
        let mut snapshot = untitled();
        snapshot.entries.push(entry(3, "Newer Edit"));
        snapshot
    }

    /// The child: commit A, mark the newer edit, then stop at `point`.
    #[test]
    fn s03_child() {
        let Some(spec) = std::env::var_os(CHILD) else {
            return;
        };
        let spec = spec.to_string_lossy().into_owned();
        let (point, profile) = spec.split_once(':').unwrap();
        let profile = PathBuf::from(profile);
        let ready = profile.join("ready");
        let cfg = config_with(
            Arc::new(StallFs {
                ready: ready.clone(),
                stall: point == "during",
            }),
            clock().0,
        );
        let (mut store, _) = Store::open(&profile, &cfg).unwrap();
        store.mark_uncommitted(1, 2).unwrap();
        store.commit(1, &untitled()).unwrap();
        store.mark_uncommitted(2, 3).unwrap();
        match point {
            "before" => std::fs::write(&ready, b"before").unwrap(),
            "during" => {
                store.commit(2, &newer()).unwrap();
                unreachable!("the stalled write never returns");
            }
            "after" => {
                store.commit(2, &newer()).unwrap();
                std::fs::write(&ready, b"after").unwrap();
            }
            _ => panic!("unknown point {point}"),
        }
        loop {
            std::thread::park();
        }
    }

    fn kill_at(point: &str) -> (tempfile::TempDir, Startup) {
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "autosave::tests::s03::s03_child"])
            .env(CHILD, format!("{point}:{}", dir.path().display()))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let ready = dir.path().join("ready");
        let end = std::time::Instant::now() + Duration::from_secs(30);
        while std::fs::read(&ready).ok().as_deref() != Some(point.as_bytes()) {
            assert!(
                std::time::Instant::now() < end,
                "bounded wait for the child at {point}"
            );
            assert!(child.try_wait().unwrap().is_none(), "the child died early");
            std::thread::sleep(Duration::from_millis(5));
        }
        child.kill().unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(9), "SIGKILL");
        }
        let (_, startup) = Store::open(dir.path(), &config()).unwrap();
        (dir, startup)
    }

    #[test]
    fn killed_before_the_commit_recovers_the_last_revision_and_discloses_the_edit() {
        let (dir, startup) = kill_at("before");
        assert!(startup.unclean);
        let offer = startup.offer.unwrap();
        assert_eq!(offer.revision.unwrap().snapshot, untitled());
        assert_eq!(offer.lost.map(|l| (l.after, l.items)), Some((1, 3)));
        assert!(!offer.interrupted);
        assert_eq!(revisions(dir.path()), [revision_name(1)]);
    }

    #[test]
    fn killed_during_the_commit_never_recovers_a_partial_schedule() {
        let (dir, startup) = kill_at("during");
        let offer = startup.offer.unwrap();
        assert_eq!(offer.revision.unwrap().snapshot, untitled());
        assert!(offer.interrupted, "the cut-off write is disclosed");
        assert_eq!(offer.lost.map(|l| (l.after, l.items)), Some((1, 3)));
        assert_eq!(offer.damaged, 0);
        assert_eq!(
            names(dir.path()),
            [revision_name(1), SESSION_FILE.into()],
            "the partial file is gone"
        );
    }

    #[test]
    fn killed_after_the_commit_recovers_the_newer_revision_without_loss() {
        let (_dir, startup) = kill_at("after");
        let offer = startup.offer.unwrap();
        let revision = offer.revision.unwrap();
        assert_eq!((revision.sequence, revision.snapshot), (2, newer()));
        assert_eq!(offer.lost, None);
        assert!(!offer.interrupted);
    }
}
