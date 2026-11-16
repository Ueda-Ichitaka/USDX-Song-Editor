//! About me: tests for the repair controller of task P7-07 (plan
//! sections 8.2, 6.7, 6.8, 6.11). The controller holds the repair mode
//! (anchor, block, tap), the anchors with their pinned times, the end
//! pin, the block range and times, and the tap session of the tap mode;
//! `preview` computes the patch the UI ghosts, `apply` commits exactly
//! that patch as one history step (so preview equals what apply
//! commits), `cancel` clears the pending preview and leaves the document
//! unchanged, and `export_entries` builds the anchors.txt content
//! (decision D-12) from the applied changes, the pinned anchors and the
//! end pin. The fixture uses BPM 240 (one beat is 62.5 ms) and no #GAP,
//! so beat b sits at 62.5 b ms.

use tempfile::TempDir;
use usdx_edit::EditError;
use usdx_edit::anchors_export::{AnchorEntry, render};
use usdx_edit::block::{Follow, block_transform};
use usdx_edit::retime::{Anchor, Lengths, Outside, Pin, StretchOptions, anchor_stretch};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;
use usdx_song_editor::controller::editor::LoadedSong;
use usdx_song_editor::controller::fingerprint::FileFingerprint;
use usdx_song_editor::controller::repair::{RepairController, RepairMode};
use usdx_song_editor::controller::tap::TapMode;

/// fixture writes a song text into a fresh temporary directory and
/// returns the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// Three lines of three notes each at beats 0/12/24; the phrase ends sit
/// at beats 12 and 24. The syllables carry trailing spaces, so each note
/// is its own word.
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a \n: 4 4 2 b \n: 8 4 4 c \n- 12\n",
    ": 12 4 0 d \n: 16 4 2 e \n: 20 4 4 f \n- 24\n",
    ": 24 4 0 g \n: 28 4 2 h \n: 32 4 4 i \nE\n",
);

/// loaded builds a document and song from the fixture text.
fn loaded() -> (TempDir, Document, Song) {
    let (dir, path) = fixture(SONG);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    (dir, doc, song)
}

/// Three lines with 24-beat gaps: line 0 at beats 0-11, line 1 at
/// 24-35, line 2 at 48-59; the phrase ends sit at beats 13 and 37. The
/// tap test moves single lines rigidly, which collides on the tight
/// fixture but not here.
const SPACED: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a \n: 4 4 2 b \n: 8 4 4 c \n- 13\n",
    ": 24 4 0 d \n: 28 4 2 e \n: 32 4 4 f \n- 37\n",
    ": 48 4 0 g \n: 52 4 2 h \n: 56 4 4 i \nE\n",
);

/// spaced builds a document and song from the wide-gapped fixture text.
fn spaced() -> (TempDir, Document, Song) {
    let (dir, path) = fixture(SPACED);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    (dir, doc, song)
}

/// loaded_song builds a LoadedSong around a fresh fixture load, the way
/// the controller's open does: fingerprint from the file bytes, a change
/// log snapshot as loaded, an empty history.
fn loaded_song() -> (TempDir, LoadedSong) {
    let (dir, path) = fixture(SONG);
    let bytes = std::fs::read(&path).expect("the fixture is read");
    let doc = classify(decode(&bytes).expect("the song decodes"));
    let song = Song::from_document(&doc);
    let changes = usdx_edit::changes::ChangeLog::snapshot(&doc, &song);
    let loaded = LoadedSong {
        path,
        fingerprint: FileFingerprint::new(bytes.len() as u64, None, &bytes),
        document: doc,
        song,
        history: usdx_edit::history::History::new(),
        changes,
        read_only: false,
    };
    (dir, loaded)
}

/// options builds the controller's stretch options: the lines before and
/// after the anchors keep their times, the lengths keep their beat
/// count, no end pin.
fn options() -> StretchOptions {
    StretchOptions {
        before: Outside::Keep,
        after: Outside::Keep,
        lengths: Lengths::Keep,
        end_pin_ms: None,
    }
}

/// note_starts lists the current start beat of every note row in row
/// order.
fn note_starts(doc: &Document) -> Vec<i64> {
    doc.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.start),
            _ => None,
        })
        .collect()
}

#[test]
fn pin_current_line_creates_and_updates_one_anchor_per_line() {
    let mut repair = RepairController::new();
    assert_eq!(repair.mode(), RepairMode::Anchor);
    repair.pin_current_line_at(0, 500.0);
    assert_eq!(
        repair.anchors(),
        &[Anchor {
            line: 0,
            target_ms: 500.0,
            pin: Pin::Start,
        }]
    );
    // Pinning the same line again updates the target, it does not add a
    // second anchor.
    repair.pin_current_line_at(0, 600.0);
    assert_eq!(repair.anchors()[0].target_ms, 600.0);
    // A second anchor on another line.
    repair.pin_current_line_at(2, 2500.0);
    assert_eq!(repair.anchors().len(), 2);
    repair.set_anchor_ms(1, 2600.0);
    assert_eq!(repair.anchors()[1].target_ms, 2600.0);
    repair.remove_anchor(0);
    assert_eq!(repair.anchors().len(), 1);
    assert_eq!(repair.anchors()[0].line, 2);
}

#[test]
fn anchor_preview_equals_what_apply_commits() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    // Line 0 pinned at beat 8 (500 ms), line 2 at beat 32 (2000 ms): the
    // whole voice shifts rigidly by 8 beats.
    repair.pin_current_line_at(0, 500.0);
    repair.pin_current_line_at(2, 2000.0);
    let patch =
        anchor_stretch(&doc, &song, 0, repair.anchors(), &options()).expect("the stretch plans");
    assert_eq!(
        repair.preview(&doc, &song, 0).expect("the preview plans"),
        &patch
    );

    // The apply commits exactly this preview as one history step.
    let (_dir2, mut loaded_song) = loaded_song();
    let mut expected = loaded_song.document.clone();
    patch.apply(&mut expected).expect("the preview applies");
    repair
        .apply(&mut loaded_song, 0)
        .expect("the apply commits");
    assert_eq!(note_starts(&loaded_song.document), note_starts(&expected));
    assert_eq!(loaded_song.song, Song::from_document(&loaded_song.document));
    assert!(loaded_song.history.is_dirty(), "the apply is one undo step");
}

#[test]
fn anchor_apply_moves_the_lines_to_the_pinned_times() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    repair.pin_current_line_at(0, 500.0);
    repair.pin_current_line_at(2, 2000.0);
    repair.preview(&doc, &song, 0).expect("the preview plans");
    let (_dir2, mut loaded_song) = loaded_song();
    repair
        .apply(&mut loaded_song, 0)
        .expect("the apply commits");
    // The segment map is rigid (k = 1): every line shifts by 8 beats,
    // the lengths keep their 4 beats.
    assert_eq!(
        note_starts(&loaded_song.document),
        vec![8, 12, 16, 20, 24, 28, 32, 36, 40]
    );
}

#[test]
fn cancel_clears_the_preview_and_leaves_the_document_unchanged() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    repair.pin_current_line_at(0, 500.0);
    repair.preview(&doc, &song, 0).expect("the preview plans");
    repair.cancel();
    assert_eq!(note_starts(&doc), vec![0, 4, 8, 12, 16, 20, 24, 28, 32]);
    assert!(!repair.has_preview(), "the cancelled preview is gone");
}

#[test]
fn block_mode_previews_the_block_transform() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    repair.set_mode(RepairMode::Block);
    repair.set_block(0, 0);
    repair.set_block_times(500.0, 1250.0);
    repair.set_follow(Follow::MoveAlong);
    let patch = block_transform(&doc, &song, 0, 0..1, 500.0, 1250.0, Follow::MoveAlong)
        .expect("the block plans");
    assert_eq!(
        repair.preview(&doc, &song, 0).expect("the preview plans"),
        &patch,
        "the block mode's preview is the block transform's patch"
    );
    // Applied, line 0 moves to beats 8..20 and the following lines shift
    // along by the block's end offset of 8 beats.
    let (_dir2, mut loaded_song) = loaded_song();
    repair
        .apply(&mut loaded_song, 0)
        .expect("the apply commits");
    assert_eq!(
        note_starts(&loaded_song.document),
        vec![8, 12, 16, 20, 24, 28, 32, 36, 40]
    );
}

#[test]
fn tap_mode_previews_the_session_result() {
    // The tap mapping shifts every tapped line rigidly onto its tap and
    // the collision pass resolves overlaps; this fixture spaces the
    // lines 24 beats apart, so the tapped lines' rigid shifts collide
    // with nothing and the expectations stay readable.
    let (_dir, doc, song) = spaced();
    let mut repair = RepairController::new();
    repair.set_mode(RepairMode::Tap);
    repair
        .start_tap_session(&doc, &song, 0, 0, 1, TapMode::Line, 1.0, 0.0, 2000.0)
        .expect("the session builds");
    {
        let session = repair.tap_session().expect("the session exists");
        session.start();
        session.tick(0.0);
        session.tick(500.0);
        session.key_down(500.0, false);
        session.tick(2000.0);
        session.key_down(2000.0, false);
    }
    let patch = repair.preview(&doc, &song, 0).expect("the preview plans");
    let mut expected = doc.clone();
    patch.apply(&mut expected).expect("the patch applies");
    // Line 0 moves rigidly onto 500 ms (beat 8), line 1 onto 2000 ms
    // (beat 32); line 2 keeps its times.
    assert_eq!(
        note_starts(&expected),
        vec![8, 12, 16, 32, 36, 40, 48, 52, 56]
    );
}

#[test]
fn export_entries_lists_the_applied_changes_pins_and_end() {
    let (_dir, mut loaded_song) = loaded_song();
    let mut repair = RepairController::new();
    // One anchor (line 0 at 500 ms) and the end pin: the pin stretches
    // the lines after the anchor so the voice's last note ends at
    // 5000 ms (plan 6.7), so all three lines changed and are exported
    // (decision D-12); line 0's entry carries its pinned time.
    repair.pin_current_line_at(0, 500.0);
    repair.set_end_pin(Some(5000.0));
    repair
        .apply(&mut loaded_song, 0)
        .expect("the apply commits");
    let (entries, end) = repair.export_entries(&loaded_song);
    assert_eq!(
        entries,
        vec![
            AnchorEntry {
                time_ms: 500.0,
                text: "a b c".to_owned(),
                source_line: 0,
            },
            AnchorEntry {
                time_ms: 2000.0,
                text: "d e f".to_owned(),
                source_line: 1,
            },
            AnchorEntry {
                time_ms: 3500.0,
                text: "g h i".to_owned(),
                source_line: 2,
            },
        ]
    );
    assert_eq!(end, Some(5000.0));
    let file = render("T", "0.1.0", "2026-11-16", &entries, end).expect("the file renders");
    assert!(file.contains("0:00.50 | a b c\n"));
    assert!(file.ends_with("end 0:05.00\n"));
}

#[test]
fn an_anchor_below_beat_zero_surfaces_the_gap_shift() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    repair.pin_current_line_at(0, -500.0);
    assert!(matches!(
        repair.preview(&doc, &song, 0),
        Err(EditError::NeedsGapShift(_))
    ));
}
