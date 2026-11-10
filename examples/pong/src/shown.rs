//! Pong as the window loop sees it: the simulation plus the sound cues that
//! its last tick caused.
//!
//! Cues are derived by reading simulation state before and after a step, so
//! they cannot affect a replay (design doc 10.2: audio is presentation).

use sage_core::host::Frame;
use sage_core::{Simulation, TickInput};
use sage_platform::Presentable;

use crate::{GameState, ID_BALL, ID_PADDLE_LEFT, ID_PADDLE_RIGHT, Pong};

const ID_WALL_TOP: u64 = 10;
const ID_WALL_BOTTOM: u64 = 11;

pub struct Shown {
    pub pong: Pong,
    cues: Vec<String>,
}

impl Shown {
    pub fn new(pong: Pong) -> Shown {
        Shown { pong, cues: vec![] }
    }
}

impl Simulation for Shown {
    fn step(&mut self, input: TickInput) {
        let (points, state) = (self.pong.points(), self.pong.state());
        self.pong.step(input);
        // Contacts are canonical pairs (a < b), and the ball (40) is above every
        // wall and paddle, so `a` is the thing the ball hit. Goals (20, 21) cue
        // through scoring instead.
        for (a, b) in self.pong.last_contact_begins() {
            debug_assert_eq!(b, ID_BALL);
            match a {
                ID_PADDLE_LEFT | ID_PADDLE_RIGHT => self.cues.push("paddle".into()),
                ID_WALL_TOP | ID_WALL_BOTTOM => self.cues.push("wall".into()),
                _ => {}
            }
        }
        if self.pong.points() != points {
            self.cues.push("score".into());
        }
        if state != GameState::GameOver && self.pong.state() == GameState::GameOver {
            self.cues.push("win".into());
        }
    }

    fn tick(&self) -> u64 {
        self.pong.tick()
    }

    fn state_hash(&self) -> u64 {
        self.pong.state_hash()
    }
}

impl Presentable for Shown {
    fn frame(&self) -> Frame {
        self.pong.frame()
    }

    fn take_cues(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cues)
    }
}
