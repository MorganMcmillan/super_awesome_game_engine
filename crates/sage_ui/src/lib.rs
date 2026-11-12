//! The UI module (design doc 9): UI elements are entities.
//!
//! - `layout`, `interact` and `draw` are pure: a [`Tree`] of [`Node`]s goes
//!   in, rectangles, events and a [`sage_core::host::UiFrame`] come out. No
//!   Flecs, no Python, no clock, so they are easy to test and deterministic.
//! - `ecs` is the flecs module: native components under `sage.ui`, one
//!   system that builds the tree from entities, runs the three steps and
//!   writes the computed state back, and the events widgets emit.
//!
//! Hierarchy is `ChildOf`, with siblings flowing in StableId (creation)
//! order. Design doc 9.1's preferred `Parent` storage does not exist at the
//! pinned Flecs (spike S1); see the note there.

pub mod access;
pub mod access_kit;
pub mod draw;
pub mod ecs;
pub mod interact;
pub mod layout;
pub mod style;
pub mod svg;
pub mod text;

/// The name the accessibility tree gives the toolkit.
pub const CODENAME_UI: &str = "sage_ui";

pub use draw::draw;
pub use ecs::{
    ImageSizes, access_outline, access_tree, access_update, answer_file_dialog, frame,
    perform_access_action, register_image, set_viewport_frame, setup,
};
pub use interact::{EventKind, UiEvent, UiInput, UiState, update};
pub use layout::{
    Align, Color, Direction, Edge, Fit, GradientNode, ImageNode, Justify, LayerNode, Node,
    Placement, Rect, ShadowNode, Size, SliceNode, Tree, Widget, layout, text_size,
};
pub use style::{Resolved, States, Variant};

/// The widget kit: prefabs for panel, text, button, toggle, slider and text
/// input, and `world.ui` to build them. Load it as a script after [`setup`]:
/// `ecs.load("ui", sage_ui::WIDGETS)`.
pub const WIDGETS: &str = include_str!("../scripts/widgets.py");

/// Popups, dialogs, tooltips, drop-downs, combo boxes, menus and context
/// menus (`ui.layer`, `ui.dialog`, `ui.tooltip`, `ui.select`, `ui.combo`,
/// `ui.menu_bar`, `ui.context_menu`). Loaded after [`WIDGETS`].
pub const POPUPS: &str = include_str!("../scripts/popups.py");

/// The table (`ui.table`): a grid with sortable headers and selectable rows.
/// Loaded after [`WIDGETS`].
pub const TABLES: &str = include_str!("../scripts/tables.py");

/// Tabs, split panes, details, a tree, radio groups, `<label for>`, forms with
/// validation, and templates with slots. Loaded after [`TABLES`].
pub const CONTAINERS: &str = include_str!("../scripts/containers.py");

/// Drag and drop (`ui.draggable`, `ui.drop_zone`), key chords
/// (`ui.shortcut`), resizable panels and a reorderable list
/// (`ui.sortable`). Loaded after [`CONTAINERS`].
pub const DRAGDROP: &str = include_str!("../scripts/dragdrop.py");

/// A canvas of vector shapes (`ui.canvas`, `ui.draw_line`...) and an embedded
/// simulation view (`ui.viewport`). Loaded after [`DRAGDROP`].
pub const CANVAS: &str = include_str!("../scripts/canvas.py");

/// Landmarks and groups for screen readers (`ui.nav`, `ui.main`,
/// `ui.section`, `ui.separator`, `ui.fieldset`). The `ui.role` function the
/// other kits call is in [`WIDGETS`]. Loaded after [`CANVAS`].
pub const ACCESS: &str = include_str!("../scripts/access.py");

/// Media players (`ui.media_controls`, `ui.audio_player`, `ui.video_player`).
/// Loaded after [`ACCESS`].
pub const MEDIA: &str = include_str!("../scripts/media.py");

/// Pickers: color (`ui.color_field`), date and time (`ui.date_field`,
/// `ui.time_field`), and files (`ui.file_field`). Loaded after [`MEDIA`].
pub const PICKERS: &str = include_str!("../scripts/pickers.py");

/// An SVG subset drawn into a canvas (`ui.svg`, `ui.svg_path`). Loaded after
/// [`PICKERS`].
pub const SVG: &str = include_str!("../scripts/svg.py");

/// Every kit script, in load order, by name.
pub const KIT: &[(&str, &str)] = &[
    ("ui", WIDGETS),
    ("ui_popups", POPUPS),
    ("ui_tables", TABLES),
    ("ui_containers", CONTAINERS),
    ("ui_dragdrop", DRAGDROP),
    ("ui_canvas", CANVAS),
    ("ui_access", ACCESS),
    ("ui_media", MEDIA),
    ("ui_pickers", PICKERS),
    ("ui_svg", SVG),
];

/// Loads the whole widget kit into the world `ecs` is bound to. Keep the
/// returned scripts alive as long as the world runs.
pub fn load_kit(ecs: &sage_ecs_py::Ecs) -> Result<Vec<sage_script::Script>, String> {
    KIT.iter()
        .map(|(name, source)| ecs.load(name, source).map_err(|e| e.0))
        .collect()
}
