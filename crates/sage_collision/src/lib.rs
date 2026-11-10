//! Collision for the simulation: our own data in, canonical contacts out.
//!
//! Design doc 8.1 and 8.2: no rigid-body engine; parry provides shapes and
//! narrow-phase queries; contacts are rows in a table, and begin/stay/end are
//! set operations between the current and previous tick.

pub mod detect;
pub mod table;

pub use detect::{BoxCollider, brute_force_pairs, detect};
pub use table::{Contact, ContactTable, Transitions};
