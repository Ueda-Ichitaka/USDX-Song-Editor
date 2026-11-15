//! About me: tests for the visible note and phrase end sets of task P5-03
//! (plan section 8.3). The models feed the note delegates and the phrase end
//! handles of the note sheet, so the pure row building is tested without
//! QML: the current line's notes with its global previous and next line
//! (user decision 2026-11-13), the line offsets, the MIDI pitches (USDX
//! stores relative pitches, the sheet axis reads C4 = 60, UI_SPEC 3.3), the
//! syllable texts and the phrase ends with their drag bounds between the
//! surrounding notes (the same bounds `usdx_edit::ops::move_phrase_end`
//! enforces), unbounded where no note sits on that side.

use tempfile::TempDir;
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowId, classify};
use usdx_format::song::{Line, Song};
use usdx_song_editor::controller::EditorController;
use usdx_song_editor::controller::notes::{
    PhraseEndRow, VisibleNote, all_lines, neighbour_lines, song_end_beat, type_char, visible_notes,
    visible_phrase_ends,
};

/// song decodes a song text into the document and its song view.
fn song(text: &str) -> (Document, Song) {
    let decoded = decode(text.as_bytes()).expect("the song decodes");
    let document = classify(decoded);
    let song = Song::from_document(&document);
    (document, song)
}

/// the lines of a song, flattened in global order.
fn lines_of(song: &Song) -> Vec<&Line> {
    song.voices.iter().flat_map(|voice| &voice.lines).collect()
}

/// Three lines: line 1 (notes at beats 0 and 4, ends at the marker 8),
/// line 2 (notes at 10 and 14, ends at 18) and line 3 (one golden note at
/// 20, no end marker).
const THREE_LINES: &str = concat!(
    "#TITLE:T\n#BPM:240\n",
    ": 0 4 0 la\n: 4 4 2 la\tla\n- 8\n",
    ": 10 4 -1 la\n: 14 4 0 la\n- 18\n",
    "* 20 4 1 la\nE\n",
);

#[test]
fn visible_notes_return_the_current_line_and_its_global_neighbours() {
    let (doc, song) = song(THREE_LINES);
    let lines = lines_of(&song);
    let notes = visible_notes(&doc, &song, &lines, 1);
    assert_eq!(
        notes,
        vec![
            VisibleNote {
                row_id: RowId(2),
                start: 0.0,
                length: 4,
                pitch: 60,
                kind: ":",
                text: "la".into(),
                line_offset: -1,
            },
            VisibleNote {
                row_id: RowId(3),
                start: 4.0,
                length: 4,
                pitch: 62,
                kind: ":",
                text: "la la".into(),
                line_offset: -1,
            },
            VisibleNote {
                row_id: RowId(5),
                start: 10.0,
                length: 4,
                pitch: 59,
                kind: ":",
                text: "la".into(),
                line_offset: 0,
            },
            VisibleNote {
                row_id: RowId(6),
                start: 14.0,
                length: 4,
                pitch: 60,
                kind: ":",
                text: "la".into(),
                line_offset: 0,
            },
            VisibleNote {
                row_id: RowId(8),
                start: 20.0,
                length: 4,
                pitch: 61,
                kind: "*",
                text: "la".into(),
                line_offset: 1,
            },
        ],
        "previous, current and next line, USDX pitches as MIDI (C4 = 60)"
    );
}

#[test]
fn visible_notes_at_the_first_line_have_no_previous_line() {
    let (doc, song) = song(THREE_LINES);
    let lines = lines_of(&song);
    // The bridge gathers the neighbours like `neighbour_lines` does: at the
    // first line the set is the current line and the next one, at the last
    // line the previous one and the current one.
    let (notes, offsets) = {
        let gathered: Vec<&Line> = vec![lines[0], lines[1]];
        let notes = visible_notes(&doc, &song, &gathered, 0);
        let offsets: Vec<i32> = notes.iter().map(|note| note.line_offset).collect();
        (notes.len(), offsets)
    };
    assert_eq!(offsets.as_slice(), &[0, 0, 1, 1]);
    assert_eq!(notes, 4);
    let gathered: Vec<&Line> = vec![lines[1], lines[2]];
    let notes = visible_notes(&doc, &song, &gathered, 1);
    let offsets: Vec<i32> = notes.iter().map(|note| note.line_offset).collect();
    assert_eq!(offsets.as_slice(), &[-1, -1, 0]);
    assert_eq!(notes.len(), 3);
}

#[test]
fn visible_notes_of_an_empty_song_are_empty() {
    let (doc, song) = song("#TITLE:T\nE\n");
    let lines = lines_of(&song);
    assert!(visible_notes(&doc, &song, &lines, 0).is_empty());
}

#[test]
fn type_char_names_the_five_note_types() {
    use usdx_format::rows::NoteType;
    assert_eq!(type_char(&NoteType::Normal), ":");
    assert_eq!(type_char(&NoteType::Golden), "*");
    assert_eq!(type_char(&NoteType::Freestyle), "F");
    assert_eq!(type_char(&NoteType::Rap), "R");
    assert_eq!(type_char(&NoteType::RapGolden), "G");
    assert_eq!(type_char(&NoteType::Other('X')), "F");
}

#[test]
fn visible_phrase_ends_carry_the_bounds_between_the_surrounding_notes() {
    let (doc, song) = song(THREE_LINES);
    let lines = lines_of(&song);
    let ends = visible_phrase_ends(&doc, &song, &lines);
    assert_eq!(
        ends,
        vec![
            PhraseEndRow {
                row_id: RowId(4),
                beat: 8.0,
                min: 8.0,
                max: 10.0,
            },
            PhraseEndRow {
                row_id: RowId(7),
                beat: 18.0,
                min: 18.0,
                max: 20.0,
            },
        ],
        "line 3 has no end marker, so only two rows"
    );
}

#[test]
fn the_last_phrase_end_has_no_upper_bound() {
    let (doc, song) = song("#TITLE:T\n#BPM:240\n: 0 4 0 la\n- 8\n: 8 4 0 la\n- 12\nE\n");
    let lines = lines_of(&song);
    let ends = visible_phrase_ends(&doc, &song, &lines);
    assert_eq!(
        ends,
        vec![
            PhraseEndRow {
                row_id: RowId(3),
                beat: 8.0,
                min: 4.0,
                max: 8.0,
            },
            PhraseEndRow {
                row_id: RowId(5),
                beat: 12.0,
                min: 12.0,
                max: f64::INFINITY,
            },
        ],
        "no note after the last marker: the upper bound is unbounded"
    );
}

#[test]
fn a_relative_song_reports_shifted_phrase_end_beats() {
    // The first marker sits at raw 2 (no offset before it), the second at
    // raw 4 + the first marker's rel-offset 2 = 6.
    let (doc, song) =
        song("#TITLE:T\n#BPM:240\n#RELATIVE:yes\n: 0 4 0 la\n- 2 2\n: 0 4 0 la\n- 4\nE\n");
    let lines = lines_of(&song);
    let ends = visible_phrase_ends(&doc, &song, &lines);
    assert_eq!(ends.len(), 2, "both lines end with a marker");
    assert_eq!(ends[0].beat, 2.0, "the first marker reads raw");
    assert_eq!(ends[1].beat, 6.0, "the second marker shifts by the offset");
}

#[test]
fn neighbour_lines_gather_the_current_line_with_its_global_neighbours() {
    let (_dir, path) = fixture(THREE_LINES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    controller.go_to_line(1);
    let (lines, current) = neighbour_lines(&controller);
    assert_eq!(current, 1, "the current line sits between its neighbours");
    assert_eq!(lines.len(), 3);
    let loaded = controller.loaded().expect("loaded");
    assert_eq!(
        lines[1].text(&loaded.document),
        "lala",
        "the middle line is the current one"
    );
}

#[test]
fn neighbour_lines_clamp_at_the_song_edges() {
    let (_dir, path) = fixture(THREE_LINES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    let (lines, current) = neighbour_lines(&controller);
    assert_eq!(
        (lines.len(), current),
        (2, 0),
        "the first line has no previous"
    );
    controller.go_to_line(2);
    let (lines, current) = neighbour_lines(&controller);
    assert_eq!((lines.len(), current), (2, 1), "the last line has no next");
}

#[test]
fn neighbour_lines_of_an_empty_controller_are_empty() {
    let controller = EditorController::new();
    let (lines, current) = neighbour_lines(&controller);
    assert!(lines.is_empty());
    assert_eq!(current, 0);
}

#[test]
fn all_lines_gather_every_line_of_the_song() {
    let (_dir, path) = fixture(THREE_LINES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    controller.go_to_line(1);
    let (lines, current) = all_lines(&controller);
    assert_eq!(lines.len(), 3, "every line of the song");
    assert_eq!(current, 1, "the index of the current line within them");
}

#[test]
fn all_lines_feed_visible_notes_for_the_whole_song() {
    let (_dir, path) = fixture(THREE_LINES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    controller.go_to_line(1);
    let (lines, current) = all_lines(&controller);
    let loaded = controller.loaded().expect("loaded");
    let notes = visible_notes(&loaded.document, &loaded.song, &lines, current);
    assert_eq!(notes.len(), 5, "every note of the song");
    assert_eq!(notes[0].line_offset, -1, "line 1 sits one before current");
    assert_eq!(notes[4].line_offset, 1, "line 3 sits one after current");
    // The line offset reaches the note's own line from the current one, so
    // the delegate's click navigation `goToLine(current + offset)` works
    // across the whole song.
    assert_eq!(notes[2].line_offset, 0, "the current line's notes");
}

#[test]
fn all_lines_clamp_when_no_line_is_current() {
    let (_dir, path) = fixture(THREE_LINES);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    let (lines, current) = all_lines(&controller);
    assert_eq!(lines.len(), 3, "the song still gathers");
    assert_eq!(current, 0, "the default current line");
}

#[test]
fn all_lines_of_an_empty_controller_are_empty() {
    let controller = EditorController::new();
    let (lines, current) = all_lines(&controller);
    assert!(lines.is_empty());
    assert_eq!(current, 0);
}

#[test]
fn song_end_beat_reaches_the_last_note_end() {
    // THREE_LINES' last note starts at beat 20 and runs 4 beats; the
    // phrase ends at 8 and 18 stay below.
    let (doc, song) = song(THREE_LINES);
    assert_eq!(
        song_end_beat(&doc, &song),
        24.0,
        "the last note ends at beat 24"
    );
}

#[test]
fn song_end_beat_reaches_a_phrase_end_after_the_notes() {
    // The phrase end marker sits after every note here.
    let (doc, song) = song("#TITLE:T\n#BPM:240\n: 0 4 0 la\n- 30\nE\n");
    assert_eq!(
        song_end_beat(&doc, &song),
        30.0,
        "the phrase end beat is the song end"
    );
}

#[test]
fn song_end_beat_of_an_empty_song_is_zero() {
    let (doc, song) = song("#TITLE:T\nE\n");
    assert_eq!(song_end_beat(&doc, &song), 0.0);
}

/// fixture writes a song text into a fresh temporary directory and returns
/// the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}
