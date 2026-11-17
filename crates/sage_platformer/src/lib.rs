//! The platformer genre kit (design doc 7 and 13.3, Milestone 5).
//!
//! A kit is a package of vocabulary, rules and assets that a game starts from
//! and then changes. This one is split the way the design asks (4.1, 8.1):
//!
//! - **Native, because it must be fast and exact:** the shared collision
//!   vocabulary (`sage.collision`), the `collision` flecs module that
//!   announces contacts, and here `sage.platformer`: `Velocity`, `Body`,
//!   `Solid`, `Look`, `Camera`, `Backdrop`, with the `kinematics` flecs
//!   module that moves bodies and collides them with solids ([`sweep`] is its
//!   pure core).
//! - **Python, because it is the game:** the five scripts in `scripts/`
//!   (components, tuning, the player, enemies and pickups, levels from text,
//!   and the state machine and camera). A game script loads after them and
//!   changes what it likes.
//!
//! [`Platformer`] is the simulation: it loads the kit and a game's scripts
//! into one world, steps at the fixed tick, and describes itself to
//! presentation as a [`UiFrame`] and a list of sound cues. This crate does not
//! depend on any window or audio device.

use flecs_ecs::prelude::*;
use sage_collision::ContactTable;
use sage_collision::ecs::{Clock, Collider, Contacts, Input, Position, Vocabulary};
use sage_core::host::UiFrame;
use sage_core::{Simulation, StateHasher, TICK_DT, TickInput};
use sage_ecs_py::{Ecs, Policy, StableIdC};
use sage_script::Script;

pub mod frame;
mod kinematics;
pub mod sweep;
pub mod vocab;

pub use frame::Hud;
pub use vocab::{Backdrop, Body, Camera, Look, Solid, Velocity};

/// The button bits the kit's rules read. A host's input map sets them.
pub mod buttons {
    pub const LEFT: u32 = 1 << 0;
    pub const RIGHT: u32 = 1 << 1;
    pub const DOWN: u32 = 1 << 2;
    pub const UP: u32 = 1 << 3;
    pub const JUMP: u32 = 1 << 4;
    pub const RUN: u32 = 1 << 5;
    pub const SPIN: u32 = 1 << 6;
    pub const PAUSE: u32 = 1 << 7;
}

pub const VOCABULARY: &str = include_str!("../scripts/vocabulary.py");
pub const PLAYER: &str = include_str!("../scripts/player.py");
pub const ACTORS: &str = include_str!("../scripts/actors.py");
pub const LEVEL: &str = include_str!("../scripts/level.py");
pub const RULES: &str = include_str!("../scripts/rules.py");

/// Every kit script, in load order, by name.
pub const KIT: &[(&str, &str)] = &[
    ("platformer_vocabulary", VOCABULARY),
    ("platformer_player", PLAYER),
    ("platformer_actors", ACTORS),
    ("platformer_level", LEVEL),
    ("platformer_rules", RULES),
];

/// The flecs module the scripts (the kit's and the game's) live in.
pub const MODULE: &str = "platformer";

pub const ID_GAME: u64 = 2;
pub const ID_PLAYER: u64 = 10;

/// The kit's sound cues: the `Tally` counter that counts it, and the cue's
/// name. Presentation plays a sound for each increase, so the simulation never
/// talks to the speakers (design doc 10.2).
pub const CUES: [(&str, &str); 11] = [
    ("jumps", "jump"),
    ("lands", "land"),
    ("wall_jumps", "wall_jump"),
    ("rolls", "roll"),
    ("dives", "dive"),
    ("spins", "spin"),
    ("coins", "coin"),
    ("stomps", "stomp"),
    ("hurts", "hurt"),
    ("checkpoints", "checkpoint"),
    ("deaths", "death"),
];

/// A test-only script: a Python system that uses the `random` module, which
/// CPython seeds from the OS. The determinism harness must catch it.
pub const CANARY: &str = r#"
import random
from sage.collision import Position
from sage import world

kit = world.platformer

@world.system(phase="platformer.SimMove", query="Player")
def nondeterminism_canary(pos: Position):
    pos.x += random.random() * 0.01
"#;

pub struct PlatformerConfig {
    pub seed: u64,
    /// The game's scripts, `(name, source)`, loaded in order after the kit.
    pub scripts: Vec<(String, String)>,
    pub nondeterminism_canary: bool,
}

impl PlatformerConfig {
    pub fn new(seed: u64, scripts: Vec<(String, String)>) -> Self {
        PlatformerConfig {
            seed,
            scripts,
            nondeterminism_canary: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Playing,
    Paused,
    Dead,
}

/// Ids of script-defined components the host reads through reflection.
struct Ids {
    game: u64,
    player: u64,
    health: u64,
    score: u64,
    tally: u64,
}

/// The state at one tick: everything the binding knows plus the contact table.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub ecs: sage_ecs_py::Snapshot,
    pub contacts: ContactTable,
}

/// Fields drop in order: the binding, then the scripts, then the world.
pub struct Platformer {
    ecs: Ecs,
    _scripts: Vec<Script>,
    ids: Ids,
    world: World,
    /// The `Tally` counters last seen, and the cues since `take_cues`.
    seen: [u32; CUES.len()],
    cues: Vec<String>,
}

impl Platformer {
    pub fn new(config: PlatformerConfig) -> Result<Self, String> {
        let world = World::new();
        let collision = Vocabulary::define(&world);
        let ecs = Ecs::new(&world, config.seed, MODULE).map_err(|e| e.0)?;
        collision.install(&world, &ecs)?;
        vocab::register(&world, &ecs)?;
        ecs.add_native_module("kinematics", kinematics::install_kinematics);

        let mut scripts = vec![];
        for (name, source) in KIT {
            scripts.push(
                ecs.load(name, source)
                    .map_err(|e| format!("kit script {name}: {}", e.0))?,
            );
        }
        for (name, source) in &config.scripts {
            scripts.push(
                ecs.load(name, source)
                    .map_err(|e| format!("script {name}: {}", e.0))?,
            );
        }
        if config.nondeterminism_canary {
            scripts.push(ecs.load("canary", CANARY).map_err(|e| e.0)?);
        }
        let look = |name: &str| -> Result<u64, String> {
            match ecs.lookup_path(&format!("{MODULE}.{name}")) {
                0 => Err(format!("the kit defines no {name} component")),
                id => Ok(id),
            }
        };
        let ids = Ids {
            game: look("Game")?,
            player: look("Player")?,
            health: look("Health")?,
            score: look("Score")?,
            tally: look("Tally")?,
        };
        let mut p = Platformer {
            ecs,
            _scripts: scripts,
            ids,
            world,
            seen: [0; CUES.len()],
            cues: vec![],
        };
        p.seen = p.tally();
        Ok(p)
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

    fn field(&self, sid: u64, component: u64, name: &str) -> f64 {
        self.ecs
            .field(self.entity(sid), component, name)
            .unwrap_or_else(|| panic!("entity {sid} has no field {name}"))
    }

    pub fn state(&self) -> GameState {
        match self.field(ID_GAME, self.ids.game, "state") as u32 {
            0 => GameState::Playing,
            1 => GameState::Paused,
            _ => GameState::Dead,
        }
    }

    pub fn play_enabled(&self) -> bool {
        self.ecs
            .modules()
            .into_iter()
            .any(|(path, on)| path == format!("{MODULE}.play") && on)
    }

    /// `(hp, max)` of the player.
    pub fn health(&self) -> (u32, u32) {
        (
            self.field(ID_PLAYER, self.ids.health, "hp") as u32,
            self.field(ID_PLAYER, self.ids.health, "max") as u32,
        )
    }

    pub fn coins(&self) -> u32 {
        self.field(ID_GAME, self.ids.score, "coins") as u32
    }

    pub fn deaths(&self) -> u32 {
        self.field(ID_GAME, self.ids.score, "deaths") as u32
    }

    pub fn stomps(&self) -> u32 {
        self.field(ID_GAME, self.ids.score, "stomps") as u32
    }

    /// The player's mode (the kit's `GROUND`, `AIR`, ... numbers).
    pub fn mode(&self) -> u32 {
        self.field(ID_PLAYER, self.ids.player, "mode") as u32
    }

    pub fn mode_name(&self) -> &'static str {
        [
            "ground", "air", "wall", "duck", "roll", "dive", "spin", "hurt",
        ]
        .get(self.mode() as usize)
        .copied()
        .unwrap_or("?")
    }

    pub fn player_position(&self) -> Position {
        self.world
            .entity_from_id(self.entity(ID_PLAYER))
            .get::<&Position>(|p| *p)
    }

    pub fn player_velocity(&self) -> Velocity {
        self.world
            .entity_from_id(self.entity(ID_PLAYER))
            .get::<&Velocity>(|v| *v)
    }

    pub fn player_collider(&self) -> Collider {
        self.world
            .entity_from_id(self.entity(ID_PLAYER))
            .get::<&Collider>(|c| *c)
    }

    pub fn player_body(&self) -> Body {
        self.world
            .entity_from_id(self.entity(ID_PLAYER))
            .get::<&Body>(|b| *b)
    }

    /// Every entity with a `Body` (the player and the walkers) and where it
    /// is, in StableId order. For tools, bots and tests.
    pub fn bodies(&self) -> Vec<(u64, Position)> {
        let mut found = vec![];
        self.world
            .new_query::<(&StableIdC, &Position, &Body)>()
            .each(|(sid, pos, _)| found.push((sid.value, *pos)));
        found.sort_by_key(|f| f.0);
        found
    }

    pub fn camera(&self) -> Camera {
        self.world.get::<&Camera>(|c| *c)
    }

    /// The `Tally` counters, in [`CUES`] order.
    pub fn tally(&self) -> [u32; CUES.len()] {
        let e = self.entity(ID_GAME);
        CUES.map(|(field, _)| {
            self.ecs
                .field(e, self.ids.tally, field)
                .unwrap_or_else(|| panic!("Tally has no {field}")) as u32
        })
    }

    /// What the heads-up display shows now.
    pub fn hud(&self) -> Hud {
        let (hp, max_hp) = self.health();
        Hud {
            hp,
            max_hp,
            coins: self.coins(),
            state: match self.state() {
                GameState::Playing => 0,
                GameState::Paused => 1,
                GameState::Dead => 2,
            },
        }
    }

    /// A read-only draw list for presentation (the host interface).
    pub fn frame(&self) -> UiFrame {
        frame::compose(&self.world, &self.hud())
    }

    /// Sound cues that happened since the last call, in the order of
    /// [`CUES`]; one per increase of a `Tally` counter. Derived by comparing
    /// state before and after a step, so they cannot affect a replay.
    pub fn take_cues(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cues)
    }

    /// Contacts that began during the last tick, as StableId pairs.
    pub fn last_contact_begins(&self) -> Vec<(u64, u64)> {
        self.world.get::<&Contacts>(|c| {
            c.0.transitions()
                .begin
                .iter()
                .map(|c| (c.a.0, c.b.0))
                .collect()
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            ecs: self.ecs.snapshot(),
            contacts: self.world.get::<&Contacts>(|c| c.0.clone()),
        }
    }

    /// Restores a snapshot taken from a `Platformer` built the same way.
    pub fn restore(&mut self, s: &Snapshot) -> Result<(), String> {
        self.ecs.restore(&s.ecs)?;
        self.world.set(Contacts(s.contacts.clone()));
        self.seen = self.tally();
        self.cues.clear();
        Ok(())
    }

    /// A `sage-save v1` document: everything the binding knows plus the
    /// contact table, which it does not.
    pub fn save(&self) -> String {
        let contacts = self.world.get::<&Contacts>(|c| c.0.to_text());
        self.ecs
            .save_game([("contacts".to_string(), contacts)].into())
    }

    /// Loads a save game into a `Platformer` built the same way.
    pub fn load(&mut self, text: &str) -> Result<(), String> {
        let loaded = self.ecs.load_game(text, Policy::Strict)?;
        let contacts = loaded
            .extra
            .get("contacts")
            .ok_or("the save game has no contact table")?;
        self.world.set(Contacts(ContactTable::from_text(contacts)?));
        self.seen = self.tally();
        self.cues.clear();
        Ok(())
    }
}

impl Simulation for Platformer {
    fn step(&mut self, input: TickInput) {
        self.world.set(Input {
            buttons: input.buttons,
        });
        self.world.get::<&mut Clock>(|c| c.tick += 1);
        self.world.progress_time(TICK_DT);
        if let Some(fault) = self.ecs.fault() {
            panic!("script error in tick {}: {fault}", self.tick());
        }
        let now = self.tally();
        for (i, (_, cue)) in CUES.iter().enumerate() {
            for _ in self.seen[i]..now[i] {
                self.cues.push((*cue).to_string());
            }
        }
        self.seen = now;
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
