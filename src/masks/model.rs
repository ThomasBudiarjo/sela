//! Model-based check of the operator's mask/cue state machine without GPUI.
//!
//! The model mirrors `operator::preview_live` (requests, deferral, preparation
//! outcomes, refresh, library Go Live, output toggles) and drives the production
//! `Masks`, `UnmaskIntent` and `Submissions` types through random and exhaustive
//! interleavings with renderer receipts that arrive in any order. The oracle is
//! what the audience sees, kept by the model's own renderer scene:
//!
//! - Masks go off only on the operator's toggle, or when an armed double-click
//!   settles.
//! - At that moment the double-clicked request is the operator's latest explicit
//!   intent (no newer click, step, Go Live, refused Go Live or pending library
//!   Go Live), and the renderer is already showing that request's slide. An
//!   earlier overlapping presentation of the same request shows the same slide.
//! - Until the next explicit intent the renderer never switches to a scene of
//!   another request.
//!
//! Failed preparation, failed render and stale results never change the scene.
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
    /// The second callback of a double-click; honoured only right after the
    /// matching `Click` with no operator step in between.
    SecondClick(u8),
    Next,
    Prev,
    GoLiveNew,
    /// Songs-tab Go Live of a song whose load is still pending.
    LibraryGoLive,
    LoadLands,
    LoadFails,
    /// Go Live with an empty Preview or a slideless item.
    RefusedGoLive,
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

/// The first `EXHAUSTIVE` steps are walked exhaustively; the rest only
/// weight the random runs.
const ALPHABET: [Step; 27] = [
    Step::Click(0),
    Step::Click(1),
    Step::DoubleClick(0),
    Step::DoubleClick(1),
    Step::SecondClick(1),
    Step::Next,
    Step::Prev,
    Step::GoLiveNew,
    Step::LibraryGoLive,
    Step::LoadLands,
    Step::LoadFails,
    Step::RefusedGoLive,
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
    Step::SecondClick(0),
    Step::Applied,
    Step::Prepared(Prep::Ok),
];
const EXHAUSTIVE: usize = 24;

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
    /// Production minus the cancel in a library Go Live.
    NoLibraryCancel,
}

/// The operator's latest explicit intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intent {
    Slide(Req),
    LoadItem(u8),
    Refused,
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
    last_explicit: Intent,
    last_click: Option<u8>,
    pending_load: Option<u8>,
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
            last_explicit: Intent::Slide(first),
            last_click: None,
            pending_load: None,
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
        self.last_explicit = Intent::Slide(req);
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

    /// `go_live`: the cancel comes first, so a refused Go Live ends the older
    /// intent too.
    fn go_live(&mut self) {
        self.unmask.cancel();
        if self.output {
            self.cancel_cues();
            self.item += 1;
            self.send(0);
        } else {
            self.refuse();
        }
    }

    fn refuse(&mut self) {
        self.unmask.cancel();
        self.lifted_for = None;
        self.last_explicit = Intent::Refused;
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
        let prior_click = self.last_click;
        if !matches!(
            step,
            Step::Prepared(_)
                | Step::Applied
                | Step::RenderFailed
                | Step::FontsLand
                | Step::DeferNext
                | Step::Refresh
                | Step::LoadLands
                | Step::LoadFails
        ) {
            self.last_click = None;
        }
        match step {
            Step::Click(i) => {
                self.click(i, false);
                self.last_click = Some(i);
            }
            Step::SecondClick(i) => {
                if prior_click == Some(i) {
                    self.click(i, true);
                }
            }
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
            Step::GoLiveNew => self.go_live(),
            Step::LibraryGoLive => {
                if self.satisfy != Satisfy::NoLibraryCancel {
                    self.unmask.cancel();
                }
                self.lifted_for = None;
                self.last_explicit = Intent::LoadItem(self.item + 1);
                self.pending_load = Some(self.item + 1);
            }
            Step::LoadLands => {
                if self.pending_load.take().is_some() {
                    self.go_live();
                }
            }
            // Production keeps the pending load and retries it.
            Step::LoadFails => {}
            Step::RefusedGoLive => self.refuse(),
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
                    let carried = self.deferred.is_some_and(|d| d.sub == req.sub)
                        || self.prep.iter().any(|p| p.sub == req.sub);
                    if !carried {
                        self.unmask.drop_submission(req.sub);
                    }
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
            (Satisfy::Submission | Satisfy::NoLibraryCancel, ..) => self.ledger.state(id),
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
            if self.last_explicit != Intent::Slide(req) {
                return Err(format!(
                    "lifted for {req:?}, but the latest intent is {:?}",
                    self.last_explicit
                ));
            }
            if self.shown != Some(req) {
                return Err(format!(
                    "lifted for {req:?} while the audience shows {:?}",
                    self.shown
                ));
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
        for step in ALPHABET[..EXHAUSTIVE].iter() {
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

/// What the audience sees after each step.
struct Seen {
    masked: bool,
    shown: Option<Req>,
}

fn replay(satisfy: Satisfy, steps: &[Step]) -> Vec<Seen> {
    let mut model = Model::new(satisfy);
    steps
        .iter()
        .map(|step| {
            model.apply(*step).unwrap();
            Seen {
                masked: model.masks.any(),
                shown: model.shown,
            }
        })
        .collect()
}

/// A same-request retry overlapping the original presentation may settle the
/// intent, and then the audience shows exactly that request.
#[test]
fn overlapping_same_request_retry_reveals_only_that_request() {
    let steps = [
        Step::DoubleClick(1),
        Step::Prepared(Prep::Ok),
        Step::Refresh,
        Step::Prepared(Prep::Ok),
        Step::Applied,
        Step::RenderFailed,
    ];
    run(Satisfy::Submission, steps).unwrap();
    let last = replay(Satisfy::Submission, &steps).pop().unwrap();
    if !last.masked {
        assert_eq!(
            last.shown,
            Some(Req {
                item: 0,
                idx: 1,
                sub: 2
            })
        );
    }
}

/// An Applied that lands between the two callbacks of a double-click is the
/// clicked slide and the latest intent, so the second callback unmasks.
#[test]
fn applied_between_double_click_callbacks_reveals_the_clicked_slide() {
    let seen = replay(
        Satisfy::Submission,
        &[
            Step::Click(1),
            Step::Prepared(Prep::Ok),
            Step::Applied,
            Step::SecondClick(1),
        ],
    );
    let last = seen.last().unwrap();
    assert!(!last.masked);
    assert_eq!(last.shown.map(|r| r.idx), Some(1));
}

/// A second callback without its first click does nothing.
#[test]
fn second_click_needs_its_first_click() {
    let seen = replay(
        Satisfy::Submission,
        &[Step::Click(1), Step::Next, Step::SecondClick(1)],
    );
    assert!(seen.last().unwrap().masked);
}

/// A pending or failed library Go Live ends the older double-click: the masks
/// stay on whatever the load and delayed B do next.
#[test]
fn library_go_live_cancels_the_older_unmask() {
    let prefix = [
        Step::DoubleClick(1),
        Step::Prepared(Prep::Ok),
        Step::LibraryGoLive,
        Step::Applied,
    ];
    for tail in [
        &[Step::LoadLands, Step::Prepared(Prep::Ok), Step::Applied][..],
        &[Step::LoadFails, Step::LoadFails][..],
    ] {
        let steps: Vec<_> = prefix.iter().chain(tail).copied().collect();
        for seen in replay(Satisfy::Submission, &steps) {
            assert!(seen.masked, "{steps:?}");
        }
    }
}

/// A refused Go Live is a newer explicit request as well.
#[test]
fn refused_go_live_cancels_the_older_unmask() {
    let steps = [
        Step::DoubleClick(1),
        Step::Prepared(Prep::Ok),
        Step::RefusedGoLive,
        Step::Applied,
    ];
    for seen in replay(Satisfy::Submission, &steps) {
        assert!(seen.masked);
    }
}

/// Negative control: without the cancel in a library Go Live, B's Applied
/// lifts the mask although the latest intent is the library load.
#[test]
fn the_model_catches_a_library_go_live_that_does_not_cancel() {
    let steps = [
        Step::DoubleClick(1),
        Step::Prepared(Prep::Ok),
        Step::LibraryGoLive,
        Step::Applied,
    ];
    run(Satisfy::Submission, steps).unwrap();
    assert!(run(Satisfy::NoLibraryCancel, steps).is_err());
    let caught = (0..60_000).any(|seed| random_run(Satisfy::NoLibraryCancel, seed).is_err());
    assert!(caught, "the random runs must catch the missing cancel");
}
