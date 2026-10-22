//! Framework-independent preparation and presentation contracts.
// Unsafe code is listed and justified in docs/security.md; new unsafe code
// needs an explicit allow and an entry there.
#![deny(unsafe_code)]
pub mod arrangement;
pub mod arrangement_edit;
pub mod background;
pub mod delivery;
pub mod fonts;
pub mod format;
#[cfg(test)]
mod fuzz;
pub mod i18n;
pub mod images;
pub mod import;
pub mod labels;
pub mod masks;
pub mod media;
pub mod output;
pub mod pane_layout;
pub mod preflight;
pub mod preparation;
pub mod scene;
pub mod schedule;
pub mod schedule_file;
pub mod scripture;
pub mod search;
pub mod settings;
pub mod slides;
pub mod song_order;
pub mod storage;
pub mod timer;
pub mod transport;
