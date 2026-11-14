//! About me: tests for the keyboard navigation and autoplay of task P5-06
//! (plan 8.6, user decisions 2026-11-14). Left/Right step through the notes
//! of the current line and stay in it: from an empty selection Right picks
//! the first note, Left at the first note selects the whole line, Right at
//! the last note stops, and with several notes selected the steps go from
//! the anchor. Up/Down (the existing next/previous line) keep the
//! selection. Autoplay (Shift+A) hands out the padded play range of the
//! current line when it is switched on and whenever the line changes while
//! it is on. Alt+Left/Right change the length of every selected note in one
//! history step.

use tempfile::TempDir;
use usdx_format::rows::RowId;
use usdx_song_editor::controller::EditorController;

/// The same two-line fixture as tests/editing.rs: three notes per line,
/// the note rows 2 to 4 (line 1) and 6 to 8 (line 2).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
    ": 12 4 0 d\n: 16 4 2 e\n: 20 4 4 f\n- 24\nE\n",
);

/// controller loads the two-line fixture and selects nothing.
fn controller() -> (TempDir, EditorController) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, SONG).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, controller)
}

/// the selected ids as a vector, for readable assertions.
fn selected(controller: &EditorController) -> Vec<RowId> {
    controller.selection().iter().copied().collect()
}

/// the note data of a row as (start, length, pitch, text).
fn note_data(controller: &EditorController, id: RowId) -> (f64, i64, i64, String) {
    let loaded = controller.loaded().expect("the song is loaded");
    let note = loaded
        .song
        .note(&loaded.document, id)
        .expect("the row is a note");
    (note.start, note.length, note.pitch, note.text)
}

/// the padded play range the autoplay hands out for the current line,
/// recomputed from the song so the test does not duplicate the timing math.
fn current_range(controller: &EditorController) -> Option<(f64, f64)> {
    let loaded = controller.loaded()?;
    let to_ms = |beat: f64| {
        loaded
            .song
            .timing
            .map(|value| value.beat_to_ms(beat))
            .unwrap_or(beat)
    };
    let line = controller.current_line_data()?;
    let start = loaded.song.note(&loaded.document, *line.notes.first()?)?;
    let end_beat = line
        .end
        .and_then(|id| loaded.song.phrase_end_beat(&loaded.document, id))
        .unwrap_or(start.start + start.length as f64);
    Some((to_ms(start.start) - 300.0, to_ms(end_beat) + 300.0))
}

// ---- Left/Right note navigation ----

#[test]
fn right_from_an_empty_selection_selects_the_first_note() {
    let (_dir, mut controller) = controller();
    // Left from an empty selection does nothing.
    controller.navigate_left();
    assert!(controller.selection().is_empty());
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(2)]);
}

#[test]
fn right_steps_through_the_line_and_stops_at_the_last_note() {
    let (_dir, mut controller) = controller();
    controller.navigate_right();
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(3)]);
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(4)]);
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(4)], "Right stops at c");
}

#[test]
fn left_steps_back_to_the_previous_note() {
    let (_dir, mut controller) = controller();
    controller.navigate_right();
    controller.navigate_right();
    controller.navigate_left();
    assert_eq!(selected(&controller), vec![RowId(2)]);
}

#[test]
fn left_at_the_first_note_selects_the_whole_line() {
    let (_dir, mut controller) = controller();
    controller.navigate_right();
    controller.navigate_left();
    assert_eq!(
        selected(&controller),
        vec![RowId(2), RowId(3), RowId(4)],
        "the whole line is selected"
    );
    assert!(controller.whole_line_selected());
    // Left again on the whole line does nothing.
    controller.navigate_left();
    assert_eq!(selected(&controller), vec![RowId(2), RowId(3), RowId(4)]);
}

#[test]
fn right_from_the_whole_line_selects_the_first_note() {
    let (_dir, mut controller) = controller();
    controller.navigate_right();
    controller.navigate_left();
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(2)]);
    assert!(!controller.whole_line_selected());
}

#[test]
fn navigating_from_a_stale_anchor_enters_the_current_line() {
    let (_dir, mut controller) = controller();
    controller.select_note(
        RowId(2),
        usdx_song_editor::controller::editing::SelectionMode::Set,
    );
    controller.next_line();
    // The selection kept row 2 (line 1), the current line is line 2 now:
    // Right enters the new line at its first note, Left at its last.
    controller.navigate_right();
    assert_eq!(selected(&controller), vec![RowId(6)]);
    controller.select_note(
        RowId(2),
        usdx_song_editor::controller::editing::SelectionMode::Set,
    );
    controller.navigate_left();
    assert_eq!(selected(&controller), vec![RowId(8)]);
}

// ---- Up/Down line navigation keeps the selection ----

#[test]
fn a_line_change_keeps_the_selection() {
    let (_dir, mut controller) = controller();
    controller.select_note(
        RowId(3),
        usdx_song_editor::controller::editing::SelectionMode::Set,
    );
    controller.next_line();
    assert_eq!(selected(&controller), vec![RowId(3)]);
    controller.previous_line();
    assert_eq!(selected(&controller), vec![RowId(3)]);
}

// ---- Autoplay ----

#[test]
fn switching_autoplay_on_plays_the_current_line_at_once() {
    let (_dir, mut controller) = controller();
    assert_eq!(controller.take_pending_play_range(), None);
    controller.set_autoplay(true);
    assert_eq!(
        controller.take_pending_play_range(),
        current_range(&controller),
        "the padded range of line 1"
    );
    // The range is consumed once.
    assert_eq!(controller.take_pending_play_range(), None);
}

#[test]
fn autoplay_plays_each_line_as_it_is_reached() {
    let (_dir, mut controller) = controller();
    controller.set_autoplay(true);
    controller.take_pending_play_range();
    controller.next_line();
    assert_eq!(
        controller.take_pending_play_range(),
        current_range(&controller),
        "the padded range of line 2"
    );
    // At the last line a further Down changes nothing and plays nothing.
    controller.next_line();
    assert_eq!(controller.take_pending_play_range(), None);
    // Navigating back plays again.
    controller.previous_line();
    assert!(controller.take_pending_play_range().is_some());
    // Clicking the line that is already current does not replay it.
    let current = controller.current_line();
    controller.go_to_line(current);
    assert_eq!(controller.take_pending_play_range(), None);
}

#[test]
fn without_autoplay_line_changes_play_nothing() {
    let (_dir, mut controller) = controller();
    controller.next_line();
    assert_eq!(controller.take_pending_play_range(), None);
    controller.set_autoplay(false);
    assert_eq!(controller.take_pending_play_range(), None);
}

// ---- Alt+Left/Right: the length of every selected note ----

#[test]
fn a_length_change_alters_every_selected_note_in_one_undo_step() {
    let (_dir, mut controller) = controller();
    controller.select_note(
        RowId(2),
        usdx_song_editor::controller::editing::SelectionMode::Set,
    );
    controller.select_note(
        RowId(3),
        usdx_song_editor::controller::editing::SelectionMode::Add,
    );
    controller
        .resize_selection_end(1)
        .expect("the length grows");
    assert_eq!(note_data(&controller, RowId(2)).1, 5);
    assert_eq!(note_data(&controller, RowId(3)).1, 5);
    controller.undo();
    assert_eq!(note_data(&controller, RowId(2)).1, 4, "one undo step");
    assert_eq!(note_data(&controller, RowId(3)).1, 4);
}

#[test]
fn a_length_change_below_one_beat_is_refused_and_changes_nothing() {
    let (_dir, mut controller) = controller();
    controller.select_note(
        RowId(2),
        usdx_song_editor::controller::editing::SelectionMode::Set,
    );
    assert_eq!(
        controller.resize_selection_end(-4),
        Err(usdx_edit::EditError::LengthBelowOne),
    );
    assert_eq!(note_data(&controller, RowId(2)).1, 4);
}
