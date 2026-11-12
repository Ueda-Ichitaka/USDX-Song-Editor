//! About me: tests for the derived song view (task P1-05, plan section 5.4).
//! They assert that lines split at end-of-phrase markers, voice changes and the
//! end row exactly as the Factory's note_types.py splits them, the implicit
//! voice 1, the duet with two voices, the line text joined from the syllables
//! and the absolute note lookup. The ignored benchmark test asserts the rebuild
//! time of a generated 3000-row song; `scripts/dev.sh check --release` runs it.

use std::time::Instant;

use usdx_format::bytes::decode;
use usdx_format::headers::Version;
use usdx_format::rows::{NoteType, RowId, classify};
use usdx_format::song::{AbsNote, Song};

/// song builds the song view of a text.
fn song(text: &str) -> Song {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    Song::from_document(&doc)
}

/// doc classifies a text.
fn doc(text: &str) -> usdx_format::rows::Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

// ---- Line splitting ----

#[test]
fn lines_split_at_end_of_phrase_markers() {
    let s = song(": 0 1 1 a\n: 1 1 1 b\n- 2\n: 3 1 1 c\nE\n");
    let voice = &s.voices[0];
    assert_eq!(voice.lines.len(), 2);
    // The first line holds notes a and b and ends at the phrase-end row 2.
    assert_eq!(voice.lines[0].notes, vec![RowId(0), RowId(1)]);
    assert_eq!(voice.lines[0].end, Some(RowId(2)));
    // The second line holds note c and has no end marker.
    assert_eq!(voice.lines[1].notes, vec![RowId(3)]);
    assert_eq!(voice.lines[1].end, None);
}

#[test]
fn lines_split_at_voice_changes() {
    let s = song("P1\n: 0 1 1 a\nP2\n: 1 1 1 b\nE\n");
    assert_eq!(s.voices[0].lines[0].notes, vec![RowId(1)]);
    assert_eq!(s.voices[0].lines[0].end, None);
    assert_eq!(s.voices[1].lines[0].notes, vec![RowId(3)]);
}

#[test]
fn lines_split_at_the_end_row() {
    let s = song(": 0 1 1 a\nE\n");
    assert_eq!(s.voices[0].lines.len(), 1);
    assert_eq!(s.voices[0].lines[0].end, None);
    assert!(s.voices[0].lines[0].notes.contains(&RowId(0)));
}

#[test]
fn an_end_of_phrase_marker_without_an_open_line_creates_no_line() {
    // The format says an end-of-phrase marker MUST NOT immediately follow
    // another one, and readers may ignore subsequent markers; the view follows
    // the Factory and only closes an open line.
    let s = song("- 5\n: 0 1 1 a\n- 6\n- 7\n: 8 1 1 b\nE\n");
    let voice = &s.voices[0];
    assert_eq!(voice.lines.len(), 2);
    assert_eq!(voice.lines[0].end, Some(RowId(2)));
    assert_eq!(voice.lines[1].notes, vec![RowId(4)]);
}

#[test]
fn empty_and_invalid_rows_are_not_part_of_lines() {
    let s = song(": 0 1 1 a\n\njunk\n: 1 1 1 b\nE\n");
    let voice = &s.voices[0];
    assert_eq!(voice.lines[0].notes, vec![RowId(0), RowId(3)]);
}

// ---- Voices ----

#[test]
fn a_song_without_voice_changes_has_implicit_voice_1() {
    let s = song(": 0 1 1 a\n- 1\n: 2 1 1 b\nE\n");
    assert_eq!(s.voices.len(), 1);
    assert_eq!(s.voices[0].number, 1);
    assert_eq!(s.voices[0].lines.len(), 2);
}

#[test]
fn a_duet_has_two_voices() {
    let s = song("P1\n: 0 1 1 a\n- 1\nP2\n: 0 1 1 b\n- 1\nE\n");
    assert_eq!(s.voices.len(), 2);
    assert_eq!(s.voices[0].number, 1);
    assert_eq!(s.voices[1].number, 2);
    assert_eq!(s.voices[0].lines.len(), 1);
    assert_eq!(s.voices[1].lines.len(), 1);
}

#[test]
fn notes_before_the_first_voice_change_belong_to_voice_1() {
    let s = song(": 0 1 1 a\nP2\n: 1 1 1 b\nE\n");
    assert_eq!(s.voices.len(), 2);
    assert_eq!(s.voices[0].number, 1);
    assert_eq!(s.voices[0].lines[0].notes, vec![RowId(0)]);
    assert_eq!(s.voices[1].number, 2);
}

#[test]
fn a_voice_change_to_the_current_voice_only_breaks_the_line() {
    // The implicit voice 1 is already current, so a P1 row is just a line break
    // and does not add a second voice 1 entry.
    let s = song(": 0 1 1 a\nP1\n: 1 1 1 b\nE\n");
    assert_eq!(s.voices.len(), 1);
    assert_eq!(s.voices[0].lines.len(), 2);
}

#[test]
fn line_indices_count_from_zero_within_the_voice() {
    let s = song("P1\n: 0 1 1 a\n- 1\n: 2 1 1 b\n- 3\nP2\n: 0 1 1 c\n- 1\nE\n");
    assert_eq!(s.voices[0].lines[0].index, 0);
    assert_eq!(s.voices[0].lines[1].index, 1);
    // The second voice counts its own lines from 0.
    assert_eq!(s.voices[1].lines[0].index, 0);
}

// ---- Line text ----

#[test]
fn the_line_text_is_joined_from_the_syllables() {
    // The word-break space lives inside the syllable: " wor" starts a new word.
    let d = doc(": 0 1 1 Hel\n: 1 1 1 lo\n: 2 1 1  wor\n- 3\n: 3 1 1 ld\nE\n");
    let s = Song::from_document(&d);
    let voice = &s.voices[0];
    // A syllable without a leading space continues the word, a syllable with a
    // leading space starts a new one.
    assert_eq!(voice.lines[0].text(&d), "Hello wor");
    assert_eq!(voice.lines[1].text(&d), "ld");
}

#[test]
fn a_tab_in_a_syllable_reads_as_a_space_in_the_line_text() {
    // The note rows keep their text verbatim; only the derived line text reads
    // a tab as a space.
    let d = doc(": 0 1 1 a\tb\nE\n");
    let s = Song::from_document(&d);
    assert_eq!(s.voices[0].lines[0].text(&d), "a b");
    assert_eq!(
        d.rows[0].raw.text, ": 0 1 1 a\tb",
        "the raw row keeps the tab"
    );
}

#[test]
fn the_line_text_is_trimmed() {
    // The syllables " hi " and "there " join to " hi there ", and the leading
    // and trailing spaces of the joined text are trimmed.
    let d = doc(": 0 1 1   hi \n: 1 1 1 there \n- 2\nE\n");
    let s = Song::from_document(&d);
    assert_eq!(s.voices[0].lines[0].text(&d), "hi there");
}

// ---- Metadata ----

#[test]
fn the_song_metadata_comes_from_the_headers() {
    let s = song("#VERSION:1.1.0\n#TITLE:T\n#BPM:120\n#RELATIVE:yes\n: 0 1 1 a\nE\n");
    assert_eq!(s.version, Version::V(1, 1, 0));
    assert!(s.relative);
    assert!(s.timing.is_some());
    assert_eq!(s.headers.get("TITLE"), Some("T"));
}

#[test]
fn a_song_without_a_version_header_is_unversioned() {
    let s = song("#TITLE:x\nE\n");
    assert_eq!(s.version, Version::Unversioned);
}

#[test]
fn a_song_without_a_relative_header_is_not_relative() {
    let s = song("#RELATIVE:no\n: 0 1 1 a\nE\n");
    assert!(!s.relative);
}

// ---- Absolute notes ----

#[test]
fn note_returns_the_absolute_note_data() {
    let d = doc(": 10 20 30 hello\nE\n");
    let s = Song::from_document(&d);
    let note = s.note(&d, RowId(0)).expect("the row is a note");
    assert_eq!(
        note,
        AbsNote {
            row: RowId(0),
            kind: NoteType::Normal,
            start: 10.0,
            length: 20,
            pitch: 30,
            text: "hello".to_owned()
        }
    );
}

#[test]
fn note_returns_none_for_rows_that_are_no_notes() {
    let d = doc(": 0 1 1 a\n- 1\nE\n");
    let s = Song::from_document(&d);
    assert_eq!(s.note(&d, RowId(1)), None);
}

#[test]
fn note_returns_none_for_an_unknown_row_id() {
    let d = doc(": 0 1 1 a\nE\n");
    let s = Song::from_document(&d);
    assert_eq!(s.note(&d, RowId(999)), None);
}

// ---- Rebuild benchmark ----

/// a_3000_row_song generates a song text with 3000 note rows.
fn a_3000_row_song() -> String {
    let mut text = String::from("#TITLE:T\n#BPM:300\n");
    for beat in 0..3000u32 {
        text.push_str(&format!(": {beat} 1 12 syl{}\n", beat % 10));
        if beat % 8 == 7 {
            text.push_str(&format!("- {}\n", beat + 1));
        }
    }
    text.push_str("E\n");
    text
}

#[test]
#[ignore = "run by scripts/dev.sh check --release"]
fn rebuilding_a_3000_row_song_stays_below_1_ms() {
    let text = a_3000_row_song();
    let d = doc(&text);
    let start = Instant::now();
    let s = Song::from_document(&d);
    let elapsed = start.elapsed();
    assert_eq!(s.voices.len(), 1);
    assert!(
        elapsed.as_secs_f64() < 0.001,
        "the rebuild took {elapsed:?}, the plan wants it below 1 ms"
    );
}
