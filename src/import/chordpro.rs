//! ChordPro song parser (M4-07). Each line is a `{directive}`, a `#` comment
//! or lyrics with inline `[chord]`s; the rules are in docs/import.md
//! ("OpenSong and ChordPro").
use super::{
    MAX_ORDER, MAX_SECTIONS, MAX_SLIDES, Parsed, ParsedSection, ParsedSong, Rejected, Warning,
    decode,
};

pub fn parse(bytes: &[u8]) -> Parsed {
    let text = decode(bytes)?;
    let mut builder = Builder::default();
    for line in text.lines() {
        builder.line(line);
    }
    builder.finish_section();
    let Builder {
        mut song,
        subtitles,
        order,
        repeated,
        ..
    } = builder;
    if song.sections.is_empty() {
        return Err(Rejected::NoLyrics);
    }
    if song.authors.is_empty() {
        song.authors = subtitles;
    } else if !subtitles.is_empty() {
        warn(&mut song, Warning::UnknownDirective("subtitle".into()));
    }
    if repeated {
        song.order = order;
    }
    if song.title.is_empty() {
        warn(&mut song, Warning::MissingTitle);
    }
    Ok(song)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Verse,
    Chorus,
    Bridge,
}

/// Where the next lyric line goes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Env {
    /// Outside any environment: a blank line ends the verse.
    #[default]
    Stanza,
    /// Inside `{start_of_*}`, with its label argument: a blank line ends the
    /// slide.
    Section(Kind, Option<String>),
    /// Inside a tab or grid, whose lines are notation, not lyrics.
    Skip,
}

#[derive(Default)]
struct Builder {
    song: ParsedSong,
    env: Env,
    /// Whether the open environment has produced a section yet.
    env_produced: bool,
    slides: Vec<String>,
    lines: Vec<String>,
    /// Sections of each `Kind` so far, for default labels.
    counts: [u32; 3],
    slide_count: usize,
    /// Every section in file order plus `{chorus}` repeats; becomes the
    /// song's order only when a repeat happened.
    order: Vec<usize>,
    repeated: bool,
    last_chorus: Option<usize>,
    subtitles: Vec<String>,
}

impl Builder {
    fn line(&mut self, raw: &str) {
        if raw.starts_with('#') {
            return;
        }
        let line = raw.trim();
        if let Some(body) = line.strip_prefix('{') {
            let body = body.strip_suffix('}').unwrap_or(body).trim();
            let end = body
                .find(|c: char| c == ':' || c.is_whitespace())
                .unwrap_or(body.len());
            let (name, rest) = body.split_at(end);
            let rest = rest.trim_start();
            let value = rest.strip_prefix(':').unwrap_or(rest).trim();
            self.directive(&name.to_ascii_lowercase(), value);
            return;
        }
        if self.env == Env::Skip {
            if !line.is_empty() {
                warn(&mut self.song, Warning::ChordsRemoved);
            }
            return;
        }
        if line.is_empty() {
            match self.env {
                Env::Stanza => self.finish_section(),
                _ => self.finish_slide(),
            }
            return;
        }
        let (text, had_chords) = strip_chords(line);
        if had_chords {
            warn(&mut self.song, Warning::ChordsRemoved);
            // A line of chords alone is not a blank line between verses.
            if text.is_empty() {
                return;
            }
        }
        self.lines.push(text);
    }

    fn directive(&mut self, name: &str, value: &str) {
        match name {
            "title" | "t" => {
                if self.song.title.is_empty() {
                    self.song.title = value.to_string();
                }
            }
            "subtitle" | "st" => {
                if !value.is_empty() {
                    self.subtitles.push(value.to_string());
                }
            }
            "artist" | "composer" | "lyricist" => {
                if !value.is_empty() && !self.song.authors.iter().any(|a| a == value) {
                    self.song.authors.push(value.to_string());
                }
            }
            "copyright" => {
                if self.song.copyright.is_empty() {
                    self.song.copyright = value.to_string();
                }
            }
            "ccli" if crate::storage::song_number_ok(value) => {
                self.song.song_number = value.to_string();
            }
            "ccli" => warn(&mut self.song, Warning::InvalidField("ccli".into())),
            "meta" => {
                let (key, rest) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
                let key = key.to_ascii_lowercase();
                match key.as_str() {
                    "title" | "subtitle" | "artist" | "composer" | "lyricist" | "copyright"
                    | "ccli" => self.directive(&key, rest.trim()),
                    _ => warn(&mut self.song, Warning::UnknownDirective(key)),
                }
            }
            "comment" | "c" | "comment_italic" | "ci" | "comment_box" | "cb" | "highlight" => {}
            "start_of_chorus" | "soc" => self.open(Env::Section(Kind::Chorus, env_label(value))),
            "start_of_verse" | "sov" => self.open(Env::Section(Kind::Verse, env_label(value))),
            "start_of_bridge" | "sob" => self.open(Env::Section(Kind::Bridge, env_label(value))),
            "start_of_tab" | "sot" | "start_of_grid" | "sog" => self.open(Env::Skip),
            "end_of_chorus" | "eoc" | "end_of_verse" | "eov" | "end_of_bridge" | "eob"
            | "end_of_tab" | "eot" | "end_of_grid" | "eog" => self.open(Env::Stanza),
            "chorus" => {
                self.finish_section();
                match self.last_chorus {
                    Some(index) => {
                        self.repeated = true;
                        self.push_order(index);
                    }
                    None => warn(
                        &mut self.song,
                        Warning::MissingOrderReference("chorus".into()),
                    ),
                }
            }
            _ => warn(&mut self.song, Warning::UnknownDirective(name.to_string())),
        }
    }

    fn open(&mut self, env: Env) {
        self.finish_section();
        self.env = env;
        self.env_produced = false;
    }

    fn finish_slide(&mut self) {
        if !self.lines.is_empty() {
            self.slides.push(self.lines.join("\n"));
            self.lines.clear();
        }
    }

    fn finish_section(&mut self) {
        self.finish_slide();
        let mut slides = std::mem::take(&mut self.slides);
        let (kind, label) = match &self.env {
            Env::Stanza => (Kind::Verse, None),
            Env::Section(kind, label) => (*kind, label.clone()),
            Env::Skip => return,
        };
        let count = &mut self.counts[kind as usize];
        let label = label.unwrap_or_else(|| default_label(kind, *count + 1));
        if slides.is_empty() {
            if matches!(self.env, Env::Section(..)) && !self.env_produced {
                warn(&mut self.song, Warning::EmptySection(label));
            }
            return;
        }
        *count += 1;
        self.env_produced = true;
        let room = MAX_SLIDES - self.slide_count;
        if self.song.sections.len() >= MAX_SECTIONS || room == 0 {
            warn(&mut self.song, Warning::LimitReached);
            return;
        }
        if slides.len() > room {
            slides.truncate(room);
            warn(&mut self.song, Warning::LimitReached);
        }
        self.slide_count += slides.len();
        self.song.sections.push(ParsedSection { label, slides });
        let index = self.song.sections.len() - 1;
        if kind == Kind::Chorus {
            self.last_chorus = Some(index);
        }
        self.push_order(index);
    }

    fn push_order(&mut self, index: usize) {
        if self.order.len() < MAX_ORDER {
            self.order.push(index);
        } else {
            warn(&mut self.song, Warning::LimitReached);
        }
    }
}

/// "Verse 1", "Verse 2"; "Chorus", "Chorus 2"; "Bridge", "Bridge 2".
fn default_label(kind: Kind, number: u32) -> String {
    match (kind, number) {
        (Kind::Verse, n) => format!("Verse {n}"),
        (Kind::Chorus, 1) => "Chorus".into(),
        (Kind::Chorus, n) => format!("Chorus {n}"),
        (Kind::Bridge, 1) => "Bridge".into(),
        (Kind::Bridge, n) => format!("Bridge {n}"),
    }
}

/// `{soc: Refrain}` and `{soc label="Refrain"}` both name the section.
fn env_label(value: &str) -> Option<String> {
    let label = match value.strip_prefix("label=") {
        Some(quoted) => quoted.trim().trim_matches(['"', '\'']).trim(),
        None => value,
    };
    (!label.is_empty()).then(|| label.to_string())
}

/// Removes every `[chord]`. Brackets nest, so `x[A[B]]y` is `xy` and no
/// pair is left behind to read as a new chord; an unmatched `[` and the text
/// after it stay. When a chord was removed, the spaces it left are collapsed.
fn strip_chords(line: &str) -> (String, bool) {
    let mut out = String::with_capacity(line.len());
    let mut opens = Vec::new();
    let mut removed = false;
    for c in line.chars() {
        match (c, opens.last()) {
            (']', Some(&start)) => {
                out.truncate(start);
                opens.pop();
                removed = true;
            }
            _ => {
                if c == '[' {
                    opens.push(out.len());
                }
                out.push(c);
            }
        }
    }
    if removed {
        out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    (out, removed)
}

fn warn(song: &mut ParsedSong, warning: Warning) {
    if !song.warnings.contains(&warning) {
        song.warn(warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::MAX_FILE_BYTES;
    use proptest::prelude::*;
    use proptest::test_runner::FileFailurePersistence;

    const HARBOR_LIGHT: &[u8] =
        include_bytes!("../../tests/fixtures/import/chordpro/harbor-light.cho");
    const UNICODE: &[u8] =
        include_bytes!("../../tests/fixtures/import/chordpro/unicode-lyrics.cho");

    fn section(label: &str, slides: &[&str]) -> ParsedSection {
        ParsedSection {
            label: label.into(),
            slides: slides.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn parse_str(text: &str) -> ParsedSong {
        parse(text.as_bytes()).unwrap()
    }

    fn labels(song: &ParsedSong) -> Vec<&str> {
        song.sections.iter().map(|s| s.label.as_str()).collect()
    }

    fn sorted(warnings: Vec<Warning>) -> Vec<String> {
        let mut out: Vec<String> = warnings.iter().map(|w| format!("{w:?}")).collect();
        out.sort();
        out
    }

    #[test]
    fn directives_chorus_and_inline_chords_import() {
        let song = parse(HARBOR_LIGHT).unwrap();
        assert_eq!(song.title, "Harbor Light");
        assert_eq!(song.authors, ["Mara Quinn"]);
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &["Out on the water the evening is long\nThe harbor light carries a song"]
                ),
                section(
                    "Chorus",
                    &["Shine on, shine on\nGuide every boat back home"]
                ),
                section(
                    "Verse 2",
                    &[
                        "Morning will find us with nets on the shore\nGrateful for all we were given and more"
                    ]
                ),
            ]
        );
        assert_eq!(song.order, [0, 1, 2, 1]);
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::UnknownDirective("key".into()),
                Warning::ChordsRemoved
            ])
        );
    }

    #[test]
    fn indonesian_accented_and_cjk_text_is_kept_exactly() {
        let song = parse(UNICODE).unwrap();
        assert_eq!(song.title, "Terang Pagi — Lagu Syukur");
        assert_eq!(song.authors, ["Rani Siregar"]);
        assert_eq!(song.copyright, "© 2026 Sela Test Fixtures");
        assert_eq!(
            song.sections,
            [
                section(
                    "Verse 1",
                    &["Terang pagi datang perlahan\nHati kami penuh sukacita"]
                ),
                section(
                    "Chorus",
                    &[
                        "Señor, tú das la canción, ¡qué alegría!\nMon cœur est léger, naïf et joyeux\n光の中で歌おう\n我们一起歌唱"
                    ]
                ),
            ]
        );
        assert_eq!(song.warnings, [Warning::ChordsRemoved]);
    }

    #[test]
    fn crlf_and_bom_parse_like_lf() {
        let lf = std::str::from_utf8(HARBOR_LIGHT)
            .unwrap()
            .replace("\r\n", "\n");
        let mut crlf = b"\xEF\xBB\xBF".to_vec();
        crlf.extend_from_slice(lf.replace('\n', "\r\n").as_bytes());
        assert_eq!(parse(&crlf).unwrap(), parse_str(&lf));
    }

    #[test]
    fn comments_never_become_lyrics() {
        let song = parse_str(
            "{title: T}\n{c: Slowly}\n{ci: Softer}\n{cb: Key change}\n{highlight: All}\n# note\nOnly line",
        );
        assert_eq!(song.sections, [section("Verse 1", &["Only line"])]);
        assert!(song.warnings.is_empty());
    }

    #[test]
    fn chords_at_line_end_nested_and_unclosed_brackets() {
        let song = parse_str(
            "{t: T}\nAmazing grace[G]\nx[A[B]]y\nOpen [G line\n[G]   [C]\nEnd [D] here [Em7/B]",
        );
        assert_eq!(
            song.sections,
            [section(
                "Verse 1",
                &["Amazing grace\nxy\nOpen [G line\nEnd here"]
            )]
        );
        assert_eq!(song.warnings, [Warning::ChordsRemoved]);
    }

    #[test]
    fn an_unclosed_bracket_alone_is_text_without_a_warning() {
        let song = parse_str("{t: T}\nHold [on");
        assert_eq!(song.sections, [section("Verse 1", &["Hold [on"])]);
        assert!(song.warnings.is_empty());
    }

    #[test]
    fn blank_lines_separate_verses_outside_and_slides_inside_environments() {
        let song = parse_str(
            "{t: T}\nOne a\nOne b\n\n\nTwo a\n{soc}\nC a\n\nC b\n{eoc}\n{sov}\nThree\n{eov}\n{sob: Turn}\nB a\n{eob}\n{sob}\nB b\n{eob}",
        );
        assert_eq!(
            song.sections,
            [
                section("Verse 1", &["One a\nOne b"]),
                section("Verse 2", &["Two a"]),
                section("Chorus", &["C a", "C b"]),
                section("Verse 3", &["Three"]),
                section("Turn", &["B a"]),
                section("Bridge 2", &["B b"]),
            ]
        );
        assert!(song.order.is_empty());
    }

    #[test]
    fn multiple_choruses_and_repeats_follow_the_last_chorus() {
        let song = parse_str(
            "{t: T}\n{chorus}\nV1\n{soc}\nFirst\n{eoc}\n{chorus}\n{start_of_chorus label=\"Final\"}\nLast\n{end_of_chorus}\nV2\n{chorus}",
        );
        assert_eq!(labels(&song), ["Verse 1", "Chorus", "Final", "Verse 2"]);
        assert_eq!(song.order, [0, 1, 1, 2, 3, 2]);
        assert_eq!(
            song.warnings,
            [Warning::MissingOrderReference("chorus".into())]
        );
    }

    #[test]
    fn empty_environments_are_reported() {
        let song = parse_str("{t: T}\n{soc}\n{eoc}\n{sov}\n[G] [C]\n{eov}\nWords");
        assert_eq!(song.sections, [section("Verse 1", &["Words"])]);
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::EmptySection("Chorus".into()),
                Warning::EmptySection("Verse 1".into()),
                Warning::ChordsRemoved,
            ])
        );
    }

    #[test]
    fn tabs_and_grids_are_skipped() {
        let song = parse_str(
            "{t: T}\nSing\n{sot}\ne|--3--|\n{eot}\n{start_of_grid}\n| G . | C . |\n{end_of_grid}\nAgain",
        );
        assert_eq!(
            song.sections,
            [
                section("Verse 1", &["Sing"]),
                section("Verse 2", &["Again"])
            ]
        );
        assert_eq!(song.warnings, [Warning::ChordsRemoved]);
    }

    #[test]
    fn metadata_directives_and_unknown_directives() {
        let song = parse_str(
            "{title:First}\n{t: Second}\n{artist: A}\n{composer: B}\n{lyricist: A}\n{meta: lyricist C}\n{st: Sub}\n{copyright: 2026 X}\n{ccli: 1234567}\n{capo: 2}\n{meta: album Y}\n{x_custom}\nWords",
        );
        assert_eq!(song.title, "First");
        assert_eq!(song.authors, ["A", "B", "C"]);
        assert_eq!(song.copyright, "2026 X");
        assert_eq!(song.song_number, "1234567");
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::UnknownDirective("capo".into()),
                Warning::UnknownDirective("album".into()),
                Warning::UnknownDirective("x_custom".into()),
                Warning::UnknownDirective("subtitle".into()),
            ])
        );
    }

    #[test]
    fn missing_title_and_bad_ccli_are_warnings() {
        let song = parse_str("{ccli: none}\nWords");
        assert_eq!(song.title, "");
        assert_eq!(song.song_number, "");
        assert_eq!(
            sorted(song.warnings),
            sorted(vec![
                Warning::InvalidField("ccli".into()),
                Warning::MissingTitle
            ])
        );
    }

    #[test]
    fn files_without_lyrics_or_too_large_are_rejected() {
        assert_eq!(parse(b""), Err(Rejected::NoLyrics));
        assert_eq!(
            parse(b"{title: T}\n{c: only a comment}\n[G] [C]\n"),
            Err(Rejected::NoLyrics)
        );
        assert_eq!(parse(b"\xc3\x28"), Err(Rejected::InvalidUtf8));
        let mut big = b"{t: T}\nWords\n".to_vec();
        big.resize(MAX_FILE_BYTES + 1, b'a');
        assert_eq!(parse(&big), Err(Rejected::TooLarge));
    }

    #[test]
    fn limits_hold_on_pathological_input() {
        let stanzas = "Line\n\n".repeat(MAX_SECTIONS + 5);
        let song = parse_str(&stanzas);
        assert_eq!(song.sections.len(), MAX_SECTIONS);

        let slides = format!("{{soc}}\n{}", "a\n\n".repeat(MAX_SLIDES + 5));
        let song = parse_str(&slides);
        assert_eq!(song.sections[0].slides.len(), MAX_SLIDES);

        let repeats = format!(
            "{{soc}}\nC\n{{eoc}}\n{}",
            "{chorus}\n".repeat(MAX_ORDER + 5)
        );
        let song = parse_str(&repeats);
        assert_eq!(song.order.len(), MAX_ORDER);
        for song in [parse_str(&stanzas), parse_str(&slides), song] {
            assert!(song.warnings.contains(&Warning::LimitReached));
        }

        let started = std::time::Instant::now();
        let brackets = "[a".repeat(MAX_FILE_BYTES / 2 - 1);
        assert_eq!(
            parse_str(&brackets).sections[0].slides[0].len(),
            brackets.len()
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn minimized_property_cases() {
        assert_eq!(
            parse_str("}[\n]").sections,
            [section("Verse 1", &["}[\n]"])]
        );
        assert_eq!(parse(b"\n\n[]\n\n"), Err(Rejected::NoLyrics));
        assert_eq!(
            parse_str("[[G]]Sing").sections,
            [section("Verse 1", &["Sing"])]
        );
    }

    fn has_chord(line: &str) -> bool {
        line.match_indices('[').any(|(i, _)| {
            line[i + 1..]
                .find(['[', ']'])
                .is_some_and(|j| line.as_bytes()[i + 1 + j] == b']')
        })
    }

    const TOKENS: &[&str] = &[
        "\n",
        "\n",
        "\n",
        "{soc}",
        "{eoc}",
        "{chorus}",
        "{sov}",
        "{eov}",
        "{sob: B}",
        "{sot}",
        "{eot}",
        "{title: T}",
        "{c: note}",
        "{key: G}",
        "{meta: artist A}",
        "{",
        "}",
        "[G]",
        "[",
        "]",
        "word ",
        "kata ",
        "光",
        "\r\n",
        "# x",
        " ",
        ":",
    ];

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
                "tests/fixtures/import/chordpro/proptest-regressions.txt",
            ))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn arbitrary_input_keeps_the_song_invariants(
            bytes in prop_oneof![
                proptest::collection::vec(any::<u8>(), 0..2048),
                proptest::collection::vec(proptest::sample::select(TOKENS), 0..300)
                    .prop_map(|t| t.concat().into_bytes()),
            ]
        ) {
            match parse(&bytes) {
                Ok(song) => {
                    prop_assert!(song.sections.len() <= MAX_SECTIONS);
                    let slides: usize = song.sections.iter().map(|s| s.slides.len()).sum();
                    prop_assert!(slides <= MAX_SLIDES);
                    prop_assert!(song.order.len() <= MAX_ORDER);
                    prop_assert!(song.order.iter().all(|&i| i < song.sections.len()));
                    for slide in song.sections.iter().flat_map(|s| &s.slides) {
                        prop_assert!(!slide.trim().is_empty());
                        prop_assert!(!slide.lines().any(has_chord), "{slide:?}");
                    }
                }
                Err(rejected) => prop_assert!(matches!(
                    rejected,
                    Rejected::NoLyrics | Rejected::InvalidUtf8
                )),
            }
        }
    }
}
