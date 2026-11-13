//! The editor core (design doc 10.6, 10.7 and milestones M3 and M4).
//!
//! - [`history`]: undo and redo as a transaction log of scene differences.
//! - [`document`]: the authored scene, the inspector generated from
//!   reflection, the outliner, selection.
//! - [`project`]: Python sources, the live world, play in editor, live
//!   reload.
//!
//! The editor's own screen is UI entities written in Python
//! (`scripts/`), as design doc 10.6 decides.

pub mod blocks;
pub mod document;
pub mod history;
pub mod project;

pub use document::{Document, EntityView, OutlinerRow};
pub use history::{History, SceneDiff};
pub use project::{Mode, Prepare, Project};
