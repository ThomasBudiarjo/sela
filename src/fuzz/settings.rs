//! `settings.txt` (`settings::parse`, `render`, `load`). The profile file is
//! written by Sela, but it survives crashes, disk errors, hand edits and
//! downgrades, so a damaged file must give defaults and never a panic.
use super::{config, corpus, mutated, within};
use crate::i18n::Language;
use crate::pane_layout::{Layout, Placement};
use crate::settings::{self, MAX_RECENT, ParseError, Settings};
use crate::song_order::{SongOrder, SongSort};
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

fn check(text: &str) {
    let parsed = within(Duration::from_secs(5), "settings parse", || {
        settings::parse(text)
    });
    assert_eq!(settings::parse(text), parsed, "same input, same result");
    match parsed {
        Ok(found) => {
            assert!(found.recent_schedules.len() <= MAX_RECENT);
            let mut unique = found.recent_schedules.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), found.recent_schedules.len(), "no duplicates");
            // Saving what was loaded keeps every setting: a load-save cycle
            // never changes the operator's monitor, recent files or layout.
            let rendered = settings::render(&found);
            assert_eq!(settings::parse(&rendered), Ok(found), "{rendered:?}");
        }
        Err(ParseError::Empty | ParseError::Header | ParseError::Line(_)) => {}
        Err(ParseError::Version(v)) => assert!(v == 0 || v > settings::VERSION),
    }
}

const TOKENS: &[&str] = &[
    "sela-settings 1\n",
    "sela-settings 2\n",
    "sela-settings 0\n",
    "sela-settings 4294967296\n",
    "output_monitor=",
    "language=",
    "id",
    "en",
    "song_sort=",
    "modified",
    "song_sort_descending=1\n",
    "recent=",
    "C:/s/a.selaschedule",
    "pane_ratios=",
    "2400,6200,6200",
    "resources_collapsed=",
    "window=",
    "-100,0,1280,720,max",
    "=",
    ",",
    "\n",
    "\r\n",
    "\r",
    "\t",
    "\u{85}",
    "\u{2028}",
    "\u{0}",
    "\u{7f}",
    " ",
    "\u{1f600}",
    "+1",
];

fn layouts() -> impl Strategy<Value = Layout> {
    let window = (
        -100_000i32..=100_000,
        -100_000i32..=100_000,
        1u32..=30_000,
        1u32..=30_000,
        any::<bool>(),
    )
        .prop_map(|(x, y, width, height, maximized)| Placement {
            x,
            y,
            width,
            height,
            maximized,
        });
    (
        1u16..9998,
        1u16..9999,
        any::<bool>(),
        proptest::option::of(window),
    )
        .prop_map(|(a, c, collapsed, window)| Layout {
            ratios: [a, a + 1 + (c % (9999 - a)), c],
            collapsed,
            window,
        })
}

/// Settings as the operator UI produces them: names and paths without
/// control characters (a Windows path or device name cannot contain them).
fn operator_settings() -> impl Strategy<Value = Settings> {
    (
        proptest::option::of("[^\\x00-\\x1f\\x7f-\\x9f]{1,40}"),
        prop_oneof![Just(Language::English), Just(Language::Indonesian)],
        proptest::collection::vec("[^\\x00-\\x1f\\x7f-\\x9f]{1,60}", 0..12),
        (
            prop_oneof![Just(SongSort::Title), Just(SongSort::Modified)],
            any::<bool>(),
        ),
        proptest::option::of(layouts()),
    )
        .prop_map(
            |(output_monitor, language, recent, (sort, descending), layout)| {
                let mut settings = Settings {
                    output_monitor,
                    language,
                    song_order: SongOrder { sort, descending },
                    layout,
                    ..Settings::default()
                };
                for path in recent.into_iter().rev() {
                    settings.remember_recent(PathBuf::from(path));
                }
                settings
            },
        )
}

fn seeds() -> Vec<Vec<u8>> {
    let mut seeds: Vec<Vec<u8>> = corpus("settings").into_iter().map(|(_, b)| b).collect();
    seeds.push(b"sela-settings 1\n".to_vec());
    seeds
}

proptest! {
    #![proptest_config(config(0x5e1a_0501, 512, "tests/fixtures/fuzz/settings.proptest-regressions"))]

    #[test]
    fn saved_settings_load_back_unchanged(settings in operator_settings()) {
        let text = settings::render(&settings);
        prop_assert_eq!(settings::parse(&text), Ok(settings));
        check(&text);
    }

    #[test]
    fn mutated_files_load_or_fall_back(bytes in mutated(seeds(), TOKENS)) {
        check(&String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn any_text_loads_or_falls_back(text in any::<String>()) {
        check(&text);
        check(&format!("sela-settings 1\n{text}"));
    }

    #[test]
    fn control_characters_never_change_a_saved_value(
        settings in operator_settings(),
        control in (0u32..64).prop_map(|n| char::from_u32(if n < 32 { n } else { 0x7f + n - 32 }).unwrap()),
        name in "[a-z]{1,8}",
    ) {
        // A path or monitor name that cannot be written faithfully is left
        // out instead of being saved as a different name.
        let mut with = settings.clone();
        let odd = PathBuf::from(format!("{name}{control}{name}"));
        with.remember_recent(odd.clone());
        with.output_monitor = Some(format!("{name}{control}"));
        let rendered = settings::render(&with);
        let loaded = settings::parse(&rendered).unwrap();
        prop_assert_eq!(&loaded.recent_schedules, &settings.recent_schedules[..settings.recent_schedules.len().min(MAX_RECENT - 1)].to_vec());
        prop_assert_eq!(loaded.output_monitor, None);
        check(&rendered);
    }
}

#[test]
fn worst_case_files_finish_within_their_time_bound() {
    let mib = 1024 * 1024;
    for text in [
        format!("sela-settings 1\n{}", "recent=x\n".repeat(mib / 9)),
        format!("sela-settings 1\n{}", "\n".repeat(mib)),
        format!("sela-settings 1\nrecent={}\n", "x".repeat(mib)),
        format!("sela-settings 1\npane_ratios={}\n", ",".repeat(mib)),
        format!(
            "sela-settings 1\n{}",
            "pane_ratios=1,2,3\n".repeat(mib / 18)
        ),
    ] {
        check(&text);
    }
}

#[test]
fn retained_corpus_replays_cleanly() {
    for (_, bytes) in corpus("settings") {
        check(&String::from_utf8_lossy(&bytes));
    }
}
