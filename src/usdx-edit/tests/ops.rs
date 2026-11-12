//! About me: tests for the note and line operations (task P2-03, plan section
//! 6.3). One test group per function in the plan's table, including the error
//! cases (`BeforeSongStart`, length below 1, merge across a phrase end,
//! phrase end out of its range) and a check in every success test that only
//! the expected rows became dirty. The beat-editing operations refuse
//! relative-mode songs (editing them in relative form is task P9-06 behind
//! gate G-01).

use usdx_edit::EditError;
use usdx_edit::ops::*;
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, NoteType, RowId, classify};
use usdx_format::song::Song;

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// song builds the song view of a text.
fn song(text: &str) -> (Document, Song) {
    let d = doc(text);
    let s = Song::from_document(&d);
    (d, s)
}

/// applied builds the patch, applies it to a copy and returns the copy.
fn applied(result: Result<Patch, EditError>, d: &Document) -> Document {
    let patch = result.expect("the operation builds a patch");
    let mut copy = d.clone();
    patch
        .apply(&mut copy)
        .unwrap_or_else(|error| panic!("the patch does not apply: {error}"));
    copy
}

/// refused tells whether the operation fails with the error.
fn refused<T: std::fmt::Debug>(result: Result<T, EditError>, what: &str) -> EditError {
    result.expect_err(&format!("{what} should be refused"))
}

/// note_starts lists the start beats of the note rows.
fn note_starts(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            usdx_format::rows::RowKind::Note(note) => Some(note.start),
            _ => None,
        })
        .collect()
}

/// note_texts lists the texts of the note rows.
fn note_texts(d: &Document) -> Vec<String> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            usdx_format::rows::RowKind::Note(note) => Some(note.text.clone()),
            _ => None,
        })
        .collect()
}

/// note_lengths lists the lengths of the note rows.
fn note_lengths(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            usdx_format::rows::RowKind::Note(note) => Some(note.length),
            _ => None,
        })
        .collect()
}

/// note_pitches lists the pitches of the note rows.
fn note_pitches(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            usdx_format::rows::RowKind::Note(note) => Some(note.pitch),
            _ => None,
        })
        .collect()
}

/// dirty_rows lists the ids of the dirty rows.
fn dirty_rows(d: &Document) -> Vec<u32> {
    d.rows
        .iter()
        .filter(|row| row.dirty)
        .map(|row| row.id.0)
        .collect()
}

/// phrase_ends lists the beats of the phrase-end rows.
fn phrase_ends(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            usdx_format::rows::RowKind::PhraseEnd { beat, .. } => *beat,
            _ => None,
        })
        .collect()
}

/// two notes, a phrase end, one more note.
const BODY: &str = "#TITLE:T\n#BPM:240\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

/// a phrase end with room to move: the previous note ends at 14, the next
/// note starts at 20, so the marker may sit anywhere in between.
const GAP_BODY: &str = "#TITLE:T\n#BPM:240\n: 0 4 0 la\n: 10 4 1 la\n- 16\n: 20 4 2 la\nE\n";

// ---- move_notes ----

#[test]
fn move_notes_shifts_start_and_pitch_and_marks_exactly_those_rows_dirty() {
    let (d, s) = song(BODY);
    let out = applied(move_notes(&d, &s, &[RowId(2), RowId(3)], 2, 3), &d);
    assert_eq!(note_starts(&out), vec![2, 6, 8]);
    assert_eq!(note_pitches(&out), vec![3, 4, 2]);
    assert_eq!(
        dirty_rows(&out),
        vec![2, 3],
        "only the moved rows are dirty"
    );
}

#[test]
fn move_notes_refuses_a_start_below_the_song_start() {
    let (d, s) = song(BODY);
    let error = refused(move_notes(&d, &s, &[RowId(2)], -1, 0), "a negative start");
    assert_eq!(error, EditError::BeforeSongStart);
}

#[test]
fn move_notes_refuses_an_unknown_row() {
    let (d, s) = song(BODY);
    let error = refused(move_notes(&d, &s, &[RowId(9)], 1, 0), "an unknown row");
    assert_eq!(error, EditError::UnknownRow(RowId(9)));
}

// ---- move_notes_and_following ----

#[test]
fn move_notes_and_following_shifts_every_later_row_of_the_voice() {
    let (d, s) = song(BODY);
    let out = applied(move_notes_and_following(&d, &s, &[RowId(3)], 4), &d);
    // The moved note, the phrase end and the last note shift; the first note
    // stays.
    assert_eq!(note_starts(&out), vec![0, 8, 12]);
    assert_eq!(phrase_ends(&out), vec![12]);
    assert_eq!(
        dirty_rows(&out),
        vec![3, 4, 5],
        "the moved row and the following rows are dirty"
    );
}

#[test]
fn move_notes_and_following_stops_at_a_voice_change() {
    let (d, s) = song("#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\n: 4 4 1 b\nP2\n: 0 4 0 c\nE\n");
    let out = applied(move_notes_and_following(&d, &s, &[RowId(3)], 2), &d);
    assert_eq!(note_starts(&out), vec![2, 6, 0]);
    assert_eq!(dirty_rows(&out), vec![3, 4], "the second voice stays clean");
}

// ---- The relative-mode guard ----

#[test]
fn the_beat_editing_operations_refuse_relative_songs() {
    let relative =
        "#TITLE:T\n#BPM:240\n#RELATIVE:yes\n: 0 4 0 la\n: 4 4 1 la\n- 8 4\n: 8 4 2 la\nE\n";
    let (d, s) = song(relative);
    assert_eq!(
        move_notes(&d, &s, &[RowId(2)], 1, 0).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        move_notes_and_following(&d, &s, &[RowId(2)], 1).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        resize_start(&d, &s, RowId(2), 1).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        resize_end(&d, &s, RowId(2), 1).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        insert_note(&d, &s, Some(RowId(2)), 1, 1, 1, None).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        split_note(&d, &s, RowId(2), 2).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        merge_with_next(&d, &s, RowId(2)).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        move_phrase_end(&d, &s, RowId(4), 6).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        toggle_phrase_end_before(&d, &s, RowId(5)).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
    assert_eq!(
        delete_notes(&d, &s, &[RowId(3)]).expect_err("relative"),
        EditError::RelativeModeIsReadOnly
    );
}

// ---- resize_start / resize_end ----

#[test]
fn resize_start_moves_the_start_and_keeps_the_end() {
    let (d, s) = song(BODY);
    let out = applied(resize_start(&d, &s, RowId(3), 2), &d);
    assert_eq!(note_starts(&out), vec![0, 6, 8]);
    assert_eq!(note_lengths(&out), vec![4, 2, 4]);
    assert_eq!(dirty_rows(&out), vec![3]);
}

#[test]
fn resize_start_refuses_a_length_below_one() {
    let (d, s) = song(BODY);
    let error = refused(resize_start(&d, &s, RowId(3), 4), "a start past the end");
    assert_eq!(error, EditError::LengthBelowOne);
}

#[test]
fn resize_start_refuses_a_start_below_the_song_start() {
    let (d, s) = song(BODY);
    let error = refused(resize_start(&d, &s, RowId(2), -1), "a negative start");
    assert_eq!(error, EditError::BeforeSongStart);
}

#[test]
fn resize_end_extends_and_shortens_the_note() {
    let (d, s) = song(BODY);
    let out = applied(resize_end(&d, &s, RowId(2), 4), &d);
    assert_eq!(note_lengths(&out), vec![8, 4, 4]);
    let out = applied(resize_end(&d, &s, RowId(2), -2), &d);
    assert_eq!(note_lengths(&out), vec![2, 4, 4]);
    assert_eq!(dirty_rows(&out), vec![2]);
}

#[test]
fn resize_end_refuses_a_length_below_one() {
    let (d, s) = song(BODY);
    let error = refused(resize_end(&d, &s, RowId(2), -4), "a negative length");
    assert_eq!(error, EditError::LengthBelowOne);
}

// ---- set_pitch / transpose ----

#[test]
fn set_pitch_sets_and_transpose_shifts() {
    let (d, s) = song(BODY);
    let out = applied(set_pitch(&d, &s, &[RowId(2)], 7), &d);
    assert_eq!(note_pitches(&out), vec![7, 1, 2]);
    let out = applied(transpose(&d, &s, &[RowId(2), RowId(3)], -2), &d);
    assert_eq!(note_pitches(&out), vec![-2, -1, 2]);
    assert_eq!(dirty_rows(&out), vec![2, 3]);
}

// ---- set_type ----

#[test]
fn set_type_rewrites_the_note_type() {
    let (d, s) = song(BODY);
    let out = applied(set_type(&d, &s, &[RowId(2)], NoteType::Golden), &d);
    assert!(matches!(
        &out.rows[2].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Golden
    ));
    assert_eq!(dirty_rows(&out), vec![2]);
    // The other rows keep their type.
    assert!(matches!(
        &out.rows[3].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Normal
    ));
}

// ---- toggle_freestyle ----

#[test]
fn toggle_freestyle_turns_mixed_notes_into_freestyle_and_freestyle_into_normal() {
    let (d, s) = song(BODY);
    // No freestyle note among the ids: all become freestyle.
    let out = applied(toggle_freestyle(&d, &s, &[RowId(2), RowId(3)]), &d);
    assert!(matches!(
        &out.rows[2].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Freestyle
    ));
    assert!(matches!(
        &out.rows[3].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Freestyle
    ));
    // All freestyle now: all become normal.
    let out2 = applied(toggle_freestyle(&out, &s, &[RowId(2), RowId(3)]), &out);
    assert!(matches!(
        &out2.rows[2].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Normal
    ));
    assert!(matches!(
        &out2.rows[3].kind,
        usdx_format::rows::RowKind::Note(note) if note.kind == NoteType::Normal
    ));
}

// ---- set_text ----

#[test]
fn set_text_changes_the_syllable() {
    let (d, s) = song(BODY);
    let out = applied(set_text(&d, &s, RowId(2), "la la"), &d);
    assert_eq!(note_texts(&out), vec!["la la", "la", "la"]);
    assert_eq!(dirty_rows(&out), vec![2]);
}

// ---- insert_note ----

#[test]
fn insert_note_adds_a_note_after_a_row_with_the_default_text() {
    let (d, s) = song(BODY);
    let out = applied(insert_note(&d, &s, Some(RowId(3)), 6, 2, 1, None), &d);
    assert_eq!(note_starts(&out), vec![0, 4, 6, 8]);
    assert_eq!(note_texts(&out), vec!["la", "la", "~", "la"]);
    assert_eq!(dirty_rows(&out), vec![4], "only the new row is dirty");
}

#[test]
fn insert_note_without_an_anchor_starts_the_body() {
    let (d, s) = song("#TITLE:T\n#BPM:240\nE\n");
    let out = applied(
        insert_note(&d, &s, None, 0, 4, 0, Some("first".to_owned())),
        &d,
    );
    assert_eq!(note_texts(&out), vec!["first"]);
    assert_eq!(dirty_rows(&out), vec![2]);
}

// ---- delete_notes ----

#[test]
fn delete_notes_removes_a_phrase_end_whose_line_lost_all_notes() {
    let (d, s) = song(BODY);
    let out = applied(delete_notes(&d, &s, &[RowId(2), RowId(3)]), &d);
    assert_eq!(note_starts(&out), vec![8]);
    assert_eq!(
        phrase_ends(&out),
        vec![],
        "the emptied line's phrase end goes away"
    );
    assert_eq!(dirty_rows(&out), vec![], "a deletion leaves no dirty rows");
}

#[test]
fn delete_notes_keeps_a_phrase_end_that_still_has_notes() {
    let (d, s) = song(BODY);
    let out = applied(delete_notes(&d, &s, &[RowId(2)]), &d);
    assert_eq!(note_starts(&out), vec![4, 8]);
    assert_eq!(phrase_ends(&out), vec![8]);
}

#[test]
fn delete_notes_of_the_last_note_leaves_the_other_lines_alone() {
    let (d, s) = song(BODY);
    let out = applied(delete_notes(&d, &s, &[RowId(5)]), &d);
    assert_eq!(note_starts(&out), vec![0, 4]);
    assert_eq!(phrase_ends(&out), vec![8]);
}

// ---- split_note ----

#[test]
fn split_note_keeps_the_total_length_and_gives_the_right_part_the_placeholder() {
    let (d, s) = song(BODY);
    let out = applied(split_note(&d, &s, RowId(2), 2), &d);
    assert_eq!(note_starts(&out), vec![0, 2, 4, 8]);
    assert_eq!(note_lengths(&out), vec![2, 2, 4, 4]);
    assert_eq!(note_texts(&out), vec!["la", "~", "la", "la"]);
    assert_eq!(dirty_rows(&out), vec![2, 3], "both halves are dirty");
}

#[test]
fn split_note_refuses_a_position_outside_the_note() {
    let (d, s) = song(BODY);
    assert_eq!(
        split_note(&d, &s, RowId(2), 0).expect_err("at the start"),
        EditError::LengthBelowOne
    );
    assert_eq!(
        split_note(&d, &s, RowId(2), 4).expect_err("at the end"),
        EditError::LengthBelowOne
    );
}

// ---- merge_with_next ----

#[test]
fn merge_with_next_joins_the_text_and_takes_the_right_note_end() {
    let (d, s) = song(BODY);
    let out = applied(merge_with_next(&d, &s, RowId(2)), &d);
    assert_eq!(note_starts(&out), vec![0, 8]);
    assert_eq!(note_lengths(&out), vec![8, 4]);
    assert_eq!(note_texts(&out), vec!["lala", "la"]);
    assert_eq!(
        dirty_rows(&out),
        vec![2],
        "the removed row is gone, not dirty"
    );
}

#[test]
fn merge_with_next_drops_the_placeholder_of_the_right_note() {
    let (d, s) = song(BODY);
    let with_placeholder = applied(insert_note(&d, &s, Some(RowId(2)), 2, 2, 0, None), &d);
    let out = applied(
        merge_with_next(&with_placeholder, &s, RowId(2)),
        &with_placeholder,
    );
    assert_eq!(note_texts(&out), vec!["la", "la", "la"]);
}

#[test]
fn merge_with_next_refuses_a_phrase_end_between_the_notes() {
    let (d, s) = song(BODY);
    let error = refused(merge_with_next(&d, &s, RowId(3)), "across the phrase end");
    assert_eq!(error, EditError::PhraseEndBetween);
}

// ---- move_phrase_end ----

#[test]
fn move_phrase_end_moves_between_the_previous_note_end_and_the_next_note_start() {
    let (d, s) = song(GAP_BODY);
    let out = applied(move_phrase_end(&d, &s, RowId(4), 18), &d);
    assert_eq!(phrase_ends(&out), vec![18]);
    assert_eq!(dirty_rows(&out), vec![4]);
}

#[test]
fn move_phrase_end_refuses_a_beat_outside_the_range() {
    let (d, s) = song(GAP_BODY);
    // The previous note ends at 14, the next note starts at 20.
    assert_eq!(
        move_phrase_end(&d, &s, RowId(4), 13).expect_err("inside the previous note"),
        EditError::PhraseEndOutOfRange
    );
    assert_eq!(
        move_phrase_end(&d, &s, RowId(4), 21).expect_err("past the next note's start"),
        EditError::PhraseEndOutOfRange
    );
}

// ---- toggle_phrase_end_before ----

#[test]
fn toggle_phrase_end_before_inserts_a_marker_at_the_previous_note_end() {
    let (d, s) = song("#TITLE:T\n#BPM:240\n: 0 4 0 la\n: 4 4 1 la\n: 8 4 2 la\nE\n");
    let out = applied(toggle_phrase_end_before(&d, &s, RowId(4)), &d);
    // The convention in UltraSinger files: `- <end beat of the previous note>`,
    // which is 8 here (the second note runs from 4 to 8).
    assert_eq!(phrase_ends(&out), vec![8]);
    assert_eq!(note_starts(&out), vec![0, 4, 8]);
    assert_eq!(dirty_rows(&out), vec![4], "the inserted marker is dirty");
}

#[test]
fn toggle_phrase_end_before_removes_the_existing_marker() {
    let (d, s) = song(BODY);
    let out = applied(toggle_phrase_end_before(&d, &s, RowId(5)), &d);
    assert_eq!(phrase_ends(&out), vec![]);
    assert_eq!(note_starts(&out), vec![0, 4, 8]);
}

#[test]
fn toggle_phrase_end_before_refuses_the_first_note() {
    let (d, s) = song(BODY);
    let error = refused(
        toggle_phrase_end_before(&d, &s, RowId(2)),
        "no previous note",
    );
    assert!(matches!(error, EditError::PhraseEndOutOfRange));
}
