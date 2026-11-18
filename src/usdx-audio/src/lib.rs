//! The usdx-audio library.
//!
//! About me: this crate holds the audio side of the editor: the playbin3-based player
//! with play range and rate control over GStreamer, the waveform peak extraction and
//! vocal activity analysis, the analysis cache and the audio length probe. It exists so that the editor can
//! play the song at any position and speed and show the waveform the notes are
//! aligned to, independent of Qt.
//!
//! Tasks P4-01 to P4-03 and P7-10 of Plan/IMPLEMENTATION_PLAN.md fill it.

pub mod testmedia;

pub mod analysis;
pub mod cache;
pub mod duration;
pub mod player;

pub use analysis::{Activity, Peaks, analyze};
pub use duration::duration_ms;
pub use player::{AudioError, PlayState, Player, PlayerEvent, Track};
