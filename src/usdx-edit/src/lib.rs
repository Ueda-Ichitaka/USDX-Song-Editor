//! The usdx-edit library.
//!
//! About me: this crate holds the edit operations on top of the usdx-format document
//! model: undoable patches and the history stack, note and header operations, line
//! text planning, the collision pass and retiming algorithms (anchor stretch, block
//! transform, tap mapping), racing detection, the anchors.txt export and the format
//! upgrade planner. It exists so that every user action is one invertible patch and
//! the file format invariants are enforced in one place, without Qt or GStreamer.
//!
//! This skeleton is filled in by tasks P2-01 to P2-05 and phase 7 of
//! Plan/IMPLEMENTATION_PLAN.md.

pub mod changes;
pub mod header_ops;
pub mod history;
pub mod ops;
pub mod patch;

pub use history::History;
pub use patch::{EditError, Patch, RowOp};
