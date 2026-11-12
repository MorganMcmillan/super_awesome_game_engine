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
/// Held while typing: extend the selection (Shift).
pub const NAV_SHIFT: u32 = 1 << 14;
/// Held while moving: by word, or to the start or end of the whole text (Ctrl).
pub const NAV_WORD: u32 = 1 << 15;
pub const NAV_SELECT_ALL: u32 = 1 << 16;
pub const NAV_COPY: u32 = 1 << 17;
pub const NAV_CUT: u32 = 1 << 18;
pub const NAV_PASTE: u32 = 1 << 19;
pub const NAV_UNDO: u32 = 1 << 20;
pub const NAV_REDO: u32 = 1 << 21;

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
    /// An image asset (`AssetId`, the hash of its normalized path) stretched
    /// over a rectangle. `uv` is the part of the image shown (`[u0, v0, u1,
    /// v1]`, 0 to 1); `tint` multiplies its colors.
    Image {
        asset: u64,
        pos: [f32; 2],
        size: [f32; 2],
        uv: [f32; 4],
        tint: [u8; 4],
        clip: Option<[f32; 4]>,
    },
    /// A 9-slice: the image's corners (`insets`: left, top, right, bottom, in
    /// image pixels) keep their size, its edges and center stretch.
    Slice {
        asset: u64,
        pos: [f32; 2],
        size: [f32; 2],
        insets: [f32; 4],
        tint: [u8; 4],
        clip: Option<[f32; 4]>,
    },
    /// A filled rectangle with rounded corners.
    Round {
        pos: [f32; 2],
        size: [f32; 2],
        radius: f32,
        color: [u8; 4],
        clip: Option<[f32; 4]>,
    },
    /// A soft shadow under a rounded rectangle: the rectangle grown by
    /// `blur` on every side, fading to nothing at its edge.
    Shadow {
        pos: [f32; 2],
        size: [f32; 2],
        radius: f32,
        blur: f32,
        color: [u8; 4],
        clip: Option<[f32; 4]>,
    },
    /// A rectangle with a color at each corner (top-left, top-right,
    /// bottom-right, bottom-left), blended across it.
    Gradient {
        pos: [f32; 2],
        size: [f32; 2],
        colors: [[u8; 4]; 4],
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
