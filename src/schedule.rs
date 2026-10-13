//! Operator schedule: an ordered list of entries, each pinning one immutable
//! song revision. Entry IDs are session-local identities for selection and
//! Live. They are not persisted, so reopening a schedule allocates fresh IDs
//! that never match an entry from an earlier schedule in the same session.
//! The schedule's document is a `.selaschedule` file (see `schedule_file`).
use crate::storage::{MAX_ITEMS, Version};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub version: Version,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// `storage::MAX_ITEMS` entries already.
    Full,
    /// A stored schedule and its resolved songs disagree.
    Invalid,
}

#[derive(Debug, Default)]
pub struct Schedule {
    entries: Vec<Entry>,
    next: u64,
    path: Option<PathBuf>,
    /// Revisions as last saved or opened; differs from `entries` when dirty.
    baseline: Vec<Version>,
}

impl Schedule {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The schedule file; `None` while untitled.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// File name shown for the schedule; `None` while untitled.
    pub fn name(&self) -> Option<String> {
        self.path.as_deref().map(crate::schedule_file::display_name)
    }

    pub fn is_dirty(&self) -> bool {
        self.entries
            .iter()
            .map(|e| e.version)
            .ne(self.baseline.iter().copied())
    }

    pub fn index(&self, id: EntryId) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    pub fn get(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn allocate(&mut self) -> EntryId {
        self.next += 1;
        EntryId(self.next)
    }

    /// Inserts before the entry at `at`, or appends when `at` is past the end.
    /// The same revision may appear more than once; each gets its own entry.
    pub fn insert(&mut self, at: usize, version: Version, title: String) -> Result<EntryId, Error> {
        if self.entries.len() >= MAX_ITEMS {
            return Err(Error::Full);
        }
        let id = self.allocate();
        let at = at.min(self.entries.len());
        self.entries.insert(at, Entry { id, version, title });
        Ok(id)
    }

    pub fn push(&mut self, version: Version, title: String) -> Result<EntryId, Error> {
        self.insert(usize::MAX, version, title)
    }

    pub fn remove(&mut self, id: EntryId) -> Option<Entry> {
        let index = self.index(id)?;
        Some(self.entries.remove(index))
    }

    /// Moves the entry so it ends up at `index` (clamped). Returns whether the
    /// order changed.
    pub fn move_to(&mut self, id: EntryId, index: usize) -> bool {
        let Some(from) = self.index(id) else {
            return false;
        };
        let to = index.min(self.entries.len() - 1);
        if from == to {
            return false;
        }
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        true
    }

    /// One step towards the start (`up`) or the end. False at the boundary.
    pub fn move_by(&mut self, id: EntryId, up: bool) -> bool {
        match self.index(id) {
            Some(0) if up => false,
            Some(index) if up => self.move_to(id, index - 1),
            Some(index) => self.move_to(id, index + 1),
            None => false,
        }
    }

    /// The next (`forward`) or previous entry. Without a current entry, or
    /// when it was removed, starts at the first (forward) or last entry.
    /// `None` at either end: navigation stops rather than wrapping.
    pub fn neighbor(&self, from: Option<EntryId>, forward: bool) -> Option<EntryId> {
        let target = match from.and_then(|id| self.index(id)) {
            Some(index) if forward => index + 1,
            Some(index) => index.checked_sub(1)?,
            None if forward => 0,
            None => self.entries.len().checked_sub(1)?,
        };
        self.entries.get(target).map(|e| e.id)
    }

    /// The pinned revisions in order: what a save writes.
    pub fn versions(&self) -> Vec<Version> {
        self.entries.iter().map(|e| e.version).collect()
    }

    /// Records a completed save of `versions` to `path`. Edits made while the
    /// save was in flight stay dirty.
    pub fn mark_saved(&mut self, path: PathBuf, versions: Vec<Version>) {
        self.path = Some(path);
        self.baseline = versions;
    }

    /// Replaces the contents with a schedule file's items (`version`, song
    /// title), in file order.
    pub fn open(&mut self, path: PathBuf, items: Vec<(Version, String)>) -> Result<(), Error> {
        if items.len() > MAX_ITEMS {
            return Err(Error::Invalid);
        }
        self.baseline = items.iter().map(|(version, _)| *version).collect();
        self.entries = items
            .into_iter()
            .map(|(version, title)| Entry {
                id: self.allocate(),
                version,
                title,
            })
            .collect();
        self.path = Some(path);
        Ok(())
    }

    /// An empty, untitled schedule. Entry IDs keep counting.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.path = None;
        self.baseline.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Id;

    fn v(n: u8, revision: i64) -> Version {
        Version {
            id: Id([n; 16]),
            revision,
        }
    }

    fn order(s: &Schedule) -> Vec<EntryId> {
        s.entries().iter().map(|e| e.id).collect()
    }

    #[test]
    fn duplicates_are_separate_entries() {
        let mut s = Schedule::default();
        let a = s.push(v(1, 1), "A".into()).unwrap();
        let b = s.push(v(1, 1), "A".into()).unwrap();
        assert_ne!(a, b);
        assert_eq!(s.entries().len(), 2);
        assert_eq!(s.remove(b).unwrap().id, b);
        assert_eq!(order(&s), [a]);
        assert!(s.remove(b).is_none());
    }

    #[test]
    fn insert_clamps_and_limit_is_enforced() {
        let mut s = Schedule::default();
        let a = s.push(v(1, 1), "A".into()).unwrap();
        let b = s.insert(0, v(2, 1), "B".into()).unwrap();
        let c = s.insert(99, v(3, 1), "C".into()).unwrap();
        assert_eq!(order(&s), [b, a, c]);
        while s.entries().len() < MAX_ITEMS {
            s.push(v(4, 1), "D".into()).unwrap();
        }
        assert_eq!(s.push(v(5, 1), "E".into()), Err(Error::Full));
        assert_eq!(s.entries().len(), MAX_ITEMS);
    }

    #[test]
    fn reorder_keeps_identity() {
        let mut s = Schedule::default();
        let ids: Vec<_> = (1..=4)
            .map(|n| s.push(v(n, 1), n.to_string()).unwrap())
            .collect();
        assert!(s.move_to(ids[0], 2));
        assert_eq!(order(&s), [ids[1], ids[2], ids[0], ids[3]]);
        assert!(s.move_to(ids[3], 0));
        assert_eq!(order(&s), [ids[3], ids[1], ids[2], ids[0]]);
        assert!(!s.move_to(ids[0], 99), "already last");
        assert!(!s.move_by(ids[3], true), "already first");
        assert!(s.move_by(ids[3], false));
        assert_eq!(order(&s), [ids[1], ids[3], ids[2], ids[0]]);
        assert!(s.move_by(ids[0], true));
        assert_eq!(order(&s), [ids[1], ids[3], ids[0], ids[2]]);
        let removed = s.remove(ids[1]).unwrap().id;
        assert!(!s.move_to(removed, 0));
        assert_eq!(s.get(ids[0]).unwrap().version, v(1, 1));
    }

    #[test]
    fn neighbor_stops_at_the_ends() {
        let mut s = Schedule::default();
        assert_eq!(s.neighbor(None, true), None);
        assert_eq!(s.neighbor(None, false), None);
        let a = s.push(v(1, 1), "A".into()).unwrap();
        let b = s.push(v(2, 1), "B".into()).unwrap();
        assert_eq!(s.neighbor(None, true), Some(a));
        assert_eq!(s.neighbor(None, false), Some(b));
        assert_eq!(s.neighbor(Some(a), true), Some(b));
        assert_eq!(s.neighbor(Some(b), true), None);
        assert_eq!(s.neighbor(Some(a), false), None);
        assert_eq!(s.neighbor(Some(b), false), Some(a));
        s.remove(a);
        assert_eq!(s.neighbor(Some(a), true), Some(b), "removed restarts");
    }

    #[test]
    fn dirty_tracks_saved_revisions() {
        let mut s = Schedule::default();
        assert!(!s.is_dirty());
        let a = s.push(v(1, 1), "A".into()).unwrap();
        let b = s.push(v(2, 1), "B".into()).unwrap();
        assert!(s.is_dirty());
        let snapshot = s.versions();
        s.push(v(3, 1), "C".into()).unwrap();
        s.mark_saved(PathBuf::from("C:/x/Sunday.selaschedule"), snapshot);
        assert!(s.is_dirty(), "edit during the save stays unsaved");
        let c = s.neighbor(Some(b), true).unwrap();
        s.remove(c);
        assert!(!s.is_dirty());
        assert_eq!(s.name().as_deref(), Some("Sunday.selaschedule"));
        s.move_to(a, 1);
        assert!(s.is_dirty());
        s.move_to(a, 0);
        assert!(!s.is_dirty(), "restored order is clean");
    }

    #[test]
    fn open_allocates_fresh_ids_and_validates() {
        let mut s = Schedule::default();
        let old = s.push(v(1, 1), "A".into()).unwrap();
        let too_many = (0..=MAX_ITEMS).map(|n| (v(1, 1), n.to_string())).collect();
        assert_eq!(
            s.open(PathBuf::from("a.selaschedule"), too_many),
            Err(Error::Invalid)
        );
        assert_eq!(order(&s), [old], "failed open keeps contents");
        s.open(
            PathBuf::from("Evening.selaschedule"),
            vec![
                (v(1, 1), "A".into()),
                (v(1, 1), "A".into()),
                (v(2, 3), "B".into()),
            ],
        )
        .unwrap();
        assert!(!s.is_dirty());
        assert_eq!(s.name().as_deref(), Some("Evening.selaschedule"));
        assert!(
            s.get(old).is_none(),
            "IDs from another schedule never match"
        );
        assert_eq!(s.versions(), [v(1, 1), v(1, 1), v(2, 3)]);
        let ids = order(&s);
        assert_ne!(ids[0], ids[1]);
        s.clear();
        assert!(!s.is_dirty());
        assert_eq!((s.path(), s.name()), (None, None));
        let fresh = s.push(v(1, 1), "A".into()).unwrap();
        assert!(!ids.contains(&fresh));
    }
}
