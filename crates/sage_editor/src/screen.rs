//! The editor's screen: UI entities written in Python over the engine's own
//! widget kit (design doc 10.6).
//!
//! The screen is one script in several files (`scripts/`). They are loaded in
//! [`PARTS`] order into one namespace, as if they were one file, with the
//! `editor` object ([`crate::bridge::EditorHost`]) among their globals. The
//! host calls the script's `refresh_views` after every UI tick, outside the
//! systems: views are rebuilt after the tick's events were delivered, so an
//! observer never changes the widgets it is reacting to (and entities made
//! inside a system cannot be read back until it ends).

/// The palette and the style prefabs, loaded before the screen.
pub const THEME: &str = include_str!("../scripts/theme.py");

/// The screen's files, in load order.
pub const PARTS: &[(&str, &str)] = &[
    ("base", include_str!("../scripts/base.py")),
    ("commands", include_str!("../scripts/commands.py")),
    ("frame", include_str!("../scripts/frame.py")),
    ("views", include_str!("../scripts/views.py")),
    ("blocks_view", include_str!("../scripts/blocks_view.py")),
    ("blocks_canvas", include_str!("../scripts/blocks_canvas.py")),
    ("blocks_edit", include_str!("../scripts/blocks_edit.py")),
    ("timing", include_str!("../scripts/timing.py")),
    ("main", include_str!("../scripts/main.py")),
];

/// The function the host calls after every UI tick.
pub const REFRESH: &str = "refresh_views";

/// The UI's size in UI units: 1280x800 pixels at scale 2.
pub const SIZE: [f32; 2] = [640.0, 400.0];

/// The color behind the screen (the theme's darkest).
pub const CLEAR: [u8; 4] = [15, 13, 11, 255];

/// The id the scene viewport takes its frame from (`ui.viewport(.., 1)`).
pub const SCENE_VIEW: u32 = 1;

/// The editor's text face, Liberation Sans (SIL OFL 1.1; see
/// `docs/dependencies.md`): the bytes of the `.ttf`, for the application to
/// rasterize at the scale it draws.
pub const FACE: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");

/// The game the editor opens when given no project.
pub const SAMPLE_GAME: &str = include_str!("../sample/game.py");
