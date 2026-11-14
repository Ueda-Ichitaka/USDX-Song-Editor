//! About me: tests for the mouse-editing controller of task P5-04 (plan
//! sections 8.3, 8.4). The selection commands (click selects, Ctrl adds,
//! Shift extends between the anchor and the click, the rubber band picks
//! every note intersecting the band) and the drag commits (move the
//! selection, shift the following notes, resize an edge, exactly one undo
//! step per drag) are tested on a real loaded song; the snapping of a
//! dragged edge to a neighbour's edge (Alt turns it off) is tested as a
//! pure function on the beat axis with an 8 px threshold.

use tempfile::TempDir;
use usdx_edit::EditError;
use usdx_format::rows::RowId;
use usdx_song_editor::controller::EditorController;
use usdx_song_editor::controller::editing::{Edge, SelectionMode, snapped_move, snapped_resize};

/// fixture writes a song text into a fresh temporary directory and returns
/// the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// Two lines of three notes each; the #BPM and #TITLE headers occupy the
/// first two rows, so the note rows are 2 to 4 (line 1) and 6 to 8
/// (line 2).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
    ": 12 4 0 d\n: 16 4 2 e\n: 20 4 4 f\n- 24\nE\n",
);

/// controller loads the two-line fixture and selects nothing.
fn controller() -> (TempDir, EditorController) {
    let (dir, path) = fixture(SONG);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, controller)
}

/// the selected ids as a vector, for readable assertions.
fn selected(controller: &EditorController) -> Vec<RowId> {
    controller.selection().iter().copied().collect()
}

// ---- Selection ----

#[test]
fn a_click_replaces_the_selection_and_sets_the_anchor() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    assert_eq!(selected(&controller), vec![RowId(2)]);
    controller.select_note(RowId(7), SelectionMode::Set);
    assert_eq!(selected(&controller), vec![RowId(7)]);
}

#[test]
fn ctrl_click_adds_the_note_to_the_selection() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(4), SelectionMode::Add);
    assert_eq!(selected(&controller), vec![RowId(2), RowId(4)]);
}

#[test]
fn shift_click_extends_from_the_anchor_to_the_click() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(8), SelectionMode::Extend);
    assert_eq!(
        selected(&controller),
        vec![RowId(2), RowId(3), RowId(4), RowId(6), RowId(7), RowId(8)],
        "the notes between the anchor and the click, in row order"
    );
    // Extending backwards selects from the previous anchor again; the
    // clicked note becomes the new anchor.
    controller.select_note(RowId(3), SelectionMode::Extend);
    assert_eq!(
        selected(&controller),
        vec![RowId(3), RowId(4), RowId(6), RowId(7), RowId(8)]
    );
}

#[test]
fn a_shift_click_without_an_anchor_selects_the_note() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(6), SelectionMode::Extend);
    assert_eq!(selected(&controller), vec![RowId(6)]);
}

#[test]
fn clearing_the_selection_empties_it_and_drops_the_anchor() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.clear_selection();
    assert!(controller.selection().is_empty());
    controller.select_note(RowId(4), SelectionMode::Extend);
    assert_eq!(selected(&controller), vec![RowId(4)], "no anchor any more");
}

#[test]
fn the_rubber_band_selects_every_note_intersecting_the_band() {
    let (_dir, mut controller) = controller();
    // The band covers beats 3 to 17 in MIDI pitches 60 to 62 (USDX
    // pitches 0 and 2): note a ends at 4 and its last beat overlaps the
    // band start, note b starts at 4 with pitch 2, note d starts at 12
    // with pitch 0, note e at 16 with pitch 2; notes c and f sit on
    // pitch 4 (MIDI 64), outside the band.
    controller.select_range(3.0, 17.0, 60, 62);
    assert_eq!(
        selected(&controller),
        vec![RowId(2), RowId(3), RowId(6), RowId(7)]
    );
    // The band bounds may arrive swapped (dragged upwards or leftwards).
    controller.clear_selection();
    controller.select_range(17.0, 3.0, 62, 60);
    assert_eq!(
        selected(&controller),
        vec![RowId(2), RowId(3), RowId(6), RowId(7)]
    );
}

#[test]
fn the_rubber_band_replaces_the_previous_selection() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_range(100.0, 200.0, 60, 62);
    assert!(controller.selection().is_empty(), "no note in the band");
}

// ---- Move and resize commits ----

#[test]
fn moving_the_selection_moves_every_selected_note_in_one_undo_step() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(3), SelectionMode::Add);
    controller.move_selection(2, 1).expect("the move applies");
    let loaded = controller.loaded().expect("loaded");
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .start,
        2.0
    );
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .pitch,
        1
    );
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(3))
            .expect("a note")
            .start,
        6.0
    );
    // Note c stays where it was.
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(4))
            .expect("a note")
            .start,
        8.0
    );
    // Exactly one undo step.
    assert_eq!(loaded.history.undo_label(), Some("Move notes"));
}

#[test]
fn undoing_a_drag_restores_the_song() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.move_selection(2, -1).expect("the move applies");
    controller.undo().expect("undone");
    let loaded = controller.loaded().expect("loaded");
    // The song view follows the document.
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .start,
        0.0
    );
}

#[test]
fn a_move_below_the_song_start_is_refused_and_changes_nothing() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    let error = controller.move_selection(-1, 0).unwrap_err();
    assert!(matches!(error, EditError::BeforeSongStart));
    let loaded = controller.loaded().expect("loaded");
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .start,
        0.0,
        "the refused move changes nothing"
    );
    assert!(loaded.history.undo_label().is_none(), "no history step");
}

#[test]
fn moving_with_an_empty_selection_changes_nothing() {
    let (_dir, mut controller) = controller();
    controller.move_selection(2, 0).expect("a no-op");
    let loaded = controller.loaded().expect("loaded");
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .start,
        0.0
    );
    assert!(loaded.history.undo_label().is_none());
}

#[test]
fn moving_the_selection_and_the_following_notes_shifts_the_rest_of_the_voice() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(6), SelectionMode::Set);
    controller
        .move_selection_and_following(4)
        .expect("the shift applies");
    let loaded = controller.loaded().expect("loaded");
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(6))
            .expect("a note")
            .start,
        16.0
    );
    // The note after the selected one and the line end move too.
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(7))
            .expect("a note")
            .start,
        20.0
    );
    // The first line stays where it was.
    assert_eq!(
        loaded
            .song
            .note(&loaded.document, RowId(2))
            .expect("a note")
            .start,
        0.0
    );
}

#[test]
fn resizing_the_start_and_the_end_keeps_one_undo_step_each() {
    let (_dir, mut controller) = controller();
    controller
        .resize_note(RowId(2), Edge::Start, 1)
        .expect("the resize applies");
    controller
        .resize_note(RowId(2), Edge::End, 2)
        .expect("the resize applies");
    let loaded = controller.loaded().expect("loaded");
    let note = loaded
        .song
        .note(&loaded.document, RowId(2))
        .expect("a note");
    assert_eq!(note.start, 1.0, "the start moved");
    assert_eq!(note.length, 5, "4 - 1 + 2");
    assert_eq!(loaded.history.undo_label(), Some("Resize note end"));
}

#[test]
fn a_resize_below_one_beat_is_refused() {
    let (_dir, mut controller) = controller();
    let error = controller
        .resize_note(RowId(2), Edge::Start, 4)
        .unwrap_err();
    assert!(matches!(error, EditError::LengthBelowOne));
    let error = controller.resize_note(RowId(2), Edge::End, -4).unwrap_err();
    assert!(matches!(error, EditError::LengthBelowOne));
}

// ---- Snapping ----

/// The snapping fixtures. The notes sit on integer beats, so a small zoom
/// makes the 8 px threshold wider than half a beat and the snap differs
/// from the plain rounding: at 4 px per beat the threshold is 2 beats.
///
/// `snap_song` has a gap of 1 beat before note b (6..8) and 2 beats after
/// it (10..12); `tight_song` has b (5..7) adjacent to both neighbours
/// (a 0..5, c 8..10), where both edges can be near a neighbour edge at
/// once and the closer one wins.
fn snap_song() -> (usdx_format::rows::Document, usdx_format::song::Song) {
    song_of("#TITLE:T\n#BPM:240\n: 0 5 0 a\n: 6 2 2 b\n: 10 2 4 c\n- 14\nE\n")
}

fn tight_song() -> (usdx_format::rows::Document, usdx_format::song::Song) {
    song_of("#TITLE:T\n#BPM:240\n: 0 5 0 a\n: 5 2 2 b\n: 8 2 4 c\n- 12\nE\n")
}

/// song_of decodes a song text into the document and its song view. The
/// header rows occupy the ids 0 and 1, so the notes are 2, 3, 4.
fn song_of(text: &str) -> (usdx_format::rows::Document, usdx_format::song::Song) {
    use usdx_format::bytes::decode;
    use usdx_format::rows::classify;
    use usdx_format::song::Song;
    let decoded = decode(text.as_bytes()).expect("the song decodes");
    let document = classify(decoded);
    let song = Song::from_document(&document);
    (document, song)
}

#[test]
fn a_body_drag_snaps_the_start_edge_to_the_previous_note_end() {
    let (doc, song) = snap_song();
    // Dragging note b (6..8) left by -2.6 beats: the start edge lands at
    // 3.4, 1.6 beats from the previous note's end (5), within the 2 beat
    // threshold; the end edge is 4.6 beats from the next note's start.
    // The start snaps onto the neighbour edge: the committed delta is -1.
    let (d_beat, d_pitch) = snapped_move(&doc, &song, RowId(3), -2.6, 3, true, 4.0);
    assert_eq!((d_beat, d_pitch), (-1, 3));
    // Without snapping the plain rounded delta stays.
    let (d_beat, d_pitch) = snapped_move(&doc, &song, RowId(3), -2.6, 3, false, 4.0);
    assert_eq!((d_beat, d_pitch), (-3, 3));
}

#[test]
fn a_body_drag_snaps_the_end_edge_to_the_next_note_start() {
    let (doc, song) = snap_song();
    // Dragging note b right by +3.4: the end edge lands at 11.4, 1.4 beats
    // from the next note's start (10); the start edge is 4.4 beats from
    // the previous end. The end snaps onto 10: the committed delta is 2.
    let (d_beat, _) = snapped_move(&doc, &song, RowId(3), 3.4, 0, true, 4.0);
    assert_eq!(d_beat, 2);
    let (d_beat, _) = snapped_move(&doc, &song, RowId(3), 3.4, 0, false, 4.0);
    assert_eq!(d_beat, 3);
}

#[test]
fn snapping_picks_the_closer_edge_when_both_are_near() {
    let (doc, song) = tight_song();
    // Dragging note b (5..7) right by 0.8: the end edge is 0.2 beats from
    // the next start (8) and the start edge 0.8 from the previous end
    // (5): the end wins, the committed delta is 1.
    let (d_beat, _) = snapped_move(&doc, &song, RowId(3), 0.8, 0, true, 4.0);
    assert_eq!(d_beat, 1);
    // Dragging left by -0.8: the start edge is 0.8 from the previous end
    // and the end edge 1.8 from the next start: the start wins, delta 0.
    let (d_beat, _) = snapped_move(&doc, &song, RowId(3), -0.8, 0, true, 4.0);
    assert_eq!(d_beat, 0);
}

#[test]
fn a_resize_snaps_its_edge_to_the_neighbour_edge() {
    let (doc, song) = snap_song();
    // Resizing the start of note b (6..8) by +0.4: the start lands at
    // 6.4, 1.4 beats from the previous end (5); it snaps onto 5 (the
    // committed delta is -1, the note keeps its end).
    let d_beat = snapped_resize(&doc, &song, RowId(3), Edge::Start, 0.4, true, 4.0);
    assert_eq!(d_beat, -1);
    let d_beat = snapped_resize(&doc, &song, RowId(3), Edge::Start, 0.4, false, 4.0);
    assert_eq!(d_beat, 0);
    // Resizing the end of note b by +1.4: the end lands at 9.4, 0.6 beats
    // from the next note's start (10); it snaps onto 10 (delta 2).
    let d_beat = snapped_resize(&doc, &song, RowId(3), Edge::End, 1.4, true, 4.0);
    assert_eq!(d_beat, 2);
    let d_beat = snapped_resize(&doc, &song, RowId(3), Edge::End, 1.4, false, 4.0);
    assert_eq!(d_beat, 1);
}

#[test]
fn an_edge_far_from_every_neighbour_edge_rounds_plainly() {
    let (doc, song) = snap_song();
    // Resizing the start of note c (10..12) by +1.4: the start lands at
    // 11.4, 3.4 beats from the previous end (8): no snap.
    let d_beat = snapped_resize(&doc, &song, RowId(4), Edge::Start, 1.4, true, 4.0);
    assert_eq!(d_beat, 1);
}

#[test]
fn snapping_an_unknown_note_leaves_the_rounded_delta() {
    let (doc, song) = snap_song();
    let (d_beat, d_pitch) = snapped_move(&doc, &song, RowId(99), 1.4, 0, true, 20.0);
    assert_eq!((d_beat, d_pitch), (1, 0));
}

// ---- Note actions (task P5-05): insert, delete, split, merge, types,
// text, phrase ends, transpose; the user decisions of 2026-11-14 apply
// (insert inherits, S/M need exactly one selected note) ----

/// the document note of a row as (start, length, pitch, text).
fn note_data(controller: &EditorController, id: RowId) -> (f64, i64, i64, String) {
    let loaded = controller.loaded().expect("the song is loaded");
    let note = loaded
        .song
        .note(&loaded.document, id)
        .expect("the row is a note");
    (note.start, note.length, note.pitch, note.text)
}

/// the note type character of a row.
fn note_type(controller: &EditorController, id: RowId) -> char {
    let loaded = controller.loaded().expect("the song is loaded");
    let note = loaded
        .song
        .note(&loaded.document, id)
        .expect("the row is a note");
    usdx_song_editor::controller::notes::type_char(&note.kind)
        .chars()
        .next()
        .expect("the type char is one character")
}

#[test]
fn insert_after_a_note_inherits_and_starts_at_its_end() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller
        .insert_note_after()
        .expect("the note is inserted");
    // The new note sits between a and b: start at a's end, the length and
    // pitch inherited, the default text. The rows after the insertion
    // point shift one down.
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 0, "~".to_owned())
    );
    assert_eq!(
        note_data(&controller, RowId(4)),
        (4.0, 4, 2, "b".to_owned())
    );
    assert_eq!(
        note_data(&controller, RowId(5)),
        (8.0, 4, 4, "c".to_owned())
    );
    assert_eq!(note_type(&controller, RowId(3)), ':');
    // The new note is selected, so pressing Insert again chains notes.
    assert_eq!(selected(&controller), vec![RowId(3)]);
    // One undo step restores the song; the selection still names row 3,
    // which is b again now that the ids shifted back.
    controller.undo();
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
    assert_eq!(
        note_data(&controller, RowId(4)),
        (8.0, 4, 4, "c".to_owned())
    );
}

#[test]
fn insert_without_a_selection_chains_after_the_current_line() {
    let (_dir, mut controller) = controller();
    controller
        .insert_note_after()
        .expect("the note is inserted");
    // After the last note of the current line (c, row 4): start 12, the
    // length and pitch inherited.
    assert_eq!(
        note_data(&controller, RowId(5)),
        (12.0, 4, 4, "~".to_owned())
    );
    assert_eq!(selected(&controller), vec![RowId(5)]);
}

#[test]
fn deleting_the_whole_line_removes_its_orphaned_phrase_end() {
    let (_dir, mut controller) = controller();
    for id in [RowId(2), RowId(3), RowId(4)] {
        controller.select_note(id, SelectionMode::Add);
    }
    controller
        .delete_selection()
        .expect("the notes are deleted");
    // The phrase end of the emptied line is gone; line 2 is untouched and
    // is now the first line, so its note d starts at row 2.
    assert_eq!(
        note_data(&controller, RowId(2)),
        (12.0, 4, 0, "d".to_owned())
    );
    assert!(
        controller.selection().is_empty(),
        "deleted ids leave the selection"
    );
    controller.undo();
    assert_eq!(
        note_data(&controller, RowId(2)),
        (0.0, 4, 0, "a".to_owned())
    );
}

#[test]
fn split_at_the_playhead_beat_halves_the_selected_note() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(3), SelectionMode::Set);
    controller.split_selection_at(6).expect("the note is split");
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 2, 2, "b".to_owned())
    );
    assert_eq!(
        note_data(&controller, RowId(4)),
        (6.0, 2, 2, "~".to_owned())
    );
    controller.undo();
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
}

#[test]
fn split_without_exactly_one_selected_note_changes_nothing() {
    let (_dir, mut controller) = controller();
    assert_eq!(
        controller.split_selection_at(6),
        Ok(()),
        "no selection: no-op"
    );
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(3), SelectionMode::Add);
    assert_eq!(controller.split_selection_at(6), Ok(()), "two notes: no-op");
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
}

#[test]
fn a_split_that_would_leave_an_empty_half_is_refused() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(3), SelectionMode::Set);
    assert_eq!(
        controller.split_selection_at(4),
        Err(EditError::LengthBelowOne),
        "the left half would be empty"
    );
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
}

#[test]
fn merging_joins_the_texts_and_reaches_the_next_note_end() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(3), SelectionMode::Set);
    controller
        .merge_selection_with_next()
        .expect("the merge is applied");
    // b and c merge: the text joins, the length reaches c's end (12).
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 8, 2, "bc".to_owned())
    );
    controller.undo();
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
}

#[test]
fn a_merge_across_a_phrase_end_is_refused() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(4), SelectionMode::Set);
    assert_eq!(
        controller.merge_selection_with_next(),
        Err(EditError::PhraseEndBetween),
        "the phrase end stands between c and d"
    );
    assert_eq!(
        note_data(&controller, RowId(4)),
        (8.0, 4, 4, "c".to_owned())
    );
}

#[test]
fn the_type_keys_set_the_type_of_every_selected_note() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(6), SelectionMode::Add);
    controller.set_selection_type('G').expect("the type is set");
    assert_eq!(note_type(&controller, RowId(2)), 'G');
    assert_eq!(note_type(&controller, RowId(6)), 'G');
    assert_eq!(
        note_type(&controller, RowId(3)),
        ':',
        "unselected notes stay"
    );
    controller.undo();
    assert_eq!(note_type(&controller, RowId(2)), ':');
}

#[test]
fn the_freestyle_key_toggles_all_selected_notes() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(3), SelectionMode::Add);
    controller
        .toggle_freestyle_selection()
        .expect("the toggle is applied");
    assert_eq!(note_type(&controller, RowId(2)), 'F');
    assert_eq!(note_type(&controller, RowId(3)), 'F');
    // Toggling again turns every freestyle note back to normal.
    controller
        .toggle_freestyle_selection()
        .expect("the toggle is applied");
    assert_eq!(note_type(&controller, RowId(2)), ':');
}

#[test]
fn setting_the_text_changes_the_selected_note() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(3), SelectionMode::Set);
    controller
        .set_note_text(RowId(3), "hello")
        .expect("the text is set");
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "hello".to_owned())
    );
    controller.undo();
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
}

/// the absolute beat of a phrase end row, read fresh each time.
fn phrase_beat(controller: &EditorController, id: RowId) -> Option<f64> {
    let loaded = controller.loaded().expect("the song is loaded");
    loaded.song.phrase_end_beat(&loaded.document, id)
}

#[test]
fn the_phrase_end_key_toggles_a_marker_before_the_selected_note() {
    let (_dir, mut controller) = controller();
    // b (row 3) has a note before it and no marker yet: the toggle inserts
    // a marker carrying a's end beat (4) and splits the line there.
    controller.select_note(RowId(3), SelectionMode::Set);
    controller
        .toggle_phrase_end_before_selection()
        .expect("the marker is set");
    assert_eq!(
        phrase_beat(&controller, RowId(3)),
        Some(4.0),
        "the inserted marker carries a's end beat"
    );
    assert_eq!(
        note_data(&controller, RowId(4)),
        (4.0, 4, 2, "b".to_owned()),
        "b shifted one row down behind the marker"
    );
    assert_eq!(
        selected(&controller),
        vec![RowId(4)],
        "the selection follows b"
    );
    controller.undo();
    assert_eq!(
        phrase_beat(&controller, RowId(3)),
        None,
        "the undo removes the inserted marker again"
    );
    // The selection follows the note back through the undo (the ids of the
    // rows after the marker shifted up).
    assert_eq!(
        note_data(&controller, RowId(3)),
        (4.0, 4, 2, "b".to_owned())
    );
    // A marker already standing before the note is removed by the toggle:
    // line 1's marker stands before d, so B on d deletes it and restores
    // it on the second press.
    controller.select_note(RowId(6), SelectionMode::Set);
    controller
        .toggle_phrase_end_before_selection()
        .expect("the marker is removed");
    assert_eq!(
        phrase_beat(&controller, RowId(4)),
        None,
        "line 1's marker is gone; d shifted up to row 5"
    );
    assert_eq!(
        note_data(&controller, RowId(5)),
        (12.0, 4, 0, "d".to_owned())
    );
    assert_eq!(selected(&controller), vec![RowId(5)]);
    controller
        .toggle_phrase_end_before_selection()
        .expect("the marker is back");
    assert_eq!(phrase_beat(&controller, RowId(5)), Some(12.0));
    assert_eq!(
        note_data(&controller, RowId(6)),
        (12.0, 4, 0, "d".to_owned())
    );
}

#[test]
fn a_dragged_phrase_end_moves_and_stays_in_range() {
    // A line gap so the marker has room to move: c ends at 10, d starts
    // at 12.
    let song = concat!(
        "#TITLE:T\n#BPM:240\n",
        ": 0 4 0 a\n: 4 4 2 b\n: 8 2 4 c\n- 10\n",
        ": 12 4 0 d\n: 16 4 2 e\n- 24\nE\n",
    );
    let (dir, path) = fixture(song);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    // The marker of line 1 is row 5.
    controller
        .move_phrase_end(RowId(5), 11)
        .expect("the marker moves");
    assert_eq!(phrase_beat(&controller, RowId(5)), Some(11.0));
    assert_eq!(
        controller.move_phrase_end(RowId(5), 9),
        Err(EditError::PhraseEndOutOfRange),
        "9 is past the previous note's end 10"
    );
    assert_eq!(
        controller.move_phrase_end(RowId(5), 13),
        Err(EditError::PhraseEndOutOfRange),
        "13 is past the next note's start 12"
    );
    assert_eq!(phrase_beat(&controller, RowId(5)), Some(11.0));
    controller.undo();
    assert_eq!(phrase_beat(&controller, RowId(5)), Some(10.0));
    let _ = dir;
}

#[test]
fn transposing_moves_the_pitch_of_every_selected_note() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(2), SelectionMode::Set);
    controller.select_note(RowId(3), SelectionMode::Add);
    controller
        .transpose_selection(2)
        .expect("the transposition is applied");
    assert_eq!(note_data(&controller, RowId(2)).2, 2);
    assert_eq!(note_data(&controller, RowId(3)).2, 4);
    controller.undo();
    assert_eq!(note_data(&controller, RowId(2)).2, 0);
}
