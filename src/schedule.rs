//! Operator schedule: an ordered list of entries, each pinning one immutable
//! song revision. Entry IDs are session-local identities for selection and
//! Live. They are not persisted, so reopening a schedule allocates fresh IDs
//! that never match an entry from an earlier schedule in the same session.
//! The schedule's document is a `.selaschedule` file (see `schedule_file`).
use crate::arrangement::VariantId;
use crate::scripture::Snapshot;
use crate::storage::{MAX_ITEMS, Version};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub version: Version,
    pub title: String,
    /// The arrangement this occurrence plays; None is the song's first. It
    /// belongs to the entry, never to the library song, so a duplicate can
    /// differ from its original.
    pub arrangement: Option<VariantId>,
    /// A scripture item's pinned verses (M4-04); `None` for a song. Its
    /// `version` is `Snapshot::version`, which no library song has.
    pub scripture: Option<Arc<Snapshot>>,
}

/// What a saved schedule remembers about one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    pub version: Version,
    pub arrangement: Option<VariantId>,
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
    baseline: Vec<Pin>,
    /// Bumped whenever the document is replaced (open, clear), so a save
    /// that finishes late can tell it no longer describes the current one.
    document: u64,
}

impl Schedule {
    pub fn document(&self) -> u64 {
        self.document
    }

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
        self.pins().ne(self.baseline.iter().copied())
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
        self.entries.insert(
            at,
            Entry {
                id,
                version,
                title,
                arrangement: None,
                scripture: None,
            },
        );
        Ok(id)
    }

    /// Inserts a scripture item pinning `snapshot`, titled with its
    /// reference, before the entry at `at` (past the end appends).
    pub fn insert_scripture(
        &mut self,
        at: usize,
        snapshot: Snapshot,
        title: String,
    ) -> Result<EntryId, Error> {
        if !snapshot.valid() {
            return Err(Error::Invalid);
        }
        if self.entries.len() >= MAX_ITEMS {
            return Err(Error::Full);
        }
        let id = self.allocate();
        let at = at.min(self.entries.len());
        self.entries.insert(
            at,
            Entry {
                id,
                version: snapshot.version(),
                title,
                arrangement: None,
                scripture: Some(Arc::new(snapshot)),
            },
        );
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

    /// Inserts a copy of each entry in `ids` directly after its original, with
    /// a fresh ID. Nothing is copied when the copies would not fit. Returns
    /// the new IDs in schedule order.
    pub fn duplicate(&mut self, ids: &[EntryId]) -> Result<Vec<EntryId>, Error> {
        let originals: Vec<EntryId> = self
            .entries
            .iter()
            .map(|e| e.id)
            .filter(|id| ids.contains(id))
            .collect();
        if self.entries.len() + originals.len() > MAX_ITEMS {
            return Err(Error::Full);
        }
        let mut copies = Vec::with_capacity(originals.len());
        for original in originals {
            let id = self.allocate();
            let at = self.index(original).map_or(self.entries.len(), |i| i + 1);
            let mut entry = self.entries[at - 1].clone();
            entry.id = id;
            self.entries.insert(at, entry);
            copies.push(id);
        }
        Ok(copies)
    }

    /// Moves every entry in `ids` so the group sits just before the entry
    /// that is at `before` now (`usize::MAX` or past the end: the end). The
    /// group keeps its relative order. Returns whether the order changed.
    pub fn move_group(&mut self, ids: &[EntryId], before: usize) -> bool {
        let before = before.min(self.entries.len());
        let selected = |e: &Entry| ids.contains(&e.id);
        let landing = self.entries[..before]
            .iter()
            .filter(|e| !selected(e))
            .count();
        let (group, mut rest): (Vec<Entry>, Vec<Entry>) =
            self.entries.iter().cloned().partition(selected);
        if group.is_empty() {
            return false;
        }
        let tail = rest.split_off(landing);
        let moved = rest
            .into_iter()
            .chain(group)
            .chain(tail)
            .collect::<Vec<_>>();
        let changed = moved
            .iter()
            .map(|e| e.id)
            .ne(self.entries.iter().map(|e| e.id));
        self.entries = moved;
        changed
    }

    /// Moves the group one step up or down as a block; entries already at
    /// that edge, or blocked by a neighbour that is also moving, stay put.
    pub fn move_group_by(&mut self, ids: &[EntryId], up: bool) -> bool {
        let mut changed = false;
        let len = self.entries.len();
        let order: Vec<usize> = if up {
            (0..len).collect()
        } else {
            (0..len).rev().collect()
        };
        for index in order {
            if !ids.contains(&self.entries[index].id) {
                continue;
            }
            let neighbour = if up {
                index.checked_sub(1)
            } else {
                (index + 1 < len).then_some(index + 1)
            };
            if let Some(other) = neighbour
                && !ids.contains(&self.entries[other].id)
            {
                self.entries.swap(index, other);
                changed = true;
            }
        }
        changed
    }

    /// Re-pins an entry to another revision of its song.
    pub fn set_version(&mut self, id: EntryId, version: Version, title: String) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(entry) => {
                entry.version = version;
                entry.title = title;
                true
            }
            None => false,
        }
    }

    /// Chooses the arrangement for one entry only. False for an unknown entry.
    pub fn set_arrangement(&mut self, id: EntryId, arrangement: Option<VariantId>) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(entry) => {
                entry.arrangement = arrangement;
                true
            }
            None => false,
        }
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

    /// Entries as a save writes them: revision plus chosen arrangement.
    pub fn pins(&self) -> impl Iterator<Item = Pin> + '_ {
        self.entries.iter().map(|e| Pin {
            version: e.version,
            arrangement: e.arrangement,
        })
    }

    /// Records a completed save of `pins` to `path`. Edits made while the
    /// save was in flight stay dirty.
    pub fn mark_saved(&mut self, path: PathBuf, pins: Vec<Pin>) {
        self.path = Some(path);
        self.baseline = pins;
    }

    /// Replaces the contents with a schedule file's items (pin, song title),
    /// in file order.
    pub fn open(&mut self, path: PathBuf, items: Vec<(Pin, String)>) -> Result<(), Error> {
        self.open_items(
            path,
            items
                .into_iter()
                .map(|(pin, title)| (pin, title, None))
                .collect(),
        )
    }

    /// `open` for a file that may hold scripture items: each comes with its
    /// snapshot, whose version must be the pin's.
    pub fn open_items(
        &mut self,
        path: PathBuf,
        items: Vec<(Pin, String, Option<Snapshot>)>,
    ) -> Result<(), Error> {
        if items.len() > MAX_ITEMS
            || items.iter().any(|(pin, _, snapshot)| {
                snapshot
                    .as_ref()
                    .is_some_and(|s| !s.valid() || s.version() != pin.version)
            })
        {
            return Err(Error::Invalid);
        }
        self.document += 1;
        self.baseline = items.iter().map(|(pin, _, _)| *pin).collect();
        self.entries = items
            .into_iter()
            .map(|(pin, title, snapshot)| Entry {
                id: self.allocate(),
                version: pin.version,
                title,
                arrangement: pin.arrangement,
                scripture: snapshot.map(Arc::new),
            })
            .collect();
        self.path = Some(path);
        Ok(())
    }

    /// An empty, untitled schedule. Entry IDs keep counting.
    pub fn clear(&mut self) {
        self.document += 1;
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

    fn pin(n: u8, revision: i64) -> Pin {
        Pin {
            version: v(n, revision),
            arrangement: None,
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

    fn six() -> (Schedule, Vec<EntryId>) {
        let mut s = Schedule::default();
        let ids = (1..=6)
            .map(|n| s.push(v(n, 1), n.to_string()).unwrap())
            .collect();
        (s, ids)
    }

    #[test]
    fn group_moves_to_the_end_keeping_relative_order() {
        let (mut s, ids) = six();
        assert!(s.move_group(&[ids[3], ids[1]], usize::MAX));
        assert_eq!(
            order(&s),
            [ids[0], ids[2], ids[4], ids[5], ids[1], ids[3]],
            "1,3,5,6,2,4"
        );
        assert!(!s.move_group(&[ids[1], ids[3]], usize::MAX), "already last");
    }

    #[test]
    fn group_drops_before_a_row_and_same_place_is_a_no_op() {
        let (mut s, ids) = six();
        assert!(s.move_group(&[ids[4], ids[5]], 1));
        assert_eq!(order(&s), [ids[0], ids[4], ids[5], ids[1], ids[2], ids[3]]);
        let (mut s, ids) = six();
        assert!(!s.move_group(&[ids[2]], 2), "before itself");
        assert!(!s.move_group(&[ids[2]], 3), "before its successor");
        assert!(s.move_group(&[ids[0]], 3));
        assert_eq!(order(&s)[..3], [ids[1], ids[2], ids[0]]);
        assert!(!s.move_group(&[], 0));
    }

    #[test]
    fn group_steps_as_a_block_and_stops_at_the_edge() {
        let (mut s, ids) = six();
        assert!(s.move_group_by(&[ids[1], ids[2], ids[4]], true));
        assert_eq!(order(&s), [ids[1], ids[2], ids[0], ids[4], ids[3], ids[5]]);
        assert!(!s.move_group_by(&[ids[1], ids[2]], true), "at the top");
        let (mut s, ids) = six();
        assert!(!s.move_group_by(&[ids[4], ids[5]], false), "at the bottom");
        assert!(s.move_group_by(&[ids[1], ids[4]], false));
        assert_eq!(order(&s), [ids[0], ids[2], ids[1], ids[3], ids[5], ids[4]]);
    }

    #[test]
    fn duplicate_is_independent_and_follows_its_original() {
        let (mut s, ids) = six();
        let copies = s.duplicate(&[ids[1]]).unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(order(&s)[1..3], [ids[1], copies[0]]);
        assert_eq!(s.get(copies[0]).unwrap().title, "2");
        assert_eq!(s.get(copies[0]).unwrap().version, v(2, 1));
        assert!(s.set_version(copies[0], v(2, 2), "2b".into()));
        assert_eq!(
            s.get(ids[1]).unwrap().version,
            v(2, 1),
            "original unchanged"
        );
        s.remove(ids[1]);
        assert!(s.get(copies[0]).is_some(), "copy survives its original");
    }

    #[test]
    fn arrangement_belongs_to_the_entry_and_is_part_of_the_saved_state() {
        let (mut s, ids) = six();
        let (full, short) = (VariantId(Id([7; 16])), VariantId(Id([8; 16])));
        assert!(s.set_arrangement(ids[1], Some(full)));
        let copies = s.duplicate(&[ids[1]]).unwrap();
        assert_eq!(
            s.get(copies[0]).unwrap().arrangement,
            Some(full),
            "copy starts equal"
        );
        assert!(s.set_arrangement(copies[0], Some(short)));
        assert_eq!(s.get(ids[1]).unwrap().arrangement, Some(full));
        assert_eq!(s.get(copies[0]).unwrap().arrangement, Some(short));
        assert!(!s.set_arrangement(EntryId(999), None));
        let saved: Vec<Pin> = s.pins().collect();
        s.mark_saved(PathBuf::from("C:/x/A.selaschedule"), saved);
        assert!(!s.is_dirty());
        s.set_arrangement(copies[0], Some(full));
        assert!(
            s.is_dirty(),
            "changing only the arrangement is an unsaved change"
        );
        s.set_arrangement(copies[0], Some(short));
        assert!(!s.is_dirty());
    }

    #[test]
    fn duplicate_is_all_or_nothing_at_the_limit() {
        let mut s = Schedule::default();
        let ids: Vec<_> = (0..MAX_ITEMS - 1)
            .map(|n| s.push(v(1, n as i64), "x".into()).unwrap())
            .collect();
        assert_eq!(s.duplicate(&[ids[0], ids[1]]), Err(Error::Full));
        assert_eq!(s.entries().len(), MAX_ITEMS - 1);
        assert_eq!(s.duplicate(&[ids[0]]).unwrap().len(), 1);
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
        let snapshot: Vec<Pin> = s.pins().collect();
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
    fn the_document_generation_changes_on_open_and_clear_only() {
        let mut s = Schedule::default();
        let first = s.document();
        s.push(v(1, 1), "A".into()).unwrap();
        s.mark_saved("a".into(), s.pins().collect());
        assert_eq!(s.document(), first);
        s.open("b".into(), vec![(pin(2, 1), "B".into())]).unwrap();
        let opened = s.document();
        assert_ne!(opened, first);
        s.clear();
        assert_ne!(s.document(), opened);
    }

    fn snapshot(text: &str) -> Snapshot {
        use crate::scripture::{Book, TranslationInfo, VerseId};
        let info = TranslationInfo {
            id: "SYN".into(),
            name: "Synthetic".into(),
            language: "en".into(),
            rights: String::new(),
            source: String::new(),
            redistributable: true,
            bundled: false,
        };
        let id = VerseId::new(Book::from_number(43).unwrap(), 3, 16).unwrap();
        Snapshot::new(&info, &[(id, text.into())])
    }

    #[test]
    fn scripture_items_pin_their_snapshot_and_reopen_with_it() {
        let mut s = Schedule::default();
        let song = s.push(v(1, 1), "Song".into()).unwrap();
        let verse = s
            .insert_scripture(0, snapshot("One"), "John 3:16 (SYN)".into())
            .unwrap();
        assert_eq!(order(&s), [verse, song]);
        let entry = s.get(verse).unwrap().clone();
        assert_eq!(entry.version, snapshot("One").version());
        assert_eq!(entry.scripture.as_deref(), Some(&snapshot("One")));
        assert!(s.get(song).unwrap().scripture.is_none());
        let copies = s.duplicate(&[verse]).unwrap();
        assert_eq!(s.get(copies[0]).unwrap().scripture, entry.scripture.clone());
        let mut bad = snapshot("One");
        bad.verses.clear();
        assert_eq!(s.insert_scripture(0, bad, "x".into()), Err(Error::Invalid));

        let pins: Vec<Pin> = s.pins().collect();
        s.mark_saved("a.selaschedule".into(), pins.clone());
        assert!(!s.is_dirty());
        let items = vec![
            (pins[0], "John 3:16 (SYN)".into(), Some(snapshot("One"))),
            (pins[2], "Song".into(), None),
        ];
        s.open_items("b.selaschedule".into(), items).unwrap();
        assert_eq!(s.entries().len(), 2);
        assert_eq!(s.entries()[0].scripture.as_deref(), Some(&snapshot("One")));
        // A snapshot whose content does not match its pin is refused.
        let mismatched = vec![(pins[0], "x".into(), Some(snapshot("Other")))];
        assert_eq!(
            s.open_items("c.selaschedule".into(), mismatched),
            Err(Error::Invalid)
        );
        assert_eq!(s.entries().len(), 2, "a refused open keeps contents");
    }

    #[test]
    fn open_allocates_fresh_ids_and_validates() {
        let mut s = Schedule::default();
        let old = s.push(v(1, 1), "A".into()).unwrap();
        let too_many = (0..=MAX_ITEMS)
            .map(|n| (pin(1, 1), n.to_string()))
            .collect();
        assert_eq!(
            s.open(PathBuf::from("a.selaschedule"), too_many),
            Err(Error::Invalid)
        );
        assert_eq!(order(&s), [old], "failed open keeps contents");
        s.open(
            PathBuf::from("Evening.selaschedule"),
            vec![
                (pin(1, 1), "A".into()),
                (pin(1, 1), "A".into()),
                (pin(2, 3), "B".into()),
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
