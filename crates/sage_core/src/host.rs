//! The host interface: the narrow contract between simulation and
//! presentation (design doc 3.2, exercise 13).
//!
//! Data flows one way for state: the simulation produces a `Frame`, a plain
//! read-only description of what to show, and presentation consumes it.
//! The only thing that flows into the simulation is `TickInput`, which is
//! recorded per tick and is the same data a replay feeds.
//!
//! Presentation crates depend on this module; this crate never depends on
//! them.

use crate::StableId;

/// What a drawable represents, so presentation can style it without
/// knowing game rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawKind {
    Court,
    Wall,
    Goal,
    Paddle,
    Ball,
}

/// An axis-aligned rectangle in world units, given by its center and half
/// extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawRect {
    pub id: StableId,
    pub kind: DrawKind,
    pub center: [f32; 2],
    pub half: [f32; 2],
}

/// Everything presentation may show for one tick. A snapshot, not a view:
/// holding it never borrows simulation state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub tick: u64,
    /// World bounds `[width, height]`; the origin is the bottom-left corner.
    pub world_size: [f32; 2],
    /// Sorted by `StableId`.
    pub rects: Vec<DrawRect>,
    /// Short status lines (score, game state), in display order.
    pub hud: Vec<String>,
}
