//! The platformer is a deterministic simulation like Pong (design doc 8.3,
//! spikes S3 and A1 to A3): the same recording gives the same per-tick hashes,
//! the nondeterminism canary is caught, and a snapshot or a save game
//! reproduces the future.

mod common;

use common::*;
use sage_core::{InputRecording, Simulation, replay_twice, run_recording};
use sage_platformer::buttons::*;
use sage_platformer::{Platformer, PlatformerConfig};

/// A level with everything in it: coins, an enemy, spikes, a flag, platforms.
pub const BUSY: &str = "
........................................
........................................
........................................
..............=====.....................
....................o...................
.....o.o...........hhh.......o..........
.@..........e....c..............e.......
###########.#######^^####.##############
###########.#######################....
";

/// A fixed pseudo-random button script (a tiny LCG, no OS randomness).
fn recording(seed: u64, ticks: u64) -> InputRecording {
    let mut rec = InputRecording::new(seed);
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ seed;
    let mut held = 0u32;
    for tick in 0..ticks {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        if (x >> 33).is_multiple_of(7) {
            // Change what is held every so often.
            let r = (x >> 40) as u32;
            held = [
                RIGHT,
                RIGHT | RUN,
                RIGHT | JUMP,
                LEFT,
                RIGHT | RUN | JUMP,
                DOWN,
                SPIN | RIGHT,
            ][(r % 7) as usize]
                | if r & 0x100 != 0 { JUMP } else { 0 };
        }
        rec.record(tick, sage_core::TickInput { buttons: held });
    }
    rec
}

fn make(seed: u64) -> Platformer {
    Platformer::new(PlatformerConfig::new(seed, game(BUSY))).unwrap()
}

#[test]
fn replaying_a_recording_twice_gives_identical_hashes() {
    let rec = recording(5, 1800);
    assert_eq!(replay_twice(make, &rec), None);
}

#[test]
fn different_input_gives_different_state() {
    let a = run_recording(&mut make(5), &recording(5, 600));
    let b = run_recording(&mut make(5), &recording(6, 600));
    assert_ne!(a.hashes.last(), b.hashes.last());
}

#[test]
fn the_python_canary_is_caught() {
    let rec = recording(5, 300);
    let canary = |seed| {
        let mut c = PlatformerConfig::new(seed, game(BUSY));
        c.nondeterminism_canary = true;
        Platformer::new(c).unwrap()
    };
    let div = replay_twice(canary, &rec).expect("the harness must catch OS-seeded random");
    assert!(div.tick.is_some(), "{div:?}");
}

#[test]
fn restoring_a_snapshot_into_a_fresh_world_reproduces_the_future() {
    let rec = recording(9, 900);
    let mut full = make(rec.seed);
    let trace = run_recording(&mut full, &rec);
    for n in [1u64, 59, 60, 200, 450] {
        let mut a = make(rec.seed);
        for t in 0..n {
            a.step(rec.get(t));
        }
        let snap = a.snapshot();
        let mut b = make(rec.seed);
        b.restore(&snap).unwrap();
        assert_eq!(b.state_hash(), a.state_hash(), "after restoring at {n}");
        for t in n..rec.len {
            b.step(rec.get(t));
            assert_eq!(
                b.state_hash(),
                trace.hashes[t as usize],
                "restored at {n}, diverged at tick {t}"
            );
        }
    }
}

#[test]
fn a_save_game_reproduces_the_future_too() {
    let rec = recording(11, 900);
    let mut full = make(rec.seed);
    let trace = run_recording(&mut full, &rec);
    let n = 400;
    let mut a = make(rec.seed);
    for t in 0..n {
        a.step(rec.get(t));
    }
    let text = a.save();
    let mut b = make(rec.seed);
    b.load(&text).unwrap();
    assert_eq!(b.state_hash(), a.state_hash());
    for t in n..rec.len {
        b.step(rec.get(t));
        assert_eq!(b.state_hash(), trace.hashes[t as usize], "tick {t}");
    }
    // Damage is an error, not a guess.
    let damaged = text.replace("sage-save v1", "sage-save v9");
    assert!(make(1).load(&damaged).is_err());
}

#[test]
fn the_busy_level_is_actually_busy_and_replays_the_same_events() {
    // Not vacuous: the scripted game moves, jumps and meets things.
    let rec = recording(5, 1800);
    let mut p = make(5);
    let mut seen = std::collections::BTreeSet::new();
    for t in 0..rec.len {
        p.step(rec.get(t));
        seen.insert(p.mode_name());
    }
    assert!(seen.len() >= 4, "only saw {seen:?}");
    assert!(p.tally().iter().sum::<u32>() > 10, "{:?}", p.tally());
    // The same cues come out of a second run.
    let cues = |seed| {
        let mut p = make(seed);
        let mut all = vec![];
        for t in 0..rec.len {
            p.step(rec.get(t));
            all.extend(p.take_cues());
        }
        all
    };
    assert_eq!(cues(5), cues(5));
}

#[test]
fn cues_do_not_change_the_state_hash() {
    let rec = recording(5, 600);
    let (mut a, mut b) = (make(5), make(5));
    for t in 0..rec.len {
        a.step(rec.get(t));
        b.step(rec.get(t));
        if t % 3 == 0 {
            let _ = b.take_cues();
        }
        assert_eq!(a.state_hash(), b.state_hash());
    }
}
