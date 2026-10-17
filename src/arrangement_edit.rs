//! Editor operations on a draft song's arrangements. Pure and
//! framework-independent: every function either applies its whole change to
//! the song or returns `false` and leaves it untouched. Callers validate the
//! result (`Song::validate`) and own undo.
//!
//! The first arrangement is the song's active one: Preview, Live and new
//! schedule items follow it (`slides::project`).
use crate::arrangement::{
    MAX_NAME_BYTES, MAX_OCCURRENCES, MAX_VARIANTS, Occurrence, OccurrenceId, SectionId, Variant,
    VariantId,
};
use crate::storage::Song;

fn occurrence(section: SectionId) -> Occurrence {
    Occurrence {
        id: OccurrenceId(SectionId::allocate().0),
        section,
    }
}

fn fresh_name(song: &Song) -> String {
    (song.variants.len() + 1..)
        .map(|n| format!("Arrangement {n}"))
        .find(|name| song.variants.iter().all(|v| &v.name != name))
        .unwrap_or_default()
}

/// The arrangement `edit` should change: the first, created from all slides in
/// stored order when the song has none yet.
pub fn ensure_variant(song: &mut Song) {
    if song.variants.is_empty() {
        song.variants.push(Variant {
            id: VariantId(SectionId::allocate().0),
            name: "Arrangement 1".into(),
            occurrences: song.sections.iter().map(|s| occurrence(s.id)).collect(),
        });
    }
}

/// Appends a new arrangement, a copy of `from` (or of the stored order when
/// there is none), and returns its index.
pub fn new_variant(song: &mut Song, from: Option<usize>) -> Option<usize> {
    if song.variants.len() >= MAX_VARIANTS {
        return None;
    }
    let name = fresh_name(song);
    if name.len() > MAX_NAME_BYTES {
        return None;
    }
    let order: Vec<SectionId> = match from.and_then(|i| song.variants.get(i)) {
        Some(variant) => variant.occurrences.iter().map(|o| o.section).collect(),
        None => song.sections.iter().map(|s| s.id).collect(),
    };
    if order.len() > MAX_OCCURRENCES {
        return None;
    }
    song.variants.push(Variant {
        id: VariantId(SectionId::allocate().0),
        name,
        occurrences: order.into_iter().map(occurrence).collect(),
    });
    Some(song.variants.len() - 1)
}

/// Removes arrangement `index`. The song falls back to stored order once none
/// is left.
pub fn delete_variant(song: &mut Song, index: usize) -> bool {
    if index >= song.variants.len() {
        return false;
    }
    song.variants.remove(index);
    true
}

/// Makes arrangement `index` the active (first) one; returns its new index.
pub fn make_active(song: &mut Song, index: usize) -> Option<usize> {
    if index >= song.variants.len() {
        return None;
    }
    let variant = song.variants.remove(index);
    song.variants.insert(0, variant);
    Some(0)
}

/// References section `section` (an index into `song.sections`) at position
/// `at` of arrangement `variant`; `at` past the end appends.
pub fn insert(song: &mut Song, variant: usize, at: usize, section: usize) -> bool {
    let Some(id) = song.sections.get(section).map(|s| s.id) else {
        return false;
    };
    match song.variants.get_mut(variant) {
        Some(v) if v.occurrences.len() < MAX_OCCURRENCES => {
            let at = at.min(v.occurrences.len());
            v.occurrences.insert(at, occurrence(id));
            true
        }
        _ => false,
    }
}

pub fn remove(song: &mut Song, variant: usize, at: usize) -> bool {
    match song.variants.get_mut(variant) {
        Some(v) if at < v.occurrences.len() => {
            v.occurrences.remove(at);
            true
        }
        _ => false,
    }
}

/// Moves one reference so it ends up at index `to`. Its ID travels with it.
pub fn move_to(song: &mut Song, variant: usize, from: usize, to: usize) -> bool {
    match song.variants.get_mut(variant) {
        Some(v) if from < v.occurrences.len() && to < v.occurrences.len() && from != to => {
            let moved = v.occurrences.remove(from);
            v.occurrences.insert(to, moved);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slides;
    use crate::storage::{Id, Section};

    fn song() -> Song {
        let section = |label: &str, lyrics: &str, n: u8| Section {
            id: SectionId(Id([n; 16])),
            label: label.into(),
            lyrics: lyrics.into(),
            format: Default::default(),
            background: None,
        };
        Song {
            title: "Edit".into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            sections: vec![
                section("Verse 1", "v1", 1),
                section("Verse 2", "v2", 2),
                section("Chorus", "c", 3),
                section("Bridge", "b", 4),
            ],
            variants: Vec::new(),
            master: None,
        }
    }

    fn order(song: &Song, variant: usize) -> Vec<&str> {
        song.variants[variant]
            .occurrences
            .iter()
            .map(|o| {
                song.sections
                    .iter()
                    .find(|s| s.id == o.section)
                    .map_or("?", |s| s.lyrics.as_str())
            })
            .collect()
    }

    #[test]
    fn add_remove_and_reorder_follow_the_contract_example() {
        let mut song = song();
        ensure_variant(&mut song);
        assert_eq!(order(&song, 0), ["v1", "v2", "c", "b"]);
        // Chorus after Verse 2, Bridge removed, Verse 2 before Verse 1.
        assert!(insert(&mut song, 0, 2, 2));
        assert_eq!(order(&song, 0), ["v1", "v2", "c", "c", "b"]);
        let bridge = song.variants[0].occurrences.len() - 1;
        assert!(remove(&mut song, 0, bridge));
        let verse2 = song.variants[0].occurrences[1].id;
        assert!(move_to(&mut song, 0, 1, 0));
        assert_eq!(order(&song, 0), ["v2", "v1", "c", "c"]);
        assert_eq!(song.variants[0].occurrences[0].id, verse2, "ID travels");
        let texts: Vec<_> = slides::slides(&song).into_iter().map(|s| s.text).collect();
        assert_eq!(texts, ["v2", "v1", "c", "c"]);
        assert!(song.validate().is_ok());
    }

    #[test]
    fn repeated_references_get_distinct_ids() {
        let mut song = song();
        ensure_variant(&mut song);
        assert!(insert(&mut song, 0, usize::MAX, 2));
        let ids: Vec<_> = song.variants[0]
            .occurrences
            .iter()
            .map(|o| o.id.0)
            .collect();
        for (i, id) in ids.iter().enumerate() {
            assert!(!ids[..i].contains(id));
        }
    }

    #[test]
    fn refused_changes_leave_the_song_untouched() {
        let mut song = song();
        let before = song.clone();
        assert!(!insert(&mut song, 0, 0, 0), "no arrangement yet");
        assert!(!remove(&mut song, 0, 0));
        assert!(!move_to(&mut song, 0, 0, 1));
        ensure_variant(&mut song);
        let with = song.clone();
        assert!(!insert(&mut song, 0, 0, 99), "unknown section");
        assert!(!remove(&mut song, 0, 99));
        assert!(!move_to(&mut song, 0, 0, 99));
        assert!(!move_to(&mut song, 0, 1, 1));
        assert_eq!(song, with);
        assert_ne!(song, before);
        while song.variants[0].occurrences.len() < MAX_OCCURRENCES {
            assert!(insert(&mut song, 0, 0, 0));
        }
        let full = song.clone();
        assert!(!insert(&mut song, 0, 0, 0), "per-arrangement limit");
        assert_eq!(song, full);
    }

    #[test]
    fn variants_are_added_named_uniquely_activated_and_deleted() {
        let mut song = song();
        assert_eq!(new_variant(&mut song, None), Some(0));
        assert_eq!(new_variant(&mut song, Some(0)), Some(1));
        assert_eq!(song.variants[0].name, "Arrangement 1");
        assert_eq!(song.variants[1].name, "Arrangement 2");
        assert!(remove(&mut song, 1, 0));
        assert_eq!(
            order(&song, 0),
            ["v1", "v2", "c", "b"],
            "copy is independent"
        );
        assert_eq!(order(&song, 1), ["v2", "c", "b"]);
        let second = song.variants[1].id;
        assert_eq!(make_active(&mut song, 1), Some(0));
        assert_eq!(song.variants[0].id, second);
        assert_eq!(
            slides::slides(&song).len(),
            3,
            "Preview follows the active one"
        );
        assert!(song.validate().is_ok());
        assert!(delete_variant(&mut song, 0));
        assert!(!delete_variant(&mut song, 5));
        assert!(delete_variant(&mut song, 0));
        assert_eq!(slides::slides(&song).len(), 4, "stored order again");
        for _ in 0..MAX_VARIANTS {
            new_variant(&mut song, None);
        }
        assert_eq!(song.variants.len(), MAX_VARIANTS);
        assert_eq!(new_variant(&mut song, None), None);
    }
}
