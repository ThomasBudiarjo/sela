//! How the Songs list is ordered: the observed Sort by choices (EW8-OBS-064)
//! plus Sela's Date modified, and the direction. Persisted in the profile
//! settings.

/// What Songs sorts by. Title, Author, Copyright, Administrator and Song
/// Number are EasyWorship's Sort by choices in its order (EW8-OBS-064);
/// Date modified is Sela's own addition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SongSort {
    /// Title, ignoring ASCII case.
    #[default]
    Title,
    /// The Authors text.
    Author,
    /// The Copyright text.
    Copyright,
    /// The Administrator property (schema 6).
    Administrator,
    /// The Song Number property, compared as a number.
    SongNumber,
    /// Time of the last save, oldest first; see `Repository::library`.
    Modified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SongOrder {
    pub sort: SongSort,
    pub descending: bool,
}

impl SongSort {
    /// Menu order: EasyWorship's choices first, then Date modified.
    pub const ALL: [SongSort; 6] = [
        SongSort::Title,
        SongSort::Author,
        SongSort::Copyright,
        SongSort::Administrator,
        SongSort::SongNumber,
        SongSort::Modified,
    ];

    pub fn code(self) -> &'static str {
        match self {
            SongSort::Title => "title",
            SongSort::Author => "author",
            SongSort::Copyright => "copyright",
            SongSort::Administrator => "administrator",
            SongSort::SongNumber => "song_number",
            SongSort::Modified => "modified",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sort| sort.code() == code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_unknown_codes_are_rejected() {
        for sort in SongSort::ALL {
            assert_eq!(SongSort::from_code(sort.code()), Some(sort));
        }
        assert_eq!(SongSort::from_code("author"), Some(SongSort::Author));
        assert_eq!(SongSort::from_code("song number"), None);
        assert_eq!(SongSort::from_code("Title"), None);
        assert_eq!(SongOrder::default().sort, SongSort::Title);
        assert!(!SongOrder::default().descending);
    }

    #[test]
    fn the_menu_lists_the_observed_choices_in_order_then_date_modified() {
        assert_eq!(
            SongSort::ALL.map(SongSort::code),
            [
                "title",
                "author",
                "copyright",
                "administrator",
                "song_number",
                "modified"
            ]
        );
    }
}
