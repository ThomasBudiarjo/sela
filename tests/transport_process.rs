//! Actual Rust child/pipes, no GPU required. Child deliberately never emits
//! Applied: its submission callback fails. Native submission is a separate test.
use sela::{
    delivery::{
        Acknowledgment, Delivery, DeliveryError, Epoch, Lane, LiveState, Outcome, RendererSession,
    },
    scene::{ContentVersion, Extent, PreparedCue, RendererCapabilities},
    transport::{Frame, PipeWorkers},
};
use std::{
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 2048,
};
#[test]
#[ignore = "subprocess fixture, run only by exchange test"]
fn pipe_child() {
    let mode = std::env::var("SELA_CUE_TEST_CHILD").unwrap();
    Frame::ready(Epoch(42), 2048)
        .write(std::io::stderr())
        .unwrap();
    if mode == "stall" {
        std::thread::sleep(Duration::from_secs(5));
        std::process::exit(0);
    }
    let mut session = RendererSession::new(Epoch(42));
    while let Ok(frame) = Frame::read(std::io::stdin()) {
        let stamp = frame.command_stamp().unwrap();
        let now = Instant::now();
        let ack = match frame.into_command(now, CAPS) {
            Ok(command) => session.accept(command, now),
            Err(_) => session.reject_preparation(stamp),
        };
        Frame::acknowledgment(ack).write(std::io::stderr()).unwrap();
        if mode != "queue" || stamp.sequence == 3 {
            while let Some(ack) = session.present(now, |_| Err(DeliveryError::RenderFailed)) {
                Frame::acknowledgment(ack).write(std::io::stderr()).unwrap();
            }
        }
    }
    std::process::exit(0); // No test harness text in binary pipe.
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn spawn(mode: &str) -> (OwnedChild, PipeWorkers) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "pipe_child", "--ignored", "--nocapture"])
        .env("SELA_CUE_TEST_CHILD", mode)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let pipes =
        PipeWorkers::new(child.stderr.take().unwrap(), child.stdin.take().unwrap()).unwrap();
    (OwnedChild(child), pipes)
}
fn receive(pipes: &PipeWorkers) -> Frame {
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(frame) = pipes.poll().unwrap() {
            return frame;
        }
        assert!(Instant::now() < end, "bounded child response deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn cue(revision: u64) -> Arc<PreparedCue> {
    Arc::new(
        PreparedCue::diagnostic_color(
            ContentVersion { id: 19, revision },
            Extent {
                width: 641,
                height: 360,
            },
            [18, 53, 109, 255],
            CAPS,
        )
        .unwrap(),
    )
}
fn frame(epoch: Epoch, revision: u64) -> Frame {
    let now = Instant::now();
    let mut d = Delivery::new(epoch);
    d.submit(cue(revision), Lane::Cue, now, now + Duration::from_secs(1))
        .unwrap();
    Frame::command(&d.take_next().unwrap(), now).unwrap()
}
#[test]
fn actual_child_exchange_rejection_epoch_and_sequence() {
    let (_child, pipes) = spawn("reject");
    assert_eq!(receive(&pipes).into_ready().unwrap(), (Epoch(42), 2048));
    let now = Instant::now();
    let mut d = Delivery::new(Epoch(42));
    let stamp = d
        .submit(cue(23), Lane::Cue, now, now + Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        d.submit(cue(3), Lane::Cue, now, now + Duration::from_secs(1)),
        Err(DeliveryError::Busy)
    );
    pipes
        .try_send(Frame::command(&d.take_next().unwrap(), now).unwrap())
        .unwrap();
    let accepted = receive(&pipes).into_acknowledgment().unwrap();
    assert_eq!(accepted.outcome, Outcome::Accepted);
    assert_eq!(accepted.stamp, stamp);
    assert!(d.acknowledge(accepted, Instant::now()));
    assert!(d.is_accepted(stamp));
    let rejected = receive(&pipes).into_acknowledgment().unwrap();
    assert_eq!(
        rejected.outcome,
        Outcome::Rejected(DeliveryError::RenderFailed)
    );
    assert!(d.acknowledge(rejected, Instant::now()));
    assert_eq!(d.live(), LiveState::Unknown);
    pipes.try_send(frame(Epoch(42), 3)).unwrap();
    assert_eq!(
        receive(&pipes).into_acknowledgment().unwrap().outcome,
        Outcome::Rejected(DeliveryError::Stale)
    );
    pipes.try_send(frame(Epoch(99), 3)).unwrap();
    assert_eq!(
        receive(&pipes).into_acknowledgment().unwrap().outcome,
        Outcome::Rejected(DeliveryError::WrongEpoch)
    );
}

#[test]
fn actual_child_enforces_lane_capacity_and_fifo() {
    let (_child, pipes) = spawn("queue");
    receive(&pipes).into_ready().unwrap();
    let now = Instant::now();
    let mut source = Delivery::new(Epoch(42));
    // Deliberately bypass the source's normal lane occupancy using synthetic
    // terminal receipts to test independent enforcement in the actual child.
    for (seq, lane) in [(1, Lane::Cue), (2, Lane::Cue), (3, Lane::Safety)] {
        let stamp = source
            .submit(cue(seq + 20), lane, now, now + Duration::from_secs(1))
            .unwrap();
        pipes
            .try_send(Frame::command(&source.take_next().unwrap(), now).unwrap())
            .unwrap();
        assert!(source.acknowledge(
            Acknowledgment {
                stamp,
                outcome: Outcome::Rejected(DeliveryError::RenderFailed)
            },
            now
        ));
    }
    for (seq, outcome) in [
        (1, Outcome::Accepted),
        (2, Outcome::Rejected(DeliveryError::Busy)),
        (3, Outcome::Accepted),
        (1, Outcome::Rejected(DeliveryError::RenderFailed)),
        (3, Outcome::Rejected(DeliveryError::RenderFailed)),
    ] {
        let ack = receive(&pipes).into_acknowledgment().unwrap();
        assert_eq!(ack.stamp.sequence, seq);
        assert_eq!(ack.outcome, outcome);
    }
}

#[test]
fn stalled_child_timeout_disconnect_and_fresh_session_no_replay() {
    let (mut child, pipes) = spawn("stall");
    receive(&pipes).into_ready().unwrap();
    let now = Instant::now();
    let mut d = Delivery::new(Epoch(42));
    d.submit(cue(23), Lane::Cue, now, now + Duration::from_millis(50))
        .unwrap();
    pipes
        .try_send(Frame::command(&d.take_next().unwrap(), now).unwrap())
        .unwrap();
    assert!(d.expire(now + Duration::from_millis(50)).is_some());
    assert_eq!(d.live(), LiveState::Unknown);
    assert_eq!(
        d.submit(cue(3), Lane::Safety, now, now + Duration::from_secs(1)),
        Err(DeliveryError::Disconnected)
    );
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        if pipes.poll().is_err() {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    // Old child reaped before new epoch. No commands are automatically retained.
    let mut fresh = Delivery::new(Epoch(43));
    assert!(fresh.take_next().is_none());
    assert_eq!(fresh.live(), LiveState::Unknown);
}
