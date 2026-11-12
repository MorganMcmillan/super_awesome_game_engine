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

pub mod draw;
pub mod ecs;
pub mod interact;
pub mod layout;
pub mod style;
pub mod text;

pub use draw::draw;
pub use ecs::{ImageSizes, frame, register_image, setup};
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

/// Every kit script, in load order, by name.
pub const KIT: &[(&str, &str)] = &[
    ("ui", WIDGETS),
    ("ui_popups", POPUPS),
    ("ui_tables", TABLES),
];

/// Loads the whole widget kit into the world `ecs` is bound to. Keep the
/// returned scripts alive as long as the world runs.
pub fn load_kit(ecs: &sage_ecs_py::Ecs) -> Result<Vec<sage_script::Script>, String> {
    KIT.iter()
        .map(|(name, source)| ecs.load(name, source).map_err(|e| e.0))
        .collect()
}
