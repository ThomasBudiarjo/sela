use super::*;
use crate::background::Background;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn song(title: &str) -> Song {
    Song {
        title: title.into(),
        authors: "Original 作者".into(),
        copyright: "© test".into(),
        license: "test-only".into(),
        administrator: String::new(),
        song_number: String::new(),
        variants: vec![Variant {
            id: VariantId(Id([9; 16])),
            name: "Short".into(),
            occurrences: vec![Occurrence {
                id: OccurrenceId(Id([8; 16])),
                section: SectionId(Id([1; 16])),
            }],
        }],
        sections: vec![
            Section {
                id: SectionId(Id([1; 16])),
                label: "Verse".into(),
                lyrics: "one\r\n\n二\n".into(),
                format: Default::default(),
                background: None,
            },
            Section {
                id: SectionId(Id([2; 16])),
                label: String::new(),
                lyrics: String::new(),
                format: Default::default(),
                background: None,
            },
        ],
        master: None,
    }
}

fn item(n: u8, title: &str) -> Item {
    Item {
        source: Version {
            id: Id([n; 16]),
            revision: i64::from(n),
        },
        song: song(title),
        arrangement: None,
    }
}

fn file(items: Vec<Item>) -> ScheduleFile {
    ScheduleFile {
        title: "Sunday".into(),
        items,
        scripture: Vec::new(),
    }
}

fn value(file: &ScheduleFile) -> Value {
    serde_json::from_slice(&encode(file).unwrap()).unwrap()
}

#[test]
fn round_trips_order_titles_and_content() {
    let original = file(vec![item(3, "Zeta"), item(1, "Alpha"), item(3, "Zeta")]);
    let decoded = decode(&encode(&original).unwrap()).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn song_properties_round_trip_and_older_files_open_without_them() {
    let mut with = item(1, "Props");
    with.song.administrator = "Acme Music".into();
    with.song.song_number = "7654321".into();
    let original = file(vec![with, item(2, "Plain")]);
    let v = value(&original);
    assert_eq!(v["items"][0]["song"]["administrator"], "Acme Music");
    assert!(v["items"][1]["song"].get("administrator").is_none());
    assert_eq!(decode(&encode(&original).unwrap()).unwrap(), original);

    let mut old = value(&file(vec![item(2, "Plain")]));
    old["version"] = 2.into();
    let opened = decode(old.to_string().as_bytes()).unwrap();
    assert_eq!(opened.items[0].song.song_number, "");
    old["items"][0]["song"]["song_number"] = "12x".into();
    assert!(
        decode(old.to_string().as_bytes()).is_err(),
        "invalid number"
    );
}

#[test]
fn round_trips_empty_schedule() {
    let original = file(Vec::new());
    assert_eq!(decode(&encode(&original).unwrap()).unwrap(), original);
}

#[test]
fn document_declares_format_and_version() {
    let v = value(&file(vec![item(1, "A")]));
    assert_eq!(v["format"], "sela-schedule");
    assert_eq!(v["version"], 3);
    assert_eq!(v["items"][0]["kind"], "song");
    assert_eq!(v["items"][0]["song"]["title"], "A");
}

#[test]
fn background_and_master_round_trip() {
    let mut s = song("Bg");
    let background = Background::color([10, 20, 30]);
    s.sections[0].background = Some(background.clone());
    s.master = Some(background);
    let original = file(vec![Item {
        source: Version {
            id: Id([4; 16]),
            revision: 2,
        },
        song: s,
        arrangement: None,
    }]);
    assert_eq!(decode(&encode(&original).unwrap()).unwrap(), original);
}

#[test]
fn each_item_keeps_its_own_arrangement_and_version_1_still_opens() {
    let chosen = VariantId(Id([9; 16]));
    let mut copy = item(1, "Same");
    copy.arrangement = Some(chosen);
    let original = file(vec![item(1, "Same"), copy]);
    let written = value(&original);
    assert!(written["items"][0].get("arrangement").is_none());
    assert_eq!(written["items"][1]["arrangement"], id_hex(chosen.0));
    assert_eq!(decode(&encode(&original).unwrap()).unwrap(), original);

    let mut old = written.clone();
    old["version"] = 1.into();
    let opened = decode(old.to_string().as_bytes()).unwrap();
    assert_eq!(
        opened.items[1].arrangement, None,
        "v1 has no arrangement choice"
    );
    assert_eq!(opened.items[1].song, original.items[1].song);

    let mut stale = written;
    stale["items"][1]["arrangement"] = id_hex(Id([42; 16])).into();
    let opened = decode(stale.to_string().as_bytes()).unwrap();
    assert_eq!(
        opened.items[1].arrangement, None,
        "unknown variant is dropped"
    );
    stale["items"][1]["arrangement"] = 5.into();
    assert_eq!(
        decode(stale.to_string().as_bytes()),
        Err(ReadError::Corrupt)
    );
}

#[test]
fn extension_is_added_once() {
    let plain = with_extension(Path::new("C:/x/Sunday"));
    assert_eq!(plain, PathBuf::from("C:/x/Sunday.selaschedule"));
    assert_eq!(with_extension(&plain), plain);
    let upper = Path::new("C:/x/Sunday.SELASCHEDULE");
    assert_eq!(with_extension(upper), upper);
    assert_eq!(
        with_extension(Path::new("C:/x/Sunday.v2")),
        PathBuf::from("C:/x/Sunday.v2.selaschedule")
    );
}

fn scripture(position: usize, translation: &str, redistributable: bool) -> ScriptureItem {
    use crate::scripture::Book;
    let john = Book::from_number(43).unwrap();
    ScriptureItem {
        position,
        title: format!("John 3:16, 18 ({translation})"),
        snapshot: Snapshot {
            translation: translation.into(),
            name: "Synthetic “Bible”".into(),
            language: "id".into(),
            redistributable,
            verses: vec![
                (VerseId::new(john, 3, 16).unwrap(), "Pertama ünïcode".into()),
                (
                    VerseId::new(john, 3, 18).unwrap(),
                    "Third \"quoted\"".into(),
                ),
            ],
        },
    }
}

#[test]
fn songs_only_schedules_stay_version_3_for_older_readers() {
    let v = value(&file(vec![item(1, "Alpha")]));
    assert_eq!(v["version"], 3);
}

#[test]
fn scripture_items_round_trip_in_place_as_version_4() {
    let mut original = file(vec![item(1, "Alpha"), item(2, "Beta")]);
    original.scripture = vec![scripture(0, "SYA", true), scripture(2, "USER", false)];
    let v = value(&original);
    assert_eq!(v["version"], 4);
    let kinds: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["scripture", "song", "scripture", "song"]);
    assert_eq!(v["items"][0]["verses"][0][0], 43_003_016);
    assert_eq!(v["items"][2]["redistributable"], false);
    assert_eq!(decode(&encode(&original).unwrap()).unwrap(), original);
    // Only scripture is fine too.
    let mut only = file(Vec::new());
    only.scripture = vec![scripture(0, "SYA", true)];
    assert_eq!(decode(&encode(&only).unwrap()).unwrap(), only);
}

#[test]
fn bad_scripture_items_are_refused_on_write_and_read() {
    let mut misplaced = file(vec![item(1, "Alpha")]);
    misplaced.scripture = vec![scripture(5, "SYA", true)];
    assert_eq!(encode(&misplaced), Err(WriteError::Invalid));
    let mut unordered = file(Vec::new());
    unordered.scripture = vec![scripture(1, "SYA", true), scripture(0, "SYA", true)];
    assert_eq!(encode(&unordered), Err(WriteError::Invalid));
    let mut empty = file(Vec::new());
    empty.scripture = vec![scripture(0, "SYA", true)];
    empty.scripture[0].snapshot.verses.clear();
    assert_eq!(encode(&empty), Err(WriteError::Invalid));

    let mut good = file(Vec::new());
    good.scripture = vec![scripture(0, "SYA", true)];
    let base = value(&good);
    let corrupt = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        decode(v.to_string().as_bytes())
    };
    assert_eq!(
        corrupt(&|v| v["items"][0]["verses"][0][0] = 99_001_001.into()),
        Err(ReadError::Corrupt),
        "a verse outside the canon"
    );
    assert_eq!(
        corrupt(&|v| v["items"][0]["verses"].as_array_mut().unwrap().reverse()),
        Err(ReadError::Corrupt),
        "verses out of order"
    );
    assert_eq!(
        corrupt(&|v| v["items"][0]["translation"] = "../x".into()),
        Err(ReadError::Corrupt)
    );
    assert_eq!(
        corrupt(&|v| v["items"][0]["verses"][0] = Value::from("x")),
        Err(ReadError::Corrupt)
    );
    // A version 3 file cannot carry scripture items.
    assert_eq!(
        corrupt(&|v| v["version"] = 3.into()),
        Err(ReadError::Corrupt)
    );
}

#[test]
fn newer_version_is_rejected_with_its_number() {
    let mut v = value(&file(Vec::new()));
    v["version"] = 5.into();
    assert_eq!(decode(v.to_string().as_bytes()), Err(ReadError::Newer(5)));
    v["version"] = 0.into();
    assert_eq!(decode(v.to_string().as_bytes()), Err(ReadError::Corrupt));
}

#[test]
fn wrong_format_and_garbage_are_corrupt() {
    for bytes in [
        &b""[..],
        b"not json",
        b"[]",
        b"{}",
        b"{\"format\":\"other\",\"version\":1}",
        &[0xff, 0xfe, 0x00, 0x80],
    ] {
        assert_eq!(decode(bytes), Err(ReadError::Corrupt));
    }
}

#[test]
fn every_truncation_is_rejected() {
    let bytes = encode(&file(vec![item(1, "A"), item(2, "B")])).unwrap();
    for end in 0..bytes.len() - 1 {
        assert!(decode(&bytes[..end]).is_err(), "prefix of {end} bytes");
    }
}

#[test]
fn item_limit_title_limit_and_unknown_kind() {
    let mut many = value(&file(vec![item(1, "A")]));
    let one = many["items"][0].clone();
    many["items"] = Value::Array(vec![one.clone(); MAX_ITEMS + 1]);
    assert_eq!(
        decode(many.to_string().as_bytes()),
        Err(ReadError::TooManyItems)
    );
    many["items"] = Value::Array(vec![one.clone(); MAX_ITEMS]);
    assert_eq!(
        decode(many.to_string().as_bytes()).unwrap().items.len(),
        MAX_ITEMS
    );

    let mut long = value(&file(Vec::new()));
    long["title"] = "x".repeat(MAX_TITLE_BYTES + 1).into();
    assert_eq!(
        decode(long.to_string().as_bytes()),
        Err(ReadError::TitleTooLong)
    );
    long["title"] = "x".repeat(MAX_TITLE_BYTES).into();
    assert!(decode(long.to_string().as_bytes()).is_ok());

    let mut kind = value(&file(vec![item(1, "A")]));
    kind["items"][0]["kind"] = "video".into();
    assert_eq!(decode(kind.to_string().as_bytes()), Err(ReadError::Corrupt));
}

#[test]
fn encode_refuses_oversized_input() {
    let mut f = file(vec![item(1, "A")]);
    f.title = "x".repeat(MAX_TITLE_BYTES + 1);
    assert_eq!(encode(&f), Err(WriteError::Invalid));
    let f = file((0..=MAX_ITEMS).map(|_| item(1, "A")).collect());
    assert_eq!(encode(&f), Err(WriteError::Invalid));
}

#[test]
fn read_refuses_oversized_files_and_folders() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.selaschedule");
    let handle = fs::File::create(&big).unwrap();
    handle.set_len(MAX_FILE_BYTES as u64 + 1).unwrap();
    assert_eq!(read(&big), Err(ReadError::TooLarge));
    assert!(matches!(read(dir.path()), Err(ReadError::Unreadable(_))));
    assert_eq!(
        read(&dir.path().join("missing.selaschedule")),
        Err(ReadError::NotFound)
    );
}

#[test]
fn read_returns_what_write_wrote() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.selaschedule");
    let original = file(vec![item(1, "A")]);
    write_atomic(&path, &encode(&original).unwrap()).unwrap();
    assert_eq!(read(&path).unwrap(), original);
    let names: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(names.len(), 1, "no temporary file is left");
}

fn existing(dir: &Path) -> (PathBuf, Vec<u8>) {
    let path = dir.join("a.selaschedule");
    let before = encode(&file(vec![item(1, "Before")])).unwrap();
    fs::write(&path, &before).unwrap();
    (path, before)
}

fn only_target(dir: &Path) {
    assert_eq!(fs::read_dir(dir).unwrap().count(), 1, "temp file removed");
}

#[test]
fn partial_write_keeps_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, before) = existing(dir.path());
    let after = encode(&file(vec![item(2, "After")])).unwrap();
    let result = write_atomic_with(
        &path,
        &after,
        |f, bytes| {
            f.write_all(&bytes[..bytes.len() / 2])?;
            Err(io::Error::other("disk full"))
        },
        |from, to| fs::rename(from, to),
    );
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    only_target(dir.path());
}

#[test]
fn failed_rename_keeps_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, before) = existing(dir.path());
    let after = encode(&file(vec![item(2, "After")])).unwrap();
    let result = write_atomic_with(
        &path,
        &after,
        |f, bytes| f.write_all(bytes),
        |_, _| Err(io::Error::other("rename failed")),
    );
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    only_target(dir.path());
}

#[test]
fn read_only_target_is_refused_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let (path, before) = existing(dir.path());
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let after = encode(&file(vec![item(2, "After")])).unwrap();
    let result = write_atomic(&path, &after);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(&path).unwrap(), before);
    only_target(dir.path());
}

#[test]
fn missing_folder_fails_without_creating_anything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gone").join("a.selaschedule");
    assert!(write_atomic(&path, b"x").is_err());
    assert!(!path.exists());
}

fn wait(worker: &Worker) -> Reply {
    let start = Instant::now();
    loop {
        if let Some(reply) = worker.poll() {
            return reply;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no reply");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn worker_writes_and_reads_off_thread() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("w.selaschedule");
    let worker = Worker::start(default_writer()).unwrap();
    let original = file(vec![item(1, "A")]);
    assert!(worker.write(path.clone(), original.clone()));
    assert!(matches!(wait(&worker), Reply::Written(_, Ok(()))));
    assert!(worker.read(path));
    match wait(&worker) {
        Reply::Read(_, Ok(read)) => assert_eq!(read, original),
        other => panic!("{other:?}"),
    }
}

#[test]
fn worker_reports_write_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("w.selaschedule");
    let writer: Writer = Arc::new(|_, _| Err(io::Error::other("injected")));
    let worker = Worker::start(writer).unwrap();
    assert!(worker.write(path.clone(), file(Vec::new())));
    assert!(matches!(
        wait(&worker),
        Reply::Written(_, Err(WriteError::Io(_)))
    ));
    assert!(!path.exists());
}

#[test]
fn submitting_returns_while_a_write_blocks() {
    let release = Arc::new(AtomicBool::new(false));
    let gate = Arc::clone(&release);
    let writer: Writer = Arc::new(move |_, _| {
        while !gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    });
    let worker = Worker::start(writer).unwrap();
    let start = Instant::now();
    assert!(worker.write(PathBuf::from("x"), file(Vec::new())));
    assert!(start.elapsed() < Duration::from_millis(250));
    assert!(worker.poll().is_none());
    release.store(true, Ordering::SeqCst);
    assert!(matches!(wait(&worker), Reply::Written(_, Ok(()))));
}

#[test]
fn full_queue_backpressures() {
    let release = Arc::new(AtomicBool::new(false));
    let gate = Arc::clone(&release);
    let writer: Writer = Arc::new(move |_, _| {
        while !gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    });
    let worker = Worker::start(writer).unwrap();
    let accepted = (0..16)
        .filter(|_| worker.write(PathBuf::from("x"), file(Vec::new())))
        .count();
    assert!(accepted <= QUEUE + 1);
    release.store(true, Ordering::SeqCst);
}
