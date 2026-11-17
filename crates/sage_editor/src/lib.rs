//! The editor core (design doc 10.6, 10.7 and milestones M3 and M4).
//!
//! - [`history`]: undo and redo as a transaction log of scene differences.
//! - [`document`]: the authored scene, the inspector generated from
//!   reflection, the outliner, selection.
//! - [`project`]: Python sources, the live world, play in editor, live
//!   reload, and a project's files on disk.
//! - [`viewport`]: what the scene viewport draws, picking and dragging.
//! - [`session`]: a project being edited: requests (play, stop, save...) and
//!   the host's part of every tick.
//! - [`bridge`]: the `editor` object the screen's scripts talk to.
//! - [`screen`]: the editor's screen, UI entities written in Python
//!   (`scripts/`), as design doc 10.6 decides.
//!
//! No window and no device: an application (`examples/editor`) puts the
//! screen in a `sage_app::UiHost` and shows it.

pub mod blocks;
pub mod bridge;
pub mod document;
pub mod history;
pub mod project;
pub mod screen;
pub mod session;
pub mod viewport;

pub use document::{Document, EntityView, OutlinerRow};
pub use history::{History, SceneDiff, SourceChange, Step};
pub use project::{Mode, Prepare, Project};
pub use session::{Request, Session};
pub use viewport::{ScenePointer, SceneView};
