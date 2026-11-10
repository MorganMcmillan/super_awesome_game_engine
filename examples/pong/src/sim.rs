//! Pong as a deterministic Flecs simulation (design doc 13.1, Appendix A).
//!
//! ```text
//! game states: Serving, Playing, GameOver   (GameState, not pipeline phases)
//!
//! while Playing:                     -> flecs module `play` is enabled
//!     ball moves, paddles follow input
//! when ball hits paddle: bounce ball -> observer on ContactBegin
//! when ball hits side:   emit goal   -> observer on ContactBegin
//! on goal(side):                     -> Python rule (scripts/rules.py)
//!     scorer = side.opponent         (relationship: (Opponent, player))
//!     scorer.points += 1
//!     go to GameOver if scorer.points == max_points, else Serving
//! on entering Serving:               -> core module, state machine
//!     reset ball, wait 1s, go to Playing
//! ```
//!
//! Pause and feature-off are done by disabling flecs modules (5.5): every
//! system and observer of the `play` module is off outside `Playing`, so the
//! rules carry no game-state term.
//!
//! Pipeline phases (timing), in order: SimInput -> SimMove -> SimDetect ->
//! SimReact, all after Flecs's OnUpdate.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_collision::{BoxCollider, ContactTable, detect};
use sage_core::host::{DrawKind, DrawRect, Frame};
use sage_core::{Rng, Simulation, StableId, StateHasher, TICK_DT, TickInput};
use sage_script::Script;

// ----- tuning ---------------------------------------------------------------

pub const COURT_W: f32 = 160.0;
pub const COURT_H: f32 = 90.0;
pub const WALL_HALF: f32 = 1.0;
pub const PADDLE_HALF: [f32; 2] = [1.0, 8.0];
pub const PADDLE_X: f32 = 6.0;
pub const PADDLE_SPEED: f32 = 90.0;
pub const BALL_HALF: f32 = 1.0;
pub const BALL_SPEED_X: f32 = 60.0;
/// Kept below `PADDLE_SPEED` so a paddle can always follow the ball.
pub const BALL_MAX_VY: f32 = 70.0;
pub const SERVE_TICKS: u32 = 60;
pub const MAX_POINTS: u32 = 5;

// ----- input ---------------------------------------------------------------

pub const LEFT_UP: u32 = 1 << 0;
pub const LEFT_DOWN: u32 = 1 << 1;
pub const RIGHT_UP: u32 = 1 << 2;
pub const RIGHT_DOWN: u32 = 1 << 3;

// ----- stable IDs -------------------------------------------------------------

pub const ID_COURT: StableId = StableId(1);
pub const ID_WALL_TOP: StableId = StableId(10);
pub const ID_WALL_BOTTOM: StableId = StableId(11);
pub const ID_GOAL_LEFT: StableId = StableId(20);
pub const ID_GOAL_RIGHT: StableId = StableId(21);
pub const ID_PADDLE_LEFT: StableId = StableId(30);
pub const ID_PADDLE_RIGHT: StableId = StableId(31);
pub const ID_BALL: StableId = StableId(40);
pub const ID_PLAYER_LEFT: StableId = StableId(50);
pub const ID_PLAYER_RIGHT: StableId = StableId(51);

// ----- components -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    pub fn from_name(s: &str) -> Option<Side> {
        match s {
            "left" => Some(Side::Left),
            "right" => Some(Side::Right),
            _ => None,
        }
    }
    fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn from_name(s: &str) -> Option<GameState> {
        match s {
            "Serving" => Some(GameState::Serving),
            "Playing" => Some(GameState::Playing),
            "GameOver" => Some(GameState::GameOver),
            _ => None,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Sid(pub StableId);

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

/// Axis-aligned box collider, by half extents.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Collider {
    pub hw: f32,
    pub hh: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Ball;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Wall;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Paddle {
    pub side: Side,
}

/// A side of the court; touching it concedes a point.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GoalSide {
    pub side: Side,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub side: Side,
    pub points: u32,
}

/// Relationship: `(Opponent, player)` on a side entity names who scores there.
#[derive(Component, Debug, Clone, Copy)]
pub struct Opponent;

// ----- events ------------------------------------------------------------------

/// One per begin transition of the contact table, in canonical order.
#[derive(Component, Debug, Clone, Copy)]
pub struct ContactBegin {
    pub a: StableId,
    pub b: StableId,
    pub normal: [f32; 2],
    pub depth: f32,
}

/// Emitted on the side entity that was hit.
#[derive(Component, Debug, Clone, Copy)]
pub struct Goal {
    pub side: Side,
}

// ----- singletons ---------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy)]
pub struct Clock {
    pub tick: u64,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CurrentInput(pub TickInput);

#[derive(Component, Debug, Clone)]
pub struct WorldRng(pub Rng);

#[derive(Component, Debug, Clone)]
pub struct Contacts(pub ContactTable);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Game {
    pub state: GameState,
    /// Requested by rules this tick; applied in SimReact.
    pub pending: Option<GameState>,
    pub winner: Option<Side>,
    pub serve_timer: u32,
    pub max_points: u32,
}

/// StableId -> Flecs entity. Ordered, so iterating it is canonical.
#[derive(Component, Debug, Clone, Default)]
pub struct Index(pub BTreeMap<StableId, u64>);

#[derive(Component, Debug, Clone, Copy)]
pub struct Phases {
    pub input: u64,
    pub movement: u64,
    pub detect: u64,
    pub react: u64,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayModule(pub u64);

/// The loaded rules script.
#[derive(Component, Clone)]
pub struct Rules(pub Arc<Script>);

/// First script error, if any. Fatal: `step` panics with it.
#[derive(Component, Debug, Clone, Default)]
pub struct ScriptFault(pub Option<String>);

// ----- flecs modules ---------------------------------------------------------

/// Always on: clock, phases, the game-state machine.
#[derive(Component)]
pub struct PongCore;

/// On only while Playing: input, movement, collision and its rules.
#[derive(Component)]
pub struct PongPlay;

fn entity_of(world: &World, id: StableId) -> EntityView<'_> {
    let raw = world.get::<&Index>(|ix| ix.0[&id]);
    world.entity_from_id(raw)
}

impl Module for PongCore {
    fn module(world: &World) {
        world.module::<PongCore>("pong::core");
        let input = world
            .entity_named("SimInput")
            .add(flecs::pipeline::Phase)
            .depends_on(flecs::pipeline::OnUpdate);
        let movement = world
            .entity_named("SimMove")
            .add(flecs::pipeline::Phase)
            .depends_on(input);
        let detect = world
            .entity_named("SimDetect")
            .add(flecs::pipeline::Phase)
            .depends_on(movement);
        let react = world
            .entity_named("SimReact")
            .add(flecs::pipeline::Phase)
            .depends_on(detect);
        world.set(Phases {
            input: *input.id(),
            movement: *movement.id(),
            detect: *detect.id(),
            react: *react.id(),
        });

        world
            .system_named::<()>("AdvanceClock")
            .kind(input)
            .run(|mut it| {
                while it.next() {}
                it.world().get::<&mut Clock>(|c| c.tick += 1);
            });

        // Immediate: entering a state enables/disables a module and moves
        // entities; doing it directly keeps the order of effects obvious.
        world
            .system_named::<()>("GameStateMachine")
            .kind(react)
            .immediate(true)
            .run(|mut it| {
                while it.next() {}
                let world = it.world();
                let pending = world.get::<&mut Game>(|g| {
                    if g.pending.is_none() && g.state == GameState::Serving {
                        g.serve_timer = g.serve_timer.saturating_sub(1);
                        if g.serve_timer == 0 {
                            g.pending = Some(GameState::Playing);
                        }
                    }
                    g.pending.take()
                });
                if let Some(next) = pending {
                    enter_state(&world, next);
                }
            });
    }
}

/// State entry actions. The only place module enablement changes.
pub fn enter_state(world: &World, next: GameState) {
    let play = world.entity_from_id(world.get::<&PlayModule>(|m| m.0));
    let ball = entity_of(world, ID_BALL);
    world.get::<&mut Game>(|g| g.state = next);
    match next {
        GameState::Serving => {
            play.disable_self();
            ball.set(Position {
                x: COURT_W / 2.0,
                y: COURT_H / 2.0,
            });
            ball.set(Velocity::default());
            world.get::<&mut Game>(|g| g.serve_timer = SERVE_TICKS);
        }
        GameState::Playing => {
            let (vx, vy) = world.get::<&mut WorldRng>(|r| {
                let dir = if r.0.next_bool() { 1.0 } else { -1.0 };
                (dir * BALL_SPEED_X, r.0.range_f32(-0.5, 0.5) * BALL_SPEED_X)
            });
            ball.set(Velocity { x: vx, y: vy });
            play.enable_self();
        }
        GameState::GameOver => {
            play.disable_self();
            ball.set(Velocity::default());
        }
    }
}

impl Module for PongPlay {
    fn module(world: &World) {
        world.module::<PongPlay>("pong::play");
        let phases = world.get::<&Phases>(|p| *p);

        world
            .system_named::<(&mut Velocity, &Paddle)>("PaddlesFollowInput")
            .kind(phases.input)
            .each_entity(|e, (vel, paddle)| {
                let input = e.world().get::<&CurrentInput>(|i| i.0);
                let (up, down) = match paddle.side {
                    Side::Left => (LEFT_UP, LEFT_DOWN),
                    Side::Right => (RIGHT_UP, RIGHT_DOWN),
                };
                vel.y = match (input.held(up), input.held(down)) {
                    (true, false) => PADDLE_SPEED,
                    (false, true) => -PADDLE_SPEED,
                    _ => 0.0,
                };
            });

        world
            .system_named::<(&mut Position, &Velocity)>("Move")
            .kind(phases.movement)
            .each(|(pos, vel)| {
                pos.x += vel.x * TICK_DT;
                pos.y += vel.y * TICK_DT;
            });

        world
            .system_named::<(&mut Position, &Paddle)>("ClampPaddles")
            .kind(phases.movement)
            .each(|(pos, _)| {
                let lo = 2.0 * WALL_HALF + PADDLE_HALF[1];
                let hi = COURT_H - lo;
                pos.y = pos.y.clamp(lo, hi);
            });

        // Writes the contact table and announces begin transitions. Flecs
        // iteration order is irrelevant: `detect` sorts by StableId.
        world
            .system_named::<(&Sid, &Position, &Collider)>("DetectContacts")
            .kind(phases.detect)
            .immediate(true)
            .run(|mut it| {
                let mut colliders = vec![];
                while it.next() {
                    let sid = it.field::<Sid>(0);
                    let pos = it.field::<Position>(1);
                    let col = it.field::<Collider>(2);
                    for i in it.iter() {
                        colliders.push(BoxCollider {
                            id: sid[i].0,
                            center: [pos[i].x, pos[i].y],
                            half: [col[i].hw, col[i].hh],
                        });
                    }
                }
                let world = it.world();
                let tick = world.get::<&Clock>(|c| c.tick);
                let begins = world.get::<&mut Contacts>(|t| {
                    t.0.advance(detect(tick, &colliders));
                    t.0.transitions().begin
                });
                let announcer = world.lookup("pong::play::ContactAnnouncer");
                for c in begins {
                    announcer.emit(&ContactBegin {
                        a: c.a,
                        b: c.b,
                        normal: c.normal,
                        depth: c.depth,
                    });
                }
            });
        world.entity_named("ContactAnnouncer");

        // when ball hits paddle (or wall): bounce ball
        world
            .observer_named::<ContactBegin, ()>("BounceBall")
            .with(flecs::Any)
            .each_iter(|it, _, _| {
                let c = *it.param();
                let world = it.world();
                let Some((other, normal)) = seen_from_ball(&c) else {
                    return;
                };
                let other_e = entity_of(&world, other);
                let ball = entity_of(&world, ID_BALL);
                if other_e.has(Paddle::id()) {
                    let paddle_y = other_e.get::<&Position>(|p| p.y);
                    ball.get::<(&mut Position, &mut Velocity)>(|(p, v)| {
                        p.x -= normal[0] * c.depth;
                        v.x = -normal[0].signum() * v.x.abs();
                        let offset = (p.y - paddle_y) / PADDLE_HALF[1];
                        v.y = (v.y + offset * 0.5 * BALL_SPEED_X).clamp(-BALL_MAX_VY, BALL_MAX_VY);
                    });
                } else if other_e.has(Wall::id()) {
                    ball.get::<(&mut Position, &mut Velocity)>(|(p, v)| {
                        p.y -= normal[1] * c.depth;
                        v.y = -normal[1].signum() * v.y.abs();
                    });
                }
            });

        // when ball hits side: emit goal(side)
        world
            .observer_named::<ContactBegin, ()>("BallHitsSide")
            .with(flecs::Any)
            .each_iter(|it, _, _| {
                let c = *it.param();
                let world = it.world();
                let Some((other, _)) = seen_from_ball(&c) else {
                    return;
                };
                let side_e = entity_of(&world, other);
                if let Some(side) = side_e.try_get::<&GoalSide>(|g| g.side) {
                    side_e.emit(&Goal { side });
                }
            });

        // on goal(side): the Python rule
        world
            .observer_named::<Goal, ()>("OnGoal")
            .with(flecs::Any)
            .each_iter(|it, _, _| {
                let goal = *it.param();
                let world = it.world();
                if let Err(e) = run_goal_rules(&world, goal) {
                    world.get::<&mut ScriptFault>(|f| {
                        f.0.get_or_insert(e);
                    });
                }
            });
    }
}

fn seen_from_ball(c: &ContactBegin) -> Option<(StableId, [f32; 2])> {
    let contact = sage_collision::Contact {
        tick: 0,
        a: c.a,
        b: c.b,
        normal: c.normal,
        depth: c.depth,
    };
    contact.seen_from(ID_BALL)
}

// ----- the Python boundary ----------------------------------------------------

/// What a goal rule sees of the world. Rules read and write through it;
/// Rust copies the results back, so Python never holds Flecs pointers.
#[pyclass(name = "PongWorld", unsendable)]
pub struct PyWorld {
    points: BTreeMap<String, u32>,
    #[pyo3(get)]
    max_points: u32,
    next_state: Option<(GameState, Option<Side>)>,
}

#[pymethods]
impl PyWorld {
    fn points(&self, player: &str) -> PyResult<u32> {
        self.points
            .get(player)
            .copied()
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(player.to_string()))
    }

    fn add_points(&mut self, player: &str, n: u32) -> PyResult<()> {
        let p = self
            .points
            .get_mut(player)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(player.to_string()))?;
        *p += n;
        Ok(())
    }

    #[pyo3(signature = (state, winner=None))]
    fn go_to(&mut self, state: &str, winner: Option<&str>) -> PyResult<()> {
        let s = GameState::from_name(state).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "unknown game state '{state}' (expected Serving, Playing or GameOver)"
            ))
        })?;
        let w = match winner {
            None => None,
            Some(name) => Some(Side::from_name(name).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!("unknown player '{name}'"))
            })?),
        };
        self.next_state = Some((s, w));
        Ok(())
    }
}

/// The event a goal rule receives.
#[pyclass(name = "GoalEvent", frozen)]
pub struct PyGoal {
    /// The side that was hit.
    #[pyo3(get)]
    side: &'static str,
    /// `side.opponent`: the player who scores, from the `(Opponent, player)`
    /// relationship on the side entity.
    #[pyo3(get)]
    scorer: &'static str,
}

fn run_goal_rules(world: &World, goal: Goal) -> Result<(), String> {
    let side_id = match goal.side {
        Side::Left => ID_GOAL_LEFT,
        Side::Right => ID_GOAL_RIGHT,
    };
    let scorer = entity_of(world, side_id)
        .target(Opponent::id(), 0)
        .ok_or("side has no (Opponent, player) relationship")?
        .get::<&Player>(|p| p.side);

    let players = [ID_PLAYER_LEFT, ID_PLAYER_RIGHT].map(|id| entity_of(world, id));
    let mut points = BTreeMap::new();
    for p in &players {
        p.get::<&Player>(|pl| points.insert(pl.side.name().to_string(), pl.points));
    }
    let max_points = world.get::<&Game>(|g| g.max_points);
    let rules = world.get::<&Rules>(|r| r.0.clone());

    let result = Python::attach(|py| -> PyResult<PyWorld> {
        let w = Bound::new(
            py,
            PyWorld {
                points,
                max_points,
                next_state: None,
            },
        )?;
        let ev = Bound::new(
            py,
            PyGoal {
                side: goal.side.name(),
                scorer: scorer.name(),
            },
        )?;
        for h in rules.handlers("goal") {
            h.func.call1(py, (w.clone(), ev.clone()))?;
        }
        let w = w.borrow();
        Ok(PyWorld {
            points: w.points.clone(),
            max_points: w.max_points,
            next_state: w.next_state,
        })
    })
    .map_err(|e| Python::attach(|py| sage_script::describe(py, &e).0))?;

    for p in &players {
        p.get::<&mut Player>(|pl| pl.points = result.points[pl.side.name()]);
    }
    if let Some((state, winner)) = result.next_state {
        world.get::<&mut Game>(|g| {
            g.pending = Some(state);
            if winner.is_some() {
                g.winner = winner;
            }
        });
    }
    Ok(())
}

// ----- the simulation -------------------------------------------------------------

pub struct PongConfig {
    pub seed: u64,
    pub max_points: u32,
    pub rules_source: String,
    /// Test-only: adds a system that iterates a randomly seeded `HashMap` in
    /// simulation code. The determinism harness must catch it (spike S3).
    pub nondeterminism_canary: bool,
}

pub const RULES_SOURCE: &str = include_str!("../scripts/rules.py");

impl PongConfig {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            max_points: MAX_POINTS,
            rules_source: RULES_SOURCE.to_string(),
            nondeterminism_canary: false,
        }
    }
}

pub struct Pong {
    world: World,
}

impl Pong {
    pub fn new(config: PongConfig) -> Result<Self, String> {
        let rules = Script::load("rules", &config.rules_source).map_err(|e| e.0)?;
        if rules.handlers("goal").count() == 0 {
            return Err("rules script defines no @sage.on(\"goal\") handler".into());
        }
        let world = World::new();
        world.set(Clock { tick: 0 });
        world.set(CurrentInput(TickInput::EMPTY));
        world.set(WorldRng(Rng::from_seed(config.seed)));
        world.set(Contacts(ContactTable::new()));
        world.set(Index::default());
        world.set(Rules(Arc::new(rules)));
        world.set(ScriptFault::default());
        world.set(Game {
            state: GameState::Serving,
            pending: None,
            winner: None,
            serve_timer: SERVE_TICKS,
            max_points: config.max_points,
        });

        world.import::<PongCore>();
        let play = world.import::<PongPlay>();
        world.set(PlayModule(*play.id()));

        let spawn = |id: StableId, name: &str| {
            let e = world.entity_named(name).set(Sid(id));
            world.get::<&mut Index>(|ix| ix.0.insert(id, *e.id()));
            e
        };
        fn box_at(e: EntityView<'_>, x: f32, y: f32, hw: f32, hh: f32) -> EntityView<'_> {
            e.set(Position { x, y }).set(Collider { hw, hh })
        }

        spawn(ID_COURT, "court");
        box_at(
            spawn(ID_WALL_TOP, "wall_top"),
            COURT_W / 2.0,
            COURT_H - WALL_HALF,
            COURT_W / 2.0,
            WALL_HALF,
        )
        .add(Wall::id());
        box_at(
            spawn(ID_WALL_BOTTOM, "wall_bottom"),
            COURT_W / 2.0,
            WALL_HALF,
            COURT_W / 2.0,
            WALL_HALF,
        )
        .add(Wall::id());
        let left_player = spawn(ID_PLAYER_LEFT, "player_left").set(Player {
            side: Side::Left,
            points: 0,
        });
        let right_player = spawn(ID_PLAYER_RIGHT, "player_right").set(Player {
            side: Side::Right,
            points: 0,
        });
        box_at(
            spawn(ID_GOAL_LEFT, "side_left"),
            -2.0,
            COURT_H / 2.0,
            2.0,
            COURT_H / 2.0,
        )
        .set(GoalSide { side: Side::Left })
        .add((Opponent::id(), right_player));
        box_at(
            spawn(ID_GOAL_RIGHT, "side_right"),
            COURT_W + 2.0,
            COURT_H / 2.0,
            2.0,
            COURT_H / 2.0,
        )
        .set(GoalSide { side: Side::Right })
        .add((Opponent::id(), left_player));
        for (id, name, side, x) in [
            (ID_PADDLE_LEFT, "paddle_left", Side::Left, PADDLE_X),
            (
                ID_PADDLE_RIGHT,
                "paddle_right",
                Side::Right,
                COURT_W - PADDLE_X,
            ),
        ] {
            box_at(
                spawn(id, name),
                x,
                COURT_H / 2.0,
                PADDLE_HALF[0],
                PADDLE_HALF[1],
            )
            .set(Velocity::default())
            .set(Paddle { side });
        }
        box_at(
            spawn(ID_BALL, "ball"),
            COURT_W / 2.0,
            COURT_H / 2.0,
            BALL_HALF,
            BALL_HALF,
        )
        .set(Velocity::default())
        .add(Ball::id());

        if config.nondeterminism_canary {
            add_canary(&world);
        }

        enter_state(&world, GameState::Serving);
        Ok(Self { world })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn game(&self) -> Game {
        self.world.get::<&Game>(|g| *g)
    }

    pub fn points(&self) -> [u32; 2] {
        [ID_PLAYER_LEFT, ID_PLAYER_RIGHT]
            .map(|id| entity_of(&self.world, id).get::<&Player>(|p| p.points))
    }

    pub fn play_module_enabled(&self) -> bool {
        let id = self.world.get::<&PlayModule>(|m| m.0);
        self.world.entity_from_id(id).is_enabled_self()
    }

    fn pos(&self, id: StableId) -> Position {
        entity_of(&self.world, id).get::<&Position>(|p| *p)
    }

    /// A simple deterministic bot: holds the button that moves `side`'s
    /// paddle towards the ball. Used to script headless runs.
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

    /// A read-only snapshot for presentation (the host interface).
    pub fn frame(&self) -> Frame {
        let ix = self.world.get::<&Index>(|ix| ix.0.clone());
        let mut rects = vec![DrawRect {
            id: ID_COURT,
            kind: DrawKind::Court,
            center: [COURT_W / 2.0, COURT_H / 2.0],
            half: [COURT_W / 2.0, COURT_H / 2.0],
        }];
        for (&id, &raw) in &ix {
            let e = self.world.entity_from_id(raw);
            let kind = if e.has(Ball::id()) {
                DrawKind::Ball
            } else if e.has(Paddle::id()) {
                DrawKind::Paddle
            } else if e.has(Wall::id()) {
                DrawKind::Wall
            } else if e.has(GoalSide::id()) {
                DrawKind::Goal
            } else {
                continue;
            };
            if let Some((p, c)) = e.try_get::<(&Position, &Collider)>(|(p, c)| (*p, *c)) {
                rects.push(DrawRect {
                    id,
                    kind,
                    center: [p.x, p.y],
                    half: [c.hw, c.hh],
                });
            }
        }
        let g = self.game();
        let [l, r] = self.points();
        let mut hud = vec![format!("{l}  :  {r}"), g.state.name().to_string()];
        if let Some(w) = g.winner {
            hud.push(format!("winner: {}", w.name()));
        }
        Frame {
            tick: self.tick(),
            world_size: [COURT_W, COURT_H],
            rects,
            hud,
        }
    }

    /// Captures every piece of state that `state_hash` covers (A3 candidate:
    /// restoring a snapshot of tick N reproduces tick N's hash and future).
    /// Includes the previous tick's contact set, without which the first
    /// tick after a restore would report every contact as a new begin (8.2).
    /// Python rule globals are not captured: rules must be stateless.
    pub fn snapshot(&self) -> Snapshot {
        let w = &self.world;
        let ix = w.get::<&Index>(|ix| ix.0.clone());
        let entities = ix
            .iter()
            .map(|(&id, &raw)| {
                let e = w.entity_from_id(raw);
                EntitySnapshot {
                    id,
                    position: e.try_get::<&Position>(|p| *p),
                    velocity: e.try_get::<&Velocity>(|v| *v),
                    points: e.try_get::<&Player>(|p| p.points),
                }
            })
            .collect();
        Snapshot {
            tick: self.tick(),
            input: w.get::<&CurrentInput>(|i| i.0),
            rng: w.get::<&WorldRng>(|r| r.0.clone()),
            game: self.game(),
            play_enabled: self.play_module_enabled(),
            contacts: w.get::<&Contacts>(|c| c.0.clone()),
            entities,
        }
    }

    /// Restores a snapshot taken from a `Pong` built with the same config.
    pub fn restore(&mut self, s: &Snapshot) {
        let w = &self.world;
        w.get::<&mut Clock>(|c| c.tick = s.tick);
        w.set(CurrentInput(s.input));
        w.set(WorldRng(s.rng.clone()));
        w.get::<&mut Game>(|g| *g = s.game);
        w.set(Contacts(s.contacts.clone()));
        let play = w.entity_from_id(w.get::<&PlayModule>(|m| m.0));
        if s.play_enabled {
            play.enable_self();
        } else {
            play.disable_self();
        }
        for es in &s.entities {
            let e = entity_of(w, es.id);
            if let Some(p) = es.position {
                e.set(p);
            }
            if let Some(v) = es.velocity {
                e.set(v);
            }
            if let Some(n) = es.points {
                e.get::<&mut Player>(|p| p.points = n);
            }
        }
    }
}

/// A copy of the simulation state at one tick. Entities are in `StableId`
/// order. Collider shapes and relationships are fixed at creation and are
/// not part of a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub input: TickInput,
    pub rng: Rng,
    pub game: Game,
    pub play_enabled: bool,
    pub contacts: ContactTable,
    pub entities: Vec<EntitySnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: StableId,
    pub position: Option<Position>,
    pub velocity: Option<Velocity>,
    pub points: Option<u32>,
}

/// The S3 canary: a simulation system whose effect depends on `HashMap`
/// iteration order, which `RandomState` randomizes per map.
fn add_canary(world: &World) {
    let phases = world.get::<&Phases>(|p| *p);
    let map: Rc<RefCell<std::collections::HashMap<u32, f32>>> = Rc::new(RefCell::new(
        (0..32).map(|k| (k, k as f32 * 1e-3)).collect(),
    ));
    world
        .system_named::<&mut Position>("NondeterminismCanary")
        .with(Ball::id())
        .kind(phases.movement)
        .each(move |pos| {
            let mut acc = 0.0f32;
            for v in map.borrow().values() {
                acc = acc * 0.5 + v; // order-dependent
            }
            pos.y += acc * 1e-3;
        });
}

impl Simulation for Pong {
    fn step(&mut self, input: TickInput) {
        self.world.set(CurrentInput(input));
        self.world.progress_time(TICK_DT);
        if let Some(fault) = self.world.get::<&ScriptFault>(|f| f.0.clone()) {
            panic!("script error in tick {}: {fault}", self.tick());
        }
    }

    fn tick(&self) -> u64 {
        self.world.get::<&Clock>(|c| c.tick)
    }

    /// Everything that determines the future, in canonical order.
    fn state_hash(&self) -> u64 {
        let w = &self.world;
        let mut h = StateHasher::new();
        h.tag("clock").u64(self.tick());
        h.tag("input").u32(w.get::<&CurrentInput>(|i| i.0.buttons));
        for s in w.get::<&WorldRng>(|r| r.0.state()) {
            h.tag("rng").u64(s);
        }
        let g = self.game();
        h.tag("game")
            .u8(g.state as u8)
            .u8(g.pending.map_or(255, |s| s as u8))
            .u8(g.winner.map_or(255, Side::code))
            .u32(g.serve_timer)
            .u32(g.max_points);
        h.tag("play_enabled").u8(self.play_module_enabled() as u8);
        let ix = w.get::<&Index>(|ix| ix.0.clone());
        for (&id, &raw) in &ix {
            let e = w.entity_from_id(raw);
            h.tag("entity").u64(id.0);
            if let Some(p) = e.try_get::<&Position>(|p| *p) {
                h.tag("pos").f32(p.x).f32(p.y);
            }
            if let Some(v) = e.try_get::<&Velocity>(|v| *v) {
                h.tag("vel").f32(v.x).f32(v.y);
            }
            if let Some(c) = e.try_get::<&Collider>(|c| *c) {
                h.tag("col").f32(c.hw).f32(c.hh);
            }
            if let Some(p) = e.try_get::<&Player>(|p| *p) {
                h.tag("player").u8(p.side.code()).u32(p.points);
            }
            h.tag("kind")
                .u8(e.has(Ball::id()) as u8)
                .u8(e.has(Wall::id()) as u8)
                .u8(e.try_get::<&Paddle>(|p| p.side.code()).unwrap_or(255))
                .u8(e.try_get::<&GoalSide>(|g| g.side.code()).unwrap_or(255));
        }
        w.get::<&Contacts>(|c| c.0.hash_into(&mut h));
        h.finish()
    }
}
