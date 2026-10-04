//! Bounded, immutable arrangement drafts. No persisted legacy-index conversion.
use crate::storage::{Id, Version};
use std::sync::Arc;

pub const MAX_SECTIONS: usize = 128;
pub const MAX_VARIANTS: usize = 16;
pub const MAX_OCCURRENCES: usize = 512;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_NAME_BYTES: usize = 256;
/// Structural payload budget (UTF-8 plus IDs/count prefixes), not an RSS limit.
pub const MAX_ARRANGEMENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionId(pub Id);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccurrenceId(pub Id);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariantId(pub Id);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub label: String,
    pub lyrics: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub id: OccurrenceId,
    pub section: SectionId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub id: VariantId,
    pub name: String,
    pub occurrences: Vec<Occurrence>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Limit,
    InvalidName,
    InvalidVersion,
    DuplicateId,
    DuplicateName,
    MissingSection,
    MissingVariant,
    MissingOccurrence,
    InvalidPosition,
}
type Result<T> = std::result::Result<T, Error>;

/// Owns exact content of one positive immutable song revision; never reads heads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSnapshot {
    version: Version,
    sections: Vec<Section>,
}
impl SourceSnapshot {
    pub fn new(version: Version, sections: Vec<Section>) -> Result<Self> {
        if version.revision <= 0 {
            return Err(Error::InvalidVersion);
        }
        if sections.len() > MAX_SECTIONS {
            return Err(Error::Limit);
        }
        let mut bytes = 0usize;
        for (index, section) in sections.iter().enumerate() {
            if sections[..index].iter().any(|s| s.id == section.id) {
                return Err(Error::DuplicateId);
            }
            if section.label.len() > MAX_NAME_BYTES {
                return Err(Error::Limit);
            }
            bytes = bytes
                .checked_add(section.label.len())
                .and_then(|n| n.checked_add(section.lyrics.len()))
                .and_then(|n| n.checked_add(24))
                .ok_or(Error::Limit)?;
            if bytes > MAX_TEXT_BYTES {
                return Err(Error::Limit);
            }
        }
        Ok(Self { version, sections })
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
}

/// Pure edits return a new validated draft; errors leave the original untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrangement {
    source: Arc<SourceSnapshot>,
    variants: Vec<Variant>,
}
pub enum Edit {
    Rename(String),
    Insert {
        position: usize,
        occurrence: Occurrence,
    },
    Remove(OccurrenceId),
    /// Destination is the final zero-based position after removal.
    Move {
        occurrence: OccurrenceId,
        position: usize,
    },
    Retarget {
        occurrence: OccurrenceId,
        section: SectionId,
    },
}
#[derive(Debug)]
pub struct ResolvedOccurrence<'a> {
    pub id: OccurrenceId,
    pub section: &'a Section,
}
impl Arrangement {
    pub fn new(source: SourceSnapshot, variants: Vec<Variant>) -> Result<Self> {
        let draft = Self {
            source: Arc::new(source),
            variants,
        };
        draft.validate()?;
        Ok(draft)
    }
    pub fn source(&self) -> &SourceSnapshot {
        &self.source
    }
    pub fn variants(&self) -> &[Variant] {
        &self.variants
    }
    fn validate(&self) -> Result<()> {
        if self.variants.len() > MAX_VARIANTS {
            return Err(Error::Limit);
        }
        let mut bytes = 4usize;
        for (index, variant) in self.variants.iter().enumerate() {
            if variant.name.trim().is_empty() {
                return Err(Error::InvalidName);
            }
            if variant.name.len() > MAX_NAME_BYTES || variant.occurrences.len() > MAX_OCCURRENCES {
                return Err(Error::Limit);
            }
            if self.variants[..index].iter().any(|v| v.id == variant.id) {
                return Err(Error::DuplicateId);
            }
            // Exact UTF-8 equality; no lossy case folding or normalization.
            if self.variants[..index]
                .iter()
                .any(|v| v.name == variant.name)
            {
                return Err(Error::DuplicateName);
            }
            bytes += 24 + variant.name.len() + 32 * variant.occurrences.len();
            if bytes > MAX_ARRANGEMENT_BYTES {
                return Err(Error::Limit);
            }
            for (position, occurrence) in variant.occurrences.iter().enumerate() {
                // Occurrence identity is scoped to a variant, never to its position.
                if variant.occurrences[..position]
                    .iter()
                    .any(|o| o.id == occurrence.id)
                {
                    return Err(Error::DuplicateId);
                }
                if !self
                    .source
                    .sections
                    .iter()
                    .any(|s| s.id == occurrence.section)
                {
                    return Err(Error::MissingSection);
                }
            }
        }
        Ok(())
    }
    /// Add or replace by explicit variant ID, preserving other variants.
    pub fn with_variant(&self, variant: Variant) -> Result<Self> {
        let mut next = self.clone();
        if let Some(v) = next.variants.iter_mut().find(|v| v.id == variant.id) {
            *v = variant;
        } else {
            next.variants.push(variant);
        }
        next.validate()?;
        Ok(next)
    }
    pub fn without_variant(&self, id: VariantId) -> Result<Self> {
        let index = self
            .variants
            .iter()
            .position(|v| v.id == id)
            .ok_or(Error::MissingVariant)?;
        let mut next = self.clone();
        next.variants.remove(index);
        Ok(next)
    }
    pub fn edit(&self, id: VariantId, edit: Edit) -> Result<Self> {
        let mut variant = self
            .variants
            .iter()
            .find(|v| v.id == id)
            .ok_or(Error::MissingVariant)?
            .clone();
        let position_of = |id| {
            variant
                .occurrences
                .iter()
                .position(|o| o.id == id)
                .ok_or(Error::MissingOccurrence)
        };
        match edit {
            Edit::Rename(name) => variant.name = name,
            Edit::Insert {
                position,
                occurrence,
            } => {
                if position > variant.occurrences.len() {
                    return Err(Error::InvalidPosition);
                }
                variant.occurrences.insert(position, occurrence);
            }
            Edit::Remove(id) => {
                let index = position_of(id)?;
                variant.occurrences.remove(index);
            }
            Edit::Move {
                occurrence,
                position,
            } => {
                let index = position_of(occurrence)?;
                if position >= variant.occurrences.len() {
                    return Err(Error::InvalidPosition);
                }
                let occurrence = variant.occurrences.remove(index);
                variant.occurrences.insert(position, occurrence);
            }
            Edit::Retarget {
                occurrence,
                section,
            } => {
                let index = position_of(occurrence)?;
                variant.occurrences[index].section = section;
            }
        }
        self.with_variant(variant)
    }
    /// All-or-error resolution against the owned revision, never a current library.
    pub fn resolve(&self, id: VariantId) -> Result<Vec<ResolvedOccurrence<'_>>> {
        let variant = self
            .variants
            .iter()
            .find(|v| v.id == id)
            .ok_or(Error::MissingVariant)?;
        variant
            .occurrences
            .iter()
            .map(|o| {
                let section = self
                    .source
                    .sections
                    .iter()
                    .find(|s| s.id == o.section)
                    .ok_or(Error::MissingSection)?;
                Ok(ResolvedOccurrence { id: o.id, section })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: usize) -> Id {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&(n as u64).to_le_bytes());
        Id(bytes)
    }
    fn sid(n: usize) -> SectionId {
        SectionId(id(n))
    }
    fn oid(n: usize) -> OccurrenceId {
        OccurrenceId(id(n))
    }
    fn vid(n: usize) -> VariantId {
        VariantId(id(n))
    }
    fn version() -> Version {
        Version {
            id: id(99),
            revision: 7,
        }
    }
    fn source() -> SourceSnapshot {
        // Scrambled source order and duplicate labels defeat index/label lookup.
        SourceSnapshot::new(
            version(),
            [
                (2, "C", "Chorus سلام\r\n"),
                (3, "Verse", "V2 a\u{301}"),
                (1, "Verse", "V1 Café"),
            ]
            .map(|(n, label, lyrics)| Section {
                id: sid(n),
                label: label.into(),
                lyrics: lyrics.into(),
            })
            .to_vec(),
        )
        .unwrap()
    }
    fn variant(n: usize, sections: &[usize]) -> Variant {
        Variant {
            id: vid(n),
            name: format!("Variant {n}"),
            occurrences: sections
                .iter()
                .enumerate()
                .map(|(i, s)| Occurrence {
                    id: oid(i + 10),
                    section: sid(*s),
                })
                .collect(),
        }
    }
    fn draft() -> Arrangement {
        Arrangement::new(
            source(),
            vec![variant(1, &[1, 2, 3, 2, 2]), variant(2, &[3, 1])],
        )
        .unwrap()
    }
    #[test]
    fn asymmetric_resolution_and_immutable_snapshot() {
        let a = draft();
        let resolved = a.resolve(vid(1)).unwrap();
        assert_eq!(
            resolved
                .iter()
                .map(|o| o.section.lyrics.as_str())
                .collect::<Vec<_>>(),
            [
                "V1 Café",
                "Chorus سلام\r\n",
                "V2 a\u{301}",
                "Chorus سلام\r\n",
                "Chorus سلام\r\n"
            ]
        );
        assert!(std::ptr::eq(resolved[1].section, resolved[3].section));
        assert_ne!(resolved[1].id, resolved[3].id);
        let mut latest = source().sections().to_vec();
        latest[0].lyrics = "changed".into();
        latest.remove(2);
        let latest = SourceSnapshot::new(
            Version {
                revision: 8,
                ..version()
            },
            latest,
        )
        .unwrap();
        assert_eq!(
            Arrangement::new(latest, a.variants().to_vec()),
            Err(Error::MissingSection)
        );
        assert_eq!(a.source().version(), version());
        assert_eq!(a.resolve(vid(1)).unwrap()[0].section.lyrics, "V1 Café");
    }
    #[test]
    fn pure_edits_preserve_identity_and_variants() {
        let a = draft();
        let b = a
            .edit(
                vid(1),
                Edit::Move {
                    occurrence: oid(13),
                    position: 0,
                },
            )
            .unwrap();
        assert_eq!(
            b.variants()[0]
                .occurrences
                .iter()
                .map(|o| o.id)
                .collect::<Vec<_>>(),
            [oid(13), oid(10), oid(11), oid(12), oid(14)]
        );
        assert_eq!(b.variants()[1], a.variants()[1]);
        let b = b
            .edit(
                vid(1),
                Edit::Move {
                    occurrence: oid(13),
                    position: 4,
                },
            )
            .unwrap();
        assert_eq!(b.variants()[0].occurrences[4].id, oid(13));
        let b = b
            .edit(
                vid(1),
                Edit::Retarget {
                    occurrence: oid(13),
                    section: sid(3),
                },
            )
            .unwrap();
        assert_eq!(b.resolve(vid(1)).unwrap()[4].section.id, sid(3));
        let b = b.edit(vid(1), Edit::Remove(oid(11))).unwrap();
        assert_eq!(b.resolve(vid(1)).unwrap().len(), 4);
        let b = b
            .edit(
                vid(1),
                Edit::Insert {
                    position: 4,
                    occurrence: Occurrence {
                        id: oid(30),
                        section: sid(2),
                    },
                },
            )
            .unwrap();
        assert_eq!(b.resolve(vid(1)).unwrap()[4].id, oid(30));
        assert_eq!(a.resolve(vid(1)).unwrap()[1].id, oid(11));
        assert_eq!(
            b.without_variant(vid(1)).unwrap().variants(),
            &a.variants()[1..]
        );
    }
    #[test]
    fn invalid_edits_are_atomic() {
        let a = draft();
        let before = a.clone();
        let cases = [
            (Edit::Rename(" \t".into()), Error::InvalidName),
            (
                Edit::Rename(a.variants()[1].name.clone()),
                Error::DuplicateName,
            ),
            (Edit::Remove(oid(90)), Error::MissingOccurrence),
            (
                Edit::Move {
                    occurrence: oid(10),
                    position: 5,
                },
                Error::InvalidPosition,
            ),
            (
                Edit::Retarget {
                    occurrence: oid(10),
                    section: sid(90),
                },
                Error::MissingSection,
            ),
            (
                Edit::Insert {
                    position: 6,
                    occurrence: Occurrence {
                        id: oid(80),
                        section: sid(1),
                    },
                },
                Error::InvalidPosition,
            ),
            (
                Edit::Insert {
                    position: 0,
                    occurrence: a.variants()[0].occurrences[0],
                },
                Error::DuplicateId,
            ),
            (
                Edit::Insert {
                    position: 0,
                    occurrence: Occurrence {
                        id: oid(80),
                        section: sid(90),
                    },
                },
                Error::MissingSection,
            ),
        ];
        for (edit, error) in cases {
            assert_eq!(a.edit(vid(1), edit), Err(error));
            assert_eq!(a, before);
        }
        assert_eq!(
            a.edit(vid(90), Edit::Rename("other".into())),
            Err(Error::MissingVariant)
        );
        assert_eq!(a.without_variant(vid(90)), Err(Error::MissingVariant));
        assert_eq!(a.resolve(vid(90)).unwrap_err(), Error::MissingVariant);
        let mut corrupt = a.clone(); // Internal corruption defense: no partial resolution.
        corrupt.variants[0].occurrences[4].section = sid(90);
        assert_eq!(corrupt.resolve(vid(1)).unwrap_err(), Error::MissingSection);
    }
    #[test]
    fn uniqueness_empty_and_unicode_policy() {
        let s = source();
        let mut sections = s.sections().to_vec();
        sections.push(sections[0].clone());
        assert_eq!(
            SourceSnapshot::new(version(), sections),
            Err(Error::DuplicateId)
        );
        assert_eq!(
            SourceSnapshot::new(
                Version {
                    revision: 0,
                    ..version()
                },
                vec![]
            ),
            Err(Error::InvalidVersion)
        );
        assert!(
            Arrangement::new(SourceSnapshot::new(version(), vec![]).unwrap(), vec![])
                .unwrap()
                .variants()
                .is_empty()
        );
        let a = Arrangement::new(s.clone(), vec![variant(1, &[])]).unwrap();
        assert!(a.resolve(vid(1)).unwrap().is_empty());
        assert!(a.without_variant(vid(1)).unwrap().variants().is_empty());
        assert_eq!(
            Arrangement::new(s.clone(), vec![variant(1, &[]), variant(1, &[])]),
            Err(Error::DuplicateId)
        );
        let a = a.edit(vid(1), Edit::Rename("é".repeat(128))).unwrap();
        assert_eq!(
            a.edit(vid(1), Edit::Rename("é".repeat(129))),
            Err(Error::Limit)
        );
        let mut v = variant(2, &[]);
        v.name = a.variants()[0].name.clone();
        assert_eq!(a.with_variant(v.clone()), Err(Error::DuplicateName));
        v.name = "e\u{301}".into();
        assert!(a.with_variant(v).is_ok());
        let empty = Section {
            id: sid(1),
            label: String::new(),
            lyrics: String::new(),
        };
        assert!(SourceSnapshot::new(version(), vec![empty]).is_ok());
    }
    #[test]
    fn count_and_byte_boundaries() {
        let section = |n| Section {
            id: sid(n),
            label: String::new(),
            lyrics: String::new(),
        };
        assert!(SourceSnapshot::new(version(), (0..MAX_SECTIONS).map(section).collect()).is_ok());
        assert_eq!(
            SourceSnapshot::new(version(), (0..=MAX_SECTIONS).map(section).collect()),
            Err(Error::Limit)
        );
        let mut s = section(1);
        s.label = "é".repeat(128);
        s.lyrics = "x".repeat(MAX_TEXT_BYTES - 24 - s.label.len());
        assert!(SourceSnapshot::new(version(), vec![s.clone()]).is_ok());
        s.lyrics.push('x');
        assert_eq!(SourceSnapshot::new(version(), vec![s]), Err(Error::Limit));
        let a = Arrangement::new(
            source(),
            (0..MAX_VARIANTS).map(|n| variant(n, &[])).collect(),
        )
        .unwrap();
        assert_eq!(a.with_variant(variant(99, &[])), Err(Error::Limit));
        let v = variant(1, &vec![1; MAX_OCCURRENCES]);
        let a = Arrangement::new(source(), vec![v.clone()]).unwrap();
        assert_eq!(
            a.edit(
                vid(1),
                Edit::Insert {
                    position: 0,
                    occurrence: Occurrence {
                        id: oid(900),
                        section: sid(1)
                    }
                }
            ),
            Err(Error::Limit)
        );
        let mut full = Vec::new();
        // 3 full variants + remaining capacity exactly meet the aggregate cap.
        for n in 0..3 {
            let mut v = v.clone();
            v.id = vid(n);
            v.name = "x".repeat(256);
            v.name.replace_range(..1, &n.to_string());
            full.push(v);
        }
        let used = 4 + 3 * (24 + 256 + 32 * MAX_OCCURRENCES);
        let remaining = MAX_ARRANGEMENT_BYTES - used;
        let count = (remaining - 24) / 32;
        let name_len = remaining - 24 - 32 * count;
        let mut last = variant(9, &vec![1; count]);
        last.name = "z".repeat(name_len);
        full.push(last);
        let a = Arrangement::new(source(), full).unwrap();
        assert_eq!(
            a.edit(vid(9), Edit::Rename("z".repeat(name_len + 1))),
            Err(Error::Limit)
        );
    }
}
