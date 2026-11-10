//! Acceptance A3 (design doc 13.1). Morgan chose both candidates
//! (exercise 16, 2026-11-09): C1 and C2 below together are A3.
//!
//! C1, contact invariants: over a full game, the contact table's begin, stay
//!     and end sets are consistent with the previous and current sets, every
//!     begin transition produces exactly one `ContactBegin` event, and a
//!     ball-paddle contact causes exactly one bounce.
//!     If it fails: the table, the event layer or the bounce rule disagree.
//! C2, snapshot restore: restoring a snapshot taken at tick N reproduces the
//!     hash recorded at tick N, and every later tick's hash.
//!     If it fails: some simulation state is missing from the snapshot (for
//!     example the previous contact set) or from the hash.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use flecs_ecs::prelude::*;
use pong::sim::{ContactBegin, Contacts, ID_BALL, ID_PADDLE_LEFT, ID_PADDLE_RIGHT, Velocity};
use pong::{GameState, Pong, PongConfig, Side};
use sage_core::{InputRecording, Simulation, StableId, run_recording};

fn scripted_recording(seed: u64) -> InputRecording {
    let mut pong = Pong::new(PongConfig::new(seed)).unwrap();
    let mut rec = InputRecording::new(seed);
    while pong.game().state != GameState::GameOver {
        let input = pong.bot(Side::Left);
        rec.record(pong.tick(), input);
        pong.step(input);
    }
    rec
}

type Pair = (StableId, StableId);

#[test]
fn c1_contact_table_invariants_hold_for_a_full_game() {
    // Seed 2026 happens to serve right five times (no paddle hits); the
    // others exercise bounces off the left bot's paddle.
    let mut total = 0;
    for seed in [1, 2, 3, 99, 2026] {
        total += check_contact_invariants(seed);
    }
    assert!(total >= 10, "too few paddle hits to be a test: {total}");
}

/// Returns the number of ball-paddle contacts in the game.
fn check_contact_invariants(seed: u64) -> u32 {
    let rec = scripted_recording(seed);
    let mut pong = Pong::new(PongConfig::new(rec.seed)).unwrap();

    // Every ContactBegin event, captured from outside the play module.
    let events: Rc<RefCell<Vec<Pair>>> = Rc::default();
    let ev = events.clone();
    pong.world()
        .observer::<ContactBegin, ()>()
        .with(flecs::Any)
        .each_iter(move |it, _, _| {
            let c = it.param();
            ev.borrow_mut().push((c.a, c.b));
        });

    let mut paddle_begins = 0;
    let mut bounces = 0;
    for tick in 0..rec.len {
        let ball_vx_before = ball_vx(&pong);
        // Detection runs this tick only if `play` is enabled when the tick
        // starts; state changes happen in SimReact, after SimDetect.
        let detection_ran = pong.play_module_enabled();
        events.borrow_mut().clear();
        pong.step(rec.get(tick));

        let (now, prev, tr) = pong.world().get::<&Contacts>(|c| {
            let set = |v: &[sage_collision::Contact]| -> BTreeSet<Pair> {
                v.iter().map(|c| c.pair()).collect()
            };
            (set(c.0.now()), set(c.0.prev()), c.0.transitions())
        });
        let set = |v: &[sage_collision::Contact]| -> BTreeSet<Pair> {
            v.iter().map(|c| c.pair()).collect()
        };
        let (begin, stay, end) = (set(&tr.begin), set(&tr.stay), set(&tr.end));

        // Set algebra (8.2).
        assert_eq!(begin, &now - &prev, "seed {seed} tick {tick}: begin");
        assert_eq!(end, &prev - &now, "seed {seed} tick {tick}: end");
        assert_eq!(stay, &now & &prev, "seed {seed} tick {tick}: stay");

        // Exactly one event per begin, in canonical order, on ticks where
        // detection ran; none otherwise (the play module is off).
        if !detection_ran {
            assert!(
                events.borrow().is_empty(),
                "seed {seed} tick {tick}: event while play disabled"
            );
        } else {
            let mut sorted = events.borrow().clone();
            sorted.sort();
            assert_eq!(
                *events.borrow(),
                sorted,
                "seed {seed} tick {tick}: events out of order"
            );
            assert_eq!(
                sorted.into_iter().collect::<BTreeSet<_>>(),
                begin,
                "seed {seed} tick {tick}"
            );
        }

        // One bounce per ball-paddle contact: the ball's x direction flips
        // exactly on ticks where a ball-paddle begin happened.
        let hit_paddle = begin
            .iter()
            .any(|&(a, b)| b == ID_BALL && (a == ID_PADDLE_LEFT || a == ID_PADDLE_RIGHT));
        let flipped = ball_vx_before != 0.0
            && ball_vx(&pong) != 0.0
            && ball_vx_before.signum() != ball_vx(&pong).signum();
        if hit_paddle {
            paddle_begins += 1;
        }
        if flipped {
            bounces += 1;
        }
        assert_eq!(
            hit_paddle, flipped,
            "seed {seed} tick {tick}: paddle contact vs bounce"
        );
        // No ball-paddle contact ever lingers into a stay.
        assert!(
            !stay
                .iter()
                .any(|&(a, b)| b == ID_BALL && (a == ID_PADDLE_LEFT || a == ID_PADDLE_RIGHT)),
            "seed {seed} tick {tick}: ball stayed inside a paddle"
        );
    }
    assert_eq!(paddle_begins, bounces, "seed {seed}");
    paddle_begins
}

fn ball_vx(pong: &Pong) -> f32 {
    let raw = pong.world().lookup_recursive("ball");
    raw.get::<&Velocity>(|v| v.x)
}

#[test]
fn c2_restoring_snapshot_reproduces_hashes() {
    let rec = scripted_recording(2026);
    let reference = run_recording(&mut Pong::new(PongConfig::new(rec.seed)).unwrap(), &rec);

    // Snapshot points: serving, mid-rally (with live contacts), near the end.
    for n in [30u64, 200, 433, rec.len - 50] {
        let mut a = Pong::new(PongConfig::new(rec.seed)).unwrap();
        for t in 0..n {
            a.step(rec.get(t));
        }
        let snap = a.snapshot();
        assert_eq!(a.state_hash(), reference.hashes[n as usize - 1]);

        // Restore into a fresh world and into a world that has moved on.
        let mut fresh = Pong::new(PongConfig::new(rec.seed)).unwrap();
        fresh.restore(&snap);
        let mut moved_on = Pong::new(PongConfig::new(rec.seed)).unwrap();
        for t in 0..(n + 37).min(rec.len) {
            moved_on.step(rec.get(t));
        }
        moved_on.restore(&snap);

        for sim in [&mut fresh, &mut moved_on] {
            assert_eq!(
                sim.state_hash(),
                reference.hashes[n as usize - 1],
                "restore at {n}"
            );
            for t in n..rec.len {
                sim.step(rec.get(t));
                assert_eq!(
                    sim.state_hash(),
                    reference.hashes[t as usize],
                    "after restore at {n}: diverged at tick {t}"
                );
            }
            assert_eq!(sim.game().winner, Some(Side::Left));
        }
    }
}

/// The failure C2 exists to catch: a snapshot without the previous contact
/// set. Dropping it must be detected.
#[test]
fn c2_detects_missing_previous_contacts() {
    let rec = scripted_recording(2026);
    let mut a = Pong::new(PongConfig::new(rec.seed)).unwrap();
    // Find a tick with a live contact (ball in a wall or paddle is pushed
    // out, but ball/side and paddle/wall overlaps persist).
    let mut n = 0;
    loop {
        a.step(rec.get(n));
        n += 1;
        if a.world().get::<&Contacts>(|c| !c.0.now().is_empty()) && a.play_module_enabled() {
            break;
        }
    }
    let good = a.snapshot();
    let mut bad = good.clone();
    bad.contacts = sage_collision::ContactTable::new();
    let mut b = Pong::new(PongConfig::new(rec.seed)).unwrap();
    b.restore(&bad);
    assert_ne!(
        b.state_hash(),
        a.state_hash(),
        "hash missed the contact table"
    );
}
