//! Slide label shortcuts (EW8-OBS-052, EW8-OBS-056): a kind letter, an
//! optional number and Enter jump Live to the labelled slide.
use crate::slides::Slide;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bridge,
    Chorus,
    Ending,
    Intro,
    Misc,
    PreChorus,
    Tag,
    Verse,
}

impl Kind {
    /// The shortcut letter from EW's label table (Options > Slide Labels).
    pub fn from_key(key: char) -> Option<Self> {
        Some(match key.to_ascii_lowercase() {
            'b' => Self::Bridge,
            'c' => Self::Chorus,
            'e' => Self::Ending,
            'i' => Self::Intro,
            'm' => Self::Misc,
            'r' => Self::PreChorus,
            't' => Self::Tag,
            'v' => Self::Verse,
            _ => return None,
        })
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_lowercase().as_str() {
            "bridge" => Self::Bridge,
            "chorus" => Self::Chorus,
            "end" | "ending" => Self::Ending,
            "intro" | "introduction" => Self::Intro,
            "misc" => Self::Misc,
            "pre-chorus" | "prechorus" | "pre chorus" => Self::PreChorus,
            "tag" => Self::Tag,
            "verse" => Self::Verse,
            _ => return None,
        })
    }
}

/// Splits a slide label such as "Verse 2" into its kind and number; a label
/// without a number counts as number 1.
pub fn parse(label: &str) -> Option<(Kind, u32)> {
    let label = label.trim();
    let digits = label.chars().rev().take_while(char::is_ascii_digit).count();
    let (name, number) = label.split_at(label.len() - digits);
    let kind = Kind::from_name(name)?;
    Some((kind, number.parse().unwrap_or(1)))
}

/// The first slide whose label has `kind` and, when given, `number`.
pub fn find(slides: &[Slide], kind: Kind, number: Option<u32>) -> Option<usize> {
    slides.iter().position(|slide| {
        parse(&slide.label)
            .is_some_and(|(k, n)| k == kind && number.is_none_or(|wanted| wanted == n))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_parse_into_kind_and_number() {
        assert_eq!(parse("Verse 2"), Some((Kind::Verse, 2)));
        assert_eq!(parse("verse 12"), Some((Kind::Verse, 12)));
        assert_eq!(parse("Chorus"), Some((Kind::Chorus, 1)));
        assert_eq!(parse("Pre-Chorus 3"), Some((Kind::PreChorus, 3)));
        assert_eq!(parse("Ending"), Some((Kind::Ending, 1)));
        assert_eq!(parse("Interlude"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("7"), None);
    }

    #[test]
    fn keys_follow_the_slide_label_table() {
        let kinds: Vec<_> = "bceimrtv".chars().map(Kind::from_key).collect();
        assert!(kinds.iter().all(Option::is_some));
        assert_eq!(Kind::from_key('R'), Some(Kind::PreChorus));
        assert_eq!(Kind::from_key('x'), None);
        assert_eq!(Kind::from_key('1'), None);
    }

    #[test]
    fn find_picks_the_first_matching_slide() {
        let slide = |label: &str| Slide {
            label: label.into(),
            text: String::new(),
            format: Default::default(),
            background: None,
        };
        let slides = [
            slide("Verse 1"),
            slide("Chorus"),
            slide("Verse 2"),
            slide("Chorus"),
        ];
        assert_eq!(find(&slides, Kind::Verse, Some(1)), Some(0));
        assert_eq!(find(&slides, Kind::Verse, Some(2)), Some(2));
        assert_eq!(find(&slides, Kind::Verse, None), Some(0));
        assert_eq!(find(&slides, Kind::Chorus, None), Some(1));
        assert_eq!(find(&slides, Kind::Verse, Some(3)), None);
        assert_eq!(find(&slides, Kind::Bridge, None), None);
    }
}
