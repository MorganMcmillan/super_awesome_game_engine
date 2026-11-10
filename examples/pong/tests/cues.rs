//! Sound cues (M1): derived from a full scripted game, they match what
//! happened, repeat exactly, and never touch the simulation state.

use pong::{GameState, Pong, PongConfig, Shown, Side};
use sage_core::Simulation;
use sage_platform::Presentable;

/// Plays the left bot against an idle right paddle to GameOver, returning
/// every cue in order and the final state hash.
fn play(seed: u64) -> (Vec<String>, u64, [u32; 2]) {
    let mut shown = Shown::new(Pong::new(PongConfig::new(seed)).unwrap());
    let mut cues = vec![];
    while shown.pong.state() != GameState::GameOver {
        assert!(shown.tick() < 60 * 60 * 10, "game did not finish");
        let input = shown.pong.bot(Side::Left);
        shown.step(input);
        cues.extend(shown.take_cues());
    }
    let points = shown.pong.points();
    (cues, shown.state_hash(), points)
}

#[test]
fn cues_match_the_events_of_a_full_game() {
    let (cues, _, points) = play(1);
    let count = |name: &str| cues.iter().filter(|c| *c == name).count();
    // One "score" per point, one "win", and it is the last cue.
    assert_eq!(count("score") as u32, points[0] + points[1]);
    assert_eq!(count("win"), 1);
    assert_eq!(cues.last().map(String::as_str), Some("win"));
    // The bot returns the ball, so there are paddle hits (at least one per point).
    assert!(count("paddle") as u32 >= points[0], "{cues:?}");
    assert!(
        cues.iter()
            .all(|c| ["paddle", "wall", "score", "win"].contains(&c.as_str()))
    );
}

#[test]
fn cues_repeat_exactly_for_the_same_seed() {
    assert_eq!(play(7), play(7));
}

#[test]
fn watching_for_cues_does_not_change_the_simulation() {
    let (_, shown_hash, _) = play(3);
    let mut bare = Pong::new(PongConfig::new(3)).unwrap();
    while bare.state() != GameState::GameOver {
        let input = bare.bot(Side::Left);
        bare.step(input);
    }
    assert_eq!(bare.state_hash(), shown_hash);
}

#[test]
fn taking_cues_drains_them() {
    let mut shown = Shown::new(Pong::new(PongConfig::new(1)).unwrap());
    while shown.pong.state() != GameState::GameOver {
        let input = shown.pong.bot(Side::Left);
        shown.step(input);
        if !shown.take_cues().is_empty() {
            assert!(shown.take_cues().is_empty());
            return;
        }
    }
    panic!("a full game produced no cues");
}
