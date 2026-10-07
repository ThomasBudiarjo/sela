//! Database-only song search benchmark (M1-07): builds a synthetic library of
//! original pseudo-word lyrics with a fixed seed in a temporary directory, then
//! times `Repository::search` over a fixed query set. No UI or rendering is
//! measured. Run in release mode:
//! `cargo run --locked --release --example search_bench [-- --songs N]`.
use sela::{
    arrangement::SectionId,
    storage::{Repository, Section, Song},
};
use std::time::{Duration, Instant};

const SEED: u64 = 0x5e1a_0007;
const VOCABULARY: usize = 6000;
const WARMUP: usize = 3;
const RUNS: usize = 100;

/// splitmix64: small, fixed and good enough for a reproducible corpus.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Pseudo-words from consonant-vowel syllables; a few carry an accent so the
/// diacritic folding path is exercised.
fn vocabulary(rng: &mut Rng) -> Vec<String> {
    const ONSETS: [&str; 16] = [
        "b", "d", "g", "h", "k", "l", "m", "n", "p", "r", "s", "t", "w", "y", "ng", "s",
    ];
    const VOWELS: [&str; 6] = ["a", "e", "i", "o", "u", "é"];
    let mut words = std::collections::BTreeSet::new();
    let mut ordered = Vec::with_capacity(VOCABULARY);
    while ordered.len() < VOCABULARY {
        let syllables = 1 + rng.below(3);
        let mut word = String::new();
        for _ in 0..syllables {
            word.push_str(ONSETS[rng.below(ONSETS.len())]);
            // The accented vowel is rare.
            let vowel = if rng.below(40) == 0 { 5 } else { rng.below(5) };
            word.push_str(VOWELS[vowel]);
        }
        if rng.below(4) == 0 {
            word.push_str(["n", "k", "h", "t"][rng.below(4)]);
        }
        if words.insert(word.clone()) {
            ordered.push(word);
        }
    }
    ordered
}

/// Skewed toward the start of the vocabulary, so a few words are very common.
fn word<'a>(rng: &mut Rng, words: &'a [String]) -> &'a str {
    &words[((rng.unit().powf(2.5)) * words.len() as f64) as usize]
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn song(rng: &mut Rng, words: &[String]) -> Song {
    let title = (0..2 + rng.below(4))
        .map(|_| capitalized(word(rng, words)))
        .collect::<Vec<_>>()
        .join(" ");
    let sections = (0..3 + rng.below(4))
        .map(|index| {
            let lyrics = (0..4)
                .map(|_| {
                    let line = (0..5 + rng.below(4))
                        .map(|_| word(rng, words).to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    // Some punctuation, as typed lyrics have.
                    match rng.below(6) {
                        0 => format!("{line},"),
                        1 => format!("{line}!"),
                        _ => line,
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Section {
                id: SectionId::allocate(),
                label: format!("Verse {}", index + 1),
                lyrics,
                format: Default::default(),
                background: None,
            }
        })
        .collect();
    Song {
        title,
        authors: format!(
            "{} {}",
            capitalized(word(rng, words)),
            capitalized(word(rng, words))
        ),
        copyright: format!(
            "© {} {}",
            1950 + rng.below(75),
            capitalized(word(rng, words))
        ),
        license: String::new(),
        sections,
        variants: Vec::new(),
        master: None,
    }
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn ms(d: Duration) -> String {
    format!("{:.3}", d.as_secs_f64() * 1000.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: search_bench [--songs N] [--keep NEW-DATABASE-PATH]";
    let mut songs = 20_000usize;
    let mut keep = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--songs") => songs = args.next().ok_or(USAGE)?.to_str().ok_or(USAGE)?.parse()?,
            Some("--keep") => keep = Some(std::path::PathBuf::from(args.next().ok_or(USAGE)?)),
            _ => return Err(USAGE.into()),
        }
    }
    if songs < 2000 {
        return Err("--songs must be at least 2000".into());
    }
    let directory = tempfile::tempdir()?;
    let path = match keep {
        Some(path) if path.exists() => return Err(format!("{} exists", path.display()).into()),
        Some(path) => path,
        None => directory.path().join("library.sqlite"),
    };
    let mut rng = Rng(SEED);
    let words = vocabulary(&mut rng);
    let mut repository = Repository::open(&path).map_err(|e| format!("{e:?}"))?;
    let mut saves = Vec::with_capacity(songs);
    let mut titles = Vec::with_capacity(songs);
    let mut lyric_lines = Vec::new();
    let build = Instant::now();
    for index in 0..songs {
        let song = song(&mut rng, &words);
        titles.push(song.title.clone());
        if index % 1000 == 0 {
            lyric_lines.push(song.sections[0].lyrics.lines().next().unwrap().to_string());
        }
        let start = Instant::now();
        repository
            .save_song(None, song)
            .map_err(|e| format!("{e:?}"))?;
        saves.push(start.elapsed());
    }
    let build = build.elapsed();
    saves.sort();
    println!(
        "corpus: {songs} songs, seed {SEED:#x}, {} vocabulary words, database {:.1} MiB",
        words.len(),
        std::fs::metadata(&path)?.len() as f64 / (1024.0 * 1024.0)
    );
    println!(
        "save_song (with index): total {:.1} s, p50 {} ms, p95 {} ms, max {} ms",
        build.as_secs_f64(),
        ms(percentile(&saves, 50.0)),
        ms(percentile(&saves, 95.0)),
        ms(*saves.last().unwrap())
    );
    drop(repository);

    let start = Instant::now();
    let mut repository = Repository::open(&path).map_err(|e| format!("{e:?}"))?;
    println!(
        "open (quick_check incl. FTS5 check, index consistency): {} ms",
        ms(start.elapsed())
    );
    let start = Instant::now();
    let healthy = repository.check_search().map_err(|e| format!("{e:?}"))?;
    println!(
        "check_search: {} ms, healthy {healthy}",
        ms(start.elapsed())
    );
    let start = Instant::now();
    let state = repository.rebuild_search().map_err(|e| format!("{e:?}"))?;
    println!("rebuild_search: {} ms, {state:?}", ms(start.elapsed()));

    // Fixed query set, derived only from the seeded corpus.
    let accented = words
        .iter()
        .find(|w| w.contains('é'))
        .ok_or("no accented word")?
        .replace('é', "e");
    let long_line = lyric_lines[1].clone();
    let queries: Vec<(&str, String)> = vec![
        ("most common word", words[0].clone()),
        ("common word", words[20].clone()),
        ("mid-frequency word", words[600].clone()),
        ("rare word", words[VOCABULARY - 1].clone()),
        ("two common words", format!("{} {}", words[0], words[1])),
        ("exact title", titles[songs / 2].clone()),
        (
            "title prefix",
            titles[songs / 3]
                .chars()
                .take(6)
                .collect::<String>()
                .trim_end()
                .to_string(),
        ),
        ("one-letter prefix", "k".into()),
        ("two-letter prefix", "ma".into()),
        ("punctuation", format!("{}, {}!", words[3], words[4])),
        ("unaccented form of an accented word", accented),
        ("no match", "zzqxv".into()),
        ("FTS syntax as text", "\" NEAR( * AND title:".into()),
        ("lyric line", long_line),
    ];
    let mut all = Vec::with_capacity(queries.len() * RUNS);
    let mut generation = 0u64;
    println!(
        "\n{:<38} {:>6} {:>5} {:>9} {:>9} {:>9}",
        "query", "hits", "trunc", "p50 ms", "p95 ms", "max ms"
    );
    for (name, query) in &queries {
        let mut samples = Vec::with_capacity(RUNS);
        let mut last = None;
        for run in 0..WARMUP + RUNS {
            generation += 1;
            let start = Instant::now();
            let results = repository
                .search(query, generation)
                .map_err(|e| format!("{name}: {e:?}"))?;
            let elapsed = start.elapsed();
            if run >= WARMUP {
                samples.push(elapsed);
            }
            last = Some(results);
        }
        let results = last.unwrap();
        all.extend_from_slice(&samples);
        samples.sort();
        println!(
            "{:<38} {:>6} {:>5} {:>9} {:>9} {:>9}",
            name,
            results.hits.len(),
            results.truncated,
            ms(percentile(&samples, 50.0)),
            ms(percentile(&samples, 95.0)),
            ms(*samples.last().unwrap())
        );
    }
    all.sort();
    println!(
        "\nall {} samples: p50 {} ms, p95 {} ms, max {} ms (target 30 ms, p95 ceiling 100 ms)",
        all.len(),
        ms(percentile(&all, 50.0)),
        ms(percentile(&all, 95.0)),
        ms(*all.last().unwrap())
    );
    Ok(())
}
