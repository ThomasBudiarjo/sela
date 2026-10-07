//! Presentations tab. Not implemented yet.

use super::*;

pub(super) fn detail() -> gpui::AnyElement {
    empty_detail(
        format!("{} library is empty", TABS[3]),
        "This resource library is not implemented.",
    )
}
