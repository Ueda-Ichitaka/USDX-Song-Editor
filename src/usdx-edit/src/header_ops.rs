//! The header operations of the edit operations.
//!
//! About me: this module builds the undoable patches for header edits:
//! setting and removing single headers, setting the `#GAP` offset in
//! milliseconds and changing the `#BPM` tempo, optionally rescaling every
//! beat so the notes stay at their audio time. Each function takes the
//! document, validates the edit and returns one `Patch`; it changes nothing
//! itself. It exists so the metadata dialogs and the tempo tools produce the
//! same invertible patches as the note edits.

use usdx_format::headers::Headers;
use usdx_format::rows::{Document, RowId, RowKind};
use usdx_format::serialize::canonical_text;
use usdx_format::timing::Timing;

use crate::ops::{body_start_index, edited, new_row};
use crate::patch::RowOp;

pub use crate::patch::{EditError, Patch};

/// set_header sets the value of the header `key`, matching the key case
/// insensitively; a missing header is added at the end of the header
/// section, before the first body row.
pub fn set_header(doc: &Document, key: &str, value: &str) -> Result<Patch, EditError> {
    let upper = key.to_uppercase();
    let (label, op) = match header_index(doc, &upper) {
        Some(index) => (format!("Set {upper}"), edited_header(doc, index, value)),
        None => {
            let index = body_start_index(doc);
            let row = new_row(
                index,
                RowKind::Header {
                    key: upper.clone(),
                    value: value.to_owned(),
                },
            );
            (
                format!("Set {upper}"),
                RowOp::Insert {
                    after: if index == 0 {
                        None
                    } else {
                        Some(RowId(index as u32 - 1))
                    },
                    row,
                },
            )
        }
    };
    Ok(Patch {
        label,
        ops: vec![op],
    })
}

/// remove_header removes the header `key`; removing a key the file does not
/// carry yields an empty patch, so the command stays idempotent.
pub fn remove_header(doc: &Document, key: &str) -> Result<Patch, EditError> {
    let upper = key.to_uppercase();
    let ops = match header_index(doc, &upper) {
        Some(index) => vec![RowOp::Remove {
            id: RowId(index as u32),
            row: doc.rows[index].clone(),
            after: if index == 0 {
                None
            } else {
                Some(RowId(index as u32 - 1))
            },
        }],
        None => Vec::new(),
    };
    Ok(Patch {
        label: format!("Remove {upper}"),
        ops,
    })
}

/// set_gap_ms writes the `#GAP` header in milliseconds, the offset from the
/// audio start to beat 0.
pub fn set_gap_ms(doc: &Document, ms: f64) -> Result<Patch, EditError> {
    if !ms.is_finite() {
        return Err(EditError::InvalidHeaderValue);
    }
    set_header(doc, "GAP", &format_decimal(ms))
}

/// set_bpm writes the `#BPM` header. With `keep_times` every note start and
/// length and every phrase-end beat is rescaled by `round(beat * new / old)`
/// with lengths at least one, so the notes stay at the same audio time.
pub fn set_bpm(doc: &Document, bpm: f64, keep_times: bool) -> Result<Patch, EditError> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(EditError::InvalidHeaderValue);
    }
    if !keep_times {
        return set_header(doc, "BPM", &format_decimal(bpm));
    }
    // Relative-mode rows store relative beats; rescaling them needs the
    // relative-form editing of task P9-06 behind gate G-01.
    let headers = Headers::from_document(doc);
    let relative = headers
        .get("RELATIVE")
        .map(|value| value.eq_ignore_ascii_case("yes"))
        .unwrap_or(false);
    if relative {
        return Err(EditError::RelativeModeIsReadOnly);
    }
    let timing = Timing::from_headers(&headers).ok_or(EditError::NoTiming)?;
    let factor = bpm / timing.bpm;
    let mut ops = Vec::new();
    for (index, row) in doc.rows.iter().enumerate() {
        match &row.kind {
            RowKind::Note(note) => {
                let start = (note.start as f64 * factor).round() as i64;
                let length = ((note.length as f64 * factor).round() as i64).max(1);
                ops.push(edited(doc, index, |kind| {
                    if let RowKind::Note(rescaled) = kind {
                        rescaled.start = start;
                        rescaled.length = length;
                    }
                }));
            }
            RowKind::PhraseEnd {
                beat: Some(beat), ..
            } => {
                let moved = (*beat as f64 * factor).round() as i64;
                ops.push(edited(doc, index, |kind| {
                    if let RowKind::PhraseEnd { beat, .. } = kind {
                        *beat = Some(moved);
                    }
                }));
            }
            _ => {}
        }
    }
    // The timing exists, so the BPM row exists too and is replaced in place.
    let bpm_index = header_index(doc, "BPM").ok_or(EditError::NoTiming)?;
    ops.push(edited_header(doc, bpm_index, &format_decimal(bpm)));
    Ok(Patch {
        label: "Set BPM".to_owned(),
        ops,
    })
}

/// header_index returns the row index of the header with the upper-case key.
fn header_index(doc: &Document, upper: &str) -> Option<usize> {
    doc.rows
        .iter()
        .position(|row| matches!(&row.kind, RowKind::Header { key, .. } if key.as_str() == upper))
}

/// edited_header builds the replace op for the header row at `index` with a
/// new value; the row becomes dirty and keeps its raw key spelling.
fn edited_header(doc: &Document, index: usize, value: &str) -> RowOp {
    let before = doc.rows[index].clone();
    let mut after = before.clone();
    if let RowKind::Header { value: current, .. } = &mut after.kind {
        *current = value.to_owned();
    }
    after.dirty = true;
    after.raw.text = canonical_text(&after);
    RowOp::Replace {
        id: RowId(index as u32),
        before,
        after,
    }
}

/// format_decimal writes a header number the way the format reads it back:
/// plain decimal notation without an exponent.
fn format_decimal(value: f64) -> String {
    format!("{value}")
}
