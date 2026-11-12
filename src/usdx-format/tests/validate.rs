//! About me: tests for the validation layer (task P1-08, plan section 5.7).
//! They assert one issue kind per inline song or fixture, the severities the
//! plan pins and the user decided (gate-free decision recorded in
//! PROGRESS.md), the clean song with no issues, and the fixture
//! `ultrasinger_1_2_0.txt` yielding exactly the Info about the unpublished
//! version 1.2.0 and no error.

use std::path::Path;

use tempfile::tempdir;
use usdx_format::bytes::decode;
use usdx_format::rows::classify;
use usdx_format::song::Song;
use usdx_format::validate::{Issue, IssueKind, Severity, validate};

/// check validates an inline song without a song folder.
fn check(text: &str) -> Vec<Issue> {
    let d = classify(decode(text.as_bytes()).expect("the text decodes"));
    let s = Song::from_document(&d);
    validate(&d, &s, None)
}

/// check_in_folder validates an inline song inside the folder.
fn check_in_folder(text: &str, folder: &Path) -> Vec<Issue> {
    let d = classify(decode(text.as_bytes()).expect("the text decodes"));
    let s = Song::from_document(&d);
    validate(&d, &s, Some(folder))
}

/// has tells whether the issues contain the kind with the severity.
fn has(issues: &[Issue], kind: IssueKind, severity: Severity) -> bool {
    issues
        .iter()
        .any(|issue| issue.kind == kind && issue.severity == severity)
}

/// count counts the issues of a kind.
fn count(issues: &[Issue], kind: IssueKind) -> usize {
    issues.iter().filter(|issue| issue.kind == kind).count()
}

/// the_headers builds the header block every inline song needs to be clean.
fn the_headers() -> String {
    "#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\n".to_owned()
}

// ---- The clean song ----

#[test]
fn a_well_formed_song_has_no_issues() {
    let issues = check(&format!(
        "{}: 0 4 0 la\n- 4\n: 4 4 1 la\nE\n",
        the_headers()
    ));
    assert!(issues.is_empty(), "the clean song has issues: {issues:?}");
}

// ---- Headers ----

#[test]
fn missing_mandatory_headers_are_errors() {
    let issues = check("#VERSION:1.0.0\nE\n");
    assert_eq!(count(&issues, IssueKind::MissingHeader("TITLE")), 1);
    assert_eq!(count(&issues, IssueKind::MissingHeader("ARTIST")), 1);
    assert_eq!(count(&issues, IssueKind::MissingHeader("BPM")), 1);
    for issue in issues {
        if matches!(issue.kind, IssueKind::MissingHeader(_)) {
            assert_eq!(issue.severity, Severity::Error);
        }
    }
}

#[test]
fn a_song_without_an_audio_reference_is_an_error() {
    let issues = check("#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::NoAudioReference,
        Severity::Error
    ));
}

#[test]
fn a_song_without_a_version_header_is_a_warning() {
    let issues = check("#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::MissingVersion,
        Severity::Warning
    ));
}

#[test]
fn an_invalid_version_value_is_an_error() {
    let issues = check("#VERSION:garbage\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::InvalidVersionSyntax,
        Severity::Error
    ));
}

#[test]
fn a_foreign_major_version_is_an_error() {
    let issues = check("#VERSION:2.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::ForeignMajorVersion,
        Severity::Error
    ));
}

#[test]
fn an_unpublished_version_is_an_info() {
    let issues = check("#VERSION:1.2.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::UnpublishedVersion,
        Severity::Info
    ));
}

#[test]
fn the_published_versions_produce_no_version_issues() {
    for version in ["1.0.0", "1.1.0"] {
        let issues = check(&format!(
            "#VERSION:{version}\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n"
        ));
        assert!(
            issues.is_empty(),
            "version {version} produced issues: {issues:?}"
        );
    }
}

#[test]
fn a_malformed_numeric_header_is_a_warning() {
    let issues = check("#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:abc\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::MalformedNumericHeader("BPM"),
        Severity::Warning
    ));
}

#[test]
fn a_negative_numeric_header_is_malformed() {
    // The format's time header syntax has no sign; a negative value is
    // malformed (the dashboard's regex allows minus, the spec does not).
    let issues = check("#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#GAP:-5\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::MalformedNumericHeader("GAP"),
        Severity::Warning
    ));
}

// ---- Encoding and BOM ----

#[test]
fn a_byte_order_mark_is_a_warning() {
    let issues = check("\u{feff}#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n");
    assert!(has(
        issues.as_slice(),
        IssueKind::ByteOrderMark,
        Severity::Warning
    ));
}

#[test]
fn a_non_utf8_encoding_is_a_warning() {
    // Windows-1252 bytes: "M\fcnchen" with an umlaut.
    let bytes = b"#VERSION:1.0.0\n#TITLE:M\xfcll\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n";
    let d = classify(decode(bytes).expect("the bytes decode as windows-1252"));
    let s = Song::from_document(&d);
    let issues = validate(&d, &s, None);
    assert!(has(
        issues.as_slice(),
        IssueKind::NotUtf8,
        Severity::Warning
    ));
}

#[test]
fn a_guessed_encoding_is_a_warning() {
    let bytes = b"#VERSION:1.0.0\n#TITLE:M\xfcll\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\nE\n";
    let d = classify(decode(bytes).expect("the bytes decode as windows-1252"));
    assert_eq!(
        d.file.encoding_source,
        usdx_format::bytes::EncodingSource::Guessed
    );
    let s = Song::from_document(&d);
    let issues = validate(&d, &s, None);
    assert!(has(
        issues.as_slice(),
        IssueKind::EncodingGuessed,
        Severity::Warning
    ));
}

// ---- File references ----

#[test]
fn a_missing_referenced_file_is_an_error() {
    let folder = tempdir().expect("the temp dir builds");
    let issues = check_in_folder(&format!("{}E\n", the_headers()), folder.path());
    assert!(has(
        issues.as_slice(),
        IssueKind::FileNotFound("MP3"),
        Severity::Error
    ));
}

#[test]
fn a_file_reference_with_different_letter_case_is_a_warning() {
    let folder = tempdir().expect("the temp dir builds");
    std::fs::write(folder.path().join("s.mp3"), b"x").expect("the file writes");
    let headers = "#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:S.mp3\n";
    let issues = check_in_folder(&format!("{headers}E\n"), folder.path());
    assert!(has(
        issues.as_slice(),
        IssueKind::FileCaseMismatch("MP3"),
        Severity::Warning
    ));
    assert!(!has(
        issues.as_slice(),
        IssueKind::FileNotFound("MP3"),
        Severity::Error
    ));
}

#[test]
fn an_existing_file_reference_produces_no_issue() {
    let folder = tempdir().expect("the temp dir builds");
    std::fs::write(folder.path().join("s.mp3"), b"x").expect("the file writes");
    let issues = check_in_folder(&format!("{}E\n", the_headers()), folder.path());
    assert!(
        issues.is_empty(),
        "the existing file produced issues: {issues:?}"
    );
}

#[test]
fn an_absolute_path_in_a_file_reference_is_an_error() {
    let folder = tempdir().expect("the temp dir builds");
    for value in ["/music/s.mp3", "C:\\music\\s.mp3"] {
        let headers = format!("#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:{value}\n");
        let issues = check_in_folder(&format!("{headers}E\n"), folder.path());
        assert!(
            has(
                issues.as_slice(),
                IssueKind::AbsolutePathInReference("MP3"),
                Severity::Error
            ),
            "the value {value} is not flagged as an absolute path"
        );
    }
}

#[test]
fn a_file_reference_is_checked_for_the_audio_cover_background_and_video_tags() {
    let folder = tempdir().expect("the temp dir builds");
    let headers = "#VERSION:1.0.0\n#TITLE:T\n#ARTIST:A\n#BPM:240\n#MP3:s.mp3\n#COVER:c.jpg\n#BACKGROUND:b.jpg\n#VIDEO:v.mp4\n";
    let issues = check_in_folder(&format!("{headers}E\n"), folder.path());
    assert!(has(
        issues.as_slice(),
        IssueKind::FileNotFound("COVER"),
        Severity::Error
    ));
    assert!(has(
        issues.as_slice(),
        IssueKind::FileNotFound("BACKGROUND"),
        Severity::Error
    ));
    assert!(has(
        issues.as_slice(),
        IssueKind::FileNotFound("VIDEO"),
        Severity::Error
    ));
}

// ---- Body rows ----

#[test]
fn an_invalid_row_is_an_error() {
    let issues = check(&format!("{}junk line\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::InvalidRow,
        Severity::Error
    ));
}

#[test]
fn an_unknown_note_type_is_a_warning() {
    let issues = check(&format!("{}X 0 4 0 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::UnknownNoteType,
        Severity::Warning
    ));
}

#[test]
fn a_negative_start_is_an_error() {
    let issues = check(&format!("{}: -3 4 0 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::NegativeStart,
        Severity::Error
    ));
}

#[test]
fn a_length_below_one_is_an_error() {
    let issues = check(&format!("{}: 0 0 0 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::LengthBelowOne,
        Severity::Error
    ));
}

#[test]
fn a_note_with_an_empty_text_is_a_warning() {
    let issues = check(&format!("{}: 0 4 0\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::NoteWithEmptyText,
        Severity::Warning
    ));
}

#[test]
fn notes_out_of_order_within_a_voice_are_a_warning() {
    let issues = check(&format!("{}: 8 4 0 la\n: 4 4 1 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::NotesOutOfOrder,
        Severity::Warning
    ));
}

#[test]
fn overlapping_notes_are_a_warning() {
    let issues = check(&format!("{}: 0 8 0 la\n: 4 4 1 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::OverlappingNotes,
        Severity::Warning
    ));
}

#[test]
fn a_phrase_end_inside_a_note_is_an_error() {
    let issues = check(&format!(
        "{}: 0 8 0 la\n- 4\n: 8 4 1 la\nE\n",
        the_headers()
    ));
    assert!(has(
        issues.as_slice(),
        IssueKind::PhraseEndInsideNote,
        Severity::Error
    ));
}

#[test]
fn two_phrase_ends_in_a_row_are_a_warning() {
    let issues = check(&format!("{}: 0 4 0 la\n- 4\n- 4\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::TwoPhraseEndsInARow,
        Severity::Warning
    ));
}

#[test]
fn a_phrase_end_before_the_first_note_is_a_warning() {
    let issues = check(&format!("{}- 0\n: 4 4 0 la\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::PhraseEndBeforeFirstNote,
        Severity::Warning
    ));
}

#[test]
fn a_phrase_end_after_the_last_note_is_a_warning() {
    let issues = check(&format!("{}: 0 4 0 la\n- 10\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::PhraseEndAfterLastNote,
        Severity::Warning
    ));
}

#[test]
fn a_phrase_end_at_the_last_note_end_is_fine() {
    let issues = check(&format!("{}: 0 4 0 la\n- 4\nE\n", the_headers()));
    assert!(
        issues.is_empty(),
        "the well-placed phrase end produced issues: {issues:?}"
    );
}

#[test]
fn a_voice_change_without_a_header_is_a_warning() {
    let issues = check(&format!("{}P2\n: 0 4 0 la\n- 4\nE\n", the_headers()));
    assert!(has(
        issues.as_slice(),
        IssueKind::VoiceChangeWithoutHeader(2),
        Severity::Warning
    ));
}

#[test]
fn a_voice_change_with_a_header_is_fine() {
    let issues = check(&format!(
        "{}#P2:singer 2\nP2\n: 0 4 0 la\n- 4\nE\n",
        the_headers()
    ));
    assert!(
        issues.is_empty(),
        "the declared voice change produced issues: {issues:?}"
    );
}

// ---- Relative mode: the checks judge the absolute beats ----

#[test]
fn the_relative_checks_use_the_absolute_beats() {
    // Raw starts both 0, but the absolute starts are 0 and 10 - in order, and
    // the second phrase end sits exactly at the second note's end.
    let text = format!(
        "{}#RELATIVE:yes\n: 0 4 0 la\n- 4 10\n: 0 4 0 la\n- 4 2\nE\n",
        the_headers()
    );
    let issues = check(&text);
    assert!(
        issues.is_empty(),
        "the relative song produced issues: {issues:?}"
    );
}

// ---- The fixture ----

#[test]
fn the_ultrasinger_fixture_yields_only_the_unpublished_version_info() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ultrasinger_1_2_0.txt"
    );
    let bytes = std::fs::read(path).expect("the fixture reads");
    let d = classify(decode(&bytes).expect("the fixture decodes"));
    let s = Song::from_document(&d);
    let issues = validate(&d, &s, None);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, IssueKind::UnpublishedVersion);
    assert_eq!(issues[0].severity, Severity::Info);
    assert!(issues[0].row.is_none());
}
