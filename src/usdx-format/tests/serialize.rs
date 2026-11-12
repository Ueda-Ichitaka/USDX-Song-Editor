//! About me: tests for the serializer (task P1-07, plan section 5.6). They
//! assert the lossless round trip (every fixture in `tests/fixtures/` and every
//! proptest-generated document saves back as the input bytes), the canonical
//! formatting of dirty rows (insta snapshots), the raw header key spelling of a
//! changed value, the uppercase key of a new header, the dominant terminator
//! for new rows and the unchanged BOM, encoding and trailing lines.

use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use usdx_format::bytes::{RawLine, Terminator, decode};
use usdx_format::rows::{Document, NoteRow, NoteType, Row, RowId, RowKind, classify};
use usdx_format::serialize::serialize;

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// ser serializes a document to text.
fn ser(doc: &Document) -> String {
    String::from_utf8(serialize(doc).expect("the document serializes")).expect("utf-8")
}

/// set_note replaces a row's kind with a note and marks it dirty.
fn set_note(doc: &mut Document, index: usize, note: NoteRow) {
    doc.rows[index].kind = RowKind::Note(note);
    doc.rows[index].dirty = true;
}

/// new_row builds a row that an edit has inserted: no raw line yet, dirty.
fn new_row(kind: RowKind) -> Row {
    Row {
        id: RowId(u32::MAX),
        raw: RawLine {
            text: String::new(),
            terminator: Terminator::None,
        },
        kind,
        dirty: true,
    }
}

// ---- Round trip: every fixture saves back as its input bytes ----

#[test]
fn every_fixture_round_trips() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut count = 0;
    for entry in fs::read_dir(&fixtures).expect("the fixtures directory exists") {
        let path = entry.expect("the fixture entry reads").path();
        let bytes = fs::read(&path).expect("the fixture reads");
        let doc = classify(decode(&bytes).expect("the fixture decodes"));
        let out = serialize(&doc).expect("the document serializes");
        assert_eq!(
            out,
            bytes,
            "the fixture {} does not round trip",
            path.display()
        );
        count += 1;
    }
    assert!(count > 0, "the fixtures directory is empty");
}

// ---- Dirty rows are formatted canonically ----

#[test]
fn a_dirty_note_is_formatted_canonically() {
    let mut d = doc(": 5 3 12 hi\nE\n");
    set_note(
        &mut d,
        0,
        NoteRow {
            kind: NoteType::Normal,
            start: 10,
            length: 3,
            pitch: 12,
            text: "hi".to_owned(),
        },
    );
    insta::assert_snapshot!(ser(&d), @": 10 3 12 hi\nE\n");
}

#[test]
fn a_leading_space_in_a_dirty_note_text_doubles_the_space() {
    let mut d = doc(": 0 1 1 wor\nE\n");
    set_note(
        &mut d,
        0,
        NoteRow {
            kind: NoteType::Normal,
            start: 0,
            length: 1,
            pitch: 1,
            text: " wor".to_owned(),
        },
    );
    insta::assert_snapshot!(ser(&d), @": 0 1 1  wor\nE\n");
}

#[test]
fn a_dirty_note_keeps_its_own_terminator() {
    let mut d = doc(": 5 3 12 hi\r\nE\r\n");
    set_note(
        &mut d,
        0,
        NoteRow {
            kind: NoteType::Golden,
            start: 5,
            length: 3,
            pitch: 12,
            text: "hi".to_owned(),
        },
    );
    insta::assert_snapshot!(ser(&d), @"* 5 3 12 hi\r\nE\r\n");
}

#[test]
fn a_dirty_phrase_end_is_formatted_with_the_beat() {
    let mut d = doc(": 0 1 1 hi\n- 8\nE\n");
    d.rows[1].kind = RowKind::PhraseEnd {
        beat: Some(12),
        rel_offset: None,
    };
    d.rows[1].dirty = true;
    insta::assert_snapshot!(ser(&d), @": 0 1 1 hi\n- 12\nE\n");
}

#[test]
fn a_changed_header_value_keeps_the_raw_key_spelling() {
    let mut d = doc("#Title:  Old \nE\n");
    d.rows[0].kind = RowKind::Header {
        key: "TITLE".to_owned(),
        value: "New".to_owned(),
    };
    d.rows[0].dirty = true;
    insta::assert_snapshot!(ser(&d), @"#Title:New\nE\n");
}

#[test]
fn a_new_header_is_written_with_the_uppercase_key() {
    let mut d = doc("#TITLE:A\n#ARTIST:B\nE\n");
    let row = new_row(RowKind::Header {
        key: "MP3".to_owned(),
        value: "song.mp3".to_owned(),
    });
    // The patch inserts the new header after the last header row (index 2).
    d.rows.insert(2, row);
    insta::assert_snapshot!(ser(&d), @"#TITLE:A\n#ARTIST:B\n#MP3:song.mp3\nE\n");
}

// ---- New rows take the dominant terminator ----

#[test]
fn a_new_row_takes_the_dominant_terminator() {
    let mut d = doc(": 0 1 1 a\r\n: 1 1 1 b\r\n: 2 1 1 c\nE\n");
    let row = new_row(RowKind::Note(NoteRow {
        kind: NoteType::Normal,
        start: 3,
        length: 1,
        pitch: 1,
        text: "d".to_owned(),
    }));
    d.rows.insert(3, row);
    insta::assert_snapshot!(ser(&d), @": 0 1 1 a\r\n: 1 1 1 b\r\n: 2 1 1 c\r\n: 3 1 1 d\nE\n");
}

#[test]
fn a_tie_of_terminators_resolves_to_lf() {
    // One LF row and one CR row; the new row gets LF because LF wins ties.
    let mut d = doc(": 0 1 1 a\n: 1 1 1 b\r");
    let row = new_row(RowKind::Note(NoteRow {
        kind: NoteType::Normal,
        start: 2,
        length: 1,
        pitch: 1,
        text: "c".to_owned(),
    }));
    d.rows.push(row);
    insta::assert_snapshot!(ser(&d), @": 0 1 1 a\n: 1 1 1 b\r: 2 1 1 c\n");
}

// ---- File-level data is written back unchanged ----

#[test]
fn the_trailing_lines_are_written_back_unchanged() {
    let mut d = doc("#TITLE:x\nE\nstill here\n");
    d.rows[0].dirty = true;
    d.rows[0].kind = RowKind::Header {
        key: "TITLE".to_owned(),
        value: "changed".to_owned(),
    };
    let bytes = serialize(&d).expect("the document serializes");
    insta::assert_snapshot!(String::from_utf8(bytes).expect("utf-8"), @"#TITLE:changed\nE\nstill here\n");
}

#[test]
fn a_document_with_a_bom_serializes_with_the_bom() {
    let with_bom = String::from("\u{feff}#TITLE:x\nE\n");
    let d = doc(&with_bom);
    assert!(d.file.bom);
    let bytes = serialize(&d).expect("the document serializes");
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
}

#[test]
fn an_unmodified_document_serializes_as_its_input() {
    // A dirty flag alone does not change the text: the raw line is written back.
    let d = doc("#TITLE:x  \n: 0 1 1 a\nE\n");
    insta::assert_snapshot!(ser(&d), @"#TITLE:x  \n: 0 1 1 a\nE\n");
}

// ---- Property test: generated documents round trip ----

/// text_strategy generates line texts without CR and LF.
fn text_strategy() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[^\r\n]*").expect("the text regex compiles")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn generated_documents_round_trip(
        bom in any::<bool>(),
        lines in proptest::collection::vec((text_strategy(), 0..4usize), 0..12),
    ) {
        let mut bytes = Vec::new();
        if bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        for (text, terminator) in &lines {
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(match terminator {
                0 => b"\n",
                1 => b"\r\n",
                2 => b"\r",
                _ => b"",
            });
        }
        let decoded = decode(&bytes)
            .unwrap_or_else(|error| panic!("the generated bytes do not decode: {error}"));
        let doc = classify(decoded);
        let out = serialize(&doc)
            .unwrap_or_else(|error| panic!("the document does not serialize: {error}"));
        prop_assert_eq!(out, bytes);
    }
}
