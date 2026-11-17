//! The app layer: where a simulation meets presentation.
//!
//! Simulation crates never depend on presentation (design doc principle 9),
//! so something has to wire a world to a window, to tests and to a command
//! line. Every example did that by hand, and the hands had started to copy
//! each other. This crate is that wiring, written once:
//!
//! - [`ui`]: [`UiHost`] is a world running the UI kit. It feeds a tick of
//!   input to the UI's singletons, reads the clipboard back, records and
//!   replays sessions, evaluates Python against the app's scripts, and
//!   renders to an image. [`UiHosted`] adds the gestures tests are written
//!   in (click, drag, type) and [`run_ui_windowed`] shows it in a window.
//! - [`cli`]: arguments, files and the `replay` command of a game.
//!
//! An app (an example, the editor) depends on this crate; nothing in the
//! engine depends on it.

pub mod cli;
pub mod ui;

pub use ui::{UiHost, UiHosted, run_ui_windowed};
