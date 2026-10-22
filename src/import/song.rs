//! Maps a `ParsedSong` onto `storage::Song`, the shape every importer's
//! result is saved in (M1-14).
use super::ParsedSong;
use crate::arrangement::{Occurrence, OccurrenceId, SectionId, Variant, VariantId};
use crate::storage::{Error, Section, Song};

/// Joins several authors in the one-line Authors field the song editor
/// edits. A semicolon, because author names may contain commas.
pub const AUTHOR_SEPARATOR: &str = "; ";

/// One `storage::Section` per slide, with fresh IDs. A section's first slide
/// carries its label and the rest have none, so they join its group in the
/// editor as in EasyWorship (EW8-OBS-022); a repeated label would start a
/// new group. A presentation order becomes the song's only arrangement, each entry
/// expanded into all slides of its section; without an order the song has no
/// arrangement and plays in stored order, like a song made in the editor.
///
/// The caller substitutes a title first when the file had none
/// (`Warning::MissingTitle`). Fails with `Error::Invalid` when the result is
/// not a valid song, for example an order whose expanded slides exceed
/// `arrangement::MAX_OCCURRENCES`.
pub fn to_song(parsed: &ParsedSong) -> Result<Song, Error> {
    let mut sections = Vec::new();
    let mut ids: Vec<Vec<SectionId>> = Vec::new();
    for section in &parsed.sections {
        let mut slides = Vec::new();
        for (index, lyrics) in section.slides.iter().enumerate() {
            let id = SectionId::allocate();
            slides.push(id);
            sections.push(Section {
                id,
                label: if index == 0 {
                    section.label.clone()
                } else {
                    String::new()
                },
                lyrics: lyrics.clone(),
                format: Default::default(),
                background: None,
            });
        }
        ids.push(slides);
    }
    let mut occurrences = Vec::new();
    for &index in &parsed.order {
        for &section in ids.get(index).ok_or(Error::Invalid)? {
            occurrences.push(Occurrence {
                id: OccurrenceId(SectionId::allocate().0),
                section,
            });
        }
    }
    let variants = if occurrences.is_empty() {
        Vec::new()
    } else {
        vec![Variant {
            id: VariantId(SectionId::allocate().0),
            name: "Arrangement 1".into(),
            occurrences,
        }]
    };
    let song = Song {
        title: parsed.title.clone(),
        authors: parsed.authors.join(AUTHOR_SEPARATOR),
        copyright: parsed.copyright.clone(),
        license: String::new(),
        administrator: parsed.administrator.clone(),
        song_number: parsed.song_number.clone(),
        sections,
        variants,
        master: None,
    };
    song.validate()?;
    Ok(song)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ParsedSection, openlyrics};
    use crate::slides;
    use crate::storage::{Id, Version};

    const VERSION: Version = Version {
        id: Id([7; 16]),
        revision: 1,
    };

    fn fixture(name: &str) -> ParsedSong {
        let path = format!(
            "{}/tests/fixtures/import/openlyrics/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        openlyrics::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn the_verse_order_becomes_the_arrangement_and_the_slide_order() {
        let song = to_song(&fixture("fields.xml")).unwrap();
        assert_eq!(song.title, "Lantern by the Road");
        assert_eq!(song.authors, "Rina Kartika; Daniel Hartono");
        assert_eq!(song.copyright, "2026 Sela Test Fixtures");
        assert_eq!(song.song_number, "1234567");
        assert_eq!(song.administrator, "Fixture Music Admin");
        let stored: Vec<(&str, &str)> = song
            .sections
            .iter()
            .map(|s| (s.label.as_str(), s.lyrics.as_str()))
            .collect();
        let v1 = "A lantern hangs beside the road\nand burns the whole night long";
        let v2 = "The travellers who pass it by\nwalk home along its glow";
        let c = "Keep the light, keep the light\nuntil the morning comes";
        let b1 = "When the wind is cold\nthe flame still holds";
        let b2 = "When the road is lost\nthe light is found";
        assert_eq!(
            stored,
            [
                ("Verse 1", v1),
                ("Verse 2", v2),
                ("Chorus", c),
                ("Bridge", b1),
                ("", b2)
            ]
        );
        let expected = [
            ("Verse 1", v1),
            ("Chorus", c),
            ("Verse 2", v2),
            ("Chorus", c),
            ("Bridge", b1),
            ("", b2),
            ("Chorus", c),
        ];
        let arrangement = song.arrangement(VERSION).unwrap();
        assert_eq!(arrangement.variants().len(), 1);
        let resolved: Vec<(&str, &str)> = arrangement
            .resolve(arrangement.variants()[0].id)
            .unwrap()
            .iter()
            .map(|o| (o.section.label.as_str(), o.section.lyrics.as_str()))
            .collect();
        assert_eq!(resolved, expected);
        let projected: Vec<(String, String)> = slides::project(&song, None)
            .slides
            .into_iter()
            .map(|s| (s.label, s.text))
            .collect();
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(l, t)| (l.to_string(), t.to_string()))
            .collect();
        assert_eq!(projected, expected);
    }

    #[test]
    fn without_an_order_the_song_plays_in_file_order() {
        let song = to_song(&fixture("unicode.xml")).unwrap();
        assert!(song.variants.is_empty());
        let labels: Vec<String> = slides::project(&song, None)
            .slides
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(labels, ["Verse 1", "Chorus", "Verse 2", "Verse 3"]);
        assert_eq!(song.authors, "Ayu Wulandari; José Peña");
    }

    #[test]
    fn every_slide_and_occurrence_gets_its_own_id() {
        let song = to_song(&fixture("losses.xml")).unwrap();
        let mut ids: Vec<Id> = song.sections.iter().map(|s| s.id.0).collect();
        ids.extend(song.variants.iter().map(|v| v.id.0));
        ids.extend(song.variants[0].occurrences.iter().map(|o| o.id.0));
        let count = ids.len();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(song.variants[0].occurrences.len(), 8);
    }

    #[test]
    fn songs_the_library_cannot_store_are_invalid() {
        let parsed = ParsedSong {
            title: "Too long".into(),
            sections: vec![ParsedSection {
                label: "Chorus".into(),
                slides: vec!["a".into(), "b".into()],
            }],
            order: vec![0; crate::arrangement::MAX_OCCURRENCES / 2 + 1],
            ..ParsedSong::default()
        };
        assert_eq!(to_song(&parsed), Err(Error::Invalid));
        let untitled = ParsedSong {
            order: vec![0],
            title: String::new(),
            ..parsed.clone()
        };
        assert_eq!(to_song(&untitled), Err(Error::Invalid));
        let dangling = ParsedSong {
            order: vec![1],
            ..parsed
        };
        assert_eq!(to_song(&dangling), Err(Error::Invalid));
    }
}
