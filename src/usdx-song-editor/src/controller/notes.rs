//! About me: the visible note and phrase end sets of the note sheet (plan
//! 8.3, task P5-03). The NotesModel and PhraseEndsModel bridges rebuild
//! their rows from these plain functions, and the lyrics lane's note data
//! (P5-02) shares the line gathering. The gathering comes in two sizes:
//! `neighbour_lines` is the current line plus its global previous and next
//! line (user decision 2026-11-13), and `all_lines` is the whole song (user
//! decision 2026-11-15, so scrolling and the scrollbar reach every line).
//! Pitches are MIDI (C4 = 60, UI_SPEC 3.3) while the USDX file stores
//! relative pitches; the phrase
//! ends carry the same bounds `usdx_edit::ops::move_phrase_end` enforces:
//! between the previous note's end and the next note's start, unbounded
//! where no note sits on that side.

use usdx_format::rows::{Document, NoteType, RowId};
use usdx_format::song::{Line, Song};

use crate::controller::EditorController;

/// The MIDI pitch of USDX relative pitch 0: the USDX format stores pitches
/// relative to C4, and the sheet's pitch rows and note names use MIDI
/// numbers (C4 = 60, UI_SPEC 3.3).
pub const PITCH_C4_MIDI: i32 = 60;

/// One visible note row for the sheet's note delegates.
#[derive(Debug, Clone, PartialEq)]
pub struct VisibleNote {
    /// The note's row id (the id the edit operations take).
    pub row_id: RowId,
    /// The absolute start beat.
    pub start: f64,
    /// The length in beats.
    pub length: i64,
    /// The pitch in MIDI numbers (C4 = 60).
    pub pitch: i32,
    /// The note type as its chip character (`:`, `*`, `F`, `R`, `G`).
    pub kind: &'static str,
    /// The syllable text, a tab read as a space, trimmed.
    pub text: String,
    /// The distance in lines from the current line to the note's own line
    /// (0 for the current line, negative for earlier lines, positive for
    /// later ones). The delegate's click navigation adds it to the current
    /// line index.
    pub line_offset: i32,
}

/// One visible phrase end row for the sheet's phrase end handles.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseEndRow {
    /// The end-of-phrase row's id (the id `move_phrase_end` takes).
    pub row_id: RowId,
    /// The absolute beat of the marker.
    pub beat: f64,
    /// The lowest allowed beat: the previous note's end.
    pub min: f64,
    /// The highest allowed beat: the next note's start.
    pub max: f64,
}

/// type_char names a note type with its chip character. Custom types read
/// as freestyle, like the lines pane's type summary.
pub fn type_char(kind: &NoteType) -> &'static str {
    match kind {
        NoteType::Normal => ":",
        NoteType::Golden => "*",
        NoteType::Freestyle | NoteType::Other(_) => "F",
        NoteType::Rap => "R",
        NoteType::RapGolden => "G",
    }
}

/// visible_notes collects the notes of the gathered lines with their line
/// offsets, ordered by line then start. `current` is the current line's
/// index within `lines`.
pub fn visible_notes(
    doc: &Document,
    song: &Song,
    lines: &[&Line],
    current: usize,
) -> Vec<VisibleNote> {
    let mut notes = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let line_offset = index as i32 - current as i32;
        for row_id in &line.notes {
            let Some(note) = song.note(doc, *row_id) else {
                continue;
            };
            notes.push(VisibleNote {
                row_id: note.row,
                start: note.start,
                length: note.length,
                pitch: note.pitch as i32 + PITCH_C4_MIDI,
                kind: type_char(&note.kind),
                text: note.text.replace('\t', " ").trim().to_owned(),
                line_offset,
            });
        }
    }
    notes.sort_by(|a, b| {
        a.line_offset
            .cmp(&b.line_offset)
            .then(a.start.total_cmp(&b.start))
    });
    notes
}

/// visible_phrase_ends collects the end-of-phrase markers of the gathered
/// lines with their drag bounds, in line order. The bounds scan the
/// document rows around the marker exactly like `move_phrase_end`: the
/// last note's end before the marker and the first note's start after it,
/// in absolute beats.
pub fn visible_phrase_ends(doc: &Document, song: &Song, lines: &[&Line]) -> Vec<PhraseEndRow> {
    let mut ends = Vec::new();
    for line in lines {
        let Some(row_id) = line.end else {
            continue;
        };
        let Some(beat) = song.phrase_end_beat(doc, row_id) else {
            continue;
        };
        // The bounds of `move_phrase_end`, read on the absolute beat axis.
        let marker_index = row_id.0 as usize;
        let min = doc.rows[..marker_index]
            .iter()
            .rev()
            .find_map(|row| {
                song.note(doc, row.id)
                    .map(|note| note.start + note.length as f64)
            })
            .unwrap_or(f64::NEG_INFINITY);
        let max = doc.rows[marker_index + 1..]
            .iter()
            .find_map(|row| song.note(doc, row.id).map(|note| note.start))
            .unwrap_or(f64::INFINITY);
        ends.push(PhraseEndRow {
            row_id,
            beat,
            min,
            max,
        });
    }
    ends
}

/// neighbour_lines gathers the current line with its global previous and
/// next line, and reports the current line's index within them. An empty
/// controller or an empty song gathers nothing.
pub fn neighbour_lines(controller: &EditorController) -> (Vec<&Line>, usize) {
    let current = controller.current_line();
    let mut lines: Vec<&Line> = Vec::new();
    let mut current_index = 0;
    if let Some(previous) = current
        .checked_sub(1)
        .and_then(|index| controller.line_data(index))
    {
        lines.push(previous);
        current_index += 1;
    }
    if let Some(line) = controller.line_data(current) {
        lines.push(line);
    }
    if let Some(next) = controller.line_data(current + 1) {
        lines.push(next);
    }
    (lines, current_index)
}

/// all_lines gathers every line of the song in global order and reports the
/// current line's index within them (user decision 2026-11-15: the sheet
/// paints the whole song, so scrolling and the scrollbar reach every line).
/// An empty controller or an empty song gathers nothing.
pub fn all_lines(controller: &EditorController) -> (Vec<&Line>, usize) {
    let lines: Vec<&Line> = (0..controller.line_count())
        .filter_map(|index| controller.line_data(index))
        .collect();
    let current = controller.current_line().min(lines.len().saturating_sub(1));
    (lines, current)
}

/// song_end_beat returns the last note end or phrase end beat across all
/// voices: the full range the sheet's horizontal scrollbar spans. An empty
/// song returns 0.
pub fn song_end_beat(doc: &Document, song: &Song) -> f64 {
    let mut end: f64 = 0.0;
    for voice in &song.voices {
        for line in &voice.lines {
            for row_id in &line.notes {
                if let Some(note) = song.note(doc, *row_id) {
                    end = end.max(note.start + note.length as f64);
                }
            }
            if let Some(row_id) = line.end {
                if let Some(beat) = song.phrase_end_beat(doc, row_id) {
                    end = end.max(beat);
                }
            }
        }
    }
    end
}
