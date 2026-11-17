//! A platformer made from the platformer kit (`sage_platformer`).
//!
//! The game is `scripts/game.py`: a few tuning numbers, a sky, a level drawn
//! as text, and one rule of its own. Everything else, the player's moves, the
//! enemies, coins, checkpoints, platforms and the camera, is the kit's.
//! This crate adds only what a game needs around that: a window, a keyboard
//! and gamepad map, sounds and a bot for tests.

use sage_assets::Image;
use sage_core::host::UiFrame;
use sage_core::{Simulation, TickInput};
use sage_platform::game_window::GameApp;
use sage_platform::{Binding, InputMap, Key, PadAxis, PadButton};
use sage_platformer::buttons::*;
use sage_platformer::{GameState, Platformer, PlatformerConfig};

pub mod assets;

pub const SCRIPT: &str = include_str!("../scripts/game.py");

/// The level picture inside the game script (between `LEVEL = """` and the
/// closing quotes), so tools and tests read the same text the game loads.
pub fn level_picture() -> &'static str {
    let start =
        SCRIPT.find("LEVEL = \"\"\"").expect("game.py has a LEVEL") + "LEVEL = \"\"\"".len();
    let end = start + SCRIPT[start..].find("\"\"\"").expect("LEVEL ends");
    &SCRIPT[start..end]
}

/// The kit plus this game's script.
pub fn config(seed: u64) -> PlatformerConfig {
    PlatformerConfig::new(seed, vec![("game".into(), SCRIPT.into())])
}

/// Keys and gamepad buttons to the kit's button bits.
pub fn input_map() -> InputMap {
    let pad = |b: PadButton| Binding::PadButton(b);
    InputMap::default()
        .bind(Key::Left, LEFT)
        .bind(Key::A, LEFT)
        .bind(Key::Right, RIGHT)
        .bind(Key::D, RIGHT)
        .bind(Key::Down, DOWN)
        .bind(Key::S, DOWN)
        .bind(Key::Up, UP)
        .bind(Key::W, UP)
        .bind(Key::Space, JUMP)
        .bind(Key::Z, JUMP)
        .bind(Key::LShift, RUN)
        .bind(Key::X, RUN)
        .bind(Key::C, SPIN)
        .bind(Key::V, SPIN)
        .bind(Key::P, PAUSE)
        .bind(Key::Return, PAUSE)
        .bind_input(pad(PadButton::DPadLeft), LEFT)
        .bind_input(pad(PadButton::DPadRight), RIGHT)
        .bind_input(pad(PadButton::DPadDown), DOWN)
        .bind_input(pad(PadButton::DPadUp), UP)
        .bind_input(pad(PadButton::South), JUMP)
        .bind_input(pad(PadButton::West), RUN)
        .bind_input(pad(PadButton::East), SPIN)
        .bind_input(pad(PadButton::Start), PAUSE)
        .bind_input(
            Binding::PadAxis {
                axis: PadAxis::LeftX,
                positive: false,
            },
            LEFT,
        )
        .bind_input(
            Binding::PadAxis {
                axis: PadAxis::LeftX,
                positive: true,
            },
            RIGHT,
        )
        .bind_input(
            Binding::PadAxis {
                axis: PadAxis::LeftY,
                positive: true,
            },
            DOWN,
        )
}

/// The game as the window loop sees it.
pub struct Game {
    pub sim: Platformer,
    images: Vec<(u64, Image)>,
}

impl Game {
    pub fn new(seed: u64) -> Result<Game, String> {
        Ok(Game {
            sim: Platformer::new(config(seed))?,
            images: vec![],
        })
    }

    /// A game around a simulation that already exists.
    pub fn around(sim: Platformer) -> Game {
        Game {
            sim,
            images: vec![],
        }
    }

    /// Registers an image the frame may draw (`Look(image=kit.asset(path))`).
    pub fn with_image(mut self, path: &str, image: Image) -> Game {
        self.images
            .push((sage_assets::AssetId::of_path(path).0, image));
        self
    }
}

impl Simulation for Game {
    fn step(&mut self, input: TickInput) {
        self.sim.step(input);
    }
    fn tick(&self) -> u64 {
        self.sim.tick()
    }
    fn state_hash(&self) -> u64 {
        self.sim.state_hash()
    }
}

impl GameApp for Game {
    fn frame(&self) -> UiFrame {
        self.sim.frame()
    }
    fn images(&self) -> Vec<(u64, Image)> {
        self.images.clone()
    }
    fn take_cues(&mut self) -> Vec<String> {
        self.sim.take_cues()
    }
}

/// A bot that plays the level from its picture: it runs right, and jumps
/// when something ahead needs it (a wall, a gap, spikes, an enemy). It is a
/// test tool, not a player: it cannot wall jump or ride platforms.
pub struct Bot {
    rows: Vec<Vec<u8>>,
    hold: u32,
}

impl Bot {
    pub fn new(picture: &str) -> Bot {
        let lines: Vec<&str> = picture
            .lines()
            .filter(|l| !l.trim_start().starts_with(';'))
            .collect();
        let first = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
        let last = lines
            .iter()
            .rposition(|l| !l.trim().is_empty())
            .unwrap_or(0);
        Bot {
            rows: lines[first..=last]
                .iter()
                .map(|l| l.bytes().collect())
                .collect(),
            hold: 0,
        }
    }

    /// The picture's size in units.
    pub fn size(&self) -> [f32; 2] {
        let w = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        [w as f32 * 16.0, self.rows.len() as f32 * 16.0]
    }

    /// What is at tile `(col, up)`, where `up` counts rows from the bottom.
    fn tile(&self, col: i64, up: i64) -> u8 {
        let n = self.rows.len() as i64;
        let row = n - 1 - up;
        if col < 0 || row < 0 || row >= n {
            return b'.';
        }
        *self.rows[row as usize].get(col as usize).unwrap_or(&b'.')
    }

    pub fn input(&mut self, p: &Platformer) -> TickInput {
        if p.state() != GameState::Playing {
            return TickInput::EMPTY;
        }
        let pos = p.player_position();
        let feet = pos.y - p.player_collider().hh;
        let (col, up) = (
            (pos.x / 16.0).floor() as i64,
            ((feet + 1.0) / 16.0).floor() as i64,
        );
        let grounded = p.player_body().grounded == 1;
        let solid = |c: u8| c == b'#' || c == b'=';
        let mut jump = false;
        if grounded && self.hold == 0 {
            for ahead in 1..=3 {
                let t = self.tile(col + ahead, up);
                let t_up = self.tile(col + ahead, up + 1);
                if (ahead <= 2 && (t == b'#' || t_up == b'#')) || t == b'^' {
                    jump = true;
                }
            }
            // An enemy close ahead (they are not where the picture put them:
            // they walk).
            let near = p.bodies().into_iter().any(|(sid, e)| {
                sid != sage_platformer::ID_PLAYER
                    && (e.x - pos.x) > 0.0
                    && (e.x - pos.x) < 56.0
                    && (e.y - pos.y).abs() < 24.0
            });
            jump |= near;
            // A gap: nothing to stand on in the next tile.
            if !solid(self.tile(col + 1, up - 1)) && !solid(self.tile(col + 1, up - 2)) {
                jump = true;
            }
            if jump {
                self.hold = 30;
            }
        }
        let mut buttons = RIGHT | RUN;
        if self.hold > 0 {
            self.hold -= 1;
            buttons |= JUMP;
        }
        TickInput { buttons }
    }
}
