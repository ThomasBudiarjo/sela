//! Seeded property and fuzz suites for every parser and decoder of untrusted
//! input (M5-02). `docs/security.md` ("Fuzzing") lists the targets, their
//! invariants, the corpus layout and how to run a longer campaign.
//!
//! Each suite:
//! - runs with a fixed RNG seed, so a run is reproducible; `PROPTEST_RNG_SEED`
//!   and `PROPTEST_CASES` override the seed and the case count;
//! - writes a failing case to `tests/fixtures/fuzz/<target>.proptest-regressions`
//!   and replays it first on every later run;
//! - replays every file in `tests/fixtures/fuzz/<target>/` (the retained,
//!   minimized regression corpus) before generating cases, and mutates those
//!   files as seeds;
//! - checks bounded time on worst-case inputs at the parser's size limit.
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::test_runner::{FileFailurePersistence, RngSeed};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod bibles;
mod codecs;
mod fonts;
mod images;
mod media;
mod portable;
mod recovery;
mod schedule;
mod scripture;
mod settings;
mod songs;
mod transport;

/// A suite's configuration: `cases` generated inputs from the fixed `seed`,
/// unless the standard proptest environment variables say otherwise.
pub(crate) fn config(seed: u64, cases: u32, regressions: &'static str) -> ProptestConfig {
    let mut config = ProptestConfig::default();
    if config.rng_seed == RngSeed::Random {
        config.rng_seed = RngSeed::Fixed(seed);
    }
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = cases;
    }
    config.failure_persistence = Some(Box::new(FileFailurePersistence::Direct(regressions)));
    // Shrinking a slow case must not stall the suite for minutes.
    config.max_shrink_time = 60_000;
    config
}

pub(crate) fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Every regular file under `tests/fixtures/<dir>`, sorted by path.
pub(crate) fn files(dir: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let root = fixtures().join(dir);
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains("proptest-regressions"))
            {
                let bytes = std::fs::read(&path).expect("corpus file is readable");
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

/// The retained corpus of `target`: `tests/fixtures/fuzz/<target>/`.
pub(crate) fn corpus(target: &str) -> Vec<(PathBuf, Vec<u8>)> {
    files(&format!("fuzz/{target}"))
}

/// One byte-level change to a seed file.
#[derive(Clone, Debug)]
pub(crate) enum Edit {
    /// XOR the byte at the index with the mask (never zero).
    Flip(Index, u8),
    /// Insert a format token, so mutations stay near the grammar.
    Insert(Index, &'static str),
    Remove(Index, usize),
    /// Repeat the slice starting at the index, `count` times in place.
    Repeat(Index, usize, u8),
    Truncate(Index),
}

pub(crate) fn edits(tokens: &'static [&'static str]) -> impl Strategy<Value = Vec<Edit>> {
    let edit = prop_oneof![
        (any::<Index>(), 1u8..=255).prop_map(|(at, mask)| Edit::Flip(at, mask)),
        (any::<Index>(), proptest::sample::select(tokens)).prop_map(|(at, t)| Edit::Insert(at, t)),
        (any::<Index>(), 1usize..64).prop_map(|(at, len)| Edit::Remove(at, len)),
        (any::<Index>(), 1usize..32, 2u8..16).prop_map(|(at, len, n)| Edit::Repeat(at, len, n)),
        any::<Index>().prop_map(Edit::Truncate),
    ];
    proptest::collection::vec(edit, 1..8)
}

/// Applies `edits` in order; every index is taken modulo the current length.
pub(crate) fn apply(mut bytes: Vec<u8>, edits: &[Edit]) -> Vec<u8> {
    for edit in edits {
        let at = |index: &Index, len: usize| index.index(len + 1).min(len);
        match edit {
            Edit::Flip(index, mask) => {
                if !bytes.is_empty() {
                    let i = index.index(bytes.len());
                    bytes[i] ^= mask;
                }
            }
            Edit::Insert(index, token) => {
                let i = at(index, bytes.len());
                bytes.splice(i..i, token.bytes());
            }
            Edit::Remove(index, len) => {
                let i = at(index, bytes.len());
                let end = (i + len).min(bytes.len());
                bytes.drain(i..end);
            }
            Edit::Repeat(index, len, count) => {
                let i = at(index, bytes.len());
                let end = (i + len).min(bytes.len());
                let slice = bytes[i..end].to_vec();
                for _ in 1..*count {
                    bytes.splice(end..end, slice.iter().copied());
                }
            }
            Edit::Truncate(index) => {
                let i = at(index, bytes.len());
                bytes.truncate(i);
            }
        }
    }
    bytes
}

/// A seed file chosen from `seeds` with a few `edits` applied.
pub(crate) fn mutated(
    seeds: Vec<Vec<u8>>,
    tokens: &'static [&'static str],
) -> impl Strategy<Value = Vec<u8>> {
    assert!(!seeds.is_empty(), "a mutation target needs seed files");
    (proptest::sample::select(seeds), edits(tokens)).prop_map(|(seed, edits)| apply(seed, &edits))
}

/// Runs `work` and fails if it takes longer than `limit`. Limits are set at
/// several times the measured debug-build time on the 4-CPU Linux container,
/// so they catch quadratic or exponential behavior, not machine noise.
pub(crate) fn within<T>(limit: Duration, what: &str, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let result = work();
    let took = started.elapsed();
    assert!(
        took <= limit,
        "{what} took {took:?}, over its {limit:?} bound"
    );
    result
}

proptest! {
    #![proptest_config(config(0x5e1a_0001, 256, "tests/fixtures/fuzz/edits.proptest-regressions"))]

    #[test]
    fn edits_apply_at_any_length_without_panicking(
        len in 0usize..300,
        edits in edits(&["<a>", "", "\\par "]),
    ) {
        let bytes = apply(vec![b'x'; len], &edits);
        // Each edit at most adds 15 copies of a 31-byte slice or a token.
        prop_assert!(bytes.len() <= len + edits.len() * 15 * 31 + 16);
    }
}
