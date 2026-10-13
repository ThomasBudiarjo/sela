//! Operator Black/Clear/Logo state. No GPUI types and no rendering.
//!
//! Transitions follow EasyWorship 8.0.49 observations EW8-OBS-015–017
//! (docs/reference-observations.md): Black and Logo replace each other, Clear
//! stacks with either, a repeated press turns a mask off, and Go Live, slide
//! navigation and output off/on keep the masks. Live double-click clearing
//! every mask in a combined state is provisional (EW8-OBS-020).

/// Identifies one explicit operator cue request across the supervisor, the
/// wire stamp and its renderer receipt. Never reused and independent of
/// `ContentVersion`; internal retries of a request keep its id.
pub type Submission = u64;
/// Submissions that no operator intent can own.
pub const UNTAGGED: Submission = 0;

/// Where one submission stands with the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionState {
    /// Preparing, queued, in flight, superseded or lost: not settled.
    Waiting,
    /// A successful Applied receipt carried exactly this submission.
    Applied,
    /// A RenderFailed, Stale or other rejecting receipt carried it.
    Failed,
}

/// Supervisor-side ledger of scene submissions: at most one wanted and one
/// in flight (the renderer's cue lane), plus the newest terminal receipt by
/// outcome. A receipt only ever describes the submission that was in flight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Submissions {
    wanted: Option<Submission>,
    in_flight: Option<Submission>,
    applied: Option<Submission>,
    failed: Option<Submission>,
}

impl Submissions {
    /// A (re-)submission replaces the wanted one and forgets a receipt already
    /// recorded for it. An earlier presentation of the same request that is
    /// still in flight may still settle it; it shows the same slide.
    pub fn want(&mut self, submission: Submission) {
        if self.applied == Some(submission) {
            self.applied = None;
        }
        if self.failed == Some(submission) {
            self.failed = None;
        }
        self.wanted = Some(submission);
    }

    /// The wanted cue no longer fits the surface and is dropped unsent.
    pub fn drop_wanted(&mut self) {
        self.wanted = None;
    }

    /// The wanted cue goes on the wire.
    pub fn send(&mut self) {
        self.in_flight = self.wanted.take();
    }

    /// The terminal receipt for the cue in flight.
    pub fn receipt(&mut self, applied: bool) {
        let Some(submission) = self.in_flight.take() else {
            return;
        };
        if applied {
            self.applied = Some(submission);
        } else {
            self.failed = Some(submission);
        }
    }

    /// The session ended; nothing it carried can settle.
    pub fn lose(&mut self) {
        *self = Self::default();
    }

    pub fn state(&self, submission: Submission) -> SubmissionState {
        if self.wanted == Some(submission) || self.in_flight == Some(submission) {
            SubmissionState::Waiting
        } else if self.applied == Some(submission) {
            SubmissionState::Applied
        } else if self.failed == Some(submission) {
            SubmissionState::Failed
        } else {
            SubmissionState::Waiting
        }
    }
}

/// Double-click unmask intent, owned by exactly one submission. Only that
/// submission's successful Applied receipt lifts the masks; anything else the
/// operator does cancels it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnmaskIntent(Option<Submission>);

impl UnmaskIntent {
    pub fn armed(&self) -> Option<Submission> {
        self.0
    }

    pub fn arm(&mut self, submission: Submission) {
        self.0 = (submission != UNTAGGED).then_some(submission);
    }

    pub fn cancel(&mut self) {
        self.0 = None;
    }

    /// The submission died before it could apply (refused, failed to
    /// prepare, substituted).
    pub fn drop_submission(&mut self, submission: Submission) {
        if self.0 == Some(submission) {
            self.0 = None;
        }
    }

    /// Keeps the intent only if it belongs to `submission`.
    pub fn retain_only(&mut self, submission: Submission) {
        if self.0.is_some_and(|s| s != submission) {
            self.0 = None;
        }
    }

    /// Settles the intent against its submission's state; true when the
    /// masks must lift now.
    pub fn settle(&mut self, state: SubmissionState) -> bool {
        match (self.0, state) {
            (Some(_), SubmissionState::Applied) => {
                self.0 = None;
                true
            }
            (Some(_), SubmissionState::Failed) => {
                self.0 = None;
                false
            }
            _ => false,
        }
    }
}

/// What the audience shows over the applied slide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layer {
    /// The applied slide, text and background.
    #[default]
    None,
    /// The applied slide's background without its text.
    Clear,
    Black,
    /// The prepared logo image instead of the slide.
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mask {
    Black,
    Clear,
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Cover {
    Black,
    Logo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Masks {
    cover: Option<Cover>,
    clear: bool,
}

impl Masks {
    pub fn is_on(&self, mask: Mask) -> bool {
        match mask {
            Mask::Black => self.cover == Some(Cover::Black),
            Mask::Logo => self.cover == Some(Cover::Logo),
            Mask::Clear => self.clear,
        }
    }

    pub fn any(&self) -> bool {
        self.cover.is_some() || self.clear
    }

    pub fn toggle(&mut self, mask: Mask) {
        match mask {
            Mask::Clear => self.clear = !self.clear,
            Mask::Black => self.toggle_cover(Cover::Black),
            Mask::Logo => self.toggle_cover(Cover::Logo),
        }
    }

    fn toggle_cover(&mut self, cover: Cover) {
        self.cover = if self.cover == Some(cover) {
            None
        } else {
            Some(cover)
        };
    }

    /// Double-click on a Live slide turns the masks off and applies that slide.
    pub fn live_double_click(&mut self) {
        *self = Self::default();
    }

    /// A cover hides a stacked Clear; turning the cover off reveals it again.
    pub fn layer(&self) -> Layer {
        match (self.cover, self.clear) {
            (Some(Cover::Black), _) => Layer::Black,
            (Some(Cover::Logo), _) => Layer::Logo,
            (None, true) => Layer::Clear,
            (None, false) => Layer::None,
        }
    }
}

#[cfg(test)]
mod model;

#[cfg(test)]
mod tests {
    use super::*;

    fn after(presses: &[Mask]) -> Masks {
        let mut masks = Masks::default();
        for mask in presses {
            masks.toggle(*mask);
        }
        masks
    }

    fn lit(masks: Masks) -> Vec<Mask> {
        [Mask::Black, Mask::Clear, Mask::Logo]
            .into_iter()
            .filter(|m| masks.is_on(*m))
            .collect()
    }

    #[test]
    fn starts_unmasked() {
        let masks = Masks::default();
        assert_eq!(masks.layer(), Layer::None);
        assert!(!masks.any());
    }

    #[test]
    fn repeated_press_toggles_each_mask() {
        for (mask, layer) in [
            (Mask::Black, Layer::Black),
            (Mask::Clear, Layer::Clear),
            (Mask::Logo, Layer::Logo),
        ] {
            let mut masks = Masks::default();
            for step in 0..3 {
                masks.toggle(mask);
                let on = step % 2 == 0;
                assert_eq!(masks.is_on(mask), on);
                assert_eq!(masks.layer(), if on { layer } else { Layer::None });
            }
        }
    }

    /// Ordered pairs M, N, N, M from RUN-W03-2026-10-06.
    #[test]
    fn ordered_pairs_match_the_observed_table() {
        use Layer as L;
        use Mask::*;
        let cases = [
            (Black, Clear, [L::Black, L::Black, L::Black, L::None]),
            (Black, Logo, [L::Black, L::Logo, L::None, L::Black]),
            (Clear, Black, [L::Clear, L::Black, L::Clear, L::None]),
            (Clear, Logo, [L::Clear, L::Logo, L::Clear, L::None]),
            (Logo, Black, [L::Logo, L::Black, L::None, L::Logo]),
            (Logo, Clear, [L::Logo, L::Logo, L::Logo, L::None]),
        ];
        for (first, second, layers) in cases {
            let mut masks = Masks::default();
            for (press, expected) in [first, second, second, first].into_iter().zip(layers) {
                masks.toggle(press);
                assert_eq!(masks.layer(), expected, "{first:?} {second:?} {press:?}");
            }
        }
    }

    #[test]
    fn black_and_logo_replace_each_other_and_clear_stacks() {
        assert_eq!(lit(after(&[Mask::Black, Mask::Logo])), [Mask::Logo]);
        assert_eq!(lit(after(&[Mask::Logo, Mask::Black])), [Mask::Black]);
        assert_eq!(
            lit(after(&[Mask::Clear, Mask::Black])),
            [Mask::Black, Mask::Clear]
        );
        assert_eq!(
            lit(after(&[Mask::Logo, Mask::Clear])),
            [Mask::Clear, Mask::Logo]
        );
    }

    /// Every reachable state, from the six ordered triples and their reversal.
    #[test]
    fn black_with_logo_is_unreachable() {
        use Mask::*;
        let orders = [
            [Black, Clear, Logo],
            [Black, Logo, Clear],
            [Clear, Black, Logo],
            [Clear, Logo, Black],
            [Logo, Black, Clear],
            [Logo, Clear, Black],
        ];
        let mut seen = std::collections::HashSet::new();
        for order in orders {
            let mut masks = Masks::default();
            for press in order.iter().chain(order.iter().rev()) {
                masks.toggle(*press);
                assert!(!(masks.is_on(Black) && masks.is_on(Logo)));
                seen.insert(lit(masks));
            }
        }
        assert_eq!(seen.len(), 6, "{seen:?}");
    }

    /// Triple Clear, Black, Logo then reverse, as recorded.
    #[test]
    fn observed_triple_sequence() {
        use Mask::*;
        let mut masks = Masks::default();
        let layers: Vec<_> = [Clear, Black, Logo, Logo, Black, Clear]
            .into_iter()
            .map(|press| {
                masks.toggle(press);
                masks.layer()
            })
            .collect();
        assert_eq!(
            layers,
            [
                Layer::Clear,
                Layer::Black,
                Layer::Logo,
                Layer::Clear,
                Layer::Black,
                Layer::Black
            ]
        );
    }

    #[test]
    fn live_double_click_turns_every_mask_off() {
        let mut masks = after(&[Mask::Clear, Mask::Logo]);
        masks.live_double_click();
        assert_eq!(masks, Masks::default());
        assert_eq!(masks.layer(), Layer::None);
    }
}
