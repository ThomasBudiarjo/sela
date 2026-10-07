//! Song search over the schema-5 FTS5 index: query escaping, result types and
//! stale-result rejection. The index and its SQL live in `storage`; see
//! `docs/search.md` for normalization, ranking and the measured budget.
use crate::storage::Version;

/// Longest accepted query, in UTF-8 bytes; longer submissions are `Invalid`.
pub const MAX_QUERY_BYTES: usize = 1024;
/// Most hits one search returns. `Results::truncated` reports that more matched.
pub const MAX_HITS: usize = 200;

/// One matching song at its current revision, with its display title unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub version: Version,
    pub title: String,
}

/// The ranked hits for one query. `generation` is the caller's value from the
/// command, returned unchanged so late replies can be recognized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Results {
    pub generation: u64,
    pub hits: Vec<Hit>,
    pub truncated: bool,
}

/// Outcome of a search index repair or rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexState {
    /// The check passed and nothing was written.
    Healthy,
    /// The index was rebuilt from the current song revisions.
    Rebuilt { songs: usize },
}

/// Builds the FTS5 MATCH expression for a user query, or `None` when the query
/// has no searchable term (empty, whitespace or punctuation only).
///
/// Every whitespace-separated term becomes one double-quoted FTS5 string, with
/// embedded quotes doubled, so operators, column filters, `NEAR(`, `*`, `^`
/// and parentheses are only ever text. The tokenizer then splits a term at
/// punctuation into an adjacent-token phrase (`kasih-Mu` is "kasih mu"). The
/// last term is a prefix query. Terms are implicitly ANDed.
pub fn expression(query: &str) -> Option<String> {
    // A quoted string without a token character is an empty FTS5 phrase, which
    // matches nothing and would empty the whole AND, so such terms are dropped.
    let terms: Vec<&str> = query
        .split_whitespace()
        .filter(|term| term.chars().any(char::is_alphanumeric))
        .collect();
    let (last, rest) = terms.split_last()?;
    let mut out = String::with_capacity(query.len() + 4 * terms.len());
    for term in rest {
        quote(&mut out, term);
        out.push(' ');
    }
    quote(&mut out, last);
    out.push('*');
    Some(out)
}

fn quote(out: &mut String, term: &str) {
    out.push('"');
    for c in term.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
}

/// Caller-side guard against stale search results. Take a new generation for
/// every query and show a reply only while `is_current` holds for its
/// generation, whether it arrives from the same worker after a cancellation
/// lost the race or from another worker out of order.
#[derive(Debug, Default)]
pub struct Generations {
    latest: u64,
}

impl Generations {
    /// The generation for a new query; every earlier one becomes stale.
    pub fn advance(&mut self) -> u64 {
        self.latest += 1;
        self.latest
    }
    pub fn is_current(&self, generation: u64) -> bool {
        generation != 0 && generation == self.latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_quoted_and_the_last_one_is_a_prefix() {
        assert_eq!(
            expression("amazing gr").as_deref(),
            Some("\"amazing\" \"gr\"*")
        );
        assert_eq!(expression("  Yesus\t").as_deref(), Some("\"Yesus\"*"));
        assert_eq!(expression("kasih-Mu").as_deref(), Some("\"kasih-Mu\"*"));
    }

    #[test]
    fn fts_syntax_is_quoted_text() {
        assert_eq!(
            expression("a\"b AND NEAR( title:x").as_deref(),
            Some("\"a\"\"b\" \"AND\" \"NEAR(\" \"title:x\"*")
        );
        assert_eq!(
            expression("^start (x)").as_deref(),
            Some("\"^start\" \"(x)\"*")
        );
    }

    #[test]
    fn queries_without_a_searchable_term_are_none() {
        for query in [
            "",
            "   ",
            "\t\n",
            "\"",
            "*",
            "\" * - ^ ( ) :",
            "\u{301}",
            "''",
        ] {
            assert_eq!(expression(query), None, "{query:?}");
        }
        assert_eq!(expression("* love").as_deref(), Some("\"love\"*"));
    }

    #[test]
    fn only_the_latest_generation_is_current() {
        let mut generations = Generations::default();
        assert!(!generations.is_current(0));
        let first = generations.advance();
        assert!(generations.is_current(first));
        let second = generations.advance();
        assert!(!generations.is_current(first));
        assert!(generations.is_current(second));
    }
}
