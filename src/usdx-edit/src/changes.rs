//! The change log for the anchors export.
//!
//! About me: this module records how the song drifted from the document as
//! loaded, in the shape decision D-12 needs for the anchors.txt export: a
//! snapshot of every voice-P1 line's first-note start time taken at load,
//! compared against the current song by row id. It exists so the export can
//! list exactly the lines the session moved, including undoing back to the
//! loaded state, without keeping document snapshots.

use std::collections::HashMap;

use usdx_format::rows::{Document, RowId};
use usdx_format::song::{Line, Song};

/// The change log of one editing session: the line start times at load.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeLog {
    /// The start time in ms of the first note of each voice-P1 line at
    /// load, keyed by the first note's row id. Without a `#BPM` timing the
    /// beat value stands in for the time.
    starts: HashMap<RowId, f64>,
}

/// One changed line: its first note and its start time in ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineChange {
    pub first_note: RowId,
    pub time_ms: f64,
}

impl ChangeLog {
    /// snapshot records the line start times of the song as loaded.
    pub fn snapshot(doc: &Document, song: &Song) -> ChangeLog {
        let mut starts = HashMap::new();
        for line in p1_lines(song) {
            if let Some(first) = line.notes.first()
                && let Some(note) = song.note(doc, *first)
            {
                starts.insert(*first, line_time(song, note.start));
            }
        }
        ChangeLog { starts }
    }

    /// changed_lines lists the voice-P1 lines whose first note's start time
    /// differs from the snapshot, in document order. A line whose first note
    /// was not loaded - a new line, or one that starts at another note after
    /// a deletion - counts as changed as well.
    pub fn changed_lines(&self, doc: &Document, song: &Song) -> Vec<LineChange> {
        let mut changes = Vec::new();
        for line in p1_lines(song) {
            let Some(first) = line.notes.first() else {
                continue;
            };
            let Some(note) = song.note(doc, *first) else {
                continue;
            };
            let time = line_time(song, note.start);
            if self.starts.get(first).is_none_or(|loaded| *loaded != time) {
                changes.push(LineChange {
                    first_note: *first,
                    time_ms: time,
                });
            }
        }
        changes
    }
}

/// p1_lines lists the lines of every voice numbered 1, in document order.
fn p1_lines(song: &Song) -> impl Iterator<Item = &Line> {
    song.voices
        .iter()
        .filter(|voice| voice.number == 1)
        .flat_map(|voice| &voice.lines)
}

/// line_time converts an absolute start beat to milliseconds from the audio
/// start; without a `#BPM` timing the beat value stands in for the time.
fn line_time(song: &Song, start: f64) -> f64 {
    song.timing
        .map(|timing| timing.beat_to_ms(start))
        .unwrap_or(start)
}
