use super::*;
use crate::{
    preparation::{PreparationEvent, Preparer},
    scene::{self, BackgroundSpec, Extent, RendererCapabilities, SceneSpec},
};
use std::{sync::atomic::AtomicBool, time::Duration};

const EPOCH: Epoch = Epoch(813);
const CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: 4096,
};

fn spec(revision: u64) -> SceneSpec {
    SceneSpec {
        version: ContentVersion { id: 37, revision },
        extent: Extent {
            width: 1920,
            height: 1080,
        },
        background: BackgroundSpec::Color([17, 41, 79, 255]),
        text: None,
    }
}
fn cue(revision: u64) -> Arc<PreparedCue> {
    Arc::new(scene::prepare(spec(revision), CAPS, &AtomicBool::new(false)).unwrap())
}
fn command(sequence: u64, revision: u64, lane: Lane, now: Instant) -> Command {
    Command {
        stamp: Stamp {
            epoch: EPOCH,
            sequence,
        },
        lane,
        cue: cue(revision),
        deadline: now + Duration::from_secs(3),
    }
}
fn submit(delivery: &mut Delivery, revision: u64, lane: Lane, now: Instant) -> Stamp {
    delivery
        .submit(cue(revision), lane, now, now + Duration::from_secs(3))
        .unwrap()
}
fn apply(delivery: &mut Delivery, renderer: &mut RendererSession, now: Instant) -> Acknowledgment {
    let accepted = renderer.accept(delivery.take_next().unwrap(), now);
    assert_eq!(accepted.outcome, Outcome::Accepted);
    assert!(delivery.acknowledge(accepted, now));
    let applied = renderer.present(now, |_| Ok(())).unwrap();
    assert!(delivery.acknowledge(applied, now));
    applied
}

#[test]
fn accepted_is_not_live_and_failed_candidate_preserves_applied_snapshot() {
    let now = Instant::now();
    let mut delivery = Delivery::new(EPOCH);
    let mut renderer = RendererSession::new(EPOCH);
    let first = submit(&mut delivery, 3, Lane::Cue, now);
    assert_eq!(delivery.live(), LiveState::Unknown);
    let accepted = renderer.accept(delivery.take_next().unwrap(), now);
    delivery.acknowledge(accepted, now);
    assert!(delivery.is_accepted(first));
    assert_eq!(delivery.live(), LiveState::Unknown);
    assert!(renderer.applied().is_none());
    let ack = renderer
        .present(now, |candidate| {
            assert_eq!(candidate.version().revision, 3);
            Ok(())
        })
        .unwrap();
    assert_eq!(delivery.live(), LiveState::Unknown);
    delivery.acknowledge(ack, now);
    let original = renderer.applied().unwrap().clone();
    submit(&mut delivery, 17, Lane::Cue, now);
    let accepted = renderer.accept(delivery.take_next().unwrap(), now);
    delivery.acknowledge(accepted, now);
    let rejected = renderer
        .present(now, |_| Err(DeliveryError::RenderFailed))
        .unwrap();
    assert_eq!(
        rejected.outcome,
        Outcome::Rejected(DeliveryError::RenderFailed)
    );
    delivery.acknowledge(rejected, now);
    assert!(Arc::ptr_eq(&original, renderer.applied().unwrap()));
    assert_eq!(delivery.live(), LiveState::Confirmed(spec(3).version));
    assert_eq!(delivery.counters().rejected, 1);
}

#[test]
fn reserved_slot_survives_normal_flood_and_cross_lane_order_is_fifo() {
    for lanes in [[Lane::Cue, Lane::Safety], [Lane::Safety, Lane::Cue]] {
        let now = Instant::now();
        let mut delivery = Delivery::new(EPOCH);
        let mut renderer = RendererSession::new(EPOCH);
        let first = submit(&mut delivery, 7, lanes[0], now);
        for _ in 0..1000 {
            assert_eq!(
                delivery.submit(cue(19), lanes[0], now, now + Duration::from_secs(3)),
                Err(DeliveryError::Busy)
            );
        }
        let second = submit(&mut delivery, 29, lanes[1], now);
        assert_eq!(second.sequence, first.sequence + 1);
        for expected in [first, second] {
            let command = delivery.take_next().unwrap();
            assert_eq!(command.stamp(), expected);
            let ack = renderer.accept(command, now);
            assert_eq!(ack.outcome, Outcome::Accepted);
            delivery.acknowledge(ack, now);
        }
        assert!(delivery.take_next().is_none());
        // Dequeuing and Accepted do not release capacity before terminal receipt.
        assert_eq!(
            delivery.submit(cue(41), lanes[0], now, now + Duration::from_secs(3)),
            Err(DeliveryError::Busy)
        );
        for revision in [7, 29] {
            let ack = renderer
                .present(now, |candidate| {
                    assert_eq!(candidate.version().revision, revision);
                    Ok(())
                })
                .unwrap();
            delivery.acknowledge(ack, now);
            assert_eq!(
                delivery.live(),
                LiveState::Confirmed(spec(revision).version)
            );
        }
        assert!(renderer.pending().is_none());
        assert_eq!(delivery.counters().overloaded, 1001);
    }
}

#[test]
fn reordered_commands_and_duplicates_never_reapply_old_content() {
    let now = Instant::now();
    let mut renderer = RendererSession::new(EPOCH);
    assert_eq!(
        renderer
            .accept(command(2, 19, Lane::Safety, now), now)
            .outcome,
        Outcome::Accepted
    );
    assert_eq!(
        renderer.accept(command(1, 3, Lane::Cue, now), now).outcome,
        Outcome::Rejected(DeliveryError::Stale)
    );
    assert_eq!(
        renderer
            .accept(command(2, 19, Lane::Safety, now), now)
            .outcome,
        Outcome::Accepted
    );
    let ack = renderer.present(now, |_| Ok(())).unwrap();
    assert_eq!(ack.outcome, Outcome::Applied);
    assert_eq!(
        renderer
            .accept(command(2, 19, Lane::Safety, now), now)
            .outcome,
        Outcome::Applied
    );
    assert!(
        renderer
            .present(now, |_| panic!("duplicate rendered twice"))
            .is_none()
    );
    assert_eq!(renderer.applied().unwrap().version().revision, 19);
}

#[test]
fn acknowledgment_permutations_do_not_regress_live_or_release_capacity_early() {
    // All 24 permutations of Accepted/Applied receipts for two FIFO commands.
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if (0..4).any(|i| (i + 1..4).any(|j| order[i] == order[j])) {
                        continue;
                    }
                    let now = Instant::now();
                    let mut delivery = Delivery::new(EPOCH);
                    let mut renderer = RendererSession::new(EPOCH);
                    submit(&mut delivery, 3, Lane::Cue, now);
                    submit(&mut delivery, 23, Lane::Safety, now);
                    let accepted_a = renderer.accept(delivery.take_next().unwrap(), now);
                    let accepted_b = renderer.accept(delivery.take_next().unwrap(), now);
                    let applied_a = renderer.present(now, |_| Ok(())).unwrap();
                    let applied_b = renderer.present(now, |_| Ok(())).unwrap();
                    let receipts = [accepted_a, applied_a, accepted_b, applied_b];
                    let mut newest_seen = 0;
                    for index in order {
                        delivery.acknowledge(receipts[index], now);
                        if index == 1 {
                            newest_seen = newest_seen.max(3);
                        }
                        if index == 3 {
                            newest_seen = 23;
                        }
                        let expected = if newest_seen == 0 {
                            LiveState::Unknown
                        } else {
                            LiveState::Confirmed(spec(newest_seen).version)
                        };
                        assert_eq!(delivery.live(), expected, "receipt order {order:?}");
                    }
                    assert_eq!(delivery.counters().applied, 2);
                    assert!(!delivery.acknowledge(applied_b, now));
                    assert!(!delivery.is_accepted(applied_a.stamp));
                }
            }
        }
    }
}

#[test]
fn deadline_is_checked_before_output_callback_and_missing_ack_is_unknown() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(3);
    let mut delivery = Delivery::new(EPOCH);
    let mut renderer = RendererSession::new(EPOCH);
    submit(&mut delivery, 2, Lane::Cue, now);
    apply(&mut delivery, &mut renderer, now);
    let stamp = submit(&mut delivery, 11, Lane::Cue, now);
    let accepted = renderer.accept(delivery.take_next().unwrap(), now);
    delivery.acknowledge(accepted, now);
    assert_eq!(delivery.expire(deadline - Duration::from_nanos(1)), None);
    let rejected = renderer
        .present(deadline, |_| panic!("expired command touched output"))
        .unwrap();
    assert_eq!(rejected.outcome, Outcome::Rejected(DeliveryError::TimedOut));
    assert_eq!(renderer.applied().unwrap().version().revision, 2);
    assert_eq!(delivery.expire(deadline), Some(stamp));
    assert_eq!(delivery.live(), LiveState::Unknown);
    assert_eq!(delivery.last_confirmed(), Some(spec(2).version));
    assert!(!delivery.acknowledge(rejected, deadline));
    assert!(delivery.take_next().is_none());
    assert_eq!(
        delivery.submit(cue(15), Lane::Safety, now, deadline),
        Err(DeliveryError::Disconnected)
    );
    assert_eq!(delivery.counters().timed_out, 1);

    let mut renderer = RendererSession::new(EPOCH);
    assert_eq!(
        renderer
            .accept(command(1, 5, Lane::Cue, now), deadline)
            .outcome,
        Outcome::Rejected(DeliveryError::TimedOut)
    );
    assert!(renderer.pending().is_none());
}

#[test]
fn disconnected_session_is_not_replayed_and_foreign_or_unsent_acks_are_ignored() {
    let now = Instant::now();
    let mut delivery = Delivery::new(EPOCH);
    let stamp = submit(&mut delivery, 13, Lane::Cue, now);
    assert!(!delivery.acknowledge(
        Acknowledgment {
            stamp,
            outcome: Outcome::Applied
        },
        now
    ));
    let command = delivery.take_next().unwrap();
    delivery.disconnect();
    assert!(!delivery.acknowledge(
        Acknowledgment {
            stamp,
            outcome: Outcome::Applied
        },
        now
    ));
    let fresh = Epoch(991);
    let mut next_delivery = Delivery::new(fresh);
    let mut renderer = RendererSession::new(fresh);
    assert!(next_delivery.take_next().is_none());
    assert_eq!(
        renderer.accept(command, now).outcome,
        Outcome::Rejected(DeliveryError::WrongEpoch)
    );
    assert!(!next_delivery.acknowledge(
        Acknowledgment {
            stamp,
            outcome: Outcome::Applied
        },
        now
    ));
    let next = submit(&mut next_delivery, 31, Lane::Cue, now);
    assert_eq!(next.sequence, 1);
    apply(&mut next_delivery, &mut renderer, now);
    assert_eq!(next_delivery.live(), LiveState::Confirmed(spec(31).version));
}

#[test]
fn renderer_backpressure_and_uncertain_stale_receipt_do_not_invent_live_state() {
    let now = Instant::now();
    let mut renderer = RendererSession::new(EPOCH);
    assert_eq!(
        renderer.accept(command(1, 3, Lane::Cue, now), now).outcome,
        Outcome::Accepted
    );
    assert_eq!(
        renderer.accept(command(2, 9, Lane::Cue, now), now).outcome,
        Outcome::Rejected(DeliveryError::Busy)
    );
    assert_eq!(
        renderer
            .accept(command(3, 27, Lane::Safety, now), now)
            .outcome,
        Outcome::Accepted
    );
    assert_eq!(renderer.pending.len(), 2);
    let mut delivery = Delivery::new(EPOCH);
    let stamp = submit(&mut delivery, 3, Lane::Cue, now);
    delivery.take_next().unwrap();
    delivery.acknowledge(
        Acknowledgment {
            stamp,
            outcome: Outcome::Rejected(DeliveryError::Stale),
        },
        now,
    );
    assert_eq!(delivery.live(), LiveState::Unknown);
    assert_eq!(
        delivery.submit(cue(17), Lane::Cue, now, now + Duration::from_secs(1)),
        Err(DeliveryError::Disconnected)
    );
}

#[test]
fn preparation_failure_cannot_replace_delivered_scene_and_recovery_works() {
    let now = Instant::now();
    let mut delivery = Delivery::new(EPOCH);
    let mut renderer = RendererSession::new(EPOCH);
    submit(&mut delivery, 5, Lane::Cue, now);
    apply(&mut delivery, &mut renderer, now);
    let mut preparer = Preparer::new(CAPS).unwrap();
    let mut broken = spec(19);
    let directory = tempfile::tempdir().unwrap();
    broken.background = BackgroundSpec::Image(scene::ResourceRef {
        version: ContentVersion {
            id: 81,
            revision: 1,
        },
        path: directory.path().join("missing"),
        sha256: [0; 32],
    });
    for (candidate, successful) in [(broken, false), (spec(31), true)] {
        preparer
            .request(candidate, now, Duration::from_secs(3))
            .unwrap();
        let watchdog = Instant::now() + Duration::from_secs(5);
        loop {
            match preparer.poll(now) {
                Some(PreparationEvent::Ready { cue, .. }) => {
                    assert!(successful);
                    delivery
                        .submit(cue, Lane::Cue, now, now + Duration::from_secs(3))
                        .unwrap();
                    break;
                }
                Some(PreparationEvent::Failed { error, .. }) => {
                    assert!(!successful);
                    assert!(matches!(error, scene::PrepareError::MissingResource(_)));
                    break;
                }
                None => {
                    assert!(Instant::now() < watchdog);
                    std::thread::yield_now();
                }
            }
        }
        assert_eq!(renderer.applied().unwrap().version().revision, 5);
        assert_eq!(delivery.live(), LiveState::Confirmed(spec(5).version));
    }
    apply(&mut delivery, &mut renderer, now);
    assert_eq!(delivery.live(), LiveState::Confirmed(spec(31).version));
}
