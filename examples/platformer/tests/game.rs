//! The platformer example as a whole: a bot plays the level to the end, the
//! game replays exactly (in this process and another), and the sound cues
//! match what happened.

use std::process::Command;

use platformer::{Bot, Game, assets, config, level_picture};
use sage_core::{InputRecording, Simulation, replay_twice, run_recording};
use sage_platform::game_window::GameApp;
use sage_platformer::Platformer;

/// The bot plays until it reaches the right end of the level.
fn bot_game(seed: u64) -> (Platformer, InputRecording) {
    let mut p = Platformer::new(config(seed)).unwrap();
    let mut bot = Bot::new(level_picture());
    let goal = bot.size()[0] - 48.0;
    let mut rec = InputRecording::new(seed);
    while p.player_position().x < goal {
        assert!(
            p.tick() < 60 * 120,
            "the bot did not finish: x={}",
            p.player_position().x
        );
        let input = bot.input(&p);
        rec.record(p.tick(), input);
        p.step(input);
    }
    (p, rec)
}

#[test]
fn the_level_can_be_played_to_the_end_without_dying() {
    let (p, _) = bot_game(1);
    assert_eq!(p.deaths(), 0);
    assert!(p.coins() >= 10, "only {} coins on the way", p.coins());
    assert!(p.tick() < 60 * 40, "{} ticks is a slow level", p.tick());
    // The game's own rule paid for hearts: ten coins make one.
    assert!(p.health().1 >= 4, "{:?}", p.health());
}

#[test]
fn the_bots_game_replays_identically_twice() {
    let (_, rec) = bot_game(3);
    assert_eq!(
        replay_twice(|seed| Platformer::new(config(seed)).unwrap(), &rec),
        None
    );
}

#[test]
fn a_recording_replays_in_another_process_with_the_same_hashes() {
    let dir = std::env::temp_dir().join(format!("platformer-replay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = env!("CARGO_BIN_EXE_platformer");
    let rec = dir.join("run.rec");
    let first = Command::new(exe)
        .args(["headless", "--seed", "4", "--record"])
        .arg(&rec)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let replay = |name: &str| {
        let hashes = dir.join(name);
        let out = Command::new(exe)
            .arg("replay")
            .arg(&rec)
            .arg("--hashes")
            .arg(&hashes)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        (
            String::from_utf8(out.stdout).unwrap(),
            std::fs::read_to_string(hashes).unwrap(),
        )
    };
    let (a, b) = (replay("a.txt"), replay("b.txt"));
    assert_eq!(a, b);
    // And the replay ends in the state the recording run ended in.
    assert_eq!(String::from_utf8(first.stdout).unwrap().trim(), a.0.trim());
    // The same recording, replayed in this process, gives the same hashes too.
    let text = std::fs::read_to_string(&rec).unwrap();
    let recording = InputRecording::from_text(&text).unwrap();
    let mut here = Platformer::new(config(recording.seed)).unwrap();
    let trace = run_recording(&mut here, &recording);
    let last = format!("{:#018x}", trace.hashes.last().unwrap());
    assert!(
        a.1.trim_end().ends_with(&last),
        "{} vs {last}",
        a.1.lines().last().unwrap()
    );
    std::fs::remove_dir_all(dir).ok();
}

/// Plays the bot's game through `Game`, collecting cues.
fn cues_of(seed: u64) -> (Vec<String>, u64) {
    let mut g = Game::new(seed).unwrap();
    let mut bot = Bot::new(level_picture());
    let goal = bot.size()[0] - 48.0;
    let mut cues = vec![];
    while g.sim.player_position().x < goal {
        assert!(g.tick() < 60 * 120);
        let input = bot.input(&g.sim);
        g.step(input);
        cues.extend(g.take_cues());
    }
    (cues, g.state_hash())
}

#[test]
fn the_cues_of_a_run_are_the_things_that_happened() {
    let (cues, _) = cues_of(1);
    let count = |name: &str| cues.iter().filter(|c| *c == name).count();
    assert!(count("jump") >= 8, "{cues:?}");
    assert!(count("land") >= 8);
    assert!(count("coin") >= 10);
    assert!(count("checkpoint") >= 1);
    assert_eq!(count("death"), 0);
    // Every cue has a sound file to play.
    let names: Vec<&str> = assets::sounds().iter().map(|s| s.0).collect();
    assert!(cues.iter().all(|c| names.contains(&c.as_str())));
    // And they repeat exactly, and never touch the simulation.
    assert_eq!(cues_of(1), cues_of(1));
    let (p, _) = bot_game(1);
    assert_eq!(cues_of(1).1, p.state_hash());
}

#[test]
fn the_picture_in_the_game_script_is_a_clean_level() {
    let lines: Vec<&str> = level_picture()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with(';'))
        .collect();
    assert!(lines.len() >= 10);
    let width = lines[0].len();
    assert!(lines.iter().all(|l| l.len() == width), "ragged rows");
    let all: String = lines.concat();
    assert_eq!(all.matches('@').count(), 1, "exactly one start");
    assert!(
        all.chars().all(|c| ".#=^ocehv@".contains(c)),
        "unknown character"
    );
    for what in ['#', '=', '^', 'o', 'c', 'e', 'h'] {
        assert!(all.contains(what), "the level never uses {what}");
    }
}
