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

// ----- UI --------------------------------------------------------------------

/// Metrics of the UI font, in UI units at scale 1. They match the built-in
/// bitmap font (`sage_assets::BitmapFont::builtin`); a test in
/// `sage_platform` keeps the two in step.
pub const UI_GLYPH: [f32; 2] = [5.0, 7.0];
pub const UI_ADVANCE: f32 = 6.0;
pub const UI_LINE_HEIGHT: f32 = 9.0;

/// Navigation bits, the UI's keyboard and gamepad input. A host sets them
/// from its own keys; they are held-state, and the UI acts on the edge of a
/// bit (pressed this tick, not held).
pub const NAV_NEXT: u32 = 1 << 0;
pub const NAV_PREV: u32 = 1 << 1;
pub const NAV_ACCEPT: u32 = 1 << 2;
pub const NAV_CANCEL: u32 = 1 << 3;
pub const NAV_LEFT: u32 = 1 << 4;
pub const NAV_RIGHT: u32 = 1 << 5;
pub const NAV_UP: u32 = 1 << 6;
pub const NAV_DOWN: u32 = 1 << 7;
pub const NAV_BACKSPACE: u32 = 1 << 8;
pub const NAV_HOME: u32 = 1 << 9;
pub const NAV_END: u32 = 1 << 10;
pub const NAV_DELETE: u32 = 1 << 11;
pub const NAV_PAGE_UP: u32 = 1 << 12;
pub const NAV_PAGE_DOWN: u32 = 1 << 13;

/// One thing the UI module wants drawn. Colors are straight-alpha RGBA.
#[derive(Clone, Debug, PartialEq)]
pub enum UiDraw {
    /// A filled rectangle, top-left corner and size.
    Rect {
        pos: [f32; 2],
        size: [f32; 2],
        color: [u8; 4],
    },
    /// Text with its top-left corner at `pos`; `scale` multiplies the font.
    /// `\n` starts a new line. Glyphs are cut off outside `clip` (left, top,
    /// right, bottom) when there is one.
    Text {
        pos: [f32; 2],
        text: String,
        scale: f32,
        color: [u8; 4],
        clip: Option<[f32; 4]>,
    },
}

/// A UI snapshot for presentation: draw commands back to front, in UI units
/// with the origin at the top-left and y down. Like [`Frame`], holding it
/// never borrows simulation state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiFrame {
    /// The UI's own size in UI units; presentation scales it to the window.
    pub size: [f32; 2],
    pub draws: Vec<UiDraw>,
}
