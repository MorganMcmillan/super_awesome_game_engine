//! Helpers shared by the kit's tests: build a world from a level picture and
//! play scripted input into it.
#![allow(dead_code)]

use sage_core::{Simulation, TickInput};
use sage_platformer::{Platformer, PlatformerConfig};

/// A game script that loads `picture` and changes nothing else.
pub fn game(picture: &str) -> Vec<(String, String)> {
    vec![(
        "game".to_string(),
        format!("from sage import world\nkit = world.platformer\nkit.load_level({picture:?})\n"),
    )]
}

pub fn with(picture: &str) -> Platformer {
    Platformer::new(PlatformerConfig::new(1, game(picture))).unwrap()
}

/// A game script with extra Python after the level is loaded.
pub fn with_script(picture: &str, extra: &str) -> Platformer {
    let mut scripts = game(picture);
    scripts[0].1.push_str(extra);
    Platformer::new(PlatformerConfig::new(1, scripts)).unwrap()
}

/// Holds `buttons` for `ticks` ticks.
pub fn hold(p: &mut Platformer, buttons: u32, ticks: u32) {
    for _ in 0..ticks {
        p.step(TickInput { buttons });
    }
}

/// Plays `(buttons, ticks)` pairs in order.
pub fn play(p: &mut Platformer, script: &[(u32, u32)]) {
    for &(b, n) in script {
        hold(p, b, n);
    }
}

/// A flat floor with the player standing on it, plenty of room around.
pub const FLAT: &str = "
....................................
....................................
....................................
....................................
.........@..........................
####################################
####################################
";
