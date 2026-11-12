//! About me: tests for the row classification layer (task P1-02, plan section 5.3).
//! They assert one test per classification rule (empty, header, end, voice,
//! phrase end, note, invalid) and one test per line shape of the unversioned
//! format ABNF, plus the document-level properties: sequential row ids, clean
//! dirty flags, verbatim trailing lines and the file metadata.

use usdx_format::bytes::{TextEncoding, decode};
use usdx_format::rows::{Document, NoteType, RowId, RowKind, classify};

/// doc classifies the rows of a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// kind returns the classification of the row at the index.
fn kind(d: &Document, index: usize) -> &RowKind {
    &d.rows[index].kind
}

// ---- Rule 1: whitespace only is Empty ----

#[test]
fn an_empty_line_is_empty() {
    let d = doc("\n: 0 1 1 a\n");
    assert_eq!(kind(&d, 0), &RowKind::Empty);
    assert!(matches!(kind(&d, 1), RowKind::Note(_)));
}

#[test]
fn a_whitespace_only_line_is_empty() {
    let d = doc("  \t \n: 0 1 1 a\n");
    assert_eq!(kind(&d, 0), &RowKind::Empty);
}

#[test]
fn a_line_of_unicode_whitespace_is_empty() {
    // The plan defines whitespace as Unicode White_Space without CR and LF, so a
    // no-break-space-only line is empty (the ABNF would allow only space and tab;
    // the plan is the wider rule).
    let d = doc("\u{A0}\n: 0 1 1 a\n");
    assert_eq!(kind(&d, 0), &RowKind::Empty);
}

// ---- Rule 2: headers before the first body row ----

#[test]
fn a_header_line_is_parsed() {
    let d = doc("#TITLE:My Song\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "TITLE".to_owned(),
            value: "My Song".to_owned()
        }
    );
    assert_eq!(kind(&d, 1), &RowKind::End);
}

#[test]
fn a_header_key_is_upper_cased_and_the_raw_line_is_kept() {
    let d = doc("#title:My Song\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "TITLE".to_owned(),
            value: "My Song".to_owned()
        }
    );
    assert_eq!(d.rows[0].raw.text, "#title:My Song");
}

#[test]
fn header_whitespace_around_key_and_value_is_trimmed() {
    let d = doc("#TITLE :  My Song \nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "TITLE".to_owned(),
            value: "My Song".to_owned()
        }
    );
}

#[test]
fn a_header_may_have_whitespace_after_the_hash() {
    // The ABNF is hash *WSP header-key, so the hash starts the line and the key
    // may follow after whitespace.
    let d = doc("# VIDEO: a.mp4\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "VIDEO".to_owned(),
            value: "a.mp4".to_owned()
        }
    );
}

#[test]
fn a_header_value_may_contain_colons() {
    // The split happens at the first colon.
    let d = doc("#VIDEO:https://example.com/v.mp4\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "VIDEO".to_owned(),
            value: "https://example.com/v.mp4".to_owned()
        }
    );
}

#[test]
fn an_empty_header_value_is_kept_as_empty() {
    // The header view (P1-03) treats an empty value as absent; the row keeps it.
    let d = doc("#TITLE:\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "TITLE".to_owned(),
            value: "".to_owned()
        }
    );
}

#[test]
fn a_header_line_without_colon_is_invalid() {
    let d = doc("#TITLE\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_header_line_without_key_is_invalid() {
    // The ABNF requires header-key = 1*header-char, so an empty key is invalid.
    let d = doc("#:x\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_header_line_after_the_body_is_invalid() {
    let d = doc(": 0 1 1 a\n#COMMENT:x\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Note(_)));
    assert!(matches!(kind(&d, 1), RowKind::Invalid { .. }));
    assert_eq!(kind(&d, 2), &RowKind::End);
}

#[test]
fn a_header_line_after_an_invalid_row_is_still_a_header() {
    // The spec defines the body as notes, end-of-phrase markers and voice
    // changes, so an invalid row does not start the body and a following
    // header line stays a header.
    let d = doc("junk\n#TITLE:x\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
    assert!(matches!(kind(&d, 1), RowKind::Header { .. }));
}

#[test]
fn a_header_line_must_start_with_the_hash() {
    // The ABNF starts the header line with the hash; a leading space is not a
    // header (and matches no other row shape).
    let d = doc(" #TITLE:x\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

// ---- Rule 3: the end row and trailing lines ----

#[test]
fn the_end_row_ends_the_body_and_the_rest_is_trailing() {
    // Everything after the E row is ignored by the format and kept verbatim as
    // trailing raw lines with their own terminators.
    let d = doc("#TITLE:x\nE\nsome junk\n: 0 1 1 a\n\n");
    assert_eq!(d.rows.len(), 2);
    assert_eq!(
        kind(&d, 0),
        &RowKind::Header {
            key: "TITLE".to_owned(),
            value: "x".to_owned()
        }
    );
    assert_eq!(kind(&d, 1), &RowKind::End);
    let texts: Vec<&str> = d.trailing.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(texts, vec!["some junk", ": 0 1 1 a", ""]);
    assert!(
        d.trailing
            .iter()
            .all(|line| line.terminator == usdx_format::bytes::Terminator::Lf)
    );
}

#[test]
fn the_end_row_is_recognized_after_trimming() {
    let d = doc("  E  \n");
    assert_eq!(d.rows.len(), 1);
    assert_eq!(kind(&d, 0), &RowKind::End);
    assert!(d.trailing.is_empty());
}

#[test]
fn an_e_with_content_is_not_an_end_row() {
    // Rule 3 needs E alone after trimming; "E x" matches no row shape.
    let d = doc("E x\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_file_without_an_end_row_has_no_trailing_lines() {
    let d = doc(": 0 1 1 a\n");
    assert!(d.trailing.is_empty());
    assert_eq!(d.rows.len(), 1);
}

#[test]
fn a_lowercase_e_is_not_an_end_row() {
    let d = doc("e\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

// ---- Rule 4: voice changes ----

#[test]
fn voice_changes_are_classified() {
    let d = doc("P1\n: 0 1 1 a\nP2\n: 0 1 1 b\nE\n");
    assert_eq!(kind(&d, 0), &RowKind::Voice(1));
    assert!(matches!(kind(&d, 1), RowKind::Note(_)));
    assert_eq!(kind(&d, 2), &RowKind::Voice(2));
    assert!(matches!(kind(&d, 3), RowKind::Note(_)));
    assert_eq!(kind(&d, 4), &RowKind::End);
}

#[test]
fn a_spaced_voice_change_is_accepted() {
    // "P 1" is accepted; the warning about the spelling is derived from the raw
    // row by the validation layer (P1-08).
    let d = doc("P 1\nE\n");
    assert_eq!(kind(&d, 0), &RowKind::Voice(1));
}

#[test]
fn trailing_whitespace_after_a_voice_change_is_allowed() {
    let d = doc("P1  \nE\n");
    assert_eq!(kind(&d, 0), &RowKind::Voice(1));
}

#[test]
fn a_voice_change_with_any_digit_is_a_voice_row() {
    // The plan classifies P followed by a digit; the unversioned format allows
    // only P1 and P2, so the P3 warning comes from the validation layer.
    let d = doc("P3\nE\n");
    assert_eq!(kind(&d, 0), &RowKind::Voice(3));
}

#[test]
fn a_voice_change_has_exactly_one_digit() {
    let d = doc("P12\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_spaced_voice_change_has_exactly_one_digit() {
    let d = doc("P 12\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

// ---- Rule 5: end-of-phrase markers ----

#[test]
fn an_end_of_phrase_marker_is_parsed() {
    let d = doc("- 10\n: 0 1 1 a\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::PhraseEnd {
            beat: Some(10),
            rel_offset: None
        }
    );
}

#[test]
fn trailing_whitespace_after_an_end_of_phrase_is_allowed() {
    let d = doc("- 10 \t\n: 0 1 1 a\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::PhraseEnd {
            beat: Some(10),
            rel_offset: None
        }
    );
}

#[test]
fn the_relative_second_number_is_kept_in_the_row() {
    // The second number is the relative-mode end-of-phrase shape; whether the
    // document may carry it is decided by the relative-mode reading (P1-06,
    // gate G-01) and the validation layer, not by the classification.
    let d = doc("- 10 5\n: 0 1 1 a\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::PhraseEnd {
            beat: Some(10),
            rel_offset: Some(5)
        }
    );
}

#[test]
fn junk_after_the_second_number_is_invalid() {
    let d = doc("- 10 5 x\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn an_end_of_phrase_marker_without_a_number_is_invalid() {
    let d = doc("-\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn an_end_of_phrase_marker_without_whitespace_is_invalid() {
    // The ABNF is dash WSP start-beat; "-10" matches no row shape.
    let d = doc("-10\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

// ---- Rule 6: notes ----

#[test]
fn a_regular_note_is_parsed() {
    let d = doc(": 10 20 30 hello\nE\n");
    assert_eq!(
        kind(&d, 0),
        &RowKind::Note(usdx_format::rows::NoteRow {
            kind: NoteType::Normal,
            start: 10,
            length: 20,
            pitch: 30,
            text: "hello".to_owned()
        })
    );
}

#[test]
fn the_standard_note_types_map_to_the_note_kinds() {
    let d = doc("* 1 1 1 a\nR 1 1 1 a\nG 1 1 1 a\nF 1 1 1 a\nE\n");
    assert_eq!(note_type(kind(&d, 0)), &NoteType::Golden);
    assert_eq!(note_type(kind(&d, 1)), &NoteType::Rap);
    assert_eq!(note_type(kind(&d, 2)), &NoteType::RapGolden);
    assert_eq!(note_type(kind(&d, 3)), &NoteType::Freestyle);
}

/// note_type returns the note kind of a note row.
fn note_type(k: &RowKind) -> &NoteType {
    match k {
        RowKind::Note(row) => &row.kind,
        other => panic!("expected a note row, got {other:?}"),
    }
}

#[test]
fn an_unknown_note_type_is_kept_with_its_character() {
    // Unknown types are kept byte-exact through the raw row and shown as
    // freestyle with a warning (the warning is derived by validation).
    let d = doc("X 1 1 1 a\nr 1 1 1 a\nE\n");
    assert_eq!(note_type(kind(&d, 0)), &NoteType::Other('X'));
    assert_eq!(note_type(kind(&d, 1)), &NoteType::Other('r'));
    assert_eq!(d.rows[0].raw.text, "X 1 1 1 a");
}

#[test]
fn tabs_separate_the_note_fields() {
    let d = doc(":\t10\t20\t30\tla\nE\n");
    let row = note_row(kind(&d, 0));
    assert_eq!(row.start, 10);
    assert_eq!(row.length, 20);
    assert_eq!(row.pitch, 30);
    assert_eq!(row.text, "la");
}

#[test]
fn multiple_whitespace_separates_fields_but_exactly_one_precedes_the_text() {
    // The plan allows one or more whitespace characters between the fields; the
    // text starts after exactly one whitespace character, so it keeps a leading
    // space of its own.
    let d = doc(":  10  20  30  la\nE\n");
    assert_eq!(note_row(kind(&d, 0)).text, " la");
}

#[test]
fn leading_spaces_in_the_note_text_are_preserved() {
    let d = doc(": 10 20 30   hi\nE\n");
    assert_eq!(note_row(kind(&d, 0)).text, "  hi");
}

#[test]
fn a_note_without_text_is_kept_with_empty_text() {
    // The validation layer warns about a note with empty text (P1-08), so the
    // classification keeps such a row as a note.
    let d = doc(": 10 20 30\n: 10 20 30 \nE\n");
    assert_eq!(note_row(kind(&d, 0)).text, "");
    assert_eq!(note_row(kind(&d, 1)).text, "");
}

/// note_row returns the note row of a note kind.
fn note_row(k: &RowKind) -> &usdx_format::rows::NoteRow {
    match k {
        RowKind::Note(row) => row,
        other => panic!("expected a note row, got {other:?}"),
    }
}

#[test]
fn negative_numbers_are_held_in_the_note_row() {
    // Negative starts and lengths are held so the validation layer can report
    // them; the editor never writes them. A negative pitch is valid per ABNF.
    let d = doc(": -5 10 20 la\n: 10 -5 20 la\n: 10 20 -12 la\nE\n");
    assert_eq!(note_row(kind(&d, 0)).start, -5);
    assert_eq!(note_row(kind(&d, 1)).length, -5);
    assert_eq!(note_row(kind(&d, 2)).pitch, -12);
}

#[test]
fn a_note_field_must_be_an_integer() {
    let d = doc(": 10 abc 20 la\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_note_needs_three_fields() {
    let d = doc(": 10 20 la\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_number_that_overflows_i64_is_invalid() {
    let d = doc(": 99999999999999999999 1 1 la\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_plus_sign_is_not_part_of_a_number() {
    // The ABNF allows an optional minus, not a plus.
    let d = doc(": +5 1 1 la\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
}

#[test]
fn a_hyphen_can_be_a_note_type() {
    // Rule 5 matches only one or two numbers after the dash, so a full note
    // shape with a dash type classifies as a note of an unknown type.
    let d = doc("- 10 20 30 la\nE\n");
    assert_eq!(note_type(kind(&d, 0)), &NoteType::Other('-'));
    assert_eq!(note_row(kind(&d, 0)).text, "la");
}

#[test]
fn a_note_type_must_be_visible_ascii_at_the_line_start() {
    // A leading space is not a note type; a non-ASCII first character is not
    // visible ASCII either.
    let d = doc(" : 10 20 30 la\n\u{E9} 10 20 30 la\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
    assert!(matches!(kind(&d, 1), RowKind::Invalid { .. }));
}

#[test]
fn unicode_note_text_is_kept_verbatim() {
    let d = doc(": 10 20 30 h\u{E9}llo\u{2192}\nE\n");
    assert_eq!(note_row(kind(&d, 0)).text, "h\u{E9}llo\u{2192}");
}

// ---- Rule 7 and document-level properties ----

#[test]
fn anything_else_is_invalid_and_kept_verbatim() {
    let d = doc("junk\nE\n");
    assert!(matches!(kind(&d, 0), RowKind::Invalid { .. }));
    assert_eq!(d.rows[0].raw.text, "junk");
}

#[test]
fn rows_have_sequential_ids_and_start_clean() {
    let d = doc("#TITLE:T\n: 0 1 1 a\n- 1\nE\n");
    for (index, row) in d.rows.iter().enumerate() {
        assert_eq!(row.id, RowId(index as u32));
        assert!(!row.dirty);
    }
}

#[test]
fn the_document_keeps_the_file_metadata() {
    let d = doc("#TITLE:x\nE\n");
    assert!(!d.file.bom);
    assert_eq!(d.file.encoding, TextEncoding::Utf8);

    let bytes: &[u8] = b"\xEF\xBB\xBF#TITLE:x\nE\n";
    let d = classify(decode(bytes).expect("the bytes decode"));
    assert!(d.file.bom);
}

#[test]
fn a_whole_song_classifies_in_order() {
    let d = doc(concat!(
        "#TITLE:T\n",
        "#ARTIST:A\n",
        "#BPM:300\n",
        "P1\n",
        ": 0 4 12 la\n",
        "- 4\n",
        ": 4 4 12 mi\n",
        "* 8 2 15 do\n",
        "P2\n",
        ": 0 2 5 si\n",
        "- 2\n",
        "E\n",
        "some trailing stuff\n"
    ));
    assert!(matches!(kind(&d, 0), RowKind::Header { .. }));
    assert!(matches!(kind(&d, 1), RowKind::Header { .. }));
    assert!(matches!(kind(&d, 2), RowKind::Header { .. }));
    assert_eq!(kind(&d, 3), &RowKind::Voice(1));
    assert!(matches!(kind(&d, 4), RowKind::Note(_)));
    assert_eq!(
        kind(&d, 5),
        &RowKind::PhraseEnd {
            beat: Some(4),
            rel_offset: None
        }
    );
    assert!(matches!(kind(&d, 6), RowKind::Note(_)));
    assert_eq!(note_type(kind(&d, 7)), &NoteType::Golden);
    assert_eq!(kind(&d, 8), &RowKind::Voice(2));
    assert!(matches!(kind(&d, 9), RowKind::Note(_)));
    assert_eq!(
        kind(&d, 10),
        &RowKind::PhraseEnd {
            beat: Some(2),
            rel_offset: None
        }
    );
    assert_eq!(kind(&d, 11), &RowKind::End);
    assert_eq!(d.rows.len(), 12);
    assert_eq!(d.trailing.len(), 1);
    assert_eq!(d.trailing[0].text, "some trailing stuff");
}
