//! Autosave revisions (`autosave::decode_revision`): files in
//! `<profile>/Recovery/` that a crash, a full disk or a hand edit may have
//! damaged, and that the recovery prompt reads at the next start.
use super::codecs::themes;
use super::schedule::snapshots;
use super::{config, edits, within};
use crate::arrangement::VariantId;
use crate::autosave::{
    Entry, MAX_REVISION_BYTES, Revision, Snapshot, decode_revision, encode_revision,
};
use crate::schedule::{ItemTheme, Pin};
use crate::schedule_file::MAX_TITLE_BYTES;
use crate::storage::{Id, MAX_ITEMS, Version};
use proptest::prelude::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn check(bytes: &[u8]) {
    let decoded = within(Duration::from_secs(5), "revision decode", || {
        decode_revision(bytes)
    });
    assert_eq!(decode_revision(bytes), decoded, "same input, same result");
    let Some(revision) = decoded else { return };
    assert!(bytes.len() as u64 <= MAX_REVISION_BYTES);
    assert!(revision.sequence > 0);
    let snapshot = &revision.snapshot;
    assert!(snapshot.entries.len() <= MAX_ITEMS && snapshot.baseline.len() <= MAX_ITEMS);
    for entry in &snapshot.entries {
        assert!(entry.title.len() <= MAX_TITLE_BYTES);
        assert!(entry.pin.version.revision > 0);
        if let Some(theme) = &entry.theme {
            assert!(theme.theme.is_valid());
        }
        if let Some(verses) = &entry.scripture {
            assert!(verses.valid());
            assert_eq!(verses.version(), entry.pin.version, "verses match the pin");
        }
    }
    // A revision that was offered can be committed again and reads back
    // identically (the recovery prompt restores from it).
    assert_eq!(decode_revision(&encode_revision(&revision)), Some(revision));
}

/// The body of a revision file with its checksum line recomputed, so edits
/// reach the JSON reader instead of stopping at the checksum.
fn resealed(body: &[u8]) -> Vec<u8> {
    let mut out = body.to_vec();
    out.push(b'\n');
    let digest: String = Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    out.extend_from_slice(format!("sha256 {digest}\n").as_bytes());
    out
}

fn body(file: &[u8]) -> &[u8] {
    let end = file.iter().position(|&b| b == b'\n').unwrap_or(file.len());
    &file[..end]
}

fn pins() -> impl Strategy<Value = Pin> {
    (
        any::<[u8; 16]>(),
        1..=i64::MAX,
        proptest::option::of(any::<[u8; 16]>()),
        proptest::option::of(any::<u64>()),
    )
        .prop_map(|(id, revision, arrangement, theme)| Pin {
            version: Version {
                id: Id(id),
                revision,
            },
            arrangement: arrangement.map(|a| VariantId(Id(a))),
            theme,
        })
}

fn entries() -> impl Strategy<Value = Entry> {
    let song = (
        pins(),
        "[^\\x00]{0,40}",
        proptest::option::of((themes(), any::<bool>())),
    )
        .prop_map(|(pin, title, theme)| Entry {
            pin,
            title,
            theme: theme.map(|(theme, explicit)| ItemTheme { theme, explicit }),
            scripture: None,
        });
    let scripture = ("[^\\x00]{0,40}", snapshots()).prop_map(|(title, verses)| Entry {
        pin: Pin {
            version: verses.version(),
            arrangement: None,
            theme: None,
        },
        title,
        theme: None,
        scripture: Some(Arc::new(verses)),
    });
    prop_oneof![3 => song, 1 => scripture]
}

fn revisions() -> impl Strategy<Value = Revision> {
    (
        1..=u64::MAX,
        any::<u64>(),
        proptest::option::of("/[a-zA-Z0-9 ._/-]{0,60}\\.selaschedule"),
        proptest::collection::vec(entries(), 0..5),
        proptest::collection::vec(pins(), 0..4),
    )
        .prop_map(|(sequence, saved_at, path, entries, baseline)| Revision {
            sequence,
            saved_at,
            snapshot: Snapshot {
                path: path.map(PathBuf::from),
                entries,
                baseline,
            },
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
    "null",
    "true",
    "0",
    "-1",
    "18446744073709551616",
    "\"version\":1",
    "\"version\":2",
    "\"version\":4",
    "\"items\":[]",
    "\"scripture\":{}",
    "\"theme\":\"01\"",
    "\"theme_stamp\":-1",
    "\"arrangement\":\"0\"",
    "\"path\":1",
    "\"song_id\":\"zz\"",
    "\\u0000",
    "\\ud800",
];

proptest! {
    #![proptest_config(config(0x5e1a_0c01, 256, "tests/fixtures/fuzz/recovery.proptest-regressions"))]

    #[test]
    fn committed_revisions_read_back_exactly(revision in revisions()) {
        let bytes = encode_revision(&revision);
        prop_assert_eq!(decode_revision(&bytes), Some(revision));
        check(&bytes);
    }

    #[test]
    fn edited_revisions_with_a_matching_checksum_read_valid_or_not_at_all(
        revision in revisions(),
        edits in edits(JSON_TOKENS),
    ) {
        let file = encode_revision(&revision);
        let edited = super::apply(body(&file).to_vec(), &edits);
        check(&resealed(&edited));
        // Without a matching checksum nothing is read at all.
        let damaged = super::apply(file.clone(), &edits);
        if damaged != file {
            check(&damaged);
        }
    }

    #[test]
    fn arbitrary_bytes_read_valid_or_not_at_all(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        check(&bytes);
        check(&resealed(&bytes));
    }
}

#[test]
fn worst_case_revisions_finish_within_their_time_bound() {
    let limit = MAX_REVISION_BYTES as usize;
    let fill = |prefix: &str, unit: &str, suffix: &str| {
        let room = limit - 80 - prefix.len() - suffix.len();
        format!("{prefix}{}{suffix}", unit.repeat(room / unit.len()))
    };
    for body in [
        fill("", "[", ""),
        fill(
            "{\"format\":\"sela-recovery\",\"version\":3,\"items\":",
            "[",
            "",
        ),
        fill(
            "{\"format\":\"sela-recovery\",\"version\":3,\"sequence\":1,\"saved_at_ms\":0,\"path\":null,\"baseline\":[],\"items\":[",
            "{\"title\":\"t\"},",
            "{}]}",
        ),
        fill("{\"a\":\"", "x", "\"}"),
    ] {
        check(&resealed(body.as_bytes()));
    }
    check(&vec![b' '; limit + 1]);
}

proptest! {
    #![proptest_config(config(0x5e1a_0c02, 256, "tests/fixtures/fuzz/recovery-session.proptest-regressions"))]

    // `Recovery/session.json`: the run state the next start reads first.
    #[test]
    fn session_files_read_valid_or_not_at_all(
        bytes in super::mutated(
            vec![
                br#"{"format":"sela-recovery-session","version":1,"state":"running","started_at_ms":1,"acknowledged":0,"uncommitted":null}"#.to_vec(),
                br#"{"format":"sela-recovery-session","version":1,"state":"offered","started_at_ms":1,"acknowledged":2,"uncommitted":{"after":1,"since_ms":2,"items":3}}"#.to_vec(),
            ],
            JSON_TOKENS,
        ),
    ) {
        let first = within(Duration::from_secs(5), "session decode", || {
            crate::autosave::decode_session(&bytes)
        });
        prop_assert_eq!(first, crate::autosave::decode_session(&bytes));
    }
}
