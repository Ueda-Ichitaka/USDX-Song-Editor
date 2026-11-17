//! About me: tests for the tap-along session controller (task P7-07,
//! plan section 8.2, UI_SPEC 4.8). The session collects Space key presses
//! as taps in audio time: a key event carries the player's playback
//! position and the session subtracts the latency, so the tap time is
//! what the user heard. The first Space starts the pre-roll (2 s before
//! the first line, the playback seek target), recording begins when the
//! playback reaches the first target, and the session finishes when the
//! playback passes the last target. Auto-repeat key events are ignored,
//! Backspace undoes the last tap, and `use_taps` builds the patch the
//! way `usdx-edit`'s tap mapping does. The fixture uses BPM 240 (one
//! beat is 62.5 ms) and no #GAP, so beat b is at 62.5 b ms.

use tempfile::TempDir;
use usdx_edit::EditError;
use usdx_edit::tap::{Tap, apply_line_taps, apply_syllable_taps};
use usdx_format::bytes::decode;
use usdx_format::rows::RowId;
use usdx_format::rows::{Document, classify};
use usdx_format::song::Song;
use usdx_song_editor::controller::tap::{TapMode, TapSession, TapState};

/// fixture writes a song text into a fresh temporary directory and
/// returns the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// Two lines of three notes each; line 0's notes sit at beats 0, 4, 8
/// (spans 0..750 ms), line 1's at 12, 16, 20 (750..1500 ms).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
    ": 12 4 0 d\n: 16 4 2 e\n: 20 4 4 f\n- 24\nE\n",
);

/// session builds a line-mode session over both lines with a 35 ms
/// latency and a 2000 ms pre-roll.
fn line_session() -> (TempDir, Document, Song, TapSession) {
    let (dir, path) = fixture(SONG);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let session = TapSession::new(&doc, &song, 0, 0, 1, TapMode::Line, 1.0, 35.0, 2000.0)
        .expect("the session builds");
    (dir, doc, song, session)
}

#[test]
fn taps_record_the_audio_time_minus_the_latency() {
    let (_dir, _doc, _song, mut session) = line_session();
    session.start();
    session.tick(0.0);
    assert_eq!(session.state(), TapState::Recording);
    session.key_down(1035.0, false);
    session.key_up(1235.0);
    assert_eq!(
        session.taps(),
        &[Tap {
            down_ms: 1000.0,
            up_ms: Some(1200.0),
        }],
        "the position minus the 35 ms latency"
    );
}

#[test]
fn auto_repeat_key_events_are_ignored() {
    let (_dir, _doc, _song, mut session) = line_session();
    // An auto-repeat Space does not even start the pre-roll.
    session.key_down(100.0, true);
    assert_eq!(session.state(), TapState::Ready);
    session.start();
    session.tick(0.0);
    session.key_down(1035.0, false);
    session.key_down(1100.0, true);
    assert_eq!(session.tap_count(), 1, "the auto-repeat press is ignored");
}

#[test]
fn undo_tap_removes_the_last_tap() {
    let (_dir, _doc, _song, mut session) = line_session();
    session.start();
    session.tick(0.0);
    session.key_down(1035.0, false);
    session.key_up(1235.0);
    session.key_down(1535.0, false);
    session.undo_tap();
    assert_eq!(session.tap_count(), 1);
    session.undo_tap();
    assert_eq!(session.tap_count(), 0);
    session.undo_tap();
    assert_eq!(session.tap_count(), 0, "undoing past the first tap is safe");
}

#[test]
fn the_first_space_starts_the_pre_roll_before_the_first_line() {
    let (_dir, _doc, _song, mut session) = line_session();
    assert_eq!(session.state(), TapState::Ready);
    assert_eq!(
        session.pre_roll_start(),
        Some(-2000.0),
        "the first line starts at 0 ms, the pre-roll is 2 s"
    );
    session.start();
    assert_eq!(session.state(), TapState::PreRoll);
    // A press during the pre-roll records nothing.
    session.key_down(500.0, false);
    assert_eq!(session.tap_count(), 0);
    // Recording begins when the playback reaches the first target.
    session.tick(-100.0);
    assert_eq!(session.state(), TapState::PreRoll);
    session.tick(0.0);
    assert_eq!(session.state(), TapState::Recording);
    // The session finishes when the playback passes the last line's end.
    session.tick(1600.0);
    assert_eq!(session.state(), TapState::Finished);
}

#[test]
fn expected_count_and_tap_count_follow_the_range() {
    let (_dir, _doc, _song, session) = line_session();
    assert_eq!(session.expected_count(), 2, "one tap per line");
    assert_eq!(session.tap_count(), 0);
    // A syllable session over one line expects one tap per note.
    let (dir, path) = fixture(SONG);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let session = TapSession::new(&doc, &song, 0, 0, 0, TapMode::Syllable, 1.0, 35.0, 2000.0)
        .expect("the session builds");
    assert_eq!(session.expected_count(), 3, "one tap per note of line 0");
    drop(dir);
}

#[test]
fn use_taps_builds_the_line_patch_the_mapping_plans() {
    let (_dir, doc, song, mut session) = line_session();
    session.start();
    session.tick(0.0);
    // A tap 2 beats late for each line: line 0 at beat 2 (125 ms), line 1
    // at beat 14 (875 ms).
    session.tick(125.0);
    session.key_down(160.0, false);
    session.key_up(410.0);
    session.tick(875.0);
    session.key_down(910.0, false);
    session.key_up(1160.0);
    let patch = session.use_taps(&doc, &song, 0).expect("the taps plan");
    let wanted = apply_line_taps(&doc, &song, 0, 0..2, session.taps())
        .expect("the mapping plans the same patch");
    assert_eq!(patch, wanted);
}

#[test]
fn use_taps_builds_the_syllable_patch_the_mapping_plans() {
    let (dir, path) = fixture(SONG);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let mut session = TapSession::new(&doc, &song, 0, 0, 0, TapMode::Syllable, 1.0, 0.0, 2000.0)
        .expect("the session builds");
    session.start();
    session.tick(0.0);
    // One tap per note of line 0, each held one beat.
    for beat in [0.0, 4.0, 8.0] {
        let ms = beat * 62.5;
        session.tick(ms);
        session.key_down(ms, false);
        session.key_up(ms + 62.5);
    }
    let notes: Vec<RowId> = doc
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| matches!(row.kind, usdx_format::rows::RowKind::Note(_)))
        .take(3)
        .map(|(index, _)| RowId(index as u32))
        .collect();
    let patch = session.use_taps(&doc, &song, 0).expect("the taps plan");
    let wanted = apply_syllable_taps(&doc, &song, 0, &notes, session.taps())
        .expect("the mapping plans the same patch");
    assert_eq!(patch, wanted);
    drop(dir);
}

#[test]
fn a_session_without_a_usable_song_is_refused() {
    let (dir, path) = fixture("#TITLE:T\n: 0 4 0 a \nE\n");
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    assert!(matches!(
        TapSession::new(&doc, &song, 0, 0, 0, TapMode::Line, 1.0, 35.0, 2000.0),
        Err(EditError::NoTiming)
    ));
    drop(dir);
}

#[test]
fn range_end_reports_the_last_targets_end_for_the_preroll_playback() {
    let (_dir, _doc, _song, session) = line_session();
    assert!(session.range_end().is_some(), "the session has targets");
}

#[test]
fn current_syllable_follows_the_taps_and_clamps_at_the_end() {
    let (_dir, _doc, _song, mut session) = line_session();
    assert_eq!(session.current_syllable(), 0, "the first target is next");
    session.start();
    session.tick(0.0);
    session.key_down(35.0, false);
    assert_eq!(session.current_syllable(), 1, "one tap recorded");
    session.key_down(835.0, false);
    assert_eq!(
        session.current_syllable(),
        2,
        "both lines tapped; the count clamps at the expected count"
    );
    session.key_down(1000.0, false);
    assert_eq!(
        session.current_syllable(),
        2,
        "extra taps do not push past the expected count"
    );
}
