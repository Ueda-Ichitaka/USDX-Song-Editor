//! About me: tests for the playback follow of the current line (user
//! decision 2026-11-15, "Follow line"): while the song plays, the current
//! line becomes the line being sung, which is the last line of the current
//! voice whose first note starts at or before the playback position. Before
//! the voice's first line nothing changes. Following never arms the
//! autoplay, because the player is already playing the song.

use tempfile::TempDir;
use usdx_song_editor::controller::EditorController;

/// Two lines at 240 BPM (62.5 ms per beat, GAP 100 ms): line 1 starts at
/// beat 0 (100 ms), line 2 at beat 12 (850 ms).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n#GAP:100\n",
    ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
    ": 12 4 0 d\n: 16 4 2 e\n: 20 4 4 f\n- 24\nE\n",
);

/// A duet: voice 1 sings lines at beats 0 and 16, voice 2 at beats 8 and
/// 24, so the voices' lines interleave in time.
const DUET: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    "P1\n: 0 4 0 a\n- 6\n: 16 4 0 b\n- 20\n",
    "P2\n: 8 4 0 c\n- 12\n: 24 4 0 d\n- 28\nE\n",
);

/// controller loads a fixture song.
fn controller(text: &str) -> (TempDir, EditorController) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, controller)
}

#[test]
fn the_line_being_sung_is_the_last_line_started() {
    let (_dir, controller) = controller(SONG);
    assert_eq!(controller.line_at_ms(100.0), Some(0));
    assert_eq!(controller.line_at_ms(849.0), Some(0));
    assert_eq!(controller.line_at_ms(850.0), Some(1));
    assert_eq!(controller.line_at_ms(60_000.0), Some(1));
}

#[test]
fn no_line_is_sung_before_the_first_one() {
    let (_dir, controller) = controller(SONG);
    assert_eq!(controller.line_at_ms(0.0), None);
    assert_eq!(controller.line_at_ms(99.0), None);
}

#[test]
fn following_moves_the_current_line() {
    let (_dir, mut controller) = controller(SONG);
    assert!(controller.follow_playback(900.0));
    assert_eq!(controller.current_line(), 1);
    // The same line again changes nothing.
    assert!(!controller.follow_playback(1000.0));
    // Seeking back follows back.
    assert!(controller.follow_playback(200.0));
    assert_eq!(controller.current_line(), 0);
}

#[test]
fn following_before_the_first_line_keeps_the_current_line() {
    let (_dir, mut controller) = controller(SONG);
    controller.go_to_line(1);
    assert!(!controller.follow_playback(0.0));
    assert_eq!(controller.current_line(), 1);
}

#[test]
fn following_never_arms_the_autoplay() {
    let (_dir, mut controller) = controller(SONG);
    controller.set_autoplay(true);
    // Switching autoplay on armed the current line; take it away first.
    let _ = controller.take_pending_play_range();
    assert!(controller.follow_playback(900.0));
    assert_eq!(controller.take_pending_play_range(), None);
}

#[test]
fn a_duet_follows_within_the_current_voice() {
    let (_dir, mut controller) = controller(DUET);
    // Global lines: 0 and 1 are voice 1, 2 and 3 are voice 2.
    assert_eq!(controller.line_count(), 4);
    // In voice 1, beat 10 (625 ms) still sings line 0; voice 2's line at
    // beat 8 does not take over.
    assert!(!controller.follow_playback(625.0));
    assert_eq!(controller.current_line(), 0);
    assert!(controller.follow_playback(1000.0));
    assert_eq!(controller.current_line(), 1);
    // In voice 2, beat 24 (1500 ms) is its second line, global index 3.
    controller.go_to_line(2);
    assert!(controller.follow_playback(1500.0));
    assert_eq!(controller.current_line(), 3);
}

#[test]
fn nothing_follows_without_a_song() {
    let mut controller = EditorController::new();
    assert_eq!(controller.line_at_ms(1000.0), None);
    assert!(!controller.follow_playback(1000.0));
}
