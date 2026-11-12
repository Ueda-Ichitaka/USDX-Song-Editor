//! About me: tests for the EditorController's load and navigation (task
//! P3-01, plan section 8.2). They assert that opening a fixture loads the
//! document, song view, history and change log and counts the lines across
//! all voices, that next/previous and goToLine move and clamp, that a
//! major-version-2 (or unparsable) song loads read-only while unversioned
//! and 1.x songs stay editable, that a failed open keeps the previously
//! loaded song, and that the file fingerprint separates content by hash.

use std::path::Path;

use tempfile::TempDir;
use usdx_song_editor::controller::{EditorController, FileFingerprint};

/// fixture writes a song text into a fresh temporary directory and returns
/// the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// a song with two lines in one voice.
const SONG: &str = "#TITLE:T\n#BPM:240\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

/// a song with two lines in voice 1 and one line in voice 2.
const TWO_VOICES: &str =
    "#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\n- 4\n: 4 4 1 b\n- 8\nP2\n: 0 4 0 c\n- 4\nE\n";

#[test]
fn open_loads_a_song_and_counts_its_lines() {
    let (_dir, path) = fixture(SONG);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    assert!(controller.is_loaded());
    assert_eq!(controller.line_count(), 2);
    assert_eq!(controller.current_line(), 0);
    assert_eq!(controller.voice_count(), 1);
    assert_eq!(controller.current_voice_index(), Some(0));
    assert!(!controller.read_only());
    assert!(!controller.is_dirty(), "a freshly loaded song is not dirty");
    assert_eq!(controller.path(), Some(Path::new(path.as_path())));
}

#[test]
fn a_fresh_controller_is_empty_and_navigation_is_a_no_op() {
    let mut controller = EditorController::new();
    assert!(!controller.is_loaded());
    assert_eq!(controller.line_count(), 0);
    assert_eq!(controller.current_line(), 0);
    assert_eq!(controller.voice_count(), 0);
    assert_eq!(controller.current_voice_index(), None);
    assert!(!controller.read_only());
    assert!(!controller.is_dirty());
    controller.next_line();
    controller.previous_line();
    controller.go_to_line(5);
    assert_eq!(controller.current_line(), 0);
}

#[test]
fn next_and_previous_move_and_clamp() {
    let (_dir, path) = fixture(TWO_VOICES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    assert_eq!(controller.line_count(), 3);
    for expected in [1, 2, 2, 2] {
        controller.next_line();
        assert_eq!(controller.current_line(), expected);
    }
    for expected in [1, 0, 0, 0] {
        controller.previous_line();
        assert_eq!(controller.current_line(), expected);
    }
}

#[test]
fn go_to_line_jumps_across_voices_and_clamps() {
    let (_dir, path) = fixture(TWO_VOICES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    controller.go_to_line(1);
    assert_eq!(controller.current_line(), 1);
    assert_eq!(controller.current_voice_index(), Some(0));
    controller.go_to_line(2);
    assert_eq!(controller.current_line(), 2);
    assert_eq!(controller.current_voice_index(), Some(1), "the line in P2");
    controller.go_to_line(99);
    assert_eq!(controller.current_line(), 2);
    controller.go_to_line(0);
    assert_eq!(controller.current_line(), 0);
}

#[test]
fn major_version_2_and_unparsable_versions_load_read_only() {
    for (text, expected) in [
        ("#TITLE:T\n#VERSION:2.0.0\n#BPM:240\n: 0 4 0 la\nE\n", true),
        ("#TITLE:T\n#VERSION:1.1.0\n#BPM:240\n: 0 4 0 la\nE\n", false),
        ("#TITLE:T\n#BPM:240\n: 0 4 0 la\nE\n", false),
        ("#TITLE:T\n#VERSION:nope\n#BPM:240\n: 0 4 0 la\nE\n", true),
    ] {
        let (_dir, path) = fixture(text);
        let mut controller = EditorController::new();
        controller.open(&path).expect("the song loads");
        assert_eq!(controller.read_only(), expected, "for {text}");
    }
}

#[test]
fn a_failed_open_keeps_the_previous_song() {
    let (_dir, path) = fixture(SONG);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    let missing = path.with_file_name("missing.txt");
    assert!(controller.open(&missing).is_err());
    assert!(controller.is_loaded());
    assert_eq!(controller.line_count(), 2);
    assert_eq!(controller.path(), Some(Path::new(path.as_path())));
}

#[test]
fn the_fingerprint_hash_separates_content_of_the_same_size() {
    let a = FileFingerprint::new(4, None, b"abc\n");
    let same = FileFingerprint::new(4, None, b"abc\n");
    let other = FileFingerprint::new(4, None, b"abd\n");
    assert_eq!(a, same);
    assert_ne!(a.hash, other.hash);
    assert_eq!(a.size, other.size);
}
