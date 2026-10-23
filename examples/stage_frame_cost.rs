//! Operator-thread cost of encoding one stage (Foldback) cue (M3-02 review):
//! `Frame::command` copies the panel and the live slide's face into a wire
//! frame on the operator UI thread, like an audience cue. This times that
//! encode for the bundled face (0.7 MiB) and a worst-case face padded to
//! `MAX_SOURCE_BYTES` (8 MiB), so the cost has a measured number and a
//! budget instead of a guess. No pipe, renderer or GPU work is measured.
//!
//! `cargo run --locked -j 3 --example stage_frame_cost [-- --runs N]`
//! (dev profile; dependencies build at opt-level 2, `sela` at 0, so the
//! numbers are an upper bound for a release build). Exits non-zero when the
//! worst-case p95 exceeds the budget.
use sela::{
    delivery::{Delivery, Epoch, Lane},
    fonts,
    scene::{ContentVersion, Extent, MAX_SOURCE_BYTES, PreparedCue, RendererCapabilities},
    stage::{self, StagePanel},
    transport::Frame,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Per-encode budget at the 8 MiB worst case: well under one 60 Hz frame,
/// so a slide change never costs the operator UI a visible frame.
const BUDGET: Duration = Duration::from_millis(8);
const WARMUP: usize = 5;

fn panel() -> StagePanel {
    StagePanel {
        current_label: "Signal Hymn · Verse 1".into(),
        current: "First original line\nSecond original line".into(),
        next_label: "Next · Chorus".into(),
        next: "Original refrain".into(),
        message: "Two minutes".into(),
        show_clock: true,
        show_next: true,
    }
}

fn cue(face: Arc<[u8]>) -> Arc<PreparedCue> {
    Arc::new(
        PreparedCue::for_stage(
            ContentVersion { id: 9, revision: 1 },
            Extent {
                width: 1920,
                height: 1080,
            },
            stage::BACKGROUND,
            panel(),
            (ContentVersion { id: 7, revision: 1 }, face, 0),
            RendererCapabilities {
                max_texture_dimension: 4096,
            },
        )
        .expect("stage cue"),
    )
}

/// Encode times for `runs` frames of `cue`, sorted.
fn measure(cue: &Arc<PreparedCue>, runs: usize) -> Vec<Duration> {
    let mut times = Vec::with_capacity(runs);
    for run in 0..WARMUP + runs {
        // A fresh session per run, so a pending cue never refuses the next.
        let mut delivery = Delivery::new(Epoch(run as u128 + 1));
        let now = Instant::now();
        delivery
            .submit(cue.clone(), Lane::Cue, now, now + Duration::from_secs(3))
            .expect("submit");
        let command = delivery.take_next().expect("command");
        let started = Instant::now();
        let frame = Frame::command(&command, now).expect("encode");
        let took = started.elapsed();
        std::hint::black_box(&frame);
        drop(frame);
        if run >= WARMUP {
            times.push(took);
        }
    }
    times.sort();
    times
}

fn at(times: &[Duration], share: f64) -> Duration {
    times[((times.len() - 1) as f64 * share).round() as usize]
}

fn main() {
    let mut runs = 50usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--runs" {
            runs = args
                .next()
                .and_then(|n| n.parse().ok())
                .filter(|n| *n > 0)
                .expect("--runs N");
        }
    }
    let bundled: Arc<[u8]> = Arc::from(fonts::BUNDLED);
    let mut padded = fonts::BUNDLED.to_vec();
    padded.resize(MAX_SOURCE_BYTES, 0);
    let worst: Arc<[u8]> = padded.into();
    let mut over = false;
    for (name, face) in [("bundled", bundled), ("8 MiB", worst)] {
        let size = face.len();
        let times = measure(&cue(face), runs);
        let p95 = at(&times, 0.95);
        println!(
            "{name:>8} face ({:.2} MiB): median {:.2} ms, p95 {:.2} ms, max {:.2} ms over {runs} encodes",
            size as f64 / (1024. * 1024.),
            at(&times, 0.5).as_secs_f64() * 1e3,
            p95.as_secs_f64() * 1e3,
            times[times.len() - 1].as_secs_f64() * 1e3,
        );
        if size == MAX_SOURCE_BYTES && p95 > BUDGET {
            over = true;
        }
    }
    println!("budget: p95 <= {} ms at 8 MiB", BUDGET.as_millis());
    if over {
        eprintln!("over budget");
        std::process::exit(1);
    }
}
