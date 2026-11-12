//! The row classification layer of the UltraStar song format.
//!
//! About me: this module turns the raw lines of a decoded file into rows with a
//! classification (`Header`, `Note`, `PhraseEnd`, `Voice`, `End`, `Empty`,
//! `Invalid`) following the plan's rule order, and collects the lines after the
//! `E` row verbatim as trailing lines. It exists so that the header and song
//! views, the serializer and the validation layer work on classified rows while
//! every raw line is kept byte-exact for a lossless save.

use crate::bytes::{DecodedFile, DecodedFileMeta, RawLine};

/// The stable identity of a row inside a loaded document.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowId(pub u32);

/// The musical kind of a note. The standard types map to their kinds; any other
/// note type character is kept in `Other` and shown as freestyle with a warning
/// derived by the validation layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteType {
    Normal,
    Golden,
    Freestyle,
    Rap,
    RapGolden,
    Other(char),
}

/// The parsed data of a note row. The numbers are `i64` so broken values such as
/// negative starts are held and reported by the validation layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRow {
    pub kind: NoteType,
    pub start: i64,
    pub length: i64,
    pub pitch: i64,
    pub text: String,
}

/// The classification of one raw line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A `#key: value` line; the key is upper-cased for the header lookup and
    /// the raw line is kept in `Row::raw`.
    Header {
        key: String,
        value: String,
    },
    Note(NoteRow),
    PhraseEnd {
        beat: Option<i64>,
        rel_offset: Option<i64>,
    },
    Voice(u8),
    End,
    Empty,
    /// A line that matches no row shape; kept verbatim and reported by the
    /// validation layer.
    Invalid {
        reason: String,
    },
}

/// One classified row of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: RowId,
    pub raw: RawLine,
    pub kind: RowKind,
    pub dirty: bool,
}

/// The classified document: the file metadata, the rows and the verbatim lines
/// after the `E` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub file: DecodedFileMeta,
    pub rows: Vec<Row>,
    pub trailing: Vec<RawLine>,
}

/// classify turns a decoded file into a document. Classification follows the
/// plan's rule order per line; it never fails, because a line that matches no
/// shape becomes an `Invalid` row and the lines after the `E` row become
/// trailing lines.
pub fn classify(file: DecodedFile) -> Document {
    let meta = file.meta();
    let mut rows = Vec::new();
    let mut trailing = Vec::new();
    let mut body_started = false;
    let mut ended = false;

    for raw in file.lines {
        if ended {
            trailing.push(raw);
            continue;
        }
        let kind = classify_line(&raw.text, &mut body_started);
        ended = matches!(kind, RowKind::End);
        rows.push(Row {
            id: RowId(rows.len() as u32),
            raw,
            kind,
            dirty: false,
        });
    }

    Document {
        file: meta,
        rows,
        trailing,
    }
}

/// classify_line classifies one line's text. The body starts at the first row
/// that the format counts as body content (a note, an end-of-phrase marker, a
/// voice change or the end row), so a stray invalid line does not push the
/// header section away.
fn classify_line(text: &str, body_started: &mut bool) -> RowKind {
    // Rule 1: a line of whitespace only is empty. Unicode `White_Space` without
    // CR and LF counts; CR and LF cannot appear because the byte layer splits
    // lines on them.
    if text.chars().all(is_row_whitespace) {
        return RowKind::Empty;
    }
    // Rule 2: a line starting with `#` is a header before the first body row.
    if text.starts_with('#') {
        if *body_started {
            return RowKind::Invalid {
                reason: "the header row stands after the body started".to_owned(),
            };
        }
        return header_kind(text);
    }
    // Rule 3: `E` alone after trimming whitespace ends the body.
    if text.trim() == "E" {
        *body_started = true;
        return RowKind::End;
    }
    // Rule 4: `P` followed by one digit is a voice change; `P 1` is accepted and
    // the validation layer derives the spelling warning from the raw row.
    if let Some(number) = voice_number(text) {
        *body_started = true;
        return RowKind::Voice(number);
    }
    // Rule 5: `-` then whitespace then one number, plus a second number in the
    // relative-mode shape. Whether the document may carry the second number is
    // decided by the relative-mode reading and the validation layer.
    if let Some(kind) = phrase_end_kind(text) {
        *body_started = true;
        return kind;
    }
    // Rule 6: a visible ASCII type character other than space and `#`, three
    // integer fields, then optionally exactly one whitespace character and the
    // verbatim text.
    if let Some(row) = note_row(text) {
        *body_started = true;
        return RowKind::Note(row);
    }
    // Rule 7: anything else is invalid and kept verbatim.
    RowKind::Invalid {
        reason: "the row matches no row shape of the format".to_owned(),
    }
}

/// header_kind parses a header line into its key and value. The key is trimmed,
/// stripped of the leading hash and upper-cased; the value is trimmed and may
/// contain colons, because the split happens at the first colon.
fn header_kind(text: &str) -> RowKind {
    let Some((key_part, value)) = text.split_once(':') else {
        return RowKind::Invalid {
            reason: "the header row has no colon".to_owned(),
        };
    };
    let key = key_part.strip_prefix('#').unwrap_or(key_part).trim();
    if key.is_empty() {
        return RowKind::Invalid {
            reason: "the header row has no key".to_owned(),
        };
    }
    RowKind::Header {
        key: key.to_uppercase(),
        value: value.trim().to_owned(),
    }
}

/// voice_number reads the voice number of a voice-change line: `P`, optional
/// whitespace, exactly one digit, then only whitespace to the end of the line.
/// Any digit classifies as a voice; the unversioned format allows only P1 and
/// P2, which the validation layer reports.
fn voice_number(text: &str) -> Option<u8> {
    let rest = text.strip_prefix('P')?;
    let rest = skip_whitespace_any(rest);
    let digit = rest.chars().next()?;
    if !digit.is_ascii_digit() {
        return None;
    }
    let rest = &rest[digit.len_utf8()..];
    if !rest.chars().all(is_row_whitespace) {
        return None;
    }
    Some(digit.to_digit(10)? as u8)
}

/// phrase_end_kind parses an end-of-phrase line: `-`, whitespace, one number and
/// only whitespace to the end, or the relative-mode shape with a second number.
/// `None` means the line is not an end-of-phrase marker and the note shape is
/// tried next.
fn phrase_end_kind(text: &str) -> Option<RowKind> {
    let rest = text.strip_prefix('-')?;
    let rest = skip_whitespace(rest)?;
    let (beat, rest) = integer_field(rest)?;
    if rest.chars().all(is_row_whitespace) {
        return Some(RowKind::PhraseEnd {
            beat: Some(beat),
            rel_offset: None,
        });
    }
    let rest = skip_whitespace(rest)?;
    let (rel_offset, rest) = integer_field(rest)?;
    if !rest.chars().all(is_row_whitespace) {
        return None;
    }
    Some(RowKind::PhraseEnd {
        beat: Some(beat),
        rel_offset: Some(rel_offset),
    })
}

/// note_row parses a note line into its type, three integer fields and the
/// verbatim text. The text is optional; the validation layer warns about a note
/// with empty text. `None` means the line is not a note.
fn note_row(text: &str) -> Option<NoteRow> {
    let type_char = text.chars().next()?;
    if !type_char.is_ascii_graphic() || type_char == '#' {
        return None;
    }
    let rest = skip_whitespace(&text[type_char.len_utf8()..])?;
    let (start, rest) = integer_field(rest)?;
    let rest = skip_whitespace(rest)?;
    let (length, rest) = integer_field(rest)?;
    let rest = skip_whitespace(rest)?;
    let (pitch, rest) = integer_field(rest)?;
    // Exactly one whitespace character separates the pitch from the text; the
    // rest of the line is the text verbatim and may start with spaces.
    let note_text = match rest.chars().next() {
        Some(c) if is_row_whitespace(c) => &rest[c.len_utf8()..],
        Some(_) => return None,
        None => "",
    };
    let kind = match type_char {
        ':' => NoteType::Normal,
        '*' => NoteType::Golden,
        'R' => NoteType::Rap,
        'G' => NoteType::RapGolden,
        'F' => NoteType::Freestyle,
        other => NoteType::Other(other),
    };
    Some(NoteRow {
        kind,
        start,
        length,
        pitch,
        text: note_text.to_owned(),
    })
}

/// integer_field reads one integer field: an optional minus followed by one or
/// more ASCII digits. It returns the value and the rest of the input. A number
/// that overflows `i64` does not parse, so the line falls through to the next
/// rule.
fn integer_field(text: &str) -> Option<(i64, &str)> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let mut end = 0usize;
    for (index, c) in digits.char_indices() {
        if c.is_ascii_digit() {
            end = index + 1;
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let value: i64 = digits[..end].parse().ok()?;
    Some((if negative { -value } else { value }, &digits[end..]))
}

/// skip_whitespace skips one or more whitespace characters.
fn skip_whitespace(text: &str) -> Option<&str> {
    let mut end = 0usize;
    for (index, c) in text.char_indices() {
        if is_row_whitespace(c) {
            end = index + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 { None } else { Some(&text[end..]) }
}

/// skip_whitespace_any skips zero or more whitespace characters.
fn skip_whitespace_any(text: &str) -> &str {
    let mut end = 0usize;
    for (index, c) in text.char_indices() {
        if is_row_whitespace(c) {
            end = index + c.len_utf8();
        } else {
            break;
        }
    }
    &text[end..]
}

/// is_row_whitespace tells whether the character counts as format whitespace:
/// Unicode `White_Space` without CR and LF, which cannot appear in a raw line's
/// text because the byte layer splits lines on them.
fn is_row_whitespace(c: char) -> bool {
    c.is_whitespace() && c != '\r' && c != '\n'
}
