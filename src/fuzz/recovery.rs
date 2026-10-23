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
                br#"{"format":"sela-recovery-session","version":1,"state":"clean","started_at_ms":1,"acknowledged":9,"uncommitted":{"after":1,"since_ms":2}}"#.to_vec(),
            ],
            JSON_TOKENS,
        ),
    ) {
        check_session(&bytes);
    }
}

/// A session file is read exactly as written or not at all: an accepted one
/// holds the file's own format, version, known state, times and a complete
/// `uncommitted` block (or null), nothing defaulted or invented, and it is
/// written back identically by the next start (review round 2: the property
/// checked only determinism).
fn check_session(bytes: &[u8]) {
    use crate::autosave::{decode_session, encode_session};
    use serde_json::{Value, json};
    let decoded = within(Duration::from_secs(5), "session decode", || {
        decode_session(bytes)
    });
    assert_eq!(decoded, decode_session(bytes), "same input, same result");
    let Some(session) = decoded else { return };
    let file: Value = serde_json::from_slice(bytes).expect("an accepted session is JSON");
    let uncommitted = match &file["uncommitted"] {
        Value::Null => Value::Null,
        u => json!({"after": u["after"], "since_ms": u["since_ms"], "items": u["items"]}),
    };
    let expected = json!({
        "format": "sela-recovery-session",
        "version": 1,
        "state": file["state"],
        "started_at_ms": file["started_at_ms"],
        "acknowledged": file["acknowledged"],
        "uncommitted": uncommitted,
    });
    assert_eq!(file["format"], expected["format"]);
    assert_eq!(file["version"], expected["version"]);
    assert!(["running", "clean", "offered"].contains(&file["state"].as_str().unwrap_or("")));
    let encoded = encode_session(&session);
    let written: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(written, expected, "the session holds what the file says");
    assert_eq!(decode_session(&encoded), Some(session), "round trip");
}

#[test]
fn incomplete_or_unknown_session_files_are_not_read() {
    let session = |state: &str, uncommitted: &str| {
        format!(
            r#"{{"format":"sela-recovery-session","version":1,"state":{state},"started_at_ms":1,"acknowledged":2,"uncommitted":{uncommitted}}}"#
        )
    };
    for (what, bytes) in [
        (
            "no items",
            session(r#""offered""#, r#"{"after":1,"since_ms":2}"#),
        ),
        (
            "no after",
            session(r#""offered""#, r#"{"since_ms":2,"items":3}"#),
        ),
        (
            "text count",
            session(r#""offered""#, r#"{"after":1,"since_ms":2,"items":"3"}"#),
        ),
        (
            "negative",
            session(r#""offered""#, r#"{"after":-1,"since_ms":2,"items":3}"#),
        ),
        (
            "fraction",
            session(r#""offered""#, r#"{"after":1.5,"since_ms":2,"items":3}"#),
        ),
        ("not a block", session(r#""offered""#, "3")),
        ("unknown state", session(r#""paused""#, "null")),
        ("no state", session("null", "null")),
        (
            "other version",
            session(r#""clean""#, "null").replace(r#""version":1"#, r#""version":2"#),
        ),
        (
            "no acknowledged",
            session(r#""clean""#, "null").replace(r#""acknowledged":2,"#, ""),
        ),
    ] {
        assert_eq!(
            crate::autosave::decode_session(bytes.as_bytes()),
            None,
            "{what}"
        );
        check_session(bytes.as_bytes());
    }
    for good in [
        session(r#""running""#, "null"),
        session(
            r#""offered""#,
            r#"{"after":1,"since_ms":2,"items":3,"extra":0}"#,
        ),
    ] {
        assert!(
            crate::autosave::decode_session(good.as_bytes()).is_some(),
            "{good}"
        );
        check_session(good.as_bytes());
    }
}
