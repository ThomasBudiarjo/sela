//! Scripture model, parsing, Bible file import, installed-translation
//! storage and the off-UI-thread provider worker (M4-03, M4-04). See
//! docs/scripture.md.
mod books;
pub mod bundled;
pub mod import;
pub mod model;
pub mod passage;
mod reference;
pub mod store;
pub mod worker;

pub use books::{Book, complete};
pub use model::{Inventory, TranslationInfo, VerseId};
pub use reference::{ParseError, Reference, Span, Verse, parse};
