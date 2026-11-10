//! Deterministic simulation core.
//!
//! Rules that hold for every crate depending on this one (CLAUDE.md):
//! no wall-clock time, no OS randomness, a fixed timestep, input as data,
//! and canonical order (by `StableId`) for anything that feeds hashing,
//! collision or physics.

pub mod hash;
pub mod host;
pub mod input;
pub mod replay;
pub mod rng;

pub use hash::StateHasher;
pub use input::{InputRecording, TickInput};
pub use replay::{Divergence, RunTrace, Simulation, first_divergence, replay_twice, run_recording};
pub use rng::Rng;

/// The project's codename. The real name is [Open] (design doc 15); this is
/// the one place it is spelled. Crate names use the `sage_` prefix.
pub const CODENAME: &str = "sage";

/// Simulation ticks per second. One tick is one fixed timestep.
pub const TICK_RATE: u32 = 60;

/// The fixed timestep in seconds.
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;

/// An identifier that is stable across runs, unlike Flecs entity IDs.
/// Canonical order for simulation data is ascending `StableId`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId(pub u64);
