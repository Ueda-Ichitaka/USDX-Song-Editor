//! The patch layer of the edit operations.
//!
//! About me: this module defines `Patch`, the one invertible unit every user
//! action produces: a list of row operations that carries enough data to
//! undo itself exactly. `apply` changes a document and fails cleanly (leaving
//! the document unchanged) when the patch was built for another document
//! state; `inverse` maps a patch to the patch that restores the previous
//! state. It exists so the history stack can undo and redo without snapshots.

use usdx_format::rows::{Document, Row, RowId};

/// An error of the edit operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    /// The patch names a row the document does not have.
    #[error("the row {0:?} does not exist in the document")]
    UnknownRow(RowId),
    /// The patch was built for a different document state.
    #[error("the patch does not fit the document: {0}")]
    StalePatch(&'static str),
    /// The edit would move a note before the song start.
    #[error("the edit would move a note before the song start")]
    BeforeSongStart,
    /// The edit would give a note a length below one beat.
    #[error("the edit would give a note a length below one beat")]
    LengthBelowOne,
    /// The phrase end would leave the range between the previous note's end
    /// and the next note's start.
    #[error(
        "the phrase end beat is outside the range between the previous note's end and the next note's start"
    )]
    PhraseEndOutOfRange,
    /// A phrase end, a voice change or the end row stands between the notes,
    /// so the edit cannot cross it.
    #[error("a line boundary stands between the notes")]
    PhraseEndBetween,
    /// A relative-mode song is read-only: its stored relative form cannot
    /// express the beat edits (editing it is task P9-06 behind gate G-01).
    #[error("a relative-mode song is read-only")]
    RelativeModeIsReadOnly,
    /// The requested header value is not a usable number.
    #[error("the header value is not a usable number")]
    InvalidHeaderValue,
    /// The operation needs a usable `#BPM` timing and the song has none.
    #[error("the song has no usable #BPM timing")]
    NoTiming,
    // Later tasks add their own variants (NeedsDecision, AnchorsOutOfOrder).
}

/// One row operation. The ids and the carried rows refer to the document
/// state the operation is applied to; a later operation of the same patch
/// sees the state the earlier ones left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowOp {
    /// Replaces the row `id`; `before` must be the row the document carries,
    /// `after` the row it shall carry.
    Replace { id: RowId, before: Row, after: Row },
    /// Inserts `row` directly after the row `after`, or at the start of the
    /// row list when `after` is `None`. The inserted row's id names the index
    /// it will receive, which makes `inverse` a pure function; the plan's
    /// sketch comment calls this position "the start of the body", which is
    /// the same position for a file whose rows begin with the body.
    Insert { after: Option<RowId>, row: Row },
    /// Removes the row `id`, which must be the row the document carries.
    /// `after` is the row the removed row stood behind, or `None` when it
    /// stood at the start of the row list.
    Remove {
        id: RowId,
        row: Row,
        after: Option<RowId>,
    },
}

/// A patch: the label shown in the undo and redo menus and the operations in
/// application order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub label: String,
    pub ops: Vec<RowOp>,
}

impl Patch {
    /// apply runs the operations in order on the document. When an operation
    /// does not fit, the operations applied before it are rolled back with
    /// their exact inverses in reverse order, so the document is left
    /// unchanged, and the error tells why.
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError> {
        for (index, op) in self.ops.iter().enumerate() {
            if let Err(error) = apply_op(op, doc) {
                for done in self.ops[..index].iter().rev() {
                    // The inverse of an op that just applied always fits the
                    // state it left behind.
                    apply_op(&inverse_op(done), doc)
                        .expect("the inverse of an applied op fits the document");
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// inverse returns the patch that undoes this one: the operations in
    /// reverse order, each inverted. The carried rows make the inversion
    /// exact, including dirty flags and raw text.
    pub fn inverse(&self) -> Patch {
        Patch {
            label: self.label.clone(),
            ops: self.ops.iter().rev().map(inverse_op).collect(),
        }
    }
}

/// apply_op applies one operation.
fn apply_op(op: &RowOp, doc: &mut Document) -> Result<(), EditError> {
    match op {
        RowOp::Replace { id, before, after } => {
            let index = id.0 as usize;
            let current = doc.rows.get(index).ok_or(EditError::UnknownRow(*id))?;
            if current != before {
                return Err(EditError::StalePatch(
                    "the row changed since the patch was built",
                ));
            }
            if after.id != *id {
                return Err(EditError::StalePatch(
                    "the replacement row carries a stale id",
                ));
            }
            doc.rows[index] = after.clone();
        }
        RowOp::Insert { after, row } => {
            let index = match after {
                Some(predecessor) => {
                    let index = predecessor.0 as usize + 1;
                    if predecessor.0 as usize >= doc.rows.len() {
                        return Err(EditError::UnknownRow(*predecessor));
                    }
                    index
                }
                None => 0,
            };
            if row.id.0 as usize != index {
                return Err(EditError::StalePatch(
                    "the inserted row carries the id of another position",
                ));
            }
            doc.rows.insert(index, row.clone());
            renumber(doc);
        }
        RowOp::Remove { id, row, after } => {
            let index = id.0 as usize;
            let current = doc.rows.get(index).ok_or(EditError::UnknownRow(*id))?;
            if current != row {
                return Err(EditError::StalePatch(
                    "the row changed since the patch was built",
                ));
            }
            let expected_after = if index == 0 {
                None
            } else {
                Some(RowId(index as u32 - 1))
            };
            if *after != expected_after {
                return Err(EditError::StalePatch(
                    "the removed row's predecessor is not the one the patch carries",
                ));
            }
            doc.rows.remove(index);
            renumber(doc);
        }
    }
    Ok(())
}

/// renumber restores the invariant that a row's id is its index.
fn renumber(doc: &mut Document) {
    for (index, row) in doc.rows.iter_mut().enumerate() {
        row.id = RowId(index as u32);
    }
}

/// inverse_op inverts one operation.
fn inverse_op(op: &RowOp) -> RowOp {
    match op {
        RowOp::Replace { id, before, after } => RowOp::Replace {
            id: *id,
            before: after.clone(),
            after: before.clone(),
        },
        RowOp::Insert { after, row } => RowOp::Remove {
            id: row.id,
            row: row.clone(),
            after: *after,
        },
        RowOp::Remove { id: _, row, after } => RowOp::Insert {
            after: *after,
            row: row.clone(),
        },
    }
}
