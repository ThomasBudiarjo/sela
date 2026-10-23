//! `.selaschedule` documents (`schedule_file::decode`). A schedule file is
//! the one document Sela opens from anywhere (USB sticks, mail, another
//! church's machine), so it is fully untrusted.
use super::codecs::{backgrounds, slide_formats, themes};
use super::{config, corpus, mutated, within};
use crate::arrangement::{Occurrence, OccurrenceId, SectionId, Variant, VariantId};
use crate::schedule::ItemTheme;
use crate::schedule_file::{
    self, Item, MAX_FILE_BYTES, MAX_TITLE_BYTES, ReadError, ScheduleFile, ScriptureItem, encode,
};
use crate::scripture::{Book, Snapshot, VerseId};
use crate::storage::{Id, MAX_ITEMS, Section, Song, Version};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use std::time::Duration;

fn check(bytes: &[u8]) {
    let decoded = within(Duration::from_secs(10), "schedule decode", || {
        schedule_file::decode(bytes)
    });
    assert_eq!(
        schedule_file::decode(bytes),
        decoded,
        "same input, same result"
    );
    match decoded {
        Ok(file) => {
            assert!(bytes.len() <= MAX_FILE_BYTES);
            assert!(file.items.len() <= MAX_ITEMS);
            assert!(file.title.len() <= MAX_TITLE_BYTES);
            assert!(file.items.len() + file.scripture.len() <= MAX_ITEMS);
            for item in &file.items {
                assert_eq!(item.song.validate(), Ok(()), "every opened song is valid");
                assert!(item.source.revision > 0);
                if let Some(id) = item.arrangement {
                    assert!(item.song.variants.iter().any(|v| v.id == id));
                }
                if let Some(theme) = &item.theme {
                    assert!(theme.theme.is_valid(), "every opened theme is valid");
                }
            }
            // Scripture items (version 5): valid snapshots at ascending
            // positions inside the schedule.
            let total = file.items.len() + file.scripture.len();
            for (n, item) in file.scripture.iter().enumerate() {
                assert!(item.snapshot.valid(), "every opened snapshot is valid");
                assert!(item.title.len() <= MAX_TITLE_BYTES);
                assert!(item.position < total);
                assert!(n == 0 || file.scripture[n - 1].position < item.position);
            }
            // What opened can be saved again, and reopens identically.
            let again = encode(&file).expect("an opened schedule can be saved");
            assert_eq!(schedule_file::decode(&again), Ok(file));
        }
        Err(ReadError::TooLarge) => assert!(bytes.len() > MAX_FILE_BYTES),
        Err(
            ReadError::Corrupt
            | ReadError::Newer(_)
            | ReadError::TooManyItems
            | ReadError::TitleTooLong,
        ) => {}
        Err(other) => panic!("decode never does I/O, but gave {other:?}"),
    }
}

fn id(tag: u8, index: usize, random: [u8; 16]) -> Id {
    let mut bytes = random;
    bytes[0] = tag;
    bytes[1] = index as u8;
    Id(bytes)
}

pub(crate) fn songs() -> impl Strategy<Value = Song> {
    let section = (
        proptest::option::of(prop_oneof![
            Just("Verse 1".to_string()),
            Just("Chorus".to_string()),
            "[^\\x00-\\x1f]{1,20}",
        ]),
        any::<String>(),
        proptest::option::of(slide_formats()),
        proptest::option::of(backgrounds()),
    );
    let variant = (
        "[A-Za-z0-9][A-Za-z0-9 ]{0,15}",
        proptest::collection::vec(any::<prop::sample::Index>(), 0..10),
    );
    (
        (
            "[^\\x00-\\x1f]{0,30}[A-Za-z]",
            any::<String>(),
            any::<String>(),
            any::<String>(),
            "[A-Za-z &]{0,20}",
            "[0-9]{0,12}",
        ),
        proptest::collection::vec(section, 1..6),
        proptest::collection::vec(variant, 0..3),
        proptest::option::of(backgrounds()),
        any::<[u8; 16]>(),
    )
        .prop_map(
            |(
                (title, authors, copyright, license, administrator, song_number),
                sections,
                variants,
                master,
                random,
            )| {
                let sections: Vec<Section> = sections
                    .into_iter()
                    .enumerate()
                    .map(|(i, (label, lyrics, format, background))| Section {
                        id: SectionId(id(1, i, random)),
                        label: label.unwrap_or_default(),
                        lyrics,
                        format: format.unwrap_or_default(),
                        background,
                    })
                    .collect();
                let variants = variants
                    .into_iter()
                    .enumerate()
                    .map(|(v, (name, picks))| Variant {
                        id: VariantId(id(2, v, random)),
                        name,
                        occurrences: picks
                            .iter()
                            .enumerate()
                            .map(|(o, pick)| Occurrence {
                                id: OccurrenceId(id(3 + v as u8, o, random)),
                                section: sections[pick.index(sections.len())].id,
                            })
                            .collect(),
                    })
                    .collect();
                Song {
                    title,
                    authors,
                    copyright,
                    license,
                    administrator,
                    song_number,
                    sections,
                    variants,
                    master,
                }
            },
        )
        .prop_filter("a valid song", |song| song.validate().is_ok())
}

/// Pinned verses as a scripture item stores them (sorted, unique, text).
pub(crate) fn snapshots() -> impl Strategy<Value = Snapshot> {
    let verse = (1u8..=66, 1u16..=150, 1u16..=176, "[^\\x00]{1,60}");
    (
        "[A-Z0-9][A-Z0-9_-]{0,15}",
        any::<String>(),
        "[a-z]{0,3}",
        any::<bool>(),
        proptest::collection::vec(verse, 1..6),
    )
        .prop_map(|(translation, name, language, redistributable, verses)| {
            let mut verses: Vec<(VerseId, String)> = verses
                .into_iter()
                .filter_map(|(book, chapter, verse, text)| {
                    Some((
                        VerseId::new(Book::from_number(book)?, chapter, verse)?,
                        text,
                    ))
                })
                .collect();
            verses.sort_by_key(|(id, _)| *id);
            verses.dedup_by_key(|(id, _)| *id);
            Snapshot {
                translation,
                name,
                language,
                redistributable,
                verses,
            }
        })
        .prop_filter("a valid snapshot", Snapshot::valid)
}

#[derive(Clone, Debug)]
enum Entry {
    Song(Box<Item>),
    Scripture(String, Snapshot),
}

fn schedules() -> impl Strategy<Value = ScheduleFile> {
    let item = (
        songs(),
        any::<[u8; 16]>(),
        1..=i64::MAX,
        proptest::option::of(any::<prop::sample::Index>()),
        proptest::option::of((themes(), any::<bool>())),
    )
        .prop_map(|(song, id, revision, pick, theme)| {
            let arrangement = pick
                .filter(|_| !song.variants.is_empty())
                .map(|pick| pick.get(&song.variants).id);
            Entry::Song(Box::new(Item {
                source: Version {
                    id: Id(id),
                    revision,
                },
                song,
                arrangement,
                theme: theme.map(|(theme, explicit)| ItemTheme { theme, explicit }),
            }))
        });
    let scripture = ("[^\\x00]{0,40}", snapshots())
        .prop_map(|(title, snapshot)| Entry::Scripture(title, snapshot));
    let entry = prop_oneof![3 => item, 1 => scripture];
    ("[^\\x00]{0,40}", proptest::collection::vec(entry, 0..5)).prop_map(|(title, entries)| {
        let mut file = ScheduleFile {
            title,
            ..Default::default()
        };
        for (position, entry) in entries.into_iter().enumerate() {
            match entry {
                Entry::Song(item) => file.items.push(*item),
                Entry::Scripture(title, snapshot) => file.scripture.push(ScriptureItem {
                    position,
                    title,
                    snapshot,
                }),
            }
        }
        file
    })
}

const JSON_TOKENS: &[&str] = &[
    "{",
    "}",
    "[",
    "]",
    ",",
    ":",
    "\"",
    "\\",
    "\\u0000",
    "\\ud800",
    "null",
    "true",
    "0",
    "-1",
    "1e999",
    "18446744073709551616",
    "\"song\"",
    "\"kind\"",
    "\"items\"",
    "\"version\":3",
    "\"version\":4",
    "\"version\":5",
    "\"kind\":\"scripture\"",
    "\"verses\":[[43003016,\"x\"]]",
    "\"theme\":\"01\"",
    "\"theme_explicit\":true",
    "\"arrangement\":\"00\"",
    "\"format\":\"zz\"",
    "\"background\":\"0101\"",
    "\"+f\"",
    " ",
    "\u{feff}",
];

fn seeds() -> Vec<Vec<u8>> {
    let mut seeds: Vec<Vec<u8>> = corpus("schedule").into_iter().map(|(_, b)| b).collect();
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    for _ in 0..8 {
        let file = schedules()
            .new_tree(&mut runner)
            .expect("a schedule")
            .current();
        seeds.push(encode(&file).expect("a generated schedule encodes"));
    }
    seeds
}

proptest! {
    #![proptest_config(config(0x5e1a_0401, 256, "tests/fixtures/fuzz/schedule.proptest-regressions"))]

    #[test]
    fn written_schedules_reopen_identically(file in schedules()) {
        let bytes = encode(&file).unwrap();
        prop_assert_eq!(schedule_file::decode(&bytes), Ok(file));
        check(&bytes);
    }

    #[test]
    fn mutated_schedules_open_valid_or_not_at_all(bytes in mutated(seeds(), JSON_TOKENS)) {
        check(&bytes);
    }

    #[test]
    fn arbitrary_bytes_never_open_invalid_songs(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        check(&bytes);
    }
}

#[test]
fn worst_case_files_finish_within_their_time_bound() {
    let head = "{\"format\":\"sela-schedule\",\"version\":3,\"title\":\"t\",\"items\":";
    let fill = |prefix: &str, unit: &str, suffix: &str| {
        let room = MAX_FILE_BYTES - prefix.len() - suffix.len();
        format!("{prefix}{}{suffix}", unit.repeat(room / unit.len()))
    };
    for input in [
        fill("", "[", ""),
        fill(head, "[", ""),
        fill(head, "{\"a\":", ""),
        fill(&format!("{head}["), "0,", "0]}"),
        fill(&format!("{head}["), "{},", "{}]}"),
        fill(
            "{\"format\":\"sela-schedule\",\"version\":3,\"title\":\"",
            "x",
            "\",\"items\":[]}",
        ),
        fill(
            &format!("{head}[{{\"kind\":\"song\",\"song\":{{\"sections\":["),
            "{},",
            "{}]}}]}",
        ),
    ] {
        check(input.as_bytes());
    }
    check(&vec![b' '; MAX_FILE_BYTES + 1]);
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (_, bytes) in corpus("schedule") {
        check(&bytes);
    }
}
