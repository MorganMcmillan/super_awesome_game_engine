//! The test of the kit idea (design doc 7): the *diff* between two games
//! made from it should be small. A whole other game here is a handful of
//! lines on top of the kit.

use sage_core::{Simulation, TickInput};
use sage_platformer::buttons::*;
use sage_platformer::{Platformer, PlatformerConfig};

/// A game on the moon: weak gravity, a different sky and one room. Nothing
/// of the kit is copied; the player, camera, enemies and coins are all
/// still there.
const MOON: &str = r#"
from sage import world
from sage.platformer import Backdrop
kit = world.platformer
kit.tune(gravity_up=260.0, gravity_down=420.0, max_fall=200.0, run_speed=110.0)
world.set(Backdrop(sky_top=kit.color(8, 8, 24), sky_bottom=kit.color(40, 24, 64), far=kit.color(70, 70, 90), near=kit.color(110, 110, 130)))
kit.load_level("""
......................
......................
......................
......................
......................
......................
......................
.@.....o....e.........
######################
""")
"#;

fn rise(script: &str) -> f32 {
    let mut p = Platformer::new(PlatformerConfig::new(
        1,
        vec![("game".into(), script.into())],
    ))
    .unwrap();
    for _ in 0..40 {
        p.step(TickInput::EMPTY);
    }
    let floor = p.player_position().y;
    let mut top = floor;
    for _ in 0..90 {
        p.step(TickInput { buttons: JUMP });
        top = top.max(p.player_position().y);
    }
    top - floor
}

#[test]
fn a_second_game_is_a_few_lines_and_plays_differently() {
    let lines = MOON.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(lines < 25, "a whole game in {lines} lines");
    // Moon gravity: a jump rises far higher than in grassland (about 70 units).
    let moon = rise(MOON);
    let grass = rise(platformer::SCRIPT);
    assert!(moon > grass * 2.0, "moon {moon} vs grassland {grass}");
}

#[test]
fn the_moon_game_has_everything_the_kit_gives_it() {
    let mut p =
        Platformer::new(PlatformerConfig::new(1, vec![("moon".into(), MOON.into())])).unwrap();
    for _ in 0..120 {
        p.step(TickInput { buttons: RIGHT });
    }
    assert!(p.coins() >= 1, "the coin was collected");
    for _ in 0..200 {
        p.step(TickInput {
            buttons: RIGHT | JUMP,
        });
    }
    assert_eq!(p.camera().w, 320.0);
    assert!(p.frame().draws.len() > 20);
}

#[test]
fn bad_tuning_names_and_bad_levels_are_errors_with_a_message() {
    let load = |extra: &str| {
        let src = format!("from sage import world\nkit = world.platformer\n{extra}");
        Platformer::new(PlatformerConfig::new(1, vec![("bad".into(), src)])).err()
    };
    let e = load("kit.tune(gravty=3)\nkit.load_level('@')").unwrap();
    assert!(e.contains("no tuning number called 'gravty'"), "{e}");
    let e = load("kit.load_level('..')").unwrap();
    assert!(e.contains("no player start"), "{e}");
    let e = load("kit.load_level('@.@')").unwrap();
    assert!(e.contains("two player starts"), "{e}");
    let e = load("kit.load_level('@?')").unwrap();
    assert!(
        e.contains("unknown character '?'") && e.contains("row 1, column 2"),
        "{e}"
    );
    let e = load("kit.load_level('   ')").unwrap();
    assert!(e.contains("empty"), "{e}");
}
