//! "Update items in Schedule": entries pin immutable song revisions, so a
//! library edit never reaches the schedule by itself. This scans the
//! schedule for entries whose song has a newer revision, shows the changed
//! slides, and re-pins entries only when the operator confirms. Live is never
//! touched: the audience keeps its scene until the next Go Live.
use super::*;

/// A slide that differs between two revisions of a song.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlideChange {
    /// 1-based slide number.
    pub(crate) number: usize,
    /// `None`: the slide did not exist in that revision.
    pub(crate) old: Option<String>,
    pub(crate) new: Option<String>,
}

/// One schedule entry that would move to a newer revision.
#[derive(Clone, Debug)]
pub(crate) struct ItemUpdate {
    pub(crate) entry: EntryId,
    pub(crate) title: String,
    pub(crate) from: Version,
    pub(crate) to: Version,
    pub(crate) changes: Vec<SlideChange>,
}

/// The reviewable result of a scan, kept while its dialog is open.
#[derive(Clone, Debug, Default)]
pub(crate) struct UpdateReview {
    pub(crate) updates: Vec<ItemUpdate>,
}

fn shown(slide: &Slide) -> String {
    let text = slide.text.split_whitespace().collect::<Vec<_>>().join(" ");
    match slide.label.trim() {
        "" => text,
        label => format!("{label} · {text}"),
    }
}

/// Slides whose label or text differ, by position.
pub(crate) fn slide_changes(old: &storage::Song, new: &storage::Song) -> Vec<SlideChange> {
    let (a, b) = (slides::slides(old), slides::slides(new));
    (0..a.len().max(b.len()))
        .filter_map(|index| {
            let (o, n) = (a.get(index), b.get(index));
            let same = match (o, n) {
                (Some(o), Some(n)) => (&o.label, &o.text) == (&n.label, &n.text),
                (None, None) => true,
                _ => false,
            };
            (!same).then(|| SlideChange {
                number: index + 1,
                old: o.map(shown),
                new: n.map(shown),
            })
        })
        .collect()
}

/// Entries whose song has a newer head in `catalog`: (entry, pinned, head).
pub(crate) fn outdated(
    schedule: &Schedule,
    catalog: &[(Version, String)],
) -> Vec<(EntryId, Version, Version)> {
    schedule
        .entries()
        .iter()
        .filter_map(|entry| {
            let (head, _) = catalog
                .iter()
                .find(|(v, _)| v.id == entry.version.id && v.revision > entry.version.revision)?;
            Some((entry.id, entry.version, *head))
        })
        .collect()
}

impl Operator {
    /// Songs menu → Update items in Schedule. Waits for a pending catalog
    /// refresh first, so a song saved a moment ago is already a head.
    pub(crate) fn begin_update_scan(&mut self) {
        if self.catalog_stale || matches!(self.request, Some(Request::Catalog)) {
            self.view.scan_wanted = true;
            return;
        }
        let pairs = outdated(&self.schedule, &self.catalog);
        if pairs.is_empty() {
            self.schedule_message = Some(t!(Msg::ScheduleUpToDate).into());
            return;
        }
        let mut versions = Vec::new();
        for (_, from, to) in &pairs {
            for version in [*from, *to] {
                if !versions.contains(&version) {
                    versions.push(version);
                }
            }
        }
        self.queue(
            storage::Command::Songs(versions.clone()),
            Request::UpdateScan { pairs, versions },
        );
    }

    /// The storage reply: builds the review and shows it, or reports that
    /// nothing differs.
    pub(crate) fn finish_update_scan(
        &mut self,
        pairs: Vec<(EntryId, Version, Version)>,
        versions: Vec<Version>,
        songs: Vec<storage::Song>,
    ) {
        let song = |version: Version| {
            versions
                .iter()
                .position(|v| *v == version)
                .and_then(|i| songs.get(i))
        };
        let updates: Vec<ItemUpdate> = pairs
            .into_iter()
            .filter_map(|(entry, from, to)| {
                let (old, new) = (song(from)?, song(to)?);
                (old != new).then(|| ItemUpdate {
                    entry,
                    title: new.title.clone(),
                    from,
                    to,
                    changes: slide_changes(old, new),
                })
            })
            .collect();
        if updates.is_empty() {
            self.schedule_message = Some(t!(Msg::ScheduleUpToDate).into());
            return;
        }
        self.view.review = Some(UpdateReview { updates });
        if !self.show_dialog_later(Dialog::UpdateItems) {
            self.view.review = None;
        }
    }

    /// Confirm: re-pins every listed entry that is still on the revision the
    /// review was built from. Only the schedule and Preview change.
    pub(crate) fn apply_update_review(&mut self) {
        let Some(review) = self.view.review.take() else {
            return;
        };
        let mut count = 0;
        for update in review.updates {
            let current = self.schedule.get(update.entry).map(|e| e.version);
            if current == Some(update.from)
                && self
                    .schedule
                    .set_version(update.entry, update.to, update.title)
            {
                count += 1;
                if self.selected_entry == Some(update.entry) {
                    self.selected_song = Some(update.to);
                }
            }
        }
        self.schedule_message = Some(t!(Msg::ScheduleItemsUpdated, count = count));
    }

    pub(crate) fn discard_update_review(&mut self) {
        self.view.review = None;
    }

    pub(crate) fn update_items_body(&self) -> gpui::AnyElement {
        let review = self.view.review.clone().unwrap_or_default();
        let caption = |text: String| {
            div()
                .text_size(px(TEXT_CAPTION))
                .text_color(rgb(MUTED))
                .child(text)
        };
        let mut list = div()
            .id("update-items-list")
            .debug_selector(|| "update-items-list".into())
            .role(Role::List)
            .aria_label(t!(Msg::MenuUpdateSchedule))
            .accessibility_id("update-items-list")
            .max_h(px(280.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (index, update) in review.updates.iter().enumerate() {
            let mut group = div()
                .id(("update-item", index))
                .debug_selector(move || format!("update-item-{index}"))
                .role(Role::ListItem)
                .aria_label(shown_title(&update.title))
                .accessibility_id(format!("update-item-{index}"))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(shown_title(&update.title)),
                );
            if update.changes.is_empty() {
                group = group.child(caption(t!(Msg::UpdateFormatOnly).into()));
            }
            for change in &update.changes {
                let none = || t!(Msg::UpdateNoSlide).to_string();
                group = group.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(caption(t!(Msg::UpdateSlideNumber, number = change.number)))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .text_size(px(TEXT_SMALL))
                                .child(
                                    div()
                                        .w(px(36.))
                                        .flex_shrink_0()
                                        .text_color(rgb(MUTED))
                                        .child(t!(Msg::UpdateOld)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(rgb(MUTED))
                                        .child(change.old.clone().unwrap_or_else(none)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .text_size(px(TEXT_SMALL))
                                .child(
                                    div()
                                        .w(px(36.))
                                        .flex_shrink_0()
                                        .text_color(rgb(MUTED))
                                        .child(t!(Msg::UpdateNew)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .child(change.new.clone().unwrap_or_else(none)),
                                ),
                        ),
                );
            }
            list = list.child(group);
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(caption(t!(Msg::UpdateItemsIntro).into()))
            .child(list)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::storage::{Id, Section};

    fn song(lyrics: &[&str]) -> storage::Song {
        storage::Song {
            title: "T".into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            administrator: String::new(),
            song_number: String::new(),
            sections: lyrics
                .iter()
                .map(|text| Section {
                    id: sela::arrangement::SectionId::allocate(),
                    label: String::new(),
                    lyrics: (*text).into(),
                    format: Default::default(),
                    background: None,
                })
                .collect(),
            variants: Vec::new(),
            master: None,
        }
    }

    fn v(n: u8, revision: i64) -> Version {
        Version {
            id: Id([n; 16]),
            revision,
        }
    }

    #[test]
    fn only_changed_slides_are_listed() {
        let old = song(&["one", "two", "three"]);
        let mut new = old.clone();
        new.sections[1].lyrics = "TWO".into();
        new.sections.pop();
        let changes = slide_changes(&old, &new);
        assert_eq!(
            changes,
            [
                SlideChange {
                    number: 2,
                    old: Some("two".into()),
                    new: Some("TWO".into())
                },
                SlideChange {
                    number: 3,
                    old: Some("three".into()),
                    new: None
                },
            ]
        );
        assert!(slide_changes(&old, &old).is_empty());
    }

    #[test]
    fn outdated_lists_entries_with_a_newer_head_only() {
        let mut schedule = Schedule::default();
        let a = schedule.push(v(1, 1), "A".into()).unwrap();
        let a2 = schedule.push(v(1, 1), "A".into()).unwrap();
        schedule.push(v(2, 4), "B".into()).unwrap();
        schedule.push(v(3, 1), "Gone".into()).unwrap();
        let catalog = vec![(v(1, 3), "A".to_owned()), (v(2, 4), "B".to_owned())];
        assert_eq!(
            outdated(&schedule, &catalog),
            [(a, v(1, 1), v(1, 3)), (a2, v(1, 1), v(1, 3))]
        );
    }
}
