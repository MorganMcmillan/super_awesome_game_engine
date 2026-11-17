//! The parts of the level the bot cannot play: the wall-jump chimney, and
//! what the picture's characters turn into.

use platformer::{Bot, config, level_picture};
use sage_core::{Simulation, TickInput};
use sage_platformer::Platformer;
use sage_platformer::buttons::*;

/// Runs the bot up to column `col`, then stops (standing still).
fn at_column(col: f32) -> Platformer {
    let mut p = Platformer::new(config(1)).unwrap();
    let mut bot = Bot::new(level_picture());
    while p.player_position().x < col * 16.0 {
        let input = bot.input(&p);
        p.step(input);
    }
    p
}

#[test]
fn the_chimney_can_be_climbed_by_wall_jumping() {
    // The chimney's walls are at columns 92 and 96; its coins at column 94.
    let mut p = at_column(88.0);
    // Stand still under the middle of the shaft, then jump into it.
    for _ in 0..60 {
        p.step(TickInput::EMPTY);
    }
    let mut x = p.player_position().x;
    let target = 94.0 * 16.0 + 8.0;
    while (x - target).abs() > 3.0 {
        let b = if x < target { RIGHT } else { LEFT };
        p.step(TickInput { buttons: b });
        x = p.player_position().x;
    }
    // A small controller: jump when grounded; in the air lean toward the
    // wall on the side we are not on; on a wall, kick off (a fresh press).
    let mut top = 0.0f32;
    let mut last_jump = false;
    let mut lean = RIGHT;
    for _ in 0..600 {
        let wall = p.player_body().wall;
        let mode = p.mode_name();
        let mut buttons = 0;
        if mode == "wall" {
            if last_jump {
                // Let go of JUMP for a tick (a kick is a fresh press), still
                // pushing into the wall.
                buttons |= if wall > 0 { RIGHT } else { LEFT };
            } else {
                // Kick off, away from the wall, toward the other one.
                lean = if wall > 0 { LEFT } else { RIGHT };
                buttons |= lean | JUMP;
            }
        } else if p.player_body().grounded == 1 {
            buttons |= JUMP | lean;
        } else {
            // Hold JUMP while rising for the full kick; lean toward the wall.
            buttons |= lean | if p.player_velocity().y > 0.0 { JUMP } else { 0 };
        }
        last_jump = buttons & JUMP != 0;
        if std::env::var_os("TRACE").is_some() {
            let q = p.player_position();
            println!(
                "{:>4} {:>6.1} {:>6.1} {} wall={} v=({:.0},{:.0})",
                p.tick(),
                q.x,
                q.y,
                mode,
                wall,
                p.player_velocity().x,
                p.player_velocity().y
            );
        }
        p.step(TickInput { buttons });
        top = top.max(p.player_position().y);
    }
    // The chimney's walls run from y = 96 to 208 (rows 2 to 8 of 14); its
    // top platform is at about y = 205. Reaching above 150 needs a kick.
    assert!(p.tally()[2] >= 1, "no wall jump happened: {:?}", p.tally());
    assert!(top > 150.0, "only climbed to {top}");
}

#[test]
fn the_pictures_characters_become_what_they_say() {
    let p = Platformer::new(config(1)).unwrap();
    // Entities in reading order from FIRST_TILE: the left invisible wall is
    // last, the player is 10, the face 11.
    let bodies = p.bodies();
    assert!(bodies.iter().any(|b| b.0 == sage_platformer::ID_PLAYER));
    // Player + 5 walkers.
    assert_eq!(bodies.len(), 1 + 5, "{bodies:?}");
    let pos = p.player_position();
    // `@` is in column 3 of row 11 (of 14): its tile's middle is (56, 40), and
    // the player stands on the tile's bottom edge, 7 units up from its feet.
    assert_eq!((pos.x, pos.y), (56.0, 39.0), "start tile");
}
