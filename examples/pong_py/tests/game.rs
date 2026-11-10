//! Pong written in Python: the Milestone 0 checks (A1, A2, A3 C2) applied to
//! the Python version.

use pong_py::{GameState, PongPy, PongPyConfig, Side};
use sage_core::{InputRecording, Simulation, replay_twice, run_recording};

/// Plays left bot vs idle right until GameOver; returns the recording.
fn scripted_game(seed: u64) -> (PongPy, InputRecording) {
    let mut p = PongPy::new(PongPyConfig::new(seed)).unwrap();
    let mut rec = InputRecording::new(seed);
    while p.state() != GameState::GameOver {
        assert!(p.tick() < 60 * 60 * 10, "game did not finish");
        let input = p.bot(Side::Left);
        rec.record(p.tick(), input);
        p.step(input);
    }
    (p, rec)
}

#[test]
fn a2_python_pong_reaches_game_over_with_left_winning_5_0() {
    for seed in [1, 2, 3, 99, 2026] {
        let (p, _) = scripted_game(seed);
        assert_eq!(p.winner(), Some(Side::Left), "seed {seed}");
        assert_eq!(p.points(), [5, 0], "seed {seed}");
        assert!(!p.play_enabled(), "play is off in GameOver");
        assert!(!p.ecs().has_opaque_state());
    }
}

#[test]
fn a1_replaying_a_recording_twice_gives_identical_hashes() {
    let (_, rec) = scripted_game(7);
    let div = replay_twice(|seed| PongPy::new(PongPyConfig::new(seed)).unwrap(), &rec);
    assert_eq!(div, None);
}

#[test]
fn a1_the_python_canary_is_caught() {
    let (_, rec) = scripted_game(7);
    let make = |seed| {
        let mut c = PongPyConfig::new(seed);
        c.nondeterminism_canary = true;
        PongPy::new(c).unwrap()
    };
    let div = replay_twice(make, &rec).expect("the harness must catch Python's OS-seeded random");
    // The canary is outside the `play` flecs module, so it runs from the
    // first tick; the initial state (before any tick) still matches.
    assert_eq!(div.tick, Some(0), "{div:?}");
}

#[test]
fn a3_c2_restoring_a_snapshot_into_a_fresh_world_reproduces_the_future() {
    let (_, rec) = scripted_game(3);
    let mut full = PongPy::new(PongPyConfig::new(rec.seed)).unwrap();
    let trace = run_recording(&mut full, &rec);
    for n in [1u64, 59, 60, 61, 200, rec.len / 2] {
        let mut a = PongPy::new(PongPyConfig::new(rec.seed)).unwrap();
        for t in 0..n {
            a.step(rec.get(t));
        }
        let snap = a.snapshot();
        let mut b = PongPy::new(PongPyConfig::new(rec.seed)).unwrap();
        b.step(rec.get(0)); // a world that has moved on
        b.restore(&snap).unwrap();
        assert_eq!(
            b.state_hash(),
            trace.hashes[n as usize - 1],
            "restored at {n}"
        );
        for t in n..(n + 300).min(rec.len) {
            b.step(rec.get(t));
            assert_eq!(
                b.state_hash(),
                trace.hashes[t as usize],
                "tick {t} after restore at {n}"
            );
        }
    }
}

#[test]
fn a3_c2_a_snapshot_missing_the_previous_contact_set_is_detected() {
    let (_, rec) = scripted_game(3);
    let mut a = PongPy::new(PongPyConfig::new(rec.seed)).unwrap();
    let mut b = PongPy::new(PongPyConfig::new(rec.seed)).unwrap();
    // Run to a tick where the ball touches something.
    let mut t = 0;
    loop {
        a.step(rec.get(t));
        b.step(rec.get(t));
        t += 1;
        if !a.snapshot().contacts.now().is_empty() && a.state() == GameState::Playing {
            break;
        }
    }
    let mut snap = a.snapshot();
    snap.contacts = sage_collision::ContactTable::new();
    b.restore(&snap).unwrap();
    assert_ne!(a.state_hash(), b.state_hash());
}

#[test]
fn script_errors_surface_with_a_traceback() {
    let mut c = PongPyConfig::new(1);
    c.script = c
        .script
        .replace("vel.x * sage.TICK_DT", "vel.x * undefined_name");
    let mut p = PongPy::new(c).unwrap();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..200 {
            p.step(sage_core::TickInput::EMPTY);
        }
    }));
    let msg = *r.unwrap_err().downcast::<String>().unwrap();
    assert!(
        msg.contains("undefined_name") && msg.contains("system pong.play.move"),
        "{msg}"
    );
}
