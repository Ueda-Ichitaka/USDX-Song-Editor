//! The serializer of the UltraStar song format.
//!
//! About me: this module writes a document back to bytes. Unmodified rows keep
//! their raw line and their own line terminator, dirty rows are formatted
//! canonically (single ASCII spaces, the raw header key spelling kept), new
//! rows take the document's dominant terminator, and the BOM, the encoding and
//! the trailing lines are written back unchanged. It exists so that
//! `serialize(classify(decode(bytes))) == bytes` holds for every input that
//! `decode` accepts and an edited song saves with as few surprises as possible.

use crate::bytes::{DecodedFile, FormatError, RawLine, Terminator, encode};
use crate::rows::{Document, NoteType, Row, RowKind};

/// serialize writes the document back to bytes in its own encoding, with the
/// BOM when the document carries one.
pub fn serialize(doc: &Document) -> Result<Vec<u8>, FormatError> {
    let dominant = dominant_terminator(doc);
    let mut lines = Vec::with_capacity(doc.rows.len() + doc.trailing.len());
    for row in &doc.rows {
        let text = if row.dirty {
            canonical_text(row)
        } else {
            row.raw.text.clone()
        };
        // A new row (no raw line yet) takes the dominant terminator; an edited
        // existing row keeps its own.
        let terminator = if row.dirty && row.raw.text.is_empty() {
            dominant
        } else {
            row.raw.terminator
        };
        lines.push(RawLine { text, terminator });
    }
    lines.extend(doc.trailing.iter().cloned());
    encode(&DecodedFile {
        bom: doc.file.bom,
        encoding: doc.file.encoding,
        encoding_source: doc.file.encoding_source,
        lines,
    })
}

/// dominant_terminator returns the most common terminator of the document's
/// rows and trailing lines; a tie resolves to LF, and a document without any
/// terminator yields LF as well.
fn dominant_terminator(doc: &Document) -> Terminator {
    let mut count_lf = 0usize;
    let mut count_crlf = 0usize;
    let mut count_cr = 0usize;
    for line in doc
        .rows
        .iter()
        .map(|row| &row.raw)
        .chain(doc.trailing.iter())
    {
        match line.terminator {
            Terminator::Lf => count_lf += 1,
            Terminator::CrLf => count_crlf += 1,
            Terminator::Cr => count_cr += 1,
            Terminator::None => {}
        }
    }
    let max = count_lf.max(count_crlf).max(count_cr);
    if max == 0 {
        return Terminator::Lf;
    }
    // LF wins ties because it is checked first.
    if count_lf == max {
        Terminator::Lf
    } else if count_crlf == max {
        Terminator::CrLf
    } else {
        Terminator::Cr
    }
}

/// canonical_text formats a dirty row: notes and phrase ends with single ASCII
/// spaces, headers with the raw key spelling kept for edited rows and the
/// uppercase key for new ones. Rows that an edit cannot produce keep their raw
/// text.
fn canonical_text(row: &Row) -> String {
    match &row.kind {
        RowKind::Header { key, value } => match row.raw.text.find(':') {
            // Keep the original spelling up to and including the first colon
            // and replace only the value.
            Some(colon) => format!("{}{}", &row.raw.text[..colon + 1], value),
            // A new header has no raw line yet and is written canonically.
            None => format!("#{key}:{value}"),
        },
        RowKind::Note(note) => format!(
            "{} {} {} {} {}",
            type_char(&note.kind),
            note.start,
            note.length,
            note.pitch,
            note.text
        ),
        RowKind::PhraseEnd { beat, .. } => match beat {
            // The absolute form; saving a relative-mode song in relative form
            // is behind gate G-01.
            Some(beat) => format!("- {beat}"),
            None => "-".to_owned(),
        },
        RowKind::Voice(number) => format!("P{number}"),
        RowKind::End => "E".to_owned(),
        RowKind::Empty | RowKind::Invalid { .. } => row.raw.text.clone(),
    }
}

/// type_char returns the note type character of a note kind.
fn type_char(kind: &NoteType) -> char {
    match kind {
        NoteType::Normal => ':',
        NoteType::Golden => '*',
        NoteType::Freestyle => 'F',
        NoteType::Rap => 'R',
        NoteType::RapGolden => 'G',
        NoteType::Other(c) => *c,
    }
}
