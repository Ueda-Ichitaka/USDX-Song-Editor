//! About me: module root of the plain-Rust controllers (plan section 8.2).
//! The controllers hold the application state without any Qt types so that
//! every user action is testable headlessly; the cxx-qt bridges in
//! src/bridge forward to them. It holds the editor controller, the file
//! fingerprint, the save service, the settings and the shell state (titles,
//! enablement, guarded transitions), the default time view of the sheet, the
//! analysis state and the column layout of the waveform lanes; the viewport,
//! repair and tap controllers are added in later tasks.

pub mod editor;
pub mod fingerprint;
pub mod lanes;
pub mod save;
pub mod settings;
pub mod shell;
pub mod view;
pub mod waveform;

pub use editor::EditorController;
pub use fingerprint::FileFingerprint;
pub use save::{SaveError, SaveService};
