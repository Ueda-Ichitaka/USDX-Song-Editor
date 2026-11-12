//! The derived, read-only song view of the UltraStar song format.
//!
//! About me: this module indexes a document into voices and lines the way the
//! Factory's note_types.py splits them (a line ends at an end-of-phrase marker,
//! a voice change or the end row), resolves the song metadata from the headers
//! and looks notes up with absolute beats. It exists so that the editor works
//! on a cheap, rebuildable view while the document keeps every raw row.

use crate::headers::{Headers, Version};
use crate::rows::{Document, NoteRow, NoteType, RowId, RowKind};
use crate::timing::Timing;

/// One line of a voice: its note rows, the end-of-phrase row that ends it (if
/// any) and its index within the voice, counting from 0 internally and shown
/// from 1 in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub notes: Vec<RowId>,
    pub end: Option<RowId>,
    pub index: usize,
}

impl Line {
    /// text joins the line's syllables. A syllable without a leading space
    /// continues the word, a tab in a syllable reads as a space, and the joined
    /// text is trimmed. The note rows keep their text verbatim.
    pub fn text(&self, doc: &Document) -> String {
        let mut joined = String::new();
        for row_id in &self.notes {
            if let Some(row) = doc.rows.get(row_id.0 as usize)
                && let RowKind::Note(note) = &row.kind
            {
                joined.push_str(&note.text.replace('\t', " "));
            }
        }
        joined.trim().to_owned()
    }
}

/// One voice of the song with its lines. A song without voice changes has the
/// implicit voice 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    pub number: u8,
    pub lines: Vec<Line>,
}

/// A note with its absolute start beat. In relative mode the absolute start is
/// the row's start shifted by the running relative offset, which the `#GAP`
/// value can make fractional.
#[derive(Clone, Debug, PartialEq)]
pub struct AbsNote {
    pub row: RowId,
    pub kind: NoteType,
    pub start: f64,
    pub length: i64,
    pub pitch: i64,
    pub text: String,
}

/// The song view of a document, rebuilt after each change.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub headers: Headers,
    pub timing: Option<Timing>,
    pub version: Version,
    pub relative: bool,
    pub voices: Vec<Voice>,
}

impl Song {
    /// from_document rebuilds the song view from a document. Songs have about
    /// 1000 rows, so a full rebuild stays cheap.
    pub fn from_document(doc: &Document) -> Song {
        let headers = Headers::from_document(doc);
        let timing = Timing::from_headers(&headers);
        let version = match headers.get("VERSION") {
            Some(value) => Version::parse(value),
            None => Version::Unversioned,
        };
        let relative = headers
            .get("RELATIVE")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        Song {
            headers,
            timing,
            version,
            relative,
            voices: build_voices(doc),
        }
    }

    /// note returns the note data of a row with absolute beats, or `None` when
    /// the row is not a note or the id is unknown.
    pub fn note(&self, doc: &Document, id: RowId) -> Option<AbsNote> {
        let row = doc.rows.get(id.0 as usize)?;
        let RowKind::Note(note) = &row.kind else {
            return None;
        };
        // In non-relative mode the start beat is the row's own start; the
        // relative-mode shift is added by the relative-mode reading.
        let start = if self.relative {
            relative_start(doc, note)
        } else {
            note.start as f64
        };
        Some(AbsNote {
            row: id,
            kind: note.kind.clone(),
            start,
            length: note.length,
            pitch: note.pitch,
            text: note.text.clone(),
        })
    }
}

/// relative_start computes the absolute start of a note in relative mode. The
/// relative-mode reading (P1-06) adds the running offset of the voice; until it
/// lands, the start is returned unshifted.
fn relative_start(doc: &Document, note: &NoteRow) -> f64 {
    let _ = doc;
    note.start as f64
}

/// build_voices walks the rows and builds the voices with their lines. A line
/// ends at an end-of-phrase marker, at a voice change or at the end row; an
/// end-of-phrase marker without an open line creates no line, and empty and
/// invalid rows belong to no line.
fn build_voices(doc: &Document) -> Vec<Voice> {
    let mut voices: Vec<Voice> = Vec::new();
    // The implicit voice 1 is current until the first voice change.
    let mut current_voice = 1u8;
    let mut current_line: Option<Line> = None;

    for row in &doc.rows {
        match &row.kind {
            RowKind::Note(_) => {
                ensure_voice(&mut voices, current_voice);
                let line = current_line.get_or_insert_with(|| Line {
                    notes: Vec::new(),
                    end: None,
                    index: open_line_count(&voices, current_voice),
                });
                line.notes.push(row.id);
            }
            RowKind::PhraseEnd { .. } => {
                if let Some(mut line) = current_line.take() {
                    line.end = Some(row.id);
                    push_line(&mut voices, current_voice, line);
                }
            }
            RowKind::Voice(number) => {
                if let Some(line) = current_line.take() {
                    push_line(&mut voices, current_voice, line);
                }
                current_voice = *number;
            }
            RowKind::End => {
                if let Some(line) = current_line.take() {
                    push_line(&mut voices, current_voice, line);
                }
            }
            RowKind::Header { .. } | RowKind::Empty | RowKind::Invalid { .. } => {}
        }
    }
    if let Some(line) = current_line.take() {
        push_line(&mut voices, current_voice, line);
    }
    voices
}

/// ensure_voice adds a voice entry with the number when it is not the current
/// last entry, so the implicit voice 1 and a later P1 row share one entry while
/// a P1 row after P2 opens a new voice 1 entry.
fn ensure_voice(voices: &mut Vec<Voice>, number: u8) {
    if voices.last().map(|voice| voice.number) != Some(number) {
        voices.push(Voice {
            number,
            lines: Vec::new(),
        });
    }
}

/// open_line_count counts the lines of the current voice.
fn open_line_count(voices: &[Voice], number: u8) -> usize {
    voices
        .last()
        .filter(|voice| voice.number == number)
        .map(|voice| voice.lines.len())
        .unwrap_or(0)
}

/// push_line appends a finished line to the current voice.
fn push_line(voices: &mut Vec<Voice>, number: u8, line: Line) {
    ensure_voice(voices, number);
    if let Some(voice) = voices.last_mut() {
        voice.lines.push(line);
    }
}
