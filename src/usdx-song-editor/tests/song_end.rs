//! About me: tests for the song end controller (user decisions 2026-11-15):
//! the scroll bounds of the sheet (from 0:00 to the later of the audio end,
//! or #END, and the last note, plus 2 s), the target of the "Go to song end"
//! jump (#END, else the audio end, else the last note's end), and the
//! "End song here" and "Remove song end" commands as one undo step each.

use tempfile::TempDir;
use usdx_format::rows::RowId;
use usdx_song_editor::controller::EditorController;
use usdx_song_editor::controller::editing::SelectionMode;
use usdx_song_editor::controller::song_end::{SCROLL_MARGIN_MS, scroll_bounds, song_end_target_ms};

/// Two f64 values equal within 1e-6.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// BPM 300: one beat is 50 ms; GAP 1000 puts beat 0 at 1 s.
const GAP_MS: f64 = 1000.0;
const BPM: f64 = 300.0;

#[test]
fn the_scroll_starts_at_zero_ms() {
    // 0 ms lies 20 beats before beat 0.
    let (min, _) = scroll_bounds(GAP_MS, BPM, None, None, 100.0);
    assert!(close(min, -20.0), "{min}");
}

#[test]
fn without_audio_the_scroll_ends_two_seconds_after_the_last_note() {
    let (_, max) = scroll_bounds(GAP_MS, BPM, None, None, 100.0);
    assert!(close(max, 100.0 + SCROLL_MARGIN_MS / 50.0), "{max}");
    assert_eq!(SCROLL_MARGIN_MS, 2000.0);
}

#[test]
fn a_longer_audio_extends_the_scroll() {
    // The audio ends at 20 s = beat 380; plus 2 s = beat 420.
    let (_, max) = scroll_bounds(GAP_MS, BPM, Some(20_000.0), None, 100.0);
    assert!(close(max, 420.0), "{max}");
    // A shorter audio does not cut the notes off.
    let (_, max) = scroll_bounds(GAP_MS, BPM, Some(2_000.0), None, 100.0);
    assert!(close(max, 140.0), "{max}");
}

#[test]
fn end_replaces_the_audio_end() {
    // #END at 6 s = beat 100; the notes end at beat 60.
    let (_, max) = scroll_bounds(GAP_MS, BPM, Some(20_000.0), Some(6_000.0), 60.0);
    assert!(close(max, 140.0), "{max}");
}

#[test]
fn the_end_jump_goes_to_end_then_audio_then_the_last_note() {
    assert_eq!(
        song_end_target_ms(Some(9_000.0), Some(6_000.0), 5_000.0),
        6_000.0
    );
    assert_eq!(song_end_target_ms(Some(9_000.0), None, 5_000.0), 9_000.0);
    assert_eq!(song_end_target_ms(None, None, 5_000.0), 5_000.0);
    // An unknown audio length arrives as 0 from the player.
    assert_eq!(song_end_target_ms(Some(0.0), None, 5_000.0), 5_000.0);
}

/// Two lines; the second line's last note starts at beat 20 (2 s).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:300\n#GAP:1000\n",
    ": 0 4 0 a \n: 4 4 0 b \n- 10\n",
    ": 12 4 0 c \n: 20 4 0 d \nE\n",
);

fn controller() -> (TempDir, EditorController) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, SONG).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, controller)
}

#[test]
fn end_song_here_writes_end_and_cuts_in_one_undo_step() {
    let (_dir, mut controller) = controller();
    assert_eq!(controller.end_ms(), None);
    controller.end_song_at(1500.0).expect("the cut applies");
    assert_eq!(controller.end_ms(), Some(1500.0));
    // Beat 10 is the cut: line 2 lost all its notes.
    assert_eq!(controller.line_count(), 1);
    assert_eq!(
        controller
            .loaded()
            .and_then(|loaded| loaded.history.undo_label().map(str::to_owned))
            .as_deref(),
        Some("End song here")
    );
    controller.undo();
    assert_eq!(controller.end_ms(), None);
    assert_eq!(controller.line_count(), 2);
}

#[test]
fn a_cut_keeps_the_current_line_in_range_and_clears_the_selection() {
    let (_dir, mut controller) = controller();
    controller.go_to_line(1);
    controller.select_note(RowId(7), SelectionMode::Set);
    controller.end_song_at(1500.0).expect("the cut applies");
    assert_eq!(controller.current_line(), 0);
    assert!(controller.selection().is_empty());
}

#[test]
fn remove_song_end_clears_end_only() {
    let (_dir, mut controller) = controller();
    controller.end_song_at(5000.0).expect("the cut applies");
    controller.remove_song_end().expect("the header goes");
    assert_eq!(controller.end_ms(), None);
    assert_eq!(controller.line_count(), 2);
}
