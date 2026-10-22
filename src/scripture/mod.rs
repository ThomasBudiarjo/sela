//! Scripture model and parsing, independent of any Bible data source.
mod books;
mod reference;

pub use books::{Book, complete};
pub use reference::{ParseError, Reference, Span, Verse, parse};
