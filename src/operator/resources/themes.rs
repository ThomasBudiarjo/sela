//! Themes tab. Not implemented yet.

use super::*;

pub(super) fn detail() -> gpui::AnyElement {
    empty_detail(
        t!(Msg::LibraryEmpty, tab = t!(TABS[4])),
        t!(Msg::LibraryNotImplemented),
    )
}
