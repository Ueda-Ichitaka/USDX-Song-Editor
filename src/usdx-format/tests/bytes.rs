//! About me: tests for the byte layer (task P1-01, plan section 5.2). They assert
//! encoding detection in the plan's order (BOM, valid UTF-8, #ENCODING header,
//! chardetng guess), the line terminator kinds, the round-trip property
//! `encode(decode(b)) == b`, the empty file, a file without final newline and the
//! exact windows-1252 round trip of the bytes 0x80 to 0x9F.

use usdx_format::bytes::{EncodingSource, Terminator, TextEncoding, decode, encode};

/// roundtrip asserts the exact inverse property of decode and encode.
fn roundtrip(bytes: &[u8]) {
    let decoded = decode(bytes).expect("the bytes decode");
    let encoded = encode(&decoded).expect("the decoded file encodes");
    assert_eq!(encoded, bytes, "encode(decode(b)) must equal b");
}

#[test]
fn utf8_without_bom_is_detected() {
    let decoded = decode(b"#TITLE:Test\nE\n").expect("plain UTF-8 decodes");
    assert!(!decoded.bom);
    assert_eq!(decoded.encoding, TextEncoding::Utf8);
    assert_eq!(decoded.encoding_source, EncodingSource::Valid);
    assert_eq!(decoded.lines.len(), 2);
    assert_eq!(decoded.lines[0].text, "#TITLE:Test");
    assert_eq!(decoded.lines[0].terminator, Terminator::Lf);
    assert_eq!(decoded.lines[1].text, "E");
    assert_eq!(decoded.lines[1].terminator, Terminator::Lf);
}

#[test]
fn utf8_multibyte_text_decodes() {
    let decoded = decode("#TITLE:Caf\u{E9}\nE\n".as_bytes()).expect("UTF-8 decodes");
    assert_eq!(decoded.lines[0].text, "#TITLE:Caf\u{E9}");
}

#[test]
fn bom_is_detected_and_roundtrips() {
    let bytes: &[u8] = b"\xEF\xBB\xBF#TITLE:Test\r\nE\n";
    let decoded = decode(bytes).expect("a BOM file decodes");
    assert!(decoded.bom);
    assert_eq!(decoded.encoding, TextEncoding::Utf8);
    assert_eq!(decoded.encoding_source, EncodingSource::Valid);
    assert_eq!(decoded.lines[0].text, "#TITLE:Test");
    assert_eq!(decoded.lines[0].terminator, Terminator::CrLf);
    roundtrip(bytes);
}

#[test]
fn encoding_header_selects_windows_1252() {
    // 0xE9 is 'é' in windows-1252; the bytes are not valid UTF-8.
    let bytes: &[u8] = b"#ENCODING:CP1252\r\n#TITLE:caf\xE9\nE\n";
    let decoded = decode(bytes).expect("the header names the encoding");
    assert!(!decoded.bom);
    assert_eq!(decoded.encoding, TextEncoding::Windows1252);
    assert_eq!(decoded.encoding_source, EncodingSource::Header);
    assert_eq!(decoded.lines[1].text, "#TITLE:caf\u{E9}");
    roundtrip(bytes);
}

#[test]
fn encoding_header_is_case_insensitive_and_knows_the_iana_name() {
    let bytes: &[u8] = b"#encoding:windows-1252\n#TITLE:caf\xE9\nE\n";
    let decoded = decode(bytes).expect("the header names the encoding");
    assert_eq!(decoded.encoding, TextEncoding::Windows1252);
    assert_eq!(decoded.encoding_source, EncodingSource::Header);
    roundtrip(bytes);
}

#[test]
fn encoding_header_selects_windows_1250() {
    // 0xE6 is 'ć' in windows-1250 and 'æ' in windows-1252.
    let bytes: &[u8] = b"#ENCODING:CP1250\n#TITLE:bu\xE6\nE\n";
    let decoded = decode(bytes).expect("the header names the encoding");
    assert_eq!(decoded.encoding, TextEncoding::Windows1250);
    assert_eq!(decoded.encoding_source, EncodingSource::Header);
    assert_eq!(decoded.lines[1].text, "#TITLE:bu\u{107}");
    roundtrip(bytes);
}

#[test]
fn unknown_encoding_header_is_an_error() {
    // The 0xE9 byte keeps the file out of the valid-UTF-8 branch, so the header
    // is the only source left and the unknown name must fail loudly.
    let result = decode(b"#ENCODING:KOI8-R\n#TITLE:caf\xE9\n");
    assert!(result.is_err(), "an unnamed encoding must fail loudly");
}

#[test]
fn cp1252_is_guessed_without_a_header() {
    // 0xE9 is 'é' and 0x80 is the euro sign in windows-1252.
    let bytes: &[u8] = b"#TITLE:Caf\xE9 5 \x80\nE\n";
    let decoded = decode(bytes).expect("the bytes decode");
    assert_eq!(decoded.encoding, TextEncoding::Windows1252);
    assert_eq!(decoded.encoding_source, EncodingSource::Guessed);
    roundtrip(bytes);
}

#[test]
fn windows_1250_is_guessed_from_polish_text() {
    // A windows-1250 encoded Polish song (0xB3 is 'l', 0xB9 is 'a', 0xEA is 'e',
    // 0xF1 is 'n', 0xEC is 'e', 0xF3 is 'o', 0xBF is 'z' with diacritics); the
    // expected texts come from the windows-1250 table of encoding_rs.
    let bytes: &[u8] = b"#TITLE:Zadaj\xea si\xea, \xbfe nadal chcesz ta\xf1czy\xec?\nZabawa w \xb3adne s\xb3owa trwa ca\xb3y wiecz\xf3r i d\xb3u\xbfej\nE\n";
    let decoded = decode(bytes).expect("the bytes decode");
    assert_eq!(decoded.encoding, TextEncoding::Windows1250);
    assert_eq!(decoded.encoding_source, EncodingSource::Guessed);
    assert_eq!(
        decoded.lines[0].text,
        "#TITLE:Zadaj\u{119} si\u{119}, \u{17C}e nadal chcesz ta\u{144}czy\u{11B}?"
    );
    assert_eq!(
        decoded.lines[1].text,
        "Zabawa w \u{142}adne s\u{142}owa trwa ca\u{142}y wiecz\u{F3}r i d\u{142}u\u{17C}ej"
    );
    roundtrip(bytes);
}

#[test]
fn all_terminator_kinds_roundtrip() {
    let bytes: &[u8] = b"A\nB\r\nC\rD";
    let decoded = decode(bytes).expect("the bytes decode");
    assert_eq!(decoded.lines.len(), 4);
    assert_eq!(decoded.lines[0].terminator, Terminator::Lf);
    assert_eq!(decoded.lines[1].terminator, Terminator::CrLf);
    assert_eq!(decoded.lines[2].terminator, Terminator::Cr);
    assert_eq!(decoded.lines[3].terminator, Terminator::None);
    assert_eq!(decoded.lines[3].text, "D");
    roundtrip(bytes);
}

#[test]
fn a_file_that_ends_with_a_newline_has_no_extra_line() {
    let decoded = decode(b"A\n").expect("the bytes decode");
    assert_eq!(decoded.lines.len(), 1);
    assert_eq!(decoded.lines[0].terminator, Terminator::Lf);
    roundtrip(b"A\n");
}

#[test]
fn a_file_without_final_newline_ends_with_none() {
    let decoded = decode(b"A\nB").expect("the bytes decode");
    assert_eq!(decoded.lines.len(), 2);
    assert_eq!(decoded.lines[1].terminator, Terminator::None);
    roundtrip(b"A\nB");
}

#[test]
fn empty_lines_are_kept() {
    let decoded = decode(b"A\n\nB\n").expect("the bytes decode");
    assert_eq!(decoded.lines.len(), 3);
    assert_eq!(decoded.lines[1].text, "");
    roundtrip(b"A\n\nB\n");
}

#[test]
fn the_empty_file_roundtrips() {
    let decoded = decode(b"").expect("the empty file decodes");
    assert!(decoded.lines.is_empty());
    roundtrip(b"");
}

#[test]
fn every_windows_1252_byte_from_0x80_to_0x9f_roundtrips() {
    // A bare high byte can be ambiguous for the detector (some guess
    // windows-1250); the exact round trip is the requirement, so the test does
    // not pin the guessed encoding here.
    for byte in 0x80u8..=0x9F {
        let bytes = [byte];
        let decoded = decode(&bytes).expect("a single windows-1252 byte decodes");
        assert_eq!(decoded.lines[0].terminator, Terminator::None);
        assert_eq!(decoded.lines[0].text.chars().count(), 1);
        roundtrip(&bytes);
    }
}

#[test]
fn the_unassigned_windows_1252_positions_map_to_the_c1_controls() {
    // WHATWG windows-1252 maps the unassigned positions to the corresponding C1
    // control characters, so the round trip stays exact. The file declares the
    // encoding in the header, which keeps the table under test deterministic.
    let expected = [
        (0x80u8, '\u{20AC}'),
        (0x81u8, '\u{81}'),
        (0x8Du8, '\u{8D}'),
        (0x8Fu8, '\u{8F}'),
        (0x90u8, '\u{90}'),
        (0x9Du8, '\u{9D}'),
        (0x9Fu8, '\u{178}'),
    ];
    for (byte, ch) in expected {
        let mut bytes = b"#ENCODING:CP1252\n".to_vec();
        bytes.push(byte);
        let decoded = decode(&bytes).expect("the byte decodes");
        assert_eq!(decoded.encoding, TextEncoding::Windows1252);
        assert_eq!(decoded.encoding_source, EncodingSource::Header);
        assert_eq!(decoded.lines[1].text, ch.to_string(), "byte 0x{byte:02X}");
        roundtrip(&bytes);
    }
}
