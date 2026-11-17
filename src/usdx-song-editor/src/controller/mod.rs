//! About me: module root of the plain-Rust controllers (plan section 8.2).
//! The controllers hold the application state without any Qt types so that
//! every user action is testable headlessly; the cxx-qt bridges in
//! src/bridge forward to them. It holds the tap latency calibration, the
//! editor controller, the file
//! fingerprint, the save service, the settings and the shell state (titles,
//! enablement, guarded transitions), the default time view of the sheet, the
//! analysis state and the column layout of the waveform lanes, and the sheet
//! viewport (beats and pitches to pixels, zoom and scroll), the rule that
//! lets the list models update in place, and the line text editing (the
//! Edit line dialog's preview and apply, the inline syllable edit), the
//! song end rules (scroll bounds, the end jump, #END cuts); the repair
//! controller (anchor, block and tap mode with preview, apply, cancel
//! and the anchors export) and the tap-along session controller.

pub mod calibration;
pub mod editing;
pub mod editor;
pub mod fingerprint;
pub mod inspector;
pub mod lanes;
pub mod line_health;
pub mod line_text;
pub mod model_update;
pub mod notes;
pub mod repair;
pub mod save;
pub mod settings;
pub mod sheet;
pub mod shell;
pub mod song_end;
pub mod tap;
pub mod view;
pub mod viewport;
pub mod waveform;

pub use editor::EditorController;
pub use fingerprint::FileFingerprint;
pub use save::{SaveError, SaveService};
