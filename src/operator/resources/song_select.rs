//! Songs list selection: plain, Ctrl (toggle) and Shift (range) clicks and
//! the matching keyboard moves. The selection is a set of song ids so a song
//! stays selected when saving moves it to a new revision; the primary
//! (previewed) song is `Operator::selected_song`.

use super::*;
use sela::storage::Id;

/// Fixed row height, shared with paging and the scrollbar.
pub(crate) const SONG_ROW_H: f32 = 28.;

#[derive(Default)]
pub(crate) struct SongSelection {
    /// Fixed end of a Shift range.
    anchor: Option<Id>,
    set: HashSet<Id>,
}

impl SongSelection {
    pub(crate) fn contains(&self, id: Id) -> bool {
        self.set.contains(&id)
    }

    pub(crate) fn only(&mut self, id: Id) {
        self.set.clear();
        self.set.insert(id);
        self.anchor = Some(id);
    }

    pub(crate) fn clear(&mut self) {
        self.set.clear();
        self.anchor = None;
    }

    /// Ctrl+click. Returns whether `id` is selected afterwards.
    pub(crate) fn toggle(&mut self, id: Id) -> bool {
        if self.set.remove(&id) {
            if self.anchor == Some(id) {
                self.anchor = None;
            }
            false
        } else {
            self.set.insert(id);
            self.anchor = Some(id);
            true
        }
    }

    /// Shift+click or Shift+move: every row from the anchor to `to`. Without
    /// an anchor in the list `fallback` is used, then `to` alone.
    pub(crate) fn range(&mut self, rows: &[(Version, String)], fallback: Option<Id>, to: usize) {
        let Some((target, _)) = rows.get(to) else {
            return;
        };
        let find = |id: Id| rows.iter().position(|(v, _)| v.id == id);
        let anchor = self
            .anchor
            .and_then(find)
            .map(|index| (index, self.anchor))
            .or_else(|| fallback.and_then(find).map(|index| (index, fallback)))
            .unwrap_or((to, Some(target.id)));
        let (low, high) = (anchor.0.min(to), anchor.0.max(to));
        self.set = rows[low..=high].iter().map(|(v, _)| v.id).collect();
        self.anchor = anchor.1;
    }

    pub(crate) fn all(&mut self, rows: &[(Version, String)]) {
        self.set = rows.iter().map(|(v, _)| v.id).collect();
        self.anchor = rows.first().map(|(v, _)| v.id);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.set.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SongStep {
    Next,
    Previous,
    First,
    Last,
    PageDown,
    PageUp,
}

impl Operator {
    pub(crate) fn song_index_of(&self, version: Version) -> Option<usize> {
        self.songs().iter().position(|(v, _)| v.id == version.id)
    }

    /// Index of the primary song while the library (not the Schedule) owns
    /// the selection.
    pub(crate) fn song_cursor(&self) -> Option<usize> {
        self.selected_song
            .filter(|_| self.selected_entry.is_none())
            .and_then(|v| self.song_index_of(v))
    }

    /// The set only counts while it still contains the primary song; any
    /// other change of `selected_song` leaves just that song selected.
    pub(crate) fn is_song_selected(&self, version: Version) -> bool {
        if self.selected_entry.is_some() {
            return false;
        }
        match self.selected_song {
            Some(primary) if self.song_selection.contains(primary.id) => {
                self.song_selection.contains(version.id)
            }
            Some(primary) => primary.id == version.id,
            None => false,
        }
    }

    /// Selected songs in list order.
    pub(crate) fn selected_songs(&self) -> Vec<(Version, String)> {
        self.songs()
            .iter()
            .filter(|(v, _)| self.is_song_selected(*v))
            .cloned()
            .collect()
    }

    fn focus_song(&mut self, version: Version) {
        self.selected_song = Some(version);
        self.clear_entry_selection();
        self.live_on_load = None;
        // A Live-pane Page Down waiting for its entry's preview is cancelled
        // by picking a song, like by picking another entry (SC2-LIVE-03).
        self.live_entry_on_load = None;
    }

    /// Plain click or keyboard move: this song alone.
    pub(crate) fn select_song(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((version, _)) = self.songs().get(index).cloned() {
            self.song_selection.only(version.id);
            self.focus_song(version);
            cx.notify();
        }
    }

    /// Ctrl+click: adds or removes one song. Removing the primary song hands
    /// Preview to the first song still selected.
    pub(crate) fn toggle_song(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((version, _)) = self.songs().get(index).cloned() else {
            return;
        };
        if self.selected_entry.is_some() {
            self.song_selection.clear();
        } else if !self.is_song_selected_in_set() {
            match self.selected_song {
                Some(primary) => self.song_selection.only(primary.id),
                None => self.song_selection.clear(),
            }
        }
        if self.song_selection.toggle(version.id) {
            self.focus_song(version);
        } else if self.selected_song.is_some_and(|v| v.id == version.id) {
            match self.selected_songs_in_set().first().copied() {
                Some(next) => self.focus_song(next),
                None => self.selected_song = None,
            }
        }
        cx.notify();
    }

    /// Shift+click or Shift+move: the range from the anchor to `index`.
    pub(crate) fn extend_song(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((version, _)) = self.songs().get(index).cloned() else {
            return;
        };
        let fallback = self
            .selected_song
            .filter(|_| self.selected_entry.is_none())
            .map(|v| v.id);
        let rows: &[(Version, String)] = self.search.hits().unwrap_or(&self.catalog);
        self.song_selection.range(rows, fallback, index);
        self.focus_song(version);
        cx.notify();
    }

    pub(crate) fn select_all_songs(&mut self, cx: &mut Context<Self>) {
        let rows: &[(Version, String)] = self.search.hits().unwrap_or(&self.catalog);
        let Some(primary) = self
            .selected_song
            .filter(|_| self.selected_entry.is_none())
            .filter(|v| rows.iter().any(|(r, _)| r.id == v.id))
            .or_else(|| rows.first().map(|(v, _)| *v))
        else {
            return;
        };
        self.song_selection.all(rows);
        self.focus_song(primary);
        cx.notify();
    }

    fn is_song_selected_in_set(&self) -> bool {
        self.selected_song
            .is_some_and(|v| self.song_selection.contains(v.id))
    }

    fn selected_songs_in_set(&self) -> Vec<Version> {
        self.songs()
            .iter()
            .filter(|(v, _)| self.song_selection.contains(v.id))
            .map(|(v, _)| *v)
            .collect()
    }

    /// Rows moved by Page Up/Down: one fewer than the list shows.
    fn song_page(&self) -> usize {
        let height = f32::from(self.song_scroll.0.borrow().base_handle.bounds().size.height);
        if height <= 0. {
            return 10;
        }
        ((height / SONG_ROW_H).floor() as usize)
            .saturating_sub(1)
            .max(1)
    }

    /// Moves the selection (`extend`: grows the range instead), scrolls the
    /// row into view and focuses it once the list has built it.
    pub(crate) fn move_song(&mut self, step: SongStep, extend: bool, cx: &mut Context<Self>) {
        let len = self.songs().len();
        if len == 0 {
            return;
        }
        let cursor = self.song_cursor();
        let page = self.song_page();
        let target = match step {
            SongStep::Next => cursor.map_or(0, |i| i + 1),
            SongStep::Previous => cursor.map_or(len - 1, |i| i.saturating_sub(1)),
            SongStep::First => 0,
            SongStep::Last => len - 1,
            SongStep::PageDown => cursor.map_or(page - 1, |i| i + page),
            SongStep::PageUp => cursor.map_or(len - 1, |i| i.saturating_sub(page)),
        }
        .min(len - 1);
        if extend {
            self.extend_song(target, cx);
        } else {
            self.select_song(target, cx);
        }
        self.reveal_song(target);
    }

    pub(crate) fn reveal_song(&mut self, index: usize) {
        self.song_scroll
            .scroll_to_item(index, ScrollStrategy::Nearest);
        self.song_focus_pending = Some(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(n: u8) -> Vec<(Version, String)> {
        (0..n)
            .map(|i| {
                (
                    Version {
                        id: Id([i; 16]),
                        revision: 1,
                    },
                    format!("Song {i}"),
                )
            })
            .collect()
    }

    fn id(n: u8) -> Id {
        Id([n; 16])
    }

    #[test]
    fn shift_selects_the_range_from_the_anchor_and_ctrl_toggles() {
        let rows = rows(8);
        let mut selection = SongSelection::default();
        selection.only(id(1));
        selection.range(&rows, None, 4);
        assert_eq!((1..=4).filter(|n| selection.contains(id(*n))).count(), 4);
        assert!(!selection.contains(id(0)) && !selection.contains(id(5)));
        // The anchor stays: a shorter range shrinks from the far end.
        selection.range(&rows, None, 2);
        assert_eq!(selection.len(), 2);
        assert!(!selection.toggle(id(2)));
        assert!(selection.toggle(id(6)));
        assert!(selection.contains(id(1)) && selection.contains(id(6)));
        assert!(!selection.contains(id(2)));
    }

    #[test]
    fn a_range_without_an_anchor_uses_the_fallback_then_the_target() {
        let rows = rows(6);
        let mut selection = SongSelection::default();
        selection.range(&rows, Some(id(2)), 4);
        assert_eq!(selection.len(), 3);
        let mut selection = SongSelection::default();
        selection.range(&rows, Some(id(99)), 3);
        assert_eq!(selection.len(), 1);
        assert!(selection.contains(id(3)));
        selection.range(&rows, None, 99);
        assert_eq!(selection.len(), 1, "an out-of-range target changes nothing");
    }

    #[test]
    fn select_all_covers_every_row() {
        let rows = rows(5);
        let mut selection = SongSelection::default();
        selection.all(&rows);
        assert_eq!(selection.len(), 5);
        selection.clear();
        assert_eq!(selection.len(), 0);
    }
}
