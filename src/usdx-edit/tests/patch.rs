//! About me: tests for the patch layer (task P2-01, plan section 6.1). They
//! assert that replace, insert and remove change the document exactly, that a
//! patch built for another document state fails cleanly and leaves the
//! document untouched, that every patch's inverse restores the original
//! document (including dirty flags and raw text), and the apply/inverse
//! identity proptest over generated documents and generated valid patches.

use proptest::prelude::*;
use usdx_edit::{EditError, Patch, RowOp};
use usdx_format::bytes::{RawLine, Terminator, decode};
use usdx_format::rows::{Document, NoteRow, NoteType, Row, RowId, RowKind, classify};

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// note_row builds a note row kind.
fn note_row(start: i64, pitch: i64, text: &str) -> RowKind {
    RowKind::Note(NoteRow {
        kind: NoteType::Normal,
        start,
        length: 4,
        pitch,
        text: text.to_owned(),
    })
}

/// raw builds a raw line with the given text and an LF terminator.
fn raw(text: &str) -> RawLine {
    RawLine {
        text: text.to_owned(),
        terminator: Terminator::Lf,
    }
}

/// row builds a row for a patch.
fn row(id: u32, kind: RowKind, text: &str) -> Row {
    Row {
        id: RowId(id),
        raw: raw(text),
        kind,
        dirty: false,
    }
}

/// applied runs a patch on a copy of the document and returns the copy.
fn applied(patch: &Patch, doc: &Document) -> Document {
    let mut copy = doc.clone();
    patch
        .apply(&mut copy)
        .unwrap_or_else(|error| panic!("the patch does not apply: {error}"));
    copy
}

/// restored runs a patch and its inverse on a copy and returns the copy.
fn restored(patch: &Patch, doc: &Document) -> Document {
    let mut copy = doc.clone();
    patch
        .apply(&mut copy)
        .unwrap_or_else(|error| panic!("the patch does not apply: {error}"));
    patch
        .inverse()
        .apply(&mut copy)
        .unwrap_or_else(|error| panic!("the inverse does not apply: {error}"));
    copy
}

/// changed_row clones a document row with a changed start beat and dirty flag.
fn changed_row(doc: &Document, index: usize) -> Row {
    let mut after = doc.rows[index].clone();
    if let RowKind::Note(note) = &mut after.kind {
        note.start += 1;
    }
    after.dirty = true;
    after
}

// ---- Replace ----

#[test]
fn a_replace_changes_the_row_in_place() {
    let d = doc("#TITLE:T\n: 0 4 0 la\nE\n");
    let before = d.rows[1].clone();
    let after = changed_row(&d, 1);
    let patch = Patch {
        label: "move note".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(1),
            before,
            after,
        }],
    };
    let out = applied(&patch, &d);
    assert!(out.rows[1].dirty);
    assert!(matches!(&out.rows[1].kind, RowKind::Note(note) if note.start == 1));
    assert_eq!(
        out.rows.len(),
        d.rows.len(),
        "a replace keeps the row count"
    );
}

#[test]
fn a_replace_with_a_mismatched_before_fails_cleanly() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "stale".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(0),
            before: row(0, note_row(9, 0, "other"), ": 9 4 0 other"),
            after: changed_row(&d, 0),
        }],
    };
    let mut copy = d.clone();
    let error = patch
        .apply(&mut copy)
        .expect_err("the stale patch is refused");
    assert!(matches!(error, EditError::StalePatch(_)));
    assert_eq!(copy, d, "a refused patch leaves the document unchanged");
}

#[test]
fn a_replace_for_an_unknown_row_fails_cleanly() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "gone".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(7),
            before: row(7, note_row(0, 0, "x"), ": 0 4 0 x"),
            after: row(7, note_row(1, 0, "x"), ": 1 4 0 x"),
        }],
    };
    let mut copy = d.clone();
    let error = patch.apply(&mut copy).expect_err("the row does not exist");
    assert_eq!(error, EditError::UnknownRow(RowId(7)));
    assert_eq!(copy, d);
}

// ---- Insert ----

#[test]
fn an_insert_without_a_predecessor_lands_at_the_start() {
    // after: None inserts at the start of the row list; the plan's comment
    // says "start of the body", which is the same position for a file whose
    // rows begin with the body.
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "add first note".to_owned(),
        ops: vec![RowOp::Insert {
            after: None,
            row: row(0, note_row(0, 0, "new"), ": 0 4 0 new"),
        }],
    };
    let out = applied(&patch, &d);
    assert_eq!(out.rows.len(), 3);
    assert!(matches!(&out.rows[0].kind, RowKind::Note(note) if note.text == "new"));
    // The later rows are renumbered.
    assert_eq!(out.rows[1].id, RowId(1));
    assert_eq!(out.rows[1].raw.text, ": 0 4 0 la");
}

#[test]
fn an_insert_after_a_header_row_starts_the_body() {
    let d = doc("#TITLE:T\n: 0 4 0 la\nE\n");
    let patch = Patch {
        label: "add note".to_owned(),
        ops: vec![RowOp::Insert {
            after: Some(RowId(0)),
            row: row(1, note_row(4, 1, "new"), ": 4 4 1 new"),
        }],
    };
    let out = applied(&patch, &d);
    assert_eq!(out.rows[1].raw.text, ": 4 4 1 new");
    assert_eq!(out.rows[2].raw.text, ": 0 4 0 la");
    assert_eq!(out.rows[2].id, RowId(2));
}

#[test]
fn an_insert_with_a_mismatched_row_id_fails_cleanly() {
    // The row id names the index the inserted row will receive; a patch built
    // for another document state carries a stale id.
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "stale".to_owned(),
        ops: vec![RowOp::Insert {
            after: None,
            row: row(1, note_row(0, 0, "new"), ": 0 4 0 new"),
        }],
    };
    let mut copy = d.clone();
    let error = patch.apply(&mut copy).expect_err("the id is stale");
    assert!(matches!(error, EditError::StalePatch(_)));
    assert_eq!(copy, d);
}

#[test]
fn an_insert_after_an_unknown_row_fails_cleanly() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "gone".to_owned(),
        ops: vec![RowOp::Insert {
            after: Some(RowId(5)),
            row: row(6, note_row(0, 0, "new"), ": 0 4 0 new"),
        }],
    };
    let mut copy = d.clone();
    let error = patch.apply(&mut copy).expect_err("the row does not exist");
    assert_eq!(error, EditError::UnknownRow(RowId(5)));
    assert_eq!(copy, d);
}

// ---- Remove ----

#[test]
fn a_remove_takes_the_row_out_and_shifts_the_ids() {
    let d = doc("#TITLE:T\n: 0 4 0 la\n: 4 4 1 la\nE\n");
    let patch = Patch {
        label: "delete note".to_owned(),
        ops: vec![RowOp::Remove {
            id: RowId(1),
            row: d.rows[1].clone(),
            after: Some(RowId(0)),
        }],
    };
    let out = applied(&patch, &d);
    assert_eq!(out.rows.len(), 3);
    assert_eq!(out.rows[1].raw.text, ": 4 4 1 la");
    assert_eq!(out.rows[1].id, RowId(1));
}

#[test]
fn a_remove_of_the_first_row_carries_no_predecessor() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "delete first".to_owned(),
        ops: vec![RowOp::Remove {
            id: RowId(0),
            row: d.rows[0].clone(),
            after: None,
        }],
    };
    let out = applied(&patch, &d);
    assert_eq!(out.rows[0].kind, RowKind::End);
}

#[test]
fn a_remove_for_an_unknown_row_fails_cleanly() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "gone".to_owned(),
        ops: vec![RowOp::Remove {
            id: RowId(3),
            row: row(3, note_row(0, 0, "x"), ": 0 4 0 x"),
            after: Some(RowId(2)),
        }],
    };
    let mut copy = d.clone();
    let error = patch.apply(&mut copy).expect_err("the row does not exist");
    assert_eq!(error, EditError::UnknownRow(RowId(3)));
    assert_eq!(copy, d);
}

// ---- Inverse ----

#[test]
fn the_inverse_of_a_replace_restores_the_document() {
    let d = doc("#TITLE:T\n: 0 4 0 la\n- 4\n: 4 4 1 la\nE\n");
    let patch = Patch {
        label: "move note".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(1),
            before: d.rows[1].clone(),
            after: changed_row(&d, 1),
        }],
    };
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn the_inverse_of_an_insert_removes_the_row_again() {
    let d = doc("#TITLE:T\n: 0 4 0 la\nE\n");
    let patch = Patch {
        label: "add note".to_owned(),
        ops: vec![RowOp::Insert {
            after: Some(RowId(0)),
            row: row(1, note_row(4, 1, "new"), ": 4 4 1 new"),
        }],
    };
    let inverse = patch.inverse();
    assert!(matches!(inverse.ops[0], RowOp::Remove { .. }));
    assert_eq!(inverse.label, "add note");
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn the_inverse_of_an_insert_at_the_start_restores_the_document() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "add first".to_owned(),
        ops: vec![RowOp::Insert {
            after: None,
            row: row(0, note_row(0, 0, "new"), ": 0 4 0 new"),
        }],
    };
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn the_inverse_of_a_remove_inserts_the_row_again() {
    let d = doc("#TITLE:T\n: 0 4 0 la\n: 4 4 1 la\nE\n");
    let patch = Patch {
        label: "delete note".to_owned(),
        ops: vec![RowOp::Remove {
            id: RowId(1),
            row: d.rows[1].clone(),
            after: Some(RowId(0)),
        }],
    };
    let inverse = patch.inverse();
    assert!(matches!(inverse.ops[0], RowOp::Insert { .. }));
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn the_inverse_of_a_remove_of_the_first_row_restores_the_document() {
    let d = doc(": 0 4 0 la\nE\n");
    let patch = Patch {
        label: "delete first".to_owned(),
        ops: vec![RowOp::Remove {
            id: RowId(0),
            row: d.rows[0].clone(),
            after: None,
        }],
    };
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn a_dirty_flag_and_raw_text_survive_the_round_trip() {
    let d = doc("#TITLE:T\n: 0 4 0 la\nE\n");
    let before = d.rows[1].clone();
    let mut after = before.clone();
    after.dirty = true;
    if let RowKind::Note(note) = &mut after.kind {
        note.length = 8;
    }
    let patch = Patch {
        label: "stretch".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(1),
            before,
            after,
        }],
    };
    assert_eq!(restored(&patch, &d), d);
}

#[test]
fn two_ops_undo_in_reverse_order() {
    let d = doc("#TITLE:T\n: 0 4 0 la\n: 4 4 1 la\nE\n");
    let insert = Patch {
        label: "insert".to_owned(),
        ops: vec![RowOp::Insert {
            after: Some(RowId(2)),
            row: row(3, note_row(8, 2, "new"), ": 8 4 2 new"),
        }],
    };
    let after_insert = applied(&insert, &d);
    let replace = Patch {
        label: "replace".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(1),
            before: after_insert.rows[1].clone(),
            after: changed_row(&after_insert, 1),
        }],
    };
    let mut copy = after_insert.clone();
    replace.apply(&mut copy).expect("the replace applies");
    replace
        .inverse()
        .apply(&mut copy)
        .expect("the inverse applies");
    insert
        .inverse()
        .apply(&mut copy)
        .expect("the inverse applies");
    assert_eq!(copy, d);
}

// ---- Property test: apply/inverse identity ----

/// text_strategy generates line texts without CR and LF.
fn text_strategy() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[^\r\n]*").expect("the text regex compiles")
}

/// document_strategy generates a decoded document from random lines.
fn document_strategy() -> impl Strategy<Value = Document> {
    (
        proptest::collection::vec((text_strategy(), 0..3usize), 0..12),
        any::<bool>(),
    )
        .prop_map(|(lines, bom)| {
            let mut bytes = Vec::new();
            if bom {
                bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            for (text, terminator) in &lines {
                bytes.extend_from_slice(text.as_bytes());
                bytes.extend_from_slice(match terminator {
                    0 => b"\n",
                    1 => b"\r\n",
                    _ => b"\r",
                });
            }
            let file = decode(&bytes).expect("the generated bytes decode");
            classify(file)
        })
}

/// patch_choice picks the strategy inputs for a random valid patch.
type PatchChoice = (u8, usize, i64, bool);

/// patch_choice_strategy generates the choices of a random valid patch.
fn patch_choice_strategy() -> impl Strategy<Value = PatchChoice> {
    (0..3u8, 0..16usize, 0..60i64, any::<bool>())
}

/// build_patch builds a valid patch for the document from the choices.
fn build_patch(doc: &Document, choice: PatchChoice) -> Patch {
    let (kind, index, start, dirty) = choice;
    let len = doc.rows.len();
    let note = |id: u32| Row {
        id: RowId(id),
        raw: raw(&format!(": {start} 4 1 la")),
        kind: note_row(start, 1, "la"),
        dirty,
    };
    match kind {
        // Replace an existing row.
        0 if len > 0 => {
            let i = index % len;
            let mut after = doc.rows[i].clone();
            if let RowKind::Note(note_data) = &mut after.kind {
                note_data.start = start;
            }
            after.dirty = dirty;
            Patch {
                label: "replace".to_owned(),
                ops: vec![RowOp::Replace {
                    id: RowId(i as u32),
                    before: doc.rows[i].clone(),
                    after,
                }],
            }
        }
        // Remove an existing row.
        2 if len > 0 => {
            let i = index % len;
            Patch {
                label: "remove".to_owned(),
                ops: vec![RowOp::Remove {
                    id: RowId(i as u32),
                    row: doc.rows[i].clone(),
                    after: if i == 0 {
                        None
                    } else {
                        Some(RowId(i as u32 - 1))
                    },
                }],
            }
        }
        // Insert (the fallback for an empty document).
        _ => {
            let (after, id) = if len == 0 || index % 3 == 0 {
                (None, 0u32)
            } else {
                (Some(RowId((index % len) as u32)), (index % len) as u32 + 1)
            };
            Patch {
                label: "insert".to_owned(),
                ops: vec![RowOp::Insert {
                    after,
                    row: note(id),
                }],
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn generated_patches_invert_exactly(
        doc in document_strategy(),
        choice in patch_choice_strategy(),
    ) {
        let patch = build_patch(&doc, choice);
        let mut copy = doc.clone();
        patch
            .apply(&mut copy)
            .unwrap_or_else(|error| panic!("the valid patch does not apply: {error}"));
        patch
            .inverse()
            .apply(&mut copy)
            .unwrap_or_else(|error| panic!("the inverse does not apply: {error}"));
        prop_assert_eq!(copy, doc);
    }
}
