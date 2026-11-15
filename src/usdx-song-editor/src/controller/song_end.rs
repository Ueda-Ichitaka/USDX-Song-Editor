//! The song end controller (user decisions 2026-11-15).
//!
//! About me: this module holds the rules around the end of a song. The
//! sheet's time scroll runs from 0:00 to the later of the audio end and the
//! last note or phrase end, plus a 2 s margin; a `#END` header takes the
//! audio end's place, because the song stops there in USDX
//! (`scroll_bounds`). "Go to song end" jumps to `#END`, else the audio end,
//! else the last note's end (`song_end_target_ms`). "End song here" writes
//! `#END` at the playhead and removes everything after it, and "Remove song
//! end" drops the header, one undo step each; they exist to cut off a
//! video's endcard without touching the audio file.

use usdx_edit::{EditError, header_ops, ops};
use usdx_format::headers::parse_decimal;

use crate::controller::editor::EditorController;

/// The margin after the song or audio end the scroll still shows.
pub const SCROLL_MARGIN_MS: f64 = 2000.0;

/// ms_per_beat is the USDX beat length: 15000 / bpm ms.
fn ms_per_beat(bpm: f64) -> f64 {
    15000.0 / bpm
}

/// scroll_bounds returns the beat range the sheet may show: from 0:00 to
/// the later of the audio end (`#END` in its place when set) and the last
/// note or phrase end, plus the margin. An audio length of 0 or less counts
/// as unknown.
pub fn scroll_bounds(
    gap_ms: f64,
    bpm: f64,
    audio_end_ms: Option<f64>,
    end_ms: Option<f64>,
    song_end_beat: f64,
) -> (f64, f64) {
    let beat_of = |ms: f64| (ms - gap_ms) / ms_per_beat(bpm);
    let audio_end = end_ms.or(audio_end_ms.filter(|ms| *ms > 0.0));
    let notes_end_ms = gap_ms + song_end_beat * ms_per_beat(bpm);
    let end_ms = audio_end.map_or(notes_end_ms, |ms| ms.max(notes_end_ms));
    (beat_of(0.0), beat_of(end_ms + SCROLL_MARGIN_MS))
}

/// song_end_target_ms returns the time "Go to song end" jumps to: `#END`,
/// else the audio end, else the last note's end. An audio length of 0 or
/// less counts as unknown.
pub fn song_end_target_ms(
    audio_end_ms: Option<f64>,
    end_ms: Option<f64>,
    notes_end_ms: f64,
) -> f64 {
    end_ms
        .or(audio_end_ms.filter(|ms| *ms > 0.0))
        .unwrap_or(notes_end_ms)
}

impl EditorController {
    /// end_ms returns the song's `#END` in ms, `None` without a usable one.
    pub fn end_ms(&self) -> Option<f64> {
        self.loaded()?
            .song
            .headers
            .get("END")
            .and_then(parse_decimal)
    }

    /// end_song_at writes `#END` at the time and removes the notes and
    /// phrase ends after it, one undo step. The selection clears (rows
    /// behind the cut go) and the current line stays in range.
    pub fn end_song_at(&mut self, ms: f64) -> Result<(), EditError> {
        let patch = {
            let loaded = self
                .loaded()
                .ok_or(EditError::StalePatch("no song is loaded"))?;
            ops::end_song_at(&loaded.document, &loaded.song, ms)?
        };
        self.commit(patch)?;
        self.selection.clear();
        self.anchor = None;
        self.go_to_line(self.current_line());
        Ok(())
    }

    /// remove_song_end drops the `#END` header, one undo step; without one
    /// it changes nothing.
    pub fn remove_song_end(&mut self) -> Result<(), EditError> {
        let patch = {
            let loaded = self
                .loaded()
                .ok_or(EditError::StalePatch("no song is loaded"))?;
            header_ops::remove_header(&loaded.document, "END")?
        };
        if patch.ops.is_empty() {
            return Ok(());
        }
        self.commit(patch)
    }
}
