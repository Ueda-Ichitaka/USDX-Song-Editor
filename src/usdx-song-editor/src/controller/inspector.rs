//! The inspector state (UI_SPEC 4.3 item 3, task P5-05).
//!
//! About me: this module derives what the note inspector shows for the
//! current selection as a pure function over the document and the song
//! view, so the bridge only publishes the result and the QML renders it.
//! A single selection shows the note's values (with the note name and the
//! pitch editability of freestyle, rap and rap-golden notes); several
//! selected notes show their count, the common type and the common pitch
//! (a pitch change then applies a transposition); nothing selected shows
//! the empty state.

use std::collections::BTreeSet;

use usdx_format::rows::{Document, NoteType, RowId};
use usdx_format::song::Song;

use crate::controller::notes::{PITCH_C4_MIDI, type_char};
use crate::controller::sheet::pitch_name;

/// What the inspector shows for the selection.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorState {
    /// Nothing selected: every field disabled and empty.
    None,
    /// One selected note: the editable values.
    Single {
        /// "Note 2 · " - the 1-based note index within its line; QML
        /// appends the note text in DemiBold with spaces as U+2423.
        label: String,
        /// The type character of the note.
        type_code: char,
        /// The start beat.
        start: i64,
        /// The length in beats (at least 1).
        length: i64,
        /// The MIDI pitch (C4 = 60, the sheet axis).
        pitch: i64,
        /// The pitch as a note name ("D4").
        pitch_name: String,
        /// The syllable text as it is edited (raw, spaces as spaces).
        text: String,
        /// False for freestyle, rap and rap-golden notes: the Pitch field
        /// greys out with its value kept (UI_SPEC 4.3).
        pitch_editable: bool,
    },
    /// Several selected notes: the summary. Start, Length and Text are
    /// disabled; a pitch change applies a transposition.
    Multi {
        /// How many notes are selected.
        count: usize,
        /// The common type character, or `None` when the types differ.
        type_code: Option<char>,
        /// The common MIDI pitch, or `None` when the pitches differ.
        common_pitch: Option<i64>,
    },
}

/// inspector_state derives the inspector contents of a selection; the ids
/// may arrive in any order and hold duplicates.
pub fn inspector_state(document: &Document, song: &Song, selection: &[RowId]) -> InspectorState {
    let selection: BTreeSet<RowId> = selection.iter().copied().collect();
    if selection.is_empty() {
        return InspectorState::None;
    }
    if selection.len() == 1 {
        let Some(note) = song.note(document, selection.iter().copied().next().expect("len 1"))
        else {
            return InspectorState::None;
        };
        let pitch = i64::from(PITCH_C4_MIDI) + note.pitch;
        InspectorState::Single {
            label: format!("Note {} \u{b7} ", note_index_in_line(song, note.row)),
            type_code: type_char(&note.kind).chars().next().unwrap_or(':'),
            start: note.start.round() as i64,
            length: note.length,
            pitch,
            pitch_name: pitch_name(pitch as i32),
            text: note.text,
            pitch_editable: matches!(note.kind, NoteType::Normal | NoteType::Golden),
        }
    } else {
        let mut types: Vec<char> = Vec::new();
        let mut pitches: Vec<i64> = Vec::new();
        for id in &selection {
            let Some(note) = song.note(document, *id) else {
                continue;
            };
            types.push(type_char(&note.kind).chars().next().unwrap_or(':'));
            pitches.push(i64::from(PITCH_C4_MIDI) + note.pitch);
        }
        types.sort();
        types.dedup();
        pitches.sort();
        pitches.dedup();
        InspectorState::Multi {
            count: selection.len(),
            type_code: if types.len() == 1 { types.pop() } else { None },
            common_pitch: if pitches.len() == 1 {
                pitches.pop()
            } else {
                None
            },
        }
    }
}

/// note_index_in_line finds the 1-based position of a note within its
/// line; an unknown row falls back to 1.
fn note_index_in_line(song: &Song, id: RowId) -> usize {
    for voice in &song.voices {
        for line in &voice.lines {
            if let Some(index) = line.notes.iter().position(|note| *note == id) {
                return index + 1;
            }
        }
    }
    1
}
