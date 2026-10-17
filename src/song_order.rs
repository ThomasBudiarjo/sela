//! How the Songs list is ordered: the observed Sort by choices (EW8-OBS-064)
//! Sela supports, plus the direction. Persisted in the profile settings.

/// What Songs sorts by. EasyWorship also lists Author, Copyright,
/// Administrator and Song Number; those are tracked in the backlog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SongSort {
    /// Title, ignoring ASCII case.
    #[default]
    Title,
    /// Order of the last save, oldest first; see `Repository::library`.
    Modified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SongOrder {
    pub sort: SongSort,
    pub descending: bool,
}

impl SongSort {
    pub const ALL: [SongSort; 2] = [SongSort::Title, SongSort::Modified];

    pub fn code(self) -> &'static str {
        match self {
            SongSort::Title => "title",
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
        assert_eq!(SongSort::from_code("author"), None);
        assert_eq!(SongOrder::default().sort, SongSort::Title);
        assert!(!SongOrder::default().descending);
    }
}
