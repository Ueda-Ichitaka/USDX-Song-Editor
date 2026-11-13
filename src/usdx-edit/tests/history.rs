//! About me: tests for the undo history (task P2-02, plan section 6.2). They
//! assert that commit applies the patch, undo and redo return the step labels
//! and restore the document, a refused commit leaves the stacks and the
//! document unchanged, a new commit clears the redo stack, and the saved
//! position drives `is_dirty` across undo and redo.

use usdx_edit::{EditError, History, Patch, RowOp};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowId, RowKind, classify};

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// replace_patch builds a replace patch that bumps the first note's start
/// beat by one; the `before` row is taken from the document, as the patch
/// builders in P2-03 do.
fn replace_patch(doc: &Document, label: &str) -> Patch {
    let before = doc.rows[0].clone();
    let mut after = before.clone();
    let raw_text = match &after.kind {
        RowKind::Note(note) => format!(": {} 4 0 {}", note.start + 1, note.text),
        other => panic!("the first row is not a note: {other:?}"),
    };
    if let RowKind::Note(note) = &mut after.kind {
        note.start += 1;
    }
    after.dirty = true;
    after.raw.text = raw_text;
    Patch {
        label: label.to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(0),
            before,
            after,
        }],
    }
}

/// first_note_start reads the first note's start beat.
fn first_note_start(doc: &Document) -> i64 {
    match &doc.rows[0].kind {
        RowKind::Note(note) => note.start,
        other => panic!("the first row is not a note: {other:?}"),
    }
}

#[test]
fn a_commit_applies_the_patch() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "move");
    history.commit(&mut d, patch).expect("the commit applies");
    assert_eq!(first_note_start(&d), 1);
}

#[test]
fn undo_restores_the_document_and_returns_the_label() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "move");
    history.commit(&mut d, patch).expect("applies");
    assert_eq!(history.undo(&mut d), Some("move".to_owned()));
    assert_eq!(first_note_start(&d), 0);
}

#[test]
fn redo_reapplies_the_patch_and_returns_the_label() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "move");
    history.commit(&mut d, patch).expect("applies");
    history.undo(&mut d);
    assert_eq!(history.redo(&mut d), Some("move".to_owned()));
    assert_eq!(first_note_start(&d), 1);
}

#[test]
fn an_empty_history_undoes_and_redoes_nothing() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    assert_eq!(history.undo(&mut d), None);
    assert_eq!(history.redo(&mut d), None);
    assert_eq!(first_note_start(&d), 0);
}

#[test]
fn a_new_commit_clears_the_redo_stack() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "first");
    history.commit(&mut d, patch).expect("applies");
    history.undo(&mut d);
    let patch = replace_patch(&d, "second");

    history.commit(&mut d, patch).expect("applies");
    assert_eq!(history.redo(&mut d), None);
    assert_eq!(history.undo(&mut d), Some("second".to_owned()));
}

#[test]
fn the_labels_peek_at_the_next_step() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    assert_eq!(history.undo_label(), None);
    assert_eq!(history.redo_label(), None);
    let patch = replace_patch(&d, "move");

    history.commit(&mut d, patch).expect("applies");
    assert_eq!(history.undo_label(), Some("move"));
    assert_eq!(history.redo_label(), None);
    history.undo(&mut d);
    assert_eq!(history.undo_label(), None);
    assert_eq!(history.redo_label(), Some("move"));
    history.redo(&mut d);
    assert_eq!(history.undo_label(), Some("move"));
}

#[test]
fn several_steps_undo_and_redo_in_order() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    for label in ["first", "second", "third"] {
        let patch = replace_patch(&d, label);

        history.commit(&mut d, patch).expect("applies");
    }
    assert_eq!(first_note_start(&d), 3);
    assert_eq!(history.undo(&mut d), Some("third".to_owned()));
    assert_eq!(first_note_start(&d), 2);
    assert_eq!(history.undo(&mut d), Some("second".to_owned()));
    assert_eq!(first_note_start(&d), 1);
    assert_eq!(history.redo(&mut d), Some("second".to_owned()));
    assert_eq!(first_note_start(&d), 2);
}

#[test]
fn a_refused_commit_leaves_everything_unchanged() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "good");
    history.commit(&mut d, patch).expect("applies");
    let before = d.clone();
    let stale = Patch {
        label: "stale".to_owned(),
        ops: vec![RowOp::Replace {
            id: RowId(5),
            before: d.rows[0].clone(),
            after: d.rows[0].clone(),
        }],
    };
    assert_eq!(
        history.commit(&mut d, stale),
        Err(EditError::UnknownRow(RowId(5)))
    );
    assert_eq!(d, before);
    assert_eq!(history.undo_label(), Some("good"));
    assert!(history.is_dirty());
}

#[test]
fn the_saved_position_drives_is_dirty() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    assert!(!history.is_dirty(), "a fresh document is not dirty");
    let patch = replace_patch(&d, "first");

    history.commit(&mut d, patch).expect("applies");
    assert!(history.is_dirty());
    history.mark_saved();
    assert!(!history.is_dirty());
    let patch = replace_patch(&d, "second");

    history.commit(&mut d, patch).expect("applies");
    assert!(history.is_dirty());
    // Undoing back to the saved step clears the flag.
    history.undo(&mut d);
    assert!(!history.is_dirty());
    // Undoing past the saved step dirties the document again: it no longer
    // matches the saved file.
    history.undo(&mut d);
    assert!(history.is_dirty());
    history.redo(&mut d);
    assert!(!history.is_dirty());
}

#[test]
fn a_new_edit_after_undoing_past_the_saved_step_stays_dirty() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    let patch = replace_patch(&d, "first");
    history.commit(&mut d, patch).expect("applies");
    history.mark_saved();

    // Undo the saved step, then edit differently: the stack has the saved
    // length again, but the document is not the saved file.
    history.undo(&mut d);
    let patch = replace_patch(&d, "other");
    history.commit(&mut d, patch).expect("applies");
    assert!(
        history.is_dirty(),
        "the new branch differs from the saved file"
    );
    assert_eq!(
        history.unsaved_steps(),
        2,
        "one undone step plus one new step"
    );

    // Undoing the new edit reaches the branch point, still not the saved file.
    history.undo(&mut d);
    assert!(history.is_dirty());
    assert_eq!(history.unsaved_steps(), 1);

    // Saving again makes the current position the saved one.
    history.redo(&mut d);
    history.mark_saved();
    assert!(!history.is_dirty());
    assert_eq!(history.unsaved_steps(), 0);
}

#[test]
fn unsaved_steps_counts_the_edits_between_here_and_the_saved_file() {
    let mut d = doc(": 0 4 0 la\nE\n");
    let mut history = History::new();
    assert_eq!(history.unsaved_steps(), 0, "a fresh document is clean");

    let patch = replace_patch(&d, "first");
    history.commit(&mut d, patch).expect("applies");
    let patch = replace_patch(&d, "second");
    history.commit(&mut d, patch).expect("applies");
    assert_eq!(history.unsaved_steps(), 2);

    history.mark_saved();
    assert_eq!(history.unsaved_steps(), 0);

    // Undoing to the saved position counts down, undoing past it counts up
    // again.
    history.undo(&mut d);
    assert_eq!(history.unsaved_steps(), 1);
    history.undo(&mut d);
    assert_eq!(history.unsaved_steps(), 2);
    history.redo(&mut d);
    assert_eq!(history.unsaved_steps(), 1);
    history.redo(&mut d);
    assert_eq!(history.unsaved_steps(), 0);
}
