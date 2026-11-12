//! About me: module root of the plain-Rust controllers (plan section 8.2).
//! The controllers hold the application state without any Qt types so that
//! every user action is testable headlessly; the cxx-qt bridges in
//! src/bridge forward to them. It starts with the editor controller and the
//! file fingerprint and grows with the save service, viewport, repair and
//! tap controllers in later tasks.

pub mod editor;
pub mod fingerprint;
pub mod save;

pub use editor::EditorController;
pub use fingerprint::FileFingerprint;
pub use save::{SaveError, SaveService};
