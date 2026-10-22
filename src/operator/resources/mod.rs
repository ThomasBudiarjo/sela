//! Resource library tabs under the show panes.

use super::*;

mod media;
mod presentations;
mod scriptures;
mod song_search;
mod song_select;
mod songs;
mod themes;

pub(super) use song_search::SongSearch;
pub(super) use song_select::{SONG_ROW_H, SongSelection};
pub(super) use songs::{
    ClearSongSearch, DeleteSongRow, ExtendFirstSongRow, ExtendLastSongRow, ExtendNextSongRow,
    ExtendPreviousSongRow, FirstSongRow, LastSongRow, NextSongRow, PageDownSongRow, PageUpSongRow,
    PreviousSongRow, SEARCH_TAB_INDEX, SelectAllSongRows,
};
pub(crate) use themes::{ThemeJob, ThemeState};

/// Tab labels, translated at render time.
pub(crate) const TABS: [Msg; 5] = [
    Msg::TabSongs,
    Msg::TabScriptures,
    Msg::TabMedia,
    Msg::TabPresentations,
    Msg::TabThemes,
];

/// Static element ids/selectors for the tabs; ids never change with language
/// so tests and scripts can find the controls in any language.
pub(crate) const TAB_IDS: [&str; 5] = ["Songs", "Scriptures", "Media", "Presentations", "Themes"];

pub(crate) fn empty_detail(title: impl Into<SharedString>, text: &'static str) -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(div().font_weight(FontWeight::MEDIUM).child(title.into()))
        .child(
            div()
                .mt_2()
                .text_size(px(TEXT_SMALL))
                .text_color(rgb(MUTED))
                .child(text),
        )
        .into_any_element()
}

impl Operator {
    /// The detail area of the selected resource tab.
    pub(crate) fn resource_detail(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.tab {
            0 => self.song_list(cx),
            1 => scriptures::detail(),
            2 => self.image_list(cx),
            3 => presentations::detail(),
            _ => self.theme_list(cx),
        }
    }
}
