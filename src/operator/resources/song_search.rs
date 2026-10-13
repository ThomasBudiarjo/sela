//! Songs search controller: typed text, one generation per query, a single
//! pending-submission slot and the newest accepted results. Framework
//! independent; the operator submits `take_command` through the storage worker.
use sela::{
    search::{Generations, Results},
    storage::{Command, Version},
};

/// Owner decision: no full-text query below two characters. EasyWorship
/// filters from one (EW8-OBS-045); Sela shows the unfiltered list instead.
pub(crate) const MIN_QUERY_CHARS: usize = 2;

#[derive(Default)]
pub(crate) struct SongSearch {
    generations: Generations,
    /// The trimmed query being searched; empty while unfiltered.
    query: String,
    /// The newest query not yet handed to the worker. A newer one replaces it.
    wanted: Option<(String, u64)>,
    hits: Option<Vec<(Version, String)>>,
    #[cfg(test)]
    submitted: u64,
}

impl SongSearch {
    pub(crate) fn active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Takes the field's text. Returns true when the effective query changed.
    pub(crate) fn set_text(&mut self, text: &str) -> bool {
        let trimmed = text.trim();
        let query = if trimmed.chars().count() >= MIN_QUERY_CHARS {
            trimmed
        } else {
            ""
        };
        if query == self.query {
            return false;
        }
        self.query = query.to_string();
        self.restart();
        true
    }

    /// Re-runs the current query, e.g. after a song was saved or deleted.
    pub(crate) fn refresh(&mut self) {
        if self.active() {
            self.restart();
        }
    }

    fn restart(&mut self) {
        // Advancing also makes any reply still in flight stale.
        let generation = self.generations.advance();
        if self.active() {
            self.wanted = Some((self.query.clone(), generation));
        } else {
            self.wanted = None;
            self.hits = None;
        }
    }

    pub(crate) fn take_command(&mut self) -> Option<Command> {
        let (query, generation) = self.wanted.take()?;
        #[cfg(test)]
        {
            self.submitted += 1;
        }
        Some(Command::Search { query, generation })
    }

    /// Shows `results` only while they answer the newest query.
    pub(crate) fn accept(&mut self, results: Results) -> bool {
        if !self.generations.is_current(results.generation) {
            return false;
        }
        self.hits = Some(
            results
                .hits
                .into_iter()
                .map(|hit| (hit.version, hit.title))
                .collect(),
        );
        true
    }

    /// `accept`, then deselects a library selection that is not among the
    /// hits. Preview keeps what it shows and no other song is previewed.
    pub(crate) fn apply(
        &mut self,
        results: Results,
        schedule_selected: bool,
        selected_song: &mut Option<Version>,
    ) -> bool {
        if !self.accept(results) {
            return false;
        }
        if !schedule_selected
            && let Some(selected) = *selected_song
            && !self
                .hits()
                .is_some_and(|hits| hits.iter().any(|(v, _)| v.id == selected.id))
        {
            *selected_song = None;
        }
        true
    }

    /// The filtered rows, or `None` while the list is unfiltered.
    pub(crate) fn hits(&self) -> Option<&[(Version, String)]> {
        self.hits.as_deref()
    }

    pub(crate) fn forget(&mut self, version: Version) {
        if let Some(hits) = &mut self.hits {
            hits.retain(|(v, _)| *v != version);
        }
    }

    #[cfg(test)]
    pub(crate) fn submitted(&self) -> u64 {
        self.submitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::{search::Hit, storage::Id};

    fn version(n: u8) -> Version {
        Version {
            id: Id([n; 16]),
            revision: 1,
        }
    }

    fn results(generation: u64, titles: &[&str]) -> Results {
        Results {
            generation,
            hits: titles
                .iter()
                .enumerate()
                .map(|(i, title)| Hit {
                    version: version(i as u8),
                    title: (*title).into(),
                })
                .collect(),
            truncated: false,
        }
    }

    fn generation(command: Option<Command>) -> u64 {
        match command {
            Some(Command::Search { generation, .. }) => generation,
            other => panic!("expected a search, got {other:?}"),
        }
    }

    #[test]
    fn fewer_than_two_characters_issue_no_search() {
        let mut search = SongSearch::default();
        for text in ["", "g", " g ", "   "] {
            assert!(!search.set_text(text));
            assert!(search.take_command().is_none());
        }
        assert_eq!(search.submitted(), 0);
        assert!(search.hits().is_none() && !search.active());
        assert!(search.set_text("gr"));
        generation(search.take_command());
        assert_eq!(search.submitted(), 1);
    }

    #[test]
    fn shortening_below_two_returns_to_the_unfiltered_list_and_drops_late_replies() {
        let mut search = SongSearch::default();
        search.set_text("gr");
        let g = generation(search.take_command());
        assert!(search.accept(results(g, &["Grace"])));
        assert_eq!(search.hits().map(<[_]>::len), Some(1));
        assert!(search.set_text("g"));
        assert!(search.hits().is_none());
        assert!(
            !search.accept(results(g, &["Grace"])),
            "late reply is stale"
        );
        assert!(search.hits().is_none());
    }

    #[test]
    fn a_newer_query_replaces_the_pending_one_and_stale_replies_are_dropped() {
        let mut search = SongSearch::default();
        search.set_text("gr");
        let older = generation(search.take_command());
        search.set_text("gra");
        search.set_text("grac");
        let newer = generation(search.take_command());
        assert!(newer > older);
        assert!(search.take_command().is_none(), "only the newest is sent");
        assert!(search.accept(results(newer, &["Grace"])));
        assert!(!search.accept(results(older, &["Gray", "Great"])));
        let titles: Vec<_> = search.hits().unwrap().iter().map(|(_, t)| t).collect();
        assert_eq!(titles, ["Grace"]);
    }

    #[test]
    fn whitespace_changes_do_not_resubmit_and_refresh_reruns_the_query() {
        let mut search = SongSearch::default();
        search.set_text("gr");
        let first = generation(search.take_command());
        assert!(!search.set_text(" gr "));
        assert!(search.take_command().is_none());
        search.refresh();
        let second = generation(search.take_command());
        assert!(second > first);
        assert!(!search.accept(results(first, &[])));
    }

    #[test]
    fn refresh_while_unfiltered_does_nothing() {
        let mut search = SongSearch::default();
        search.refresh();
        assert!(search.take_command().is_none());
    }
}
