//! The beat-to-millisecond timing of the UltraStar song format.
//!
//! About me: this module converts between beats and milliseconds using the
//! `#BPM` and `#GAP` headers, with the format's implicit quadrupling of the BPM
//! value. It exists so that the song view, the audio playback and the editor
//! place notes at the same times the game computes.

use crate::headers::{Headers, parse_decimal};

/// The timing of a song: the `#BPM` value in beats per minute and the `#GAP`
/// value in milliseconds from the audio start to beat 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub bpm: f64,
    pub gap_ms: f64,
}

impl Timing {
    /// from_headers builds the timing from the header view. It is `None`
    /// without a valid `#BPM` header or with a `#BPM` of 0; a missing `#GAP`
    /// means 0.
    pub fn from_headers(headers: &Headers) -> Option<Timing> {
        let bpm = headers.get("BPM").and_then(parse_decimal)?;
        if bpm == 0.0 {
            return None;
        }
        let gap_ms = headers.get("GAP").and_then(parse_decimal).unwrap_or(0.0);
        Some(Timing { bpm, gap_ms })
    }

    /// ms_per_beat returns the length of one beat in milliseconds. The BPM
    /// value is implicitly quadrupled by the format.
    pub fn ms_per_beat(&self) -> f64 {
        60_000.0 / (self.bpm * 4.0)
    }

    /// beat_to_ms returns the millisecond position of a beat.
    pub fn beat_to_ms(&self, beat: f64) -> f64 {
        self.gap_ms + beat * self.ms_per_beat()
    }

    /// ms_to_beat returns the beat at a millisecond position.
    pub fn ms_to_beat(&self, ms: f64) -> f64 {
        (ms - self.gap_ms) / self.ms_per_beat()
    }
}
