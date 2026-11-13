//! The usdx-audio library.
//!
//! About me: this crate holds the audio side of the editor: the playbin3-based player
//! with play range and rate control over GStreamer, the waveform peak extraction and
//! vocal activity analysis, and the analysis cache. It exists so that the editor can
//! play the song at any position and speed and show the waveform the notes are
//! aligned to, independent of Qt.
//!
//! This skeleton is filled in by tasks P4-01 to P4-03 of Plan/IMPLEMENTATION_PLAN.md.

pub mod testmedia;

pub mod analysis;
pub mod cache;
pub mod player;

pub use analysis::{Activity, Peaks, analyze};
pub use player::{AudioError, PlayState, Player, PlayerEvent, Track};
