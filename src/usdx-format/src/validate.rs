//! The validation layer of the UltraStar song format.
//!
//! About me: this module checks a document and its song view for format and
//! consistency problems and reports them as issues with a severity, a kind, an
//! optional row and a message. The kinds follow the karaoke-dashboard's
//! `app/format_check.py` so both tools name the shared problems the same way;
//! the editor's checks are stricter in places (the spec-strict numeric syntax
//! without a sign, the structural body checks the dashboard does not have).
//! It exists so that the checks panel and the properties page can show what
//! needs attention before a song is saved.

use std::collections::HashMap;
use std::path::Path;

use crate::headers::parse_decimal;
use crate::rows::{Document, NoteType, RowId, RowKind};
use crate::song::Song;

/// The severity of an issue, shown as Information, Warning or Error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// The kind of an issue. Tags and voice numbers are carried in the kind so the
/// checks panel can group and filter them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueKind {
    MissingHeader(&'static str),
    NoAudioReference,
    MissingVersion,
    InvalidVersionSyntax,
    ForeignMajorVersion,
    UnpublishedVersion,
    MalformedNumericHeader(&'static str),
    ByteOrderMark,
    NotUtf8,
    EncodingGuessed,
    FileNotFound(&'static str),
    FileCaseMismatch(&'static str),
    AbsolutePathInReference(&'static str),
    VoiceChangeWithoutHeader(u8),
    InvalidRow,
    UnknownNoteType,
    NegativeStart,
    LengthBelowOne,
    NotesOutOfOrder,
    OverlappingNotes,
    PhraseEndInsideNote,
    TwoPhraseEndsInARow,
    PhraseEndBeforeFirstNote,
    PhraseEndAfterLastNote,
    NoteWithEmptyText,
}

impl IssueKind {
    /// severity returns the severity of the kind. The plan pins most of them;
    /// the user decided the rest (see PROGRESS.md, task P1-08).
    pub fn severity(&self) -> Severity {
        match self {
            IssueKind::MissingHeader(_)
            | IssueKind::NoAudioReference
            | IssueKind::InvalidVersionSyntax
            | IssueKind::ForeignMajorVersion
            | IssueKind::FileNotFound(_)
            | IssueKind::AbsolutePathInReference(_)
            | IssueKind::InvalidRow
            | IssueKind::NegativeStart
            | IssueKind::LengthBelowOne
            | IssueKind::PhraseEndInsideNote => Severity::Error,
            IssueKind::MissingVersion
            | IssueKind::MalformedNumericHeader(_)
            | IssueKind::ByteOrderMark
            | IssueKind::NotUtf8
            | IssueKind::EncodingGuessed
            | IssueKind::FileCaseMismatch(_)
            | IssueKind::VoiceChangeWithoutHeader(_)
            | IssueKind::UnknownNoteType
            | IssueKind::NotesOutOfOrder
            | IssueKind::OverlappingNotes
            | IssueKind::TwoPhraseEndsInARow
            | IssueKind::PhraseEndBeforeFirstNote
            | IssueKind::PhraseEndAfterLastNote
            | IssueKind::NoteWithEmptyText => Severity::Warning,
            IssueKind::UnpublishedVersion => Severity::Info,
        }
    }
}

/// One validation issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub kind: IssueKind,
    pub row: Option<RowId>,
    pub message: String,
}

/// The file reference tags the format uses, in the dashboard's order.
const FILE_REF_TAGS: [&str; 5] = ["MP3", "AUDIO", "COVER", "BACKGROUND", "VIDEO"];

/// The numeric header tags whose values must parse as format decimals. `YEAR`
/// is included like the dashboard does; the check is spec-strict, so a
/// negative value is malformed.
const NUMERIC_TAGS: [&str; 5] = ["BPM", "GAP", "START", "END", "YEAR"];

/// The published versions of the format specification; 1.1.0 is historically
/// equivalent to 1.0.0. Every other 1.x.y version reads as version 1.
const PUBLISHED_VERSIONS: [(u32, u32, u32); 2] = [(1, 0, 0), (1, 1, 0)];

/// validate checks the document and its song view and returns the issues:
/// header issues first, then the row issues in row order, then the file
/// references (only with a song folder), then the encoding and BOM warnings.
pub fn validate(doc: &Document, song: &Song, folder: Option<&Path>) -> Vec<Issue> {
    let mut issues = Vec::new();
    check_headers(song, &mut issues);
    check_rows(doc, &mut issues);
    check_voices_declared(doc, song, &mut issues);
    check_voice_structure(doc, &mut issues);
    if let Some(folder) = folder {
        check_file_references(song, folder, &mut issues);
    }
    check_file_encoding(doc, &mut issues);
    issues
}

/// issue builds an issue with the severity of its kind.
fn issue(kind: IssueKind, row: Option<RowId>, message: String) -> Issue {
    Issue {
        severity: kind.severity(),
        kind,
        row,
        message,
    }
}

/// check_headers checks the mandatory tags, the audio reference, the version
/// and the numeric header values.
fn check_headers(song: &Song, issues: &mut Vec<Issue>) {
    for tag in ["TITLE", "ARTIST", "BPM"] {
        if doc_song_header(song, tag).is_none() {
            issues.push(issue(
                IssueKind::MissingHeader(tag),
                None,
                format!("Missing mandatory #{tag} tag."),
            ));
        }
    }
    if song.headers.get("MP3").is_none() && song.headers.get("AUDIO").is_none() {
        issues.push(issue(
            IssueKind::NoAudioReference,
            None,
            "No audio reference (#MP3 or #AUDIO).".to_owned(),
        ));
    }
    match song.version {
        crate::headers::Version::Unversioned => issues.push(issue(
            IssueKind::MissingVersion,
            None,
            "Missing #VERSION tag.".to_owned(),
        )),
        crate::headers::Version::Invalid(_) => issues.push(issue(
            IssueKind::InvalidVersionSyntax,
            None,
            "The #VERSION value is not a valid version triplet.".to_owned(),
        )),
        crate::headers::Version::V(major, minor, patch) => {
            if major != 1 {
                issues.push(issue(
                    IssueKind::ForeignMajorVersion,
                    None,
                    format!(
                        "The major version {major} is not supported; the song opens read-only."
                    ),
                ));
            } else if !PUBLISHED_VERSIONS.contains(&(major, minor, patch)) {
                issues.push(issue(
                    IssueKind::UnpublishedVersion,
                    None,
                    format!("The version {major}.{minor}.{patch} is not a published format version; read as version 1."),
                ));
            }
        }
    }
    for tag in NUMERIC_TAGS {
        if let Some(value) = song.headers.get(tag)
            && parse_decimal(value).is_none()
        {
            issues.push(issue(
                IssueKind::MalformedNumericHeader(tag),
                None,
                format!("Malformed #{tag} value: {value:?}."),
            ));
        }
    }
}

/// doc_song_header looks a header value up in the song view's headers; an
/// empty value counts as absent.
fn doc_song_header<'a>(song: &'a Song, tag: &str) -> Option<&'a str> {
    song.headers.get(tag).filter(|value| !value.is_empty())
}
/// check_rows reports the invalid rows and the per-note problems in row order.
fn check_rows(doc: &Document, issues: &mut Vec<Issue>) {
    for row in &doc.rows {
        match &row.kind {
            RowKind::Invalid { reason } => issues.push(issue(
                IssueKind::InvalidRow,
                Some(row.id),
                format!("Unrecognized row: {reason}."),
            )),
            RowKind::Note(note) => {
                if let NoteType::Other(type_char) = &note.kind {
                    issues.push(issue(
                        IssueKind::UnknownNoteType,
                        Some(row.id),
                        format!("The note type {type_char:?} is not a standard type."),
                    ));
                }
                if note.start < 0 {
                    issues.push(issue(
                        IssueKind::NegativeStart,
                        Some(row.id),
                        format!("The note starts at a negative beat ({}).", note.start),
                    ));
                }
                if note.length < 1 {
                    issues.push(issue(
                        IssueKind::LengthBelowOne,
                        Some(row.id),
                        format!("The note length {} is below 1.", note.length),
                    ));
                }
                if note.text.is_empty() {
                    issues.push(issue(
                        IssueKind::NoteWithEmptyText,
                        Some(row.id),
                        "The note has no text.".to_owned(),
                    ));
                }
            }
            _ => {}
        }
    }
}

/// check_voices_declared warns about every voice change whose `#Pn` header is
/// missing, once per voice number.
fn check_voices_declared(doc: &Document, song: &Song, issues: &mut Vec<Issue>) {
    let mut reported: HashMap<u8, ()> = HashMap::new();
    for row in &doc.rows {
        if let RowKind::Voice(number) = row.kind {
            if reported.insert(number, ()).is_some() {
                continue;
            }
            if doc_song_header(song, &format!("P{number}")).is_none() {
                issues.push(issue(
                    IssueKind::VoiceChangeWithoutHeader(number),
                    Some(row.id),
                    format!("Voice change P{number} used without a matching #P{number} header."),
                ));
            }
        }
    }
}

/// One event of a voice's body in row order, so the phrase-end checks can see
/// which notes came before a phrase end.
#[derive(Clone, Copy)]
enum VoiceEvent {
    Note { row: RowId, start: f64, length: i64 },
    PhraseEnd { row: RowId, beat: f64 },
}

/// check_voice_structure checks the body per voice: the note order, the note
/// overlaps and the phrase-end placement. In relative mode the absolute beats
/// decide, so the walk tracks the running offset per voice; a phrase end's
/// own rel-offset is added after its beat has been interpreted, like the USDX
/// implementation does.
fn check_voice_structure(doc: &Document, issues: &mut Vec<Issue>) {
    let mut voices: HashMap<u8, Vec<VoiceEvent>> = HashMap::new();
    let mut rel: HashMap<u8, f64> = HashMap::new();
    let mut current_voice = 1u8;

    for row in &doc.rows {
        match &row.kind {
            RowKind::Note(note) => {
                let offset = rel.get(&current_voice).copied().unwrap_or(0.0);
                voices
                    .entry(current_voice)
                    .or_default()
                    .push(VoiceEvent::Note {
                        row: row.id,
                        start: offset + note.start as f64,
                        length: note.length,
                    });
            }
            RowKind::PhraseEnd { beat, rel_offset } => {
                if let Some(beat) = beat {
                    let offset = rel.get(&current_voice).copied().unwrap_or(0.0);
                    voices
                        .entry(current_voice)
                        .or_default()
                        .push(VoiceEvent::PhraseEnd {
                            row: row.id,
                            beat: offset + *beat as f64,
                        });
                }
                if let Some(offset) = rel_offset {
                    *rel.entry(current_voice).or_default() += *offset as f64;
                }
            }
            RowKind::Voice(number) => current_voice = *number,
            RowKind::End => break,
            RowKind::Header { .. } | RowKind::Empty | RowKind::Invalid { .. } => {}
        }
    }

    // The voices are checked in the order of their first appearance.
    let mut voice_numbers: Vec<u8> = voices.keys().copied().collect();
    voice_numbers.sort_by_key(|number| {
        voices[number]
            .first()
            .map(|event| match event {
                VoiceEvent::Note { row, .. } | VoiceEvent::PhraseEnd { row, .. } => row.0,
            })
            .unwrap_or(u32::MAX)
    });
    for number in voice_numbers {
        check_one_voice(&voices[&number], issues);
    }
}

/// check_one_voice checks one voice's events in row order.
fn check_one_voice(events: &[VoiceEvent], issues: &mut Vec<Issue>) {
    let notes: Vec<(RowId, f64, i64)> = events
        .iter()
        .filter_map(|event| match *event {
            VoiceEvent::Note { row, start, length } => Some((row, start, length)),
            VoiceEvent::PhraseEnd { .. } => None,
        })
        .collect();

    let mut previous: Option<(f64, i64)> = None;
    for (row, start, length) in &notes {
        if let Some((previous_start, previous_length)) = previous {
            if *start < previous_start {
                issues.push(issue(
                    IssueKind::NotesOutOfOrder,
                    Some(*row),
                    format!(
                        "The note starts at beat {}, before the previous note at beat {}.",
                        format_start(*start),
                        format_start(previous_start)
                    ),
                ));
            }
            if (previous_start + previous_length as f64) > *start {
                issues.push(issue(
                    IssueKind::OverlappingNotes,
                    Some(*row),
                    format!(
                        "The note overlaps the previous note ({} < {} + {previous_length}).",
                        format_start(*start),
                        format_start(previous_start)
                    ),
                ));
            }
        }
        previous = Some((*start, *length));
    }

    for (index, event) in events.iter().enumerate() {
        let VoiceEvent::PhraseEnd { row, beat } = *event else {
            continue;
        };
        // Two phrase ends in a row: the previous event of the voice is a
        // phrase end too.
        if index > 0 && matches!(events[index - 1], VoiceEvent::PhraseEnd { .. }) {
            issues.push(issue(
                IssueKind::TwoPhraseEndsInARow,
                Some(row),
                "Two phrase ends in a row.".to_owned(),
            ));
        }
        // A phrase end inside a note: only the notes before it in row order
        // count; a phrase end at the start of the next note is the normal
        // line-break position.
        if let Some((_, note_start, _)) = events[..index]
            .iter()
            .filter_map(|event| match *event {
                VoiceEvent::Note {
                    row: _,
                    start,
                    length,
                } => Some((row, start, length)),
                VoiceEvent::PhraseEnd { .. } => None,
            })
            .find(|(_, start, length)| *start <= beat && beat < *start + *length as f64)
        {
            issues.push(issue(
                IssueKind::PhraseEndInsideNote,
                Some(row),
                format!(
                    "The phrase end at beat {} lies inside the note at beat {}.",
                    format_start(beat),
                    format_start(note_start)
                ),
            ));
        }
        if let Some((_, first_start, _)) = notes.first()
            && beat < *first_start
        {
            issues.push(issue(
                IssueKind::PhraseEndBeforeFirstNote,
                Some(row),
                format!(
                    "The phrase end at beat {} lies before the first note at beat {}.",
                    format_start(beat),
                    format_start(*first_start)
                ),
            ));
        }
        if let Some((_, last_start, last_length)) = notes.last() {
            let last_end = *last_start + *last_length as f64;
            if beat > last_end {
                issues.push(issue(
                    IssueKind::PhraseEndAfterLastNote,
                    Some(row),
                    format!(
                        "The phrase end at beat {} lies after the last note (ends at beat {}).",
                        format_start(beat),
                        format_start(last_end)
                    ),
                ));
            }
        }
    }
}

/// format_start formats a beat value without a trailing `.0`.
fn format_start(beat: f64) -> String {
    if beat.fract() == 0.0 {
        format!("{}", beat as i64)
    } else {
        format!("{beat}")
    }
}

/// check_file_references checks the referenced media files: an absolute path
/// is an error, a file that exists only with different letter case is a
/// warning, a missing file is an error.
fn check_file_references(song: &Song, folder: &Path, issues: &mut Vec<Issue>) {
    for tag in FILE_REF_TAGS {
        let Some(value) = doc_song_header(song, tag) else {
            continue;
        };
        if is_absolute_path(value) {
            issues.push(issue(
                IssueKind::AbsolutePathInReference(tag),
                None,
                format!("The #{tag} value {value:?} is an absolute path."),
            ));
            continue;
        }
        let exact = folder.join(value);
        if exact.is_file() {
            continue;
        }
        let case_match = folder.read_dir().ok().and_then(|entries| {
            entries
                .filter_map(Result::ok)
                .find(|entry| {
                    entry.file_name().to_string_lossy().to_lowercase() == value.to_lowercase()
                        && entry.path().is_file()
                })
                .map(|entry| entry.file_name().to_string_lossy().to_string())
        });
        match case_match {
            Some(actual) => issues.push(issue(
                IssueKind::FileCaseMismatch(tag),
                None,
                format!("Referenced #{tag} file found only with different letter case: {value:?} exists as {actual:?}."),
            )),
            None => issues.push(issue(
                IssueKind::FileNotFound(tag),
                None,
                format!("Referenced #{tag} file not found: {value:?}."),
            )),
        }
    }
}

/// is_absolute_path tells whether a file reference is an absolute path: a
/// leading slash, a leading backslash or a Windows drive letter.
fn is_absolute_path(value: &str) -> bool {
    if value.starts_with('/') || value.starts_with('\\') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// check_file_encoding reports a BOM, a non-UTF-8 encoding and a guessed
/// encoding.
fn check_file_encoding(doc: &Document, issues: &mut Vec<Issue>) {
    if doc.file.bom {
        issues.push(issue(
            IssueKind::ByteOrderMark,
            None,
            "Starts with a UTF-8 byte order mark - the format wants UTF-8 without one.".to_owned(),
        ));
    }
    if doc.file.encoding != crate::bytes::TextEncoding::Utf8 {
        issues.push(issue(
            IssueKind::NotUtf8,
            None,
            format!(
                "The file is not UTF-8 (it reads as {:?}).",
                doc.file.encoding
            ),
        ));
    }
    if doc.file.encoding_source == crate::bytes::EncodingSource::Guessed {
        issues.push(issue(
            IssueKind::EncodingGuessed,
            None,
            format!("The encoding was guessed ({:?}).", doc.file.encoding),
        ));
    }
}
