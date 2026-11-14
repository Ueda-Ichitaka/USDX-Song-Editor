//! About me: tests for the inspector state of task P5-05 (UI_SPEC 4.3
//! item 3): the pure function derives what the inspector fields show for
//! the current selection - the single-note values with the note name and
//! the pitch editability of freestyle, rap and rap-golden notes, and the
//! multi-selection summary with the common type and common pitch.

use usdx_format::rows::Document;
use usdx_format::rows::RowId;
use usdx_format::song::Song;
use usdx_song_editor::controller::inspector::{InspectorState, inspector_state};

/// fixture writes a song text into a fresh temporary directory and returns
/// the directory (kept alive for the test), the document and the song view.
fn fixture(text: &str) -> (tempfile::TempDir, Document, Song) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    let bytes = std::fs::read(&path).expect("the fixture is read back");
    let decoded = usdx_format::bytes::decode(&bytes).expect("the fixture decodes");
    let document = usdx_format::rows::classify(decoded);
    let song = Song::from_document(&document);
    (dir, document, song)
}

/// Two lines of three notes each; the note rows are 2 to 4 (line 1) and
/// 6 to 8 (line 2).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
    ": 12 4 0 d\n: 16 4 2 e\n: 20 4 4 f\n- 24\nE\n",
);

#[test]
fn without_a_selection_the_inspector_is_empty() {
    let (_dir, document, song) = fixture(SONG);
    let state = inspector_state(&document, &song, &[]);
    match state {
        InspectorState::None => {}
        _ => panic!("no selection means the None state"),
    }
}

#[test]
fn a_single_selection_carries_the_note_values() {
    let (_dir, document, song) = fixture(SONG);
    let state = inspector_state(&document, &song, &[RowId(3)]);
    match state {
        InspectorState::Single {
            label,
            type_code,
            start,
            length,
            pitch,
            pitch_name,
            text,
            pitch_editable,
        } => {
            assert_eq!(label, "Note 2 · ", "b is the second note of its line");
            assert_eq!(type_code, ':');
            assert_eq!(start, 4);
            assert_eq!(length, 4);
            assert_eq!(pitch, 62, "the MIDI pitch, C4 = 60");
            assert_eq!(pitch_name, "D4");
            assert_eq!(text, "b");
            assert!(pitch_editable);
        }
        _ => panic!("one selected note means the Single state"),
    }
}

#[test]
fn the_text_field_carries_the_raw_syllable() {
    let song_text = concat!("#TITLE:T\n#BPM:240\n", "F 0 4 0 la la\n- 4\nE\n",);
    let (_dir, document, song) = fixture(song_text);
    let state = inspector_state(&document, &song, &[RowId(2)]);
    match state {
        InspectorState::Single {
            text,
            pitch_editable,
            ..
        } => {
            assert_eq!(text, "la la", "the field edits the raw syllable");
            assert!(!pitch_editable, "freestyle notes cannot change pitch");
        }
        _ => panic!("one selected note means the Single state"),
    }
}

#[test]
fn several_notes_report_their_count_and_common_type() {
    let (_dir, document, song) = fixture(SONG);
    let state = inspector_state(&document, &song, &[RowId(2), RowId(3)]);
    match state {
        InspectorState::Multi {
            count,
            type_code,
            common_pitch,
        } => {
            assert_eq!(count, 2);
            assert_eq!(type_code, Some(':'), "both notes are normal");
            assert_eq!(common_pitch, None, "60 and 62 have no common pitch");
        }
        _ => panic!("two selected notes mean the Multi state"),
    }
}

#[test]
fn a_common_pitch_is_reported_for_the_transposition() {
    let (_dir, document, song) = fixture(SONG);
    // a (pitch 0 -> MIDI 60) and d (pitch 0 -> MIDI 60) share the pitch.
    let state = inspector_state(&document, &song, &[RowId(2), RowId(6)]);
    match state {
        InspectorState::Multi {
            type_code,
            common_pitch,
            ..
        } => {
            assert_eq!(type_code, Some(':'));
            assert_eq!(common_pitch, Some(60));
        }
        _ => panic!("two selected notes mean the Multi state"),
    }
}

#[test]
fn notes_of_different_types_report_no_common_type() {
    let (_dir, document, song) = fixture(SONG);
    // Row 2 is normal, row 3 becomes golden by a type key.
    let mut document = document;
    usdx_edit::ops::set_type(
        &document,
        &song,
        &[RowId(3)],
        usdx_format::rows::NoteType::Golden,
    )
    .expect("the type is set")
    .apply(&mut document)
    .expect("the patch applies");
    let song = Song::from_document(&document);
    let state = inspector_state(&document, &song, &[RowId(2), RowId(3)]);
    match state {
        InspectorState::Multi { type_code, .. } => {
            assert_eq!(type_code, None, ": and * have no common type");
        }
        _ => panic!("two selected notes mean the Multi state"),
    }
}
