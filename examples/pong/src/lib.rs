//! Pong with every game rule in Python (`scripts/pong.py`), on the Flecs
//! binding (`sage_ecs_py`).
//!
//! The host provides only engine vocabulary, as reflected native components
//! (`sage.collision.Position`, `Collider`, the `ContactBegin` event, and the
//! `sage.Input` and `sage.Clock` singletons), plus a native `collision` flecs
//! module the script instantiates inside its `play` module. Everything else
//! (components, phases, flecs modules, systems, observers, entities and the
//! game-state machine) is declared by the script.

use flecs_ecs::prelude::*;
use sage_collision::ContactTable;
use sage_core::host::{DrawKind, DrawRect, Frame};
use sage_core::{Simulation, StableId, StateHasher, TICK_DT, TickInput};
use sage_ecs_py::{Ecs, Policy};
use sage_script::Script;

pub mod assets;
mod shown;

pub use shown::Shown;

pub const SCRIPT: &str = include_str!("../scripts/pong.py");

/// Test-only: a Python system that uses the `random` module, which CPython
/// seeds from the OS. The determinism harness must catch it (S3).
pub const CANARY: &str = r#"
import random
from sage.collision import Position

@world.system(phase="pong.SimMove", query="Ball")
def nondeterminism_canary(pos: Position):
    pos.y += random.random()
"#;

// Court layout the host needs for drawing and the bot (the rules have their
// own copy in the script).
pub const COURT_W: f32 = 160.0;
pub const COURT_H: f32 = 90.0;
pub const PADDLE_SPEED: f32 = 90.0;

pub const LEFT_UP: u32 = 1 << 0;
pub const LEFT_DOWN: u32 = 1 << 1;
pub const RIGHT_UP: u32 = 1 << 2;
pub const RIGHT_DOWN: u32 = 1 << 3;

pub const ID_GAME: u64 = 2;
pub const ID_BALL: u64 = 40;
pub const ID_PADDLE_LEFT: u64 = 30;
pub const ID_PADDLE_RIGHT: u64 = 31;
pub const ID_PLAYER_LEFT: u64 = 50;
pub const ID_PLAYER_RIGHT: u64 = 51;
const DRAWN: [u64; 7] = [10, 11, 20, 21, 30, 31, 40];

// ----- native vocabulary --------------------------------------------------------
// Shared with the platformer kit: see `sage_collision::ecs`.

pub use sage_collision::ecs::{
    Clock, Collider, ContactBegin, Contacts, Input, Position, Vocabulary,
};

// ----- the simulation ---------------------------------------------------------------

pub struct PongConfig {
    pub seed: u64,
    pub script: String,
    pub nondeterminism_canary: bool,
}

impl PongConfig {
    pub fn new(seed: u64) -> Self {
        PongConfig {
            seed,
            script: SCRIPT.to_string(),
            nondeterminism_canary: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Serving,
    Playing,
    GameOver,
}

impl GameState {
    pub fn name(self) -> &'static str {
        match self {
            GameState::Serving => "Serving",
            GameState::Playing => "Playing",
            GameState::GameOver => "GameOver",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn name(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// Ids of script-defined components the host reads through reflection.
struct Ids {
    game: u64,
    player: u64,
    ball: u64,
    paddle: u64,
    wall: u64,
    goal_side: u64,
}

/// The state at one tick: everything the binding knows plus the contact
/// table (including the previous tick's set, 8.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub ecs: sage_ecs_py::Snapshot,
    pub contacts: ContactTable,
}

/// Fields drop in order: the binding, then the scripts, then the world.
pub struct Pong {
    ecs: Ecs,
    _scripts: Vec<Script>,
    ids: Ids,
    world: World,
}

impl Pong {
    pub fn new(config: PongConfig) -> Result<Self, String> {
        let world = World::new();
        let vocab = Vocabulary::define(&world);
        let ecs = Ecs::new(&world, config.seed, "pong").map_err(|e| e.0)?;
        vocab.install(&world, &ecs)?;

        let mut scripts = vec![ecs.load("pong", &config.script).map_err(|e| e.0)?];
        if config.nondeterminism_canary {
            scripts.push(ecs.load("canary", CANARY).map_err(|e| e.0)?);
        }
        let look = |name: &str| -> Result<u64, String> {
            match ecs.lookup_path(&format!("pong.{name}")) {
                0 => Err(format!("the script defines no {name} component")),
                id => Ok(id),
            }
        };
        let ids = Ids {
            game: look("Game")?,
            player: look("Player")?,
            ball: look("Ball")?,
            paddle: look("Paddle")?,
            wall: look("Wall")?,
            goal_side: look("GoalSide")?,
        };
        Ok(Pong {
            ecs,
            _scripts: scripts,
            ids,
            world,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn ecs(&self) -> &Ecs {
        &self.ecs
    }

    fn entity(&self, sid: u64) -> u64 {
        self.ecs
            .entity_of_sid(sid)
            .unwrap_or_else(|| panic!("no entity with StableId {sid}"))
    }

    fn game_field(&self, name: &str) -> f64 {
        self.ecs
            .field(self.entity(ID_GAME), self.ids.game, name)
            .expect("the game entity has a Game component")
    }

    pub fn state(&self) -> GameState {
        match self.game_field("state") as u8 {
            0 => GameState::Serving,
            1 => GameState::Playing,
            _ => GameState::GameOver,
        }
    }

    pub fn winner(&self) -> Option<Side> {
        match self.game_field("winner") as u8 {
            0 => Some(Side::Left),
            1 => Some(Side::Right),
            _ => None,
        }
    }

    pub fn points(&self) -> [u32; 2] {
        [ID_PLAYER_LEFT, ID_PLAYER_RIGHT].map(|sid| {
            self.ecs
                .field(self.entity(sid), self.ids.player, "points")
                .expect("players have Player") as u32
        })
    }

    pub fn play_enabled(&self) -> bool {
        self.ecs
            .modules()
            .into_iter()
            .any(|(path, on)| path == "pong.play" && on)
    }

    fn pos(&self, sid: u64) -> Position {
        self.world
            .entity_from_id(self.entity(sid))
            .get::<&Position>(|p| *p)
    }

    /// A deterministic bot: holds the button that moves
    /// `side`'s paddle towards the ball.
    pub fn bot(&self, side: Side) -> TickInput {
        let (paddle, up, down) = match side {
            Side::Left => (ID_PADDLE_LEFT, LEFT_UP, LEFT_DOWN),
            Side::Right => (ID_PADDLE_RIGHT, RIGHT_UP, RIGHT_DOWN),
        };
        let dy = self.pos(ID_BALL).y - self.pos(paddle).y;
        let dead_zone = PADDLE_SPEED * TICK_DT;
        TickInput::EMPTY
            .with(up, dy > dead_zone)
            .with(down, dy < -dead_zone)
    }

    /// Contacts that began during the last tick, as StableId pairs in canonical
    /// order. Presentation uses them to pick sound cues; they never feed back.
    pub fn last_contact_begins(&self) -> Vec<(u64, u64)> {
        self.world.get::<&Contacts>(|c| {
            c.0.transitions()
                .begin
                .iter()
                .map(|c| (c.a.0, c.b.0))
                .collect()
        })
    }

    /// A read-only snapshot for presentation (the host interface).
    pub fn frame(&self) -> Frame {
        let mut rects = vec![DrawRect {
            id: StableId(1),
            kind: DrawKind::Court,
            center: [COURT_W / 2.0, COURT_H / 2.0],
            half: [COURT_W / 2.0, COURT_H / 2.0],
        }];
        for sid in DRAWN {
            let e = self.entity(sid);
            let kind = if self.ecs.has(e, self.ids.ball) {
                DrawKind::Ball
            } else if self.ecs.has(e, self.ids.paddle) {
                DrawKind::Paddle
            } else if self.ecs.has(e, self.ids.wall) {
                DrawKind::Wall
            } else if self.ecs.has(e, self.ids.goal_side) {
                DrawKind::Goal
            } else {
                continue;
            };
            if let Some((p, c)) = self
                .world
                .entity_from_id(e)
                .try_get::<(&Position, &Collider)>(|(p, c)| (*p, *c))
            {
                rects.push(DrawRect {
                    id: StableId(sid),
                    kind,
                    center: [p.x, p.y],
                    half: [c.hw, c.hh],
                });
            }
        }
        let [l, r] = self.points();
        let mut hud = vec![format!("{l}  :  {r}"), self.state().name().to_string()];
        if let Some(w) = self.winner() {
            hud.push(format!("winner: {}", w.name()));
        }
        Frame {
            tick: self.tick(),
            world_size: [COURT_W, COURT_H],
            rects,
            hud,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            ecs: self.ecs.snapshot(),
            contacts: self.world.get::<&Contacts>(|c| c.0.clone()),
        }
    }

    /// A `sage-save v1` document: everything the binding knows plus the
    /// contact table, which it does not (the previous tick's contacts decide
    /// what counts as a new begin).
    pub fn save(&self) -> String {
        let contacts = self.world.get::<&Contacts>(|c| c.0.to_text());
        self.ecs
            .save_game([("contacts".to_string(), contacts)].into())
    }

    /// Loads a save game into a `Pong` built with the same config. Play
    /// continues exactly as the saved game would have.
    pub fn load(&mut self, text: &str) -> Result<(), String> {
        let loaded = self.ecs.load_game(text, Policy::Strict)?;
        let contacts = loaded
            .extra
            .get("contacts")
            .ok_or("the save game has no contact table")?;
        self.world.set(Contacts(ContactTable::from_text(contacts)?));
        Ok(())
    }

    /// Restores a snapshot taken from a `Pong` built with the same config.
    pub fn restore(&mut self, s: &Snapshot) -> Result<(), String> {
        self.ecs.restore(&s.ecs)?;
        self.world.set(Contacts(s.contacts.clone()));
        Ok(())
    }
}

impl Simulation for Pong {
    fn step(&mut self, input: TickInput) {
        self.world.set(Input {
            buttons: input.buttons,
        });
        self.world.get::<&mut Clock>(|c| c.tick += 1);
        self.world.progress_time(TICK_DT);
        if let Some(fault) = self.ecs.fault() {
            panic!("script error in tick {}: {fault}", self.tick());
        }
    }

    fn tick(&self) -> u64 {
        self.world.get::<&Clock>(|c| c.tick)
    }

    /// Everything the binding knows (entities, components, relationships,
    /// singletons, flecs modules, RNG), then the native contact table.
    fn state_hash(&self) -> u64 {
        let mut h = StateHasher::new();
        self.ecs.hash_into(&mut h);
        self.world.get::<&Contacts>(|c| c.0.hash_into(&mut h));
        h.finish()
    }
}
