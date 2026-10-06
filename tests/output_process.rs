//! Supervisor against an actual child process speaking the renderer protocol.
//! The child is this test binary in an ignored mode; no GPU or window is used.
use sela::{
    delivery::{DeliveryError, Epoch, LiveState, Outcome, RendererSession},
    output::{ACKNOWLEDGMENT, Launch, Loss, Refusal, STARTUP, Status, Stream, Supervisor},
    scene::{ContentVersion, Extent, PreparedCue, RendererCapabilities},
    transport::Frame,
};
use std::{
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 2048,
};
const EXTENT: Extent = Extent {
    width: 640,
    height: 360,
};
const RESIZED: Extent = Extent {
    width: 800,
    height: 450,
};

#[test]
#[ignore = "subprocess fixture, run only by supervisor tests"]
fn fake_audience() {
    let mode = std::env::var("SELA_FAKE_AUDIENCE").unwrap();
    let epoch =
        Epoch(u128::from_str_radix(&std::env::var("SELA_FAKE_EPOCH").unwrap(), 16).unwrap());
    let out = || std::io::stderr();
    match mode.as_str() {
        "silent" => {
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        "wrong-epoch" => Frame::ready(Epoch(epoch.0 ^ 1), 2048).write(out()).unwrap(),
        _ => Frame::ready(epoch, 2048).write(out()).unwrap(),
    }
    Frame::surface(epoch, EXTENT).write(out()).unwrap();
    if mode == "exit" {
        std::process::exit(0);
    }
    let mut session = RendererSession::new(epoch);
    let (mut applied, mut presented) = (0, 0);
    while let Ok(frame) = Frame::read(std::io::stdin()) {
        let now = Instant::now();
        let stamp = frame.command_stamp().unwrap();
        let ack = match frame.into_command(now, CAPS) {
            Ok(command) => session.accept(command, now),
            Err(_) => session.reject_preparation(stamp),
        };
        Frame::acknowledgment(ack).write(out()).unwrap();
        if ack.outcome != Outcome::Accepted || mode == "hang" {
            continue;
        }
        // Slow presentation lets the controller queue newer intent meanwhile.
        std::thread::sleep(Duration::from_millis(150));
        presented += 1;
        let reject = mode == "reject" && presented == 2;
        let ack = session
            .present(Instant::now(), |_| {
                if reject {
                    Err(DeliveryError::RenderFailed)
                } else {
                    Ok(())
                }
            })
            .unwrap();
        Frame::acknowledgment(ack).write(out()).unwrap();
        if ack.outcome == Outcome::Applied {
            applied += 1;
            if mode == "resize" && applied == 1 {
                Frame::surface(epoch, RESIZED).write(out()).unwrap();
            }
        }
    }
    std::process::exit(0);
}

fn start(mode: &'static str) -> Supervisor {
    Supervisor::start(
        move |epoch| {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "fake_audience", "--ignored", "--nocapture"])
                .env("SELA_FAKE_AUDIENCE", mode)
                .env("SELA_FAKE_EPOCH", format!("{:x}", epoch.0));
            Launch {
                command,
                frames: Stream::Stderr,
            }
        },
        Instant::now(),
    )
}

fn until(output: &mut Supervisor, what: &str, done: impl Fn(&Supervisor) -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !done(output) {
        assert!(Instant::now() < end, "bounded wait for {what}");
        output.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn connected(mode: &'static str) -> Supervisor {
    let mut output = start(mode);
    until(&mut output, "connection", |o| {
        o.status() != Status::Starting
    });
    assert_eq!(
        output.status(),
        Status::Connected {
            extent: EXTENT,
            caps: CAPS
        }
    );
    output
}

fn cue(revision: u64, extent: Extent) -> Arc<PreparedCue> {
    Arc::new(
        PreparedCue::diagnostic_color(
            ContentVersion { id: 5, revision },
            extent,
            [10, 20, 30, 255],
            CAPS,
        )
        .unwrap(),
    )
}

fn confirmed(revision: u64) -> LiveState {
    LiveState::Confirmed(ContentVersion { id: 5, revision })
}

#[test]
fn applied_cue_is_live_and_rapid_intent_coalesces_to_latest() {
    let mut output = connected("apply");
    assert_eq!(output.live(), LiveState::Unknown);
    output.present(cue(1, EXTENT), Instant::now()).unwrap();
    assert_eq!(output.in_flight(), Some(cue(1, EXTENT).version()));
    output.present(cue(2, EXTENT), Instant::now()).unwrap();
    output.present(cue(3, EXTENT), Instant::now()).unwrap();
    assert_eq!(output.wanted(), Some(cue(3, EXTENT).version()));
    assert_eq!(output.live(), LiveState::Unknown, "Accepted is not live");
    until(&mut output, "latest applied", |o| o.live() == confirmed(3));
    assert_eq!(output.in_flight(), None);
    assert_eq!(output.counters().submitted, 2, "superseded cue never sent");
    assert_eq!(output.counters().applied, 2);
}

#[test]
fn rejected_cue_keeps_prior_confirmed_scene() {
    let mut output = connected("reject");
    output.present(cue(1, EXTENT), Instant::now()).unwrap();
    until(&mut output, "first applied", |o| o.live() == confirmed(1));
    output.present(cue(2, EXTENT), Instant::now()).unwrap();
    until(&mut output, "rejection", |o| o.rejected().is_some());
    assert_eq!(
        output.rejected(),
        Some((cue(2, EXTENT).version(), DeliveryError::RenderFailed))
    );
    assert_eq!(output.live(), confirmed(1));
    assert!(matches!(output.status(), Status::Connected { .. }));
    output.present(cue(3, EXTENT), Instant::now()).unwrap();
    until(&mut output, "recovery", |o| o.live() == confirmed(3));
}

#[test]
fn surface_change_refuses_old_extent_cues() {
    let mut output = connected("resize");
    output.present(cue(1, EXTENT), Instant::now()).unwrap();
    until(
        &mut output,
        "resize report",
        |o| matches!(o.status(), Status::Connected { extent, .. } if extent == RESIZED),
    );
    assert_eq!(output.live(), confirmed(1));
    assert_eq!(
        output.present(cue(2, EXTENT), Instant::now()),
        Err(Refusal::WrongExtent)
    );
    output.present(cue(2, RESIZED), Instant::now()).unwrap();
    until(&mut output, "resized cue applied", |o| {
        o.live() == confirmed(2)
    });
}

#[test]
fn wrong_epoch_and_exit_lose_the_session() {
    let mut output = start("wrong-epoch");
    until(&mut output, "loss", |o| o.status() != Status::Starting);
    assert_eq!(output.status(), Status::Lost(Loss::Protocol));
    // Ready/surface and exit may be observed in the same poll.
    let mut output = start("exit");
    until(&mut output, "exit", |o| {
        matches!(o.status(), Status::Lost(_))
    });
    assert_eq!(output.status(), Status::Lost(Loss::Exited));
    assert_eq!(output.live(), LiveState::Unknown);
    assert_eq!(
        output.present(cue(1, EXTENT), Instant::now()),
        Err(Refusal::NotConnected)
    );
}

#[test]
fn startup_and_acknowledgment_deadlines_make_output_unknown() {
    let mut output = start("silent");
    output.poll(Instant::now());
    assert_eq!(output.status(), Status::Starting);
    output.poll(Instant::now() + STARTUP);
    assert_eq!(output.status(), Status::Lost(Loss::StartupTimedOut));

    let mut output = connected("hang");
    let sent = Instant::now();
    output.present(cue(1, EXTENT), sent).unwrap();
    until(&mut output, "acceptance", |o| o.counters().submitted == 1);
    output.poll(sent + ACKNOWLEDGMENT);
    assert_eq!(
        output.status(),
        Status::Lost(Loss::Delivery(DeliveryError::TimedOut))
    );
    assert_eq!(output.live(), LiveState::Unknown);
}

#[test]
fn each_session_has_a_fresh_epoch() {
    let a = start("silent");
    let b = start("silent");
    assert_ne!(a.epoch(), b.epoch());
}
