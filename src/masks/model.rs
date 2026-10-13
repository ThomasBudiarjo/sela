//! Model-based check of the operator's mask/cue state machine without GPUI.
//!
//! The model mirrors `operator::preview_live` (requests, deferral, preparation
//! outcomes, refresh, output toggles) and drives the production `Masks`,
//! `UnmaskIntent` and `Submissions` types through random and exhaustive
//! interleavings with renderer receipts that arrive in any order. It asserts
//! mask safety: masks only lift on the operator's toggle or on the successful
//! Applied receipt of the exact submission that is the latest explicit
//! request, and the audience then shows that request's slide.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
enum Prep {
    Ok,
    Fail,
    Substitute,
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Click(u8),
    DoubleClick(u8),
    Next,
    Prev,
    GoLiveNew,
    Toggle(Mask),
    Output,
    Refresh,
    DeferNext,
    FontsLand,
    Prepared(Prep),
    Applied,
    RenderFailed,
}

const SLIDES: u8 = 3;

const ALPHABET: [Step; 21] = [
    Step::Click(0),
    Step::Click(1),
    Step::DoubleClick(0),
    Step::DoubleClick(1),
    Step::Next,
    Step::Prev,
    Step::GoLiveNew,
    Step::Toggle(Mask::Black),
    Step::Toggle(Mask::Clear),
    Step::Toggle(Mask::Logo),
    Step::Output,
    Step::Refresh,
    Step::DeferNext,
    Step::FontsLand,
    Step::Prepared(Prep::Ok),
    Step::Prepared(Prep::Fail),
    Step::Prepared(Prep::Substitute),
    Step::Applied,
    Step::RenderFailed,
    Step::Applied,
    Step::Prepared(Prep::Ok),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Req {
    item: u8,
    idx: u8,
    sub: u64,
}

/// How the unmask intent is satisfied: the production rule, or the rejected
/// version-matching rule used as a negative control for the model itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Satisfy {
    Submission,
    Version,
}

struct Model {
    satisfy: Satisfy,
    masks: Masks,
    unmask: UnmaskIntent,
    armed_req: Option<Req>,
    ledger: Submissions,
    sub: u64,
    output: bool,
    item: u8,
    live_slide: Option<u8>,
    deferred: Option<Req>,
    prep: VecDeque<Req>,
    defer_next: bool,
    wire_wanted: Option<Req>,
    in_flight: Option<Req>,
    shown: Option<Req>,
    presented: Option<(u64, u8, bool)>,
    last_explicit: Option<Req>,
    lifted_for: Option<Req>,
}

impl Model {
    /// Output on, slide 0 applied for request 1, Black on.
    fn new(satisfy: Satisfy) -> Self {
        let first = Req {
            item: 0,
            idx: 0,
            sub: 1,
        };
        let mut ledger = Submissions::default();
        ledger.want(1);
        ledger.send();
        ledger.receipt(true);
        let mut masks = Masks::default();
        masks.toggle(Mask::Black);
        Self {
            satisfy,
            masks,
            unmask: UnmaskIntent::default(),
            armed_req: None,
            ledger,
            sub: 1,
            output: true,
            item: 0,
            live_slide: Some(0),
            deferred: None,
            prep: VecDeque::new(),
            defer_next: false,
            wire_wanted: None,
            in_flight: None,
            shown: Some(first),
            presented: Some((1, 0, true)),
            last_explicit: Some(first),
            lifted_for: None,
        }
    }

    fn requested_slide(&self) -> Option<u8> {
        self.prep.back().map(|r| r.idx).or(self.live_slide)
    }

    fn send(&mut self, idx: u8) {
        self.sub += 1;
        self.unmask.cancel();
        self.lifted_for = None;
        let req = Req {
            item: self.item,
            idx,
            sub: self.sub,
        };
        self.last_explicit = Some(req);
        self.dispatch(req, true);
    }

    fn dispatch(&mut self, req: Req, may_defer: bool) {
        if !self.output {
            return;
        }
        self.deferred = None;
        if may_defer && self.defer_next {
            self.defer_next = false;
            self.deferred = Some(req);
        } else {
            self.prep.push_back(req);
        }
    }

    fn cancel_cues(&mut self) {
        self.prep.clear();
        self.deferred = None;
    }

    fn pump(&mut self) {
        if self.in_flight.is_none()
            && let Some(req) = self.wire_wanted.take()
        {
            self.ledger.send();
            self.in_flight = Some(req);
        }
    }

    fn click(&mut self, idx: u8, double: bool) {
        let latest = self.deferred.map(|d| d.idx).or(self.requested_slide());
        if double && latest == Some(idx) {
            self.unmask.cancel();
        } else {
            self.send(idx);
        }
        if double && self.masks.any() {
            let sub = self.sub;
            let waiting = self
                .prep
                .back()
                .is_some_and(|p| p.sub == sub && p.idx == idx)
                || self.deferred.is_some_and(|d| d.sub == sub && d.idx == idx);
            if waiting || self.presented == Some((sub, idx, true)) {
                self.unmask.arm(sub);
                self.armed_req = Some(Req {
                    item: self.item,
                    idx,
                    sub,
                });
            }
        }
    }

    fn apply(&mut self, step: Step) -> Result<(), String> {
        let masked = self.masks.any();
        let mut toggled = false;
        match step {
            Step::Click(i) => self.click(i, false),
            Step::DoubleClick(i) => {
                // A double-click delivers its first click as a plain click.
                self.click(i, false);
                self.click(i, true);
            }
            Step::Next | Step::Prev => {
                if let Some(idx) = self.requested_slide() {
                    let target = if matches!(step, Step::Next) {
                        idx + 1
                    } else {
                        idx.wrapping_sub(1)
                    };
                    if target < SLIDES {
                        self.unmask.cancel();
                        self.send(target);
                    }
                }
            }
            Step::GoLiveNew => {
                if self.output {
                    self.cancel_cues();
                    self.unmask.cancel();
                    self.item += 1;
                    self.send(0);
                }
            }
            Step::Toggle(mask) => {
                self.masks.toggle(mask);
                self.unmask.cancel();
                toggled = true;
            }
            Step::Output => {
                self.cancel_cues();
                self.output = !self.output;
                self.unmask.cancel();
                self.live_slide = None;
                self.wire_wanted = None;
                self.in_flight = None;
                self.shown = None;
                self.ledger.lose();
                self.lifted_for = None;
            }
            Step::Refresh => {
                let latest = self.deferred.or_else(|| {
                    self.prep.back().copied().or(self.live_slide.map(|idx| Req {
                        item: self.item,
                        idx,
                        sub: self.sub,
                    }))
                });
                if let Some(req) = latest {
                    self.cancel_cues();
                    self.dispatch(req, true);
                    self.unmask.retain_only(req.sub);
                }
            }
            Step::DeferNext => self.defer_next = true,
            Step::FontsLand => {
                if let Some(req) = self.deferred.take() {
                    self.dispatch(req, false);
                }
            }
            Step::Prepared(outcome) => {
                if let Some(req) = self.prep.pop_front() {
                    match outcome {
                        Prep::Fail => self.unmask.drop_submission(req.sub),
                        Prep::Ok | Prep::Substitute if !self.output => {
                            self.unmask.drop_submission(req.sub)
                        }
                        Prep::Ok | Prep::Substitute => {
                            let clean = matches!(outcome, Prep::Ok);
                            self.ledger.want(req.sub);
                            self.wire_wanted = Some(req);
                            self.pump();
                            self.live_slide = Some(req.idx);
                            self.presented = Some((req.sub, req.idx, clean));
                            if !clean {
                                self.unmask.drop_submission(req.sub);
                            }
                        }
                    }
                }
            }
            Step::Applied => {
                if let Some(req) = self.in_flight.take() {
                    self.shown = Some(req);
                    self.ledger.receipt(true);
                    self.pump();
                }
            }
            Step::RenderFailed => {
                if self.in_flight.take().is_some() {
                    self.ledger.receipt(false);
                    self.pump();
                }
            }
        }
        let lifted = self.sync();
        self.check(masked, toggled, lifted)
    }

    fn state(&self, id: Submission) -> SubmissionState {
        match (self.satisfy, self.armed_req, self.shown) {
            (Satisfy::Version, Some(armed), Some(shown))
                if (armed.item, armed.idx) == (shown.item, shown.idx) =>
            {
                SubmissionState::Applied
            }
            (Satisfy::Version, ..) => SubmissionState::Waiting,
            (Satisfy::Submission, ..) => self.ledger.state(id),
        }
    }

    /// `sync_show`: lifts the masks when the armed submission is applied.
    fn sync(&mut self) -> Option<Req> {
        let id = self.unmask.armed()?;
        if !self.output {
            return None;
        }
        let state = self.state(id);
        if self.unmask.settle(state) {
            self.masks.live_double_click();
            let req = self.armed_req;
            self.lifted_for = req;
            return req;
        }
        None
    }

    fn check(&self, was_masked: bool, toggled: bool, lifted: Option<Req>) -> Result<(), String> {
        if was_masked && !self.masks.any() && !toggled {
            let Some(req) = lifted else {
                return Err("masks went off without a toggle or a satisfied intent".into());
            };
            if Some(req) != self.last_explicit {
                return Err(format!("lifted for {req:?}, not the latest request"));
            }
            if self.ledger.state(req.sub) != SubmissionState::Applied {
                return Err(format!("lifted without an Applied receipt for {req:?}"));
            }
        }
        if let Some(req) = self.lifted_for
            && !self.masks.any()
            && self.shown != Some(req)
        {
            return Err(format!(
                "unmasked over {:?}, not the latest request {req:?}",
                self.shown
            ));
        }
        if self.masks.any() && self.masks.layer() == Layer::None {
            return Err("a mask is on but the wire layer is None".into());
        }
        Ok(())
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn run(satisfy: Satisfy, steps: impl IntoIterator<Item = Step>) -> Result<(), String> {
    let mut model = Model::new(satisfy);
    let mut trace = Vec::new();
    for step in steps {
        trace.push(step);
        model
            .apply(step)
            .map_err(|why| format!("{why}: {trace:?}"))?;
    }
    Ok(())
}

fn random_run(satisfy: Satisfy, seed: u64) -> Result<(), String> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let len = 3 + (rng.next() % 26) as usize;
    let steps: Vec<_> = (0..len)
        .map(|_| ALPHABET[(rng.next() % ALPHABET.len() as u64) as usize])
        .collect();
    run(satisfy, steps)
}

#[test]
fn random_interleavings_keep_masks_safe() {
    for seed in 0..60_000 {
        if let Err(why) = random_run(Satisfy::Submission, seed) {
            panic!("seed {seed}: {why}");
        }
    }
}

#[test]
fn every_short_sequence_keeps_masks_safe() {
    fn walk(depth: usize, prefix: &mut Vec<Step>) {
        if depth == 0 {
            if let Err(why) = run(Satisfy::Submission, prefix.iter().copied()) {
                panic!("{why}");
            }
            return;
        }
        for step in ALPHABET[..19].iter() {
            prefix.push(*step);
            walk(depth - 1, prefix);
            prefix.pop();
        }
    }
    for depth in 1..=4 {
        walk(depth, &mut Vec::new());
    }
}

/// The model must be able to see the bug it guards against: matching the
/// applied slide by reusable version lets a stale confirmation unmask.
#[test]
fn the_model_catches_version_matched_unmasking() {
    let caught = (0..60_000).any(|seed| random_run(Satisfy::Version, seed).is_err());
    assert!(caught, "version matching must violate the invariant");
}

/// Masked A, delayed B, double-click A.
#[test]
fn masked_a_delayed_b_double_click_a_waits_for_the_fresh_a() {
    let steps = [
        Step::Click(1),
        Step::Prepared(Prep::Ok),
        Step::DoubleClick(0),
        Step::Prepared(Prep::Ok),
        Step::Applied,
        Step::Applied,
    ];
    run(Satisfy::Submission, steps).unwrap();
    assert!(run(Satisfy::Version, steps).is_err());
}

/// Same-version RenderFailed retry never unmasks.
#[test]
fn same_version_render_failed_retry_never_unmasks() {
    let mut model = Model::new(Satisfy::Submission);
    for step in [
        Step::DoubleClick(0),
        Step::Prepared(Prep::Ok),
        Step::RenderFailed,
    ] {
        model.apply(step).unwrap();
    }
    assert!(model.masks.any());
    assert_eq!(model.unmask.armed(), None);
}
