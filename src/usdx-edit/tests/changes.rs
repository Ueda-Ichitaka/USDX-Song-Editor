//! About me: tests for the change log for the anchors export (task P2-05,
//! plan section 6.11, decision D-12). They assert that a snapshot taken at
//! load reports no changes, that moving a line's first note marks exactly
//! that line while a pitch change or a moved later note does not, that a
//! deleted first note marks the line under its new first note, that only
//! voice P1 is tracked, and that undoing back to the loaded state removes
//! the mark again.

use usdx_edit::changes::{ChangeLog, LineChange};
use usdx_edit::ops::{delete_notes, move_notes, set_pitch};
use usdx_edit::{EditError, History, Patch};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowId, classify};
use usdx_format::song::Song;

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// song builds the song view of a document.
fn song(d: &Document) -> Song {
    Song::from_document(d)
}

/// applied builds the patch, applies it to a copy and returns the copy.
fn applied(result: Result<Patch, EditError>, d: &mut Document) {
    let patch = result.expect("the operation builds a patch");
    patch
        .apply(d)
        .unwrap_or_else(|error| panic!("the patch does not apply: {error}"));
}

/// a song with a gap, two lines in voice P1 and a timing.
const SONG: &str = "#TITLE:T\n#BPM:240\n#GAP:500\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

#[test]
fn no_changes_after_load() {
    let d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    assert!(log.changed_lines(&d, &s).is_empty());
}

#[test]
fn a_moved_first_note_marks_its_line() {
    let mut d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    applied(move_notes(&d, &s, &[RowId(3)], 2, 0), &mut d);
    let s = song(&d);
    assert_eq!(
        log.changed_lines(&d, &s),
        vec![LineChange {
            first_note: RowId(3),
            // Beat 2 at 62.5 ms per beat after a 500 ms gap.
            time_ms: 625.0,
        }]
    );
}

#[test]
fn a_pitch_change_does_not_mark_a_line() {
    let mut d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    applied(set_pitch(&d, &s, &[RowId(3), RowId(4)], 7), &mut d);
    let s = song(&d);
    assert!(log.changed_lines(&d, &s).is_empty());
}

#[test]
fn moving_a_later_note_does_not_mark_its_line() {
    let mut d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    // Only the first note's start time decides; the second note of the line
    // moves.
    applied(move_notes(&d, &s, &[RowId(4)], 2, 0), &mut d);
    let s = song(&d);
    assert!(log.changed_lines(&d, &s).is_empty());
}

#[test]
fn a_deleted_first_note_marks_its_line_under_the_new_first_note() {
    let mut d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    applied(delete_notes(&d, &s, &[RowId(3)]), &mut d);
    let s = song(&d);
    // The line now starts at its second note (start 4, 750 ms). The deletion
    // also renumbers every later row, so the second line's first-note row id
    // is no longer in the snapshot and it counts as changed as well - the
    // by-RowId comparison is conservative about structural edits.
    assert_eq!(
        log.changed_lines(&d, &s),
        vec![
            LineChange {
                first_note: RowId(3),
                time_ms: 750.0,
            },
            LineChange {
                first_note: RowId(5),
                time_ms: 1000.0,
            },
        ]
    );
}

#[test]
fn only_voice_p1_lines_are_tracked() {
    let mut d = doc("#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\nP2\n: 4 4 1 b\nE\n");
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    // The P2 note is row 5; moving it leaves the log empty.
    applied(move_notes(&d, &s, &[RowId(5)], 2, 0), &mut d);
    let s = song(&d);
    assert!(log.changed_lines(&d, &s).is_empty());
}

#[test]
fn undo_back_removes_the_mark() {
    let mut d = doc(SONG);
    let s = song(&d);
    let log = ChangeLog::snapshot(&d, &s);
    let mut history = History::new();
    let patch = move_notes(&d, &s, &[RowId(3)], 2, 0).expect("the move builds");
    history.commit(&mut d, patch).expect("the commit applies");
    let s = song(&d);
    assert_eq!(log.changed_lines(&d, &s).len(), 1);
    history.undo(&mut d);
    let s = song(&d);
    assert!(log.changed_lines(&d, &s).is_empty());
}
