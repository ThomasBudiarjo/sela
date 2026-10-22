//! The FFmpeg subprocess boundary: the input path handed to `ffmpeg -i` and
//! the build output read back from the bundled binary. A media path comes
//! from a schedule file or the file chooser; the probe output comes from
//! whatever executable sits where FFmpeg is expected.
use super::config;
use crate::media::ffmpeg::{self, Refusal};
use proptest::prelude::*;
use std::ffi::OsString;
use std::path::Path;

/// FFmpeg's own protocol test: two or more scheme characters, then `:`.
fn looks_like_url(path: &str) -> bool {
    let scheme = path
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        .count();
    scheme >= 2 && path.as_bytes().get(scheme) == Some(&b':')
}

fn check_path(text: &str) {
    let path = Path::new(text);
    if let Ok(args) = ffmpeg::input_args(path) {
        assert!(path.is_absolute(), "{text:?}");
        assert!(!looks_like_url(text), "{text:?}");
        assert_eq!(args.last(), Some(&OsString::from(text)));
        let i = args.len() - 2;
        assert_eq!(args[i], "-i");
        // The whitelist comes before the input, so it applies to it.
        let whitelist = args.iter().position(|a| a == "-protocol_whitelist");
        assert!(whitelist.is_some_and(|w| w < i && args[w + 1] == "file"));
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0701, 512, "tests/fixtures/fuzz/media.proptest-regressions"))]

    #[test]
    fn only_absolute_local_paths_reach_ffmpeg(
        path in prop_oneof![
            any::<String>(),
            "(/|[A-Za-z]:\\\\|\\\\\\\\[?.]\\\\|[a-z+.-]{1,8}:|\\.\\./)[ -~]{0,40}",
        ],
    ) {
        check_path(&path);
    }

    #[test]
    fn build_output_is_classified_without_panicking(
        words in proptest::collection::vec(
            prop_oneof![
                Just("configuration:"),
                Just("--enable-gpl"),
                Just("--enable-nonfree"),
                Just("--enable-version3"),
                Just("ffmpeg version n8.1"),
                Just("\n"),
                Just(" "),
                Just("\u{a0}"),
                Just("configuration:--enable-gpl"),
            ],
            0..20,
        ),
    ) {
        let output = words.concat();
        let flags: Vec<&str> = output.split_whitespace().collect();
        match ffmpeg::check_buildconf(&output) {
            Ok(()) => {
                prop_assert!(flags.contains(&"configuration:"));
                prop_assert!(!flags.contains(&"--enable-gpl"));
                prop_assert!(!flags.contains(&"--enable-nonfree"));
            }
            Err(Refusal::Gpl) => prop_assert!(flags.contains(&"--enable-gpl")),
            Err(Refusal::Nonfree) => prop_assert!(flags.contains(&"--enable-nonfree")),
            Err(Refusal::Unrecognized) => prop_assert!(!flags.contains(&"configuration:")),
            Err(other) => prop_assert!(false, "{other:?}"),
        }
    }
}
