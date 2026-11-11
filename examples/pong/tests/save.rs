//! Save games (Milestone 2): saving and loading must not change the future.

use pong::{GameState, Pong, PongConfig, Side};
use sage_core::{Simulation, TickInput};

/// The bot's inputs for a whole game, with the hash after every tick.
fn full_game(seed: u64) -> (Vec<TickInput>, Vec<u64>) {
    let mut p = Pong::new(PongConfig::new(seed)).unwrap();
    let (mut inputs, mut hashes) = (vec![], vec![p.state_hash()]);
    while p.state() != GameState::GameOver {
        assert!(p.tick() < 60 * 60 * 10, "game did not finish");
        let input = p.bot(Side::Left);
        inputs.push(input);
        p.step(input);
        hashes.push(p.state_hash());
    }
    (inputs, hashes)
}

#[test]
fn loading_a_save_reproduces_every_later_hash() {
    let seed = 11;
    let (inputs, hashes) = full_game(seed);
    for n in [
        0usize,
        1,
        59,
        60,
        61,
        200,
        inputs.len() / 2,
        inputs.len() - 1,
    ] {
        let mut a = Pong::new(PongConfig::new(seed)).unwrap();
        for input in &inputs[..n] {
            a.step(*input);
        }
        let text = a.save();
        assert!(text.starts_with("sage-save v1\n"));
        // A fresh game that has run for a different time.
        let mut b = Pong::new(PongConfig::new(seed)).unwrap();
        for _ in 0..7 {
            b.step(TickInput::EMPTY);
        }
        b.load(&text).unwrap();
        assert_eq!(
            b.state_hash(),
            hashes[n],
            "hash right after loading at tick {n}"
        );
        for (i, input) in inputs[n..].iter().enumerate() {
            b.step(*input);
            assert_eq!(
                b.state_hash(),
                hashes[n + i + 1],
                "loaded at tick {n}, diverged {} ticks later",
                i + 1
            );
        }
        assert_eq!(b.state(), GameState::GameOver);
        // Saving is deterministic text.
        let mut c = Pong::new(PongConfig::new(seed)).unwrap();
        c.load(&text).unwrap();
        assert_eq!(c.save(), text);
    }
}

#[test]
fn a_save_taken_during_a_contact_keeps_the_previous_contact_set() {
    let seed = 5;
    let mut p = Pong::new(PongConfig::new(seed)).unwrap();
    let mut saved = None;
    while p.state() != GameState::GameOver && saved.is_none() {
        let input = p.bot(Side::Left);
        p.step(input);
        // The tick a contact began is the one whose table matters most.
        if !p.last_contact_begins().is_empty() {
            saved = Some((p.save(), p.tick()));
        }
    }
    let (text, tick) = saved.expect("a contact began in a full game");
    assert!(
        text.contains("contacts \"") && text.contains("now "),
        "{text}"
    );
    let mut b = Pong::new(PongConfig::new(seed)).unwrap();
    b.load(&text).unwrap();
    assert_eq!(b.tick(), tick);
    // Without the previous set the next tick would report the contact as a
    // new begin and bounce again; the loaded game must match the original.
    let input = p.bot(Side::Left);
    p.step(input);
    b.step(input);
    assert_eq!(b.state_hash(), p.state_hash());
    assert_eq!(b.last_contact_begins(), p.last_contact_begins());
}

#[test]
fn a_damaged_save_is_an_error_not_a_guess() {
    let mut p = Pong::new(PongConfig::new(1)).unwrap();
    for _ in 0..90 {
        p.step(TickInput::EMPTY);
    }
    let text = p.save();
    let mut q = Pong::new(PongConfig::new(1)).unwrap();
    assert!(q.load("not a save").is_err());
    let no_contacts: String = text
        .lines()
        .filter(|l| !l.starts_with("extra "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(q.load(&no_contacts).unwrap_err().contains("contact table"));
    let err = q
        .load(&text.replace("module pong.play 1", "module pong.nope 1"))
        .unwrap_err();
    assert!(err.contains("pong.nope"), "{err}");
    // A scene is not a save game.
    let scene = p.ecs().save_scene();
    assert!(q.load(&scene).unwrap_err().contains("scene"));
}
