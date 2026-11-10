//! Milestone 0 acceptance (design doc 13.1).
//!
//! A1: replaying one recorded input file twice gives the same world-state
//!     hash at every tick, and the harness catches a deliberate
//!     nondeterminism canary.
//! A2: a scripted headless run reaches GameOver with the expected winner
//!     and score.
//! A3: to be chosen by Morgan (exercise 16). Not implemented here.

use pong::sim::{MAX_POINTS, Pong, PongConfig, Side};
use pong::{GameState, sim};
use sage_core::{InputRecording, Simulation, TickInput, replay_twice, run_recording};

const SEED: u64 = 2026;
/// Safety cap for scripted runs: 5 points take well under this.
const MAX_TICKS: u64 = 60 * 60 * 10;

/// Plays a game where the left bot tracks the ball and the right paddle
/// never moves, recording every tick's input. Returns the recording and the
/// finished simulation.
fn scripted_game(seed: u64) -> (InputRecording, Pong) {
    let mut pong = Pong::new(PongConfig::new(seed)).unwrap();
    let mut rec = InputRecording::new(seed);
    while pong.game().state != GameState::GameOver {
        assert!(
            pong.tick() < MAX_TICKS,
            "game did not finish: {:?}",
            pong.game()
        );
        let input = pong.bot(Side::Left);
        rec.record(pong.tick(), input);
        pong.step(input);
    }
    (rec, pong)
}

#[test]
fn a2_scripted_run_reaches_game_over() {
    let (_, pong) = scripted_game(SEED);
    let game = pong.game();
    assert_eq!(game.state, GameState::GameOver);
    assert_eq!(game.winner, Some(Side::Left));
    assert_eq!(pong.points(), [MAX_POINTS, 0]);
    assert!(
        !pong.play_module_enabled(),
        "play rules must be off in GameOver"
    );
}

#[test]
fn a2_holds_for_other_seeds() {
    for seed in [1, 2, 3, 99] {
        let (_, pong) = scripted_game(seed);
        assert_eq!(pong.game().winner, Some(Side::Left), "seed {seed}");
        assert_eq!(pong.points(), [MAX_POINTS, 0], "seed {seed}");
    }
}

#[test]
fn a1_replay_twice_matches_every_tick() {
    let (rec, original) = scripted_game(SEED);
    // Through the file format, as a replay from disk would.
    let rec = InputRecording::from_text(&rec.to_text()).unwrap();
    let divergence = replay_twice(|seed| Pong::new(PongConfig::new(seed)).unwrap(), &rec);
    assert_eq!(divergence, None);

    // The replay also reproduces the outcome of the recorded run.
    let mut replayed = Pong::new(PongConfig::new(rec.seed)).unwrap();
    let trace = run_recording(&mut replayed, &rec);
    assert_eq!(trace.hashes.last().copied(), Some(original.state_hash()));
    assert_eq!(replayed.game().winner, Some(Side::Left));
}

#[test]
fn a1_canary_is_caught() {
    let (rec, _) = scripted_game(SEED);
    let divergence = replay_twice(
        |seed| {
            let mut c = PongConfig::new(seed);
            c.nondeterminism_canary = true;
            Pong::new(c).unwrap()
        },
        &rec,
    );
    let d = divergence.expect("harness missed the HashMap canary");
    // The canary lives outside the `play` module, so it runs from tick 0.
    assert_eq!(d.tick, Some(0), "{d:?}");
}

#[test]
fn different_seeds_differ() {
    let a = run_recording(&mut Pong::new(PongConfig::new(1)).unwrap(), &idle(200));
    let b = run_recording(&mut Pong::new(PongConfig::new(2)).unwrap(), &idle(200));
    assert_ne!(a.hashes.last(), b.hashes.last());
}

fn idle(ticks: u64) -> InputRecording {
    let mut r = InputRecording::new(0);
    r.record(ticks - 1, TickInput::EMPTY);
    r
}

#[test]
fn serving_waits_one_second_then_plays() {
    let mut pong = Pong::new(PongConfig::new(SEED)).unwrap();
    assert_eq!(pong.game().state, GameState::Serving);
    assert!(!pong.play_module_enabled());
    for _ in 0..sim::SERVE_TICKS - 1 {
        pong.step(TickInput::EMPTY);
        assert_eq!(pong.game().state, GameState::Serving);
    }
    pong.step(TickInput::EMPTY);
    assert_eq!(pong.game().state, GameState::Playing);
    assert!(pong.play_module_enabled());
}

/// Pause is module disabling: while Serving, input does not move paddles.
#[test]
fn input_ignored_while_play_module_disabled() {
    let mut pong = Pong::new(PongConfig::new(SEED)).unwrap();
    let before = pong.frame();
    pong.step(TickInput {
        buttons: sim::LEFT_UP | sim::RIGHT_DOWN,
    });
    let after = pong.frame();
    assert_eq!(before.rects, after.rects);
}

#[test]
fn script_errors_are_reported() {
    let mut c = PongConfig::new(SEED);
    c.rules_source =
        "import sage\n@sage.on('goal')\ndef g(world, goal):\n    world.go_to('Paused')\n".into();
    let mut pong = Pong::new(c).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..MAX_TICKS {
            pong.step(TickInput::EMPTY);
        }
    }));
    let msg = *result.unwrap_err().downcast::<String>().unwrap();
    assert!(msg.contains("unknown game state 'Paused'"), "{msg}");
}

#[test]
fn rules_without_goal_handler_are_rejected() {
    let mut c = PongConfig::new(SEED);
    c.rules_source = "x = 1\n".into();
    assert!(Pong::new(c).is_err());
}
