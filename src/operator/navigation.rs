//! Keyboard navigation of Live and Preview (EW8-OBS-014, 048-052): arrows and
//! Home/End move slides, PageDown/PageUp in the Live pane move between
//! schedule items, label shortcuts jump to a labelled slide. Slide steps stop
//! at the ends of an item (EW8-OBS-051).
use super::*;
use sela::labels;

impl Operator {
    /// Home/End: first or last slide of the live item.
    pub(crate) fn live_edge(&mut self, last: bool) {
        let Some(count) = self.live.as_ref().map(|item| item.slides.len()) else {
            self.live_message = Some(t!(Msg::LiveNothingLive).into());
            return;
        };
        let target = if last { count.checked_sub(1) } else { Some(0) };
        let Some(target) = target.filter(|_| count > 0) else {
            return;
        };
        if self.requested_slide() != Some(target) {
            self.send(target);
        }
    }

    /// PageDown/PageUp with the Live pane focused: the next or previous
    /// schedule item goes live at its first slide (EW8-OBS-050; the PageUp
    /// target is Sela's rule). The ends of the schedule stop.
    pub(crate) fn step_live_item(&mut self, forward: bool) {
        if self.live.is_none() {
            self.live_message = Some(t!(Msg::LiveNothingLive).into());
            return;
        }
        let from = self
            .live_entry_on_load
            .or_else(|| {
                self.live
                    .as_ref()
                    .and_then(|item| item.entry)
                    .filter(|id| self.schedule.get(*id).is_some())
            })
            .or(self.selected_entry);
        let Some(id) = from.and_then(|from| self.schedule.neighbor(Some(from), forward)) else {
            return;
        };
        self.select_entry(id);
        let Some(version) = self.schedule.get(id).map(|entry| entry.version) else {
            return;
        };
        if self
            .preview
            .as_ref()
            .is_some_and(|p| (p.version, p.entry) == (version, Some(id)))
        {
            self.preview_slide = 0;
            self.go_live();
        } else {
            self.live_entry_on_load = Some(id);
        }
    }

    /// A completed label shortcut acts on Live only; an unknown label, or
    /// nothing live, changes nothing (EW8-OBS-052 best guess).
    pub(crate) fn jump_label(&mut self, key: char, number: Option<u32>) {
        let Some(kind) = labels::Kind::from_key(key) else {
            return;
        };
        let Some(item) = &self.live else {
            self.live_message = Some(t!(Msg::LiveNothingLive).into());
            return;
        };
        if let Some(index) = labels::find(&item.slides, kind, number) {
            self.send(index);
        }
    }

    /// Up/Down with a Preview slide focused move the selection only
    /// (EW8-OBS-049); returns the slide now selected.
    pub(crate) fn step_preview(&mut self, forward: bool) -> Option<usize> {
        let count = self.preview.as_ref()?.slides.len();
        let target = if forward {
            self.preview_slide + 1
        } else {
            self.preview_slide.checked_sub(1)?
        };
        (target < count).then(|| {
            self.preview_slide = target;
            target
        })
    }

    /// Home/End with the Schedule focused select the first or last item.
    pub(crate) fn select_schedule_edge(&mut self, last: bool) {
        let entries = self.schedule.entries();
        let id = if last {
            entries.last()
        } else {
            entries.first()
        }
        .map(|entry| entry.id);
        if let Some(id) = id {
            self.select_entry(id);
        }
    }
}
