//! About me: tests for the repair bridge logic of task P7-08 (plan
//! sections 8.3, 6.11; UI_SPEC 4.6, 4.8). The QObject declarations are
//! thin wrappers; the logic they mirror lives in `bridge/repair.rs` and
//! is tested here without Qt: the anchor card rows the AnchorPanel
//! shows (badge, line number, line text, old and pinned time), the
//! anchors.txt file write behind `exportAnchors`, the export dialog's
//! preview payload (the rendered text, the changed and pinned counts,
//! the Factory-matching problems and the excluded lines), the ghost
//! note positions of the preview patch, the lyrics.txt lines the
//! Factory-matching check needs, the click-track WAV writer the latency
//! calibration plays, the UTC date of the export header comment, and
//! the current line's repair target (the voice and voice-local line
//! `pinCurrentLineAt` pins). The calibration WAV is 8 clicks at 1 s
//! spacing: 16-bit mono PCM with a 44-byte header, no new dependency.

use std::collections::BTreeSet;
use tempfile::TempDir;
use usdx_edit::Patch;
use usdx_edit::anchors_export::AnchorEntry;
use usdx_edit::block::Follow;
use usdx_edit::patch::RowOp;
use usdx_edit::retime::{Anchor, Pin};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;
use usdx_song_editor::bridge::repair::{
    AnchorRow, BlockSpan, HealthBasis, OverviewLine, PreviewNote, anchor_rows, block_span,
    click_track_wav, current_repair_target, default_anchors_path, export_anchors, export_preview,
    follow_i32, follow_of, line_range_target, lyrics_lines, overview_anchor_marks, overview_lines,
    preview_note_hash, preview_notes, racing_note_rows, racing_ranges, repair_section,
    section_end_ms, selection_lines, shortened_notes, tap_lines, utc_date,
};
use usdx_song_editor::controller::EditorController;
use usdx_song_editor::controller::editor::LoadedSong;
use usdx_song_editor::controller::fingerprint::FileFingerprint;
use usdx_song_editor::controller::repair::RepairController;

/// fixture writes a song text into a fresh temporary directory and
/// returns the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// Three lines of three notes each at beats 0/12/24; the syllables carry
/// trailing spaces, so each note is its own word.
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

#[test]
fn anchor_rows_list_the_card_data_with_old_and_pinned_times() {
    let (_dir, doc, song) = loaded();
    let rows = anchor_rows(
        &doc,
        &song,
        0,
        &[
            Anchor {
                line: 0,
                target_ms: 500.0,
                pin: Pin::Start,
            },
            Anchor {
                line: 2,
                target_ms: 2000.0,
                pin: Pin::Start,
            },
        ],
    );
    assert_eq!(
        rows,
        vec![
            AnchorRow {
                badge: 0,
                line_number: 1,
                text: "a b c".to_owned(),
                old_ms: 0.0,
                target_ms: 500.0,
            },
            AnchorRow {
                badge: 1,
                line_number: 3,
                text: "g h i".to_owned(),
                old_ms: 1500.0,
                target_ms: 2000.0,
            },
        ]
    );
}

#[test]
fn export_anchors_writes_the_rendered_file_and_refuses_an_invalid_set() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("anchors.txt");
    let entries = vec![AnchorEntry {
        time_ms: 500.0,
        text: "a b c".to_owned(),
        source_line: 0,
    }];
    export_anchors(&path, "T", "0.1.0", "2026-11-16", &entries, Some(5000.0))
        .expect("the file is written");
    let written = std::fs::read_to_string(&path).expect("the file exists");
    assert_eq!(
        written,
        "# anchors for T - exported by USDX Song Editor 0.1.0 on 2026-11-16\n\
         0:00.50 | a b c\n\
         end 0:05.00\n"
    );

    // An invalid set (two anchors at the same time) is refused and does
    // not touch the file.
    let bad = vec![
        AnchorEntry {
            time_ms: 500.0,
            text: "a b c".to_owned(),
            source_line: 0,
        },
        AnchorEntry {
            time_ms: 500.0,
            text: "d e f".to_owned(),
            source_line: 1,
        },
    ];
    assert!(export_anchors(&path, "T", "0.1.0", "2026-11-16", &bad, None).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).expect("the file survives"),
        written,
        "the failed export leaves the old file untouched"
    );
}

#[test]
fn lyrics_lines_read_the_file_and_skip_empty_lines() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let missing = lyrics_lines(dir.path());
    assert_eq!(missing, None, "no lyrics.txt is no error, just no lines");
    std::fs::write(
        dir.path().join("lyrics.txt"),
        "first line\n\n  second line  \n\n",
    )
    .expect("the lyrics are written");
    assert_eq!(
        lyrics_lines(dir.path()),
        Some(vec!["first line".to_owned(), "second line".to_owned()])
    );
}

#[test]
fn the_click_track_writes_eight_clicks_at_one_second_spacing() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("clicks.wav");
    let rate = 44100_u32;
    click_track_wav(&path, rate).expect("the wav is written");
    let bytes = std::fs::read(&path).expect("the wav is read");

    // The canonical 44-byte PCM WAV header.
    assert_eq!(bytes.len(), 44 + (rate * 8 * 2) as usize);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(u16_from(&bytes[20..22]), 1, "PCM");
    assert_eq!(u16_from(&bytes[22..24]), 1, "mono");
    assert_eq!(u32_from(&bytes[24..28]), rate);
    assert_eq!(u16_from(&bytes[34..36]), 16, "bits per sample");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32_from(&bytes[40..44]), rate * 8 * 2);

    // Every one-second window holds a click (the click starts at the
    // window's first sample) and is quiet again a quarter second in.
    let samples: Vec<i16> = bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect();
    for k in 0..8_usize {
        let window = &samples[k * rate as usize..(k + 1) * rate as usize];
        let peak = window.iter().map(|s| s.abs()).max().expect("non-empty");
        assert!(peak > 5000, "window {k} has no click (peak {peak})");
        let quiet = &window[rate as usize / 4..rate as usize / 2];
        let loudest = quiet.iter().map(|s| s.abs()).max().expect("non-empty");
        assert!(
            loudest < 100,
            "window {k} is not quiet a quarter second in ({loudest})"
        );
    }
}

/// u16_from reads a little-endian u16.
fn u16_from(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

/// u32_from reads a little-endian u32.
fn u32_from(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[test]
fn the_current_line_maps_to_the_voice_and_the_voice_local_line() {
    let duet = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n",
        "P2\n: 8 4 0 b \n- 12\nE\n",
    );
    let (dir, path) = fixture(duet);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song opens");
    assert_eq!(current_repair_target(&controller), Some((0, 0)));
    controller.go_to_line(1);
    assert_eq!(
        current_repair_target(&controller),
        Some((1, 0)),
        "global line 1 is the duet's second voice, line 0"
    );
    drop(dir);
}

#[test]
fn the_current_repair_target_is_none_without_a_song() {
    assert_eq!(current_repair_target(&EditorController::new()), None);
}

#[test]
fn utc_date_formats_the_epoch_and_a_known_2026_day() {
    assert_eq!(utc_date(0), "1970-01-01");
    // 2026-11-16 00:00:00 UTC.
    assert_eq!(utc_date(1_794_787_200), "2026-11-16");
    // A leap year day: 2024-02-29 00:00:00 UTC.
    assert_eq!(utc_date(1_709_164_800), "2024-02-29");
}

#[test]
fn the_ghost_notes_list_the_moved_notes_new_positions() {
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    // Line 0 pinned at 500 ms, line 2 at 2000 ms: the whole voice
    // shifts rigidly by 8 beats (the P7-07 planner tests pin this), so
    // every note lands 500 ms later.
    repair.pin_current_line_at(0, 500.0);
    repair.pin_current_line_at(2, 2000.0);
    let patch = repair
        .preview(&doc, &song, 0)
        .expect("the preview plans")
        .clone();
    assert_eq!(
        preview_notes(&doc, &patch),
        vec![
            PreviewNote {
                row_id: 2,
                start_ms: 500.0,
                end_ms: 750.0,
                pitch: 60
            },
            PreviewNote {
                row_id: 3,
                start_ms: 750.0,
                end_ms: 1000.0,
                pitch: 62
            },
            PreviewNote {
                row_id: 4,
                start_ms: 1000.0,
                end_ms: 1250.0,
                pitch: 64
            },
            PreviewNote {
                row_id: 6,
                start_ms: 1250.0,
                end_ms: 1500.0,
                pitch: 60
            },
            PreviewNote {
                row_id: 7,
                start_ms: 1500.0,
                end_ms: 1750.0,
                pitch: 62
            },
            PreviewNote {
                row_id: 8,
                start_ms: 1750.0,
                end_ms: 2000.0,
                pitch: 64
            },
            PreviewNote {
                row_id: 10,
                start_ms: 2000.0,
                end_ms: 2250.0,
                pitch: 60
            },
            PreviewNote {
                row_id: 11,
                start_ms: 2250.0,
                end_ms: 2500.0,
                pitch: 62
            },
            PreviewNote {
                row_id: 12,
                start_ms: 2500.0,
                end_ms: 2750.0,
                pitch: 64
            },
        ]
    );
}

#[test]
fn the_ghost_notes_are_empty_without_a_patch() {
    let (_dir, doc, _song) = loaded();
    assert!(
        preview_notes(
            &doc,
            &usdx_edit::Patch {
                label: String::from("empty"),
                ops: Vec::new(),
            }
        )
        .is_empty()
    );
}

#[test]
fn selection_lines_maps_the_selected_notes_to_their_global_lines() {
    let (_dir, _doc, song) = loaded();
    // Line 0's first note: the selection covers exactly line 0.
    let mut selection = BTreeSet::new();
    selection.insert(song.voices[0].lines[0].notes[0]);
    assert_eq!(selection_lines(&song, &selection), Some((0, 0)));
    // A note of line 2 widens the range to 0..=2.
    selection.insert(song.voices[0].lines[2].notes[2]);
    assert_eq!(selection_lines(&song, &selection), Some((0, 2)));
    // An empty selection: none.
    assert_eq!(selection_lines(&song, &BTreeSet::new()), None);
    // A duet selection spanning both voices is refused (the block
    // needs one voice).
    let duet = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n",
        "P2\n: 8 4 0 b \n- 12\nE\n",
    );
    let (dir, path) = fixture(duet);
    let doc2 = classify(
        decode(&std::fs::read(&path).expect("the fixture is read")).expect("the song decodes"),
    );
    let song2 = Song::from_document(&doc2);
    let mut mixed = BTreeSet::new();
    mixed.insert(song2.voices[0].lines[0].notes[0]);
    mixed.insert(song2.voices[1].lines[0].notes[0]);
    assert_eq!(selection_lines(&song2, &mixed), None);
    drop(dir);
}

#[test]
fn the_line_range_maps_global_lines_to_one_voice() {
    let duet = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n",
        "P2\n: 8 4 0 b \n- 12\nE\n",
    );
    let (dir, path) = fixture(duet);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song opens");
    assert_eq!(line_range_target(&controller, 0, 0), Some((0, 0, 0)));
    assert_eq!(
        line_range_target(&controller, 1, 1),
        Some((1, 0, 0)),
        "global line 1 is the second voice's line 0"
    );
    assert_eq!(
        line_range_target(&controller, 0, 1),
        None,
        "a range across both voices is refused"
    );
    drop(dir);
}

/// loaded_song builds a LoadedSong around the tight fixture the way the
/// controller's open does.
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

#[test]
fn the_export_preview_counts_changes_pins_and_problems() {
    let (dir, mut loaded_song) = loaded_song();
    let mut repair = RepairController::new();
    // The pin and the end pin stretch the whole voice, so all three
    // lines export; line 0 carries its pinned time (a pin wins over a
    // change).
    repair.pin_current_line_at(0, 500.0);
    repair.set_end_pin(Some(5000.0));
    repair
        .apply(&mut loaded_song, 0)
        .expect("the apply commits");

    let preview = export_preview(&repair, &loaded_song, "T", "0.1.0", "2026-11-16", None, &[]);
    assert_eq!(preview.entries.len(), 3);
    assert_eq!(preview.pinned, 1, "line 0's entry comes from the pin");
    assert_eq!(preview.end, Some(5000.0));
    assert!(preview.text.contains("0:00.50 | a b c\n"));
    assert!(preview.text.ends_with("end 0:05.00\n"));
    assert!(preview.problems.is_empty());

    // Leaving the pinned line out: the file renders without it, the
    // counts follow and the end stays.
    let preview = export_preview(
        &repair,
        &loaded_song,
        "T",
        "0.1.0",
        "2026-11-16",
        None,
        &[0],
    );
    assert_eq!(preview.entries.len(), 2);
    assert_eq!(preview.pinned, 0);
    assert!(!preview.text.contains("a b c"));
    assert!(preview.text.ends_with("end 0:05.00\n"));

    // The Factory-matching check against lyrics.txt: "g h i" is not in
    // the lyrics at all, so it warns; "a b c" and "d e f" bind to the
    // same occurrences as the song lines they came from.
    std::fs::write(dir.path().join("lyrics.txt"), "a b c\nd e f\na b c again\n")
        .expect("the lyrics are written");
    let lyrics = lyrics_lines(dir.path()).expect("the lyrics exist");
    let preview = export_preview(
        &repair,
        &loaded_song,
        "T",
        "0.1.0",
        "2026-11-16",
        Some(&lyrics),
        &[],
    );
    assert!(
        preview
            .problems
            .iter()
            .all(|problem| problem.message.contains("binds to lyrics line")
                || problem.message.contains("not found")),
        "the problems are the Factory-matching warnings: {:?}",
        preview.problems
    );
    assert_eq!(preview.problems.len(), 1, "g h i is not in the lyrics");
    drop(dir);
}

#[test]
fn the_export_preview_reports_a_render_error_apart_from_the_problems() {
    let (_dir, loaded_song) = loaded_song();
    let mut repair = RepairController::new();
    // The pin is exported at 2000 ms; an end pin below the last anchor
    // breaks the render's rules (the Factory needs increasing times).
    repair.pin_current_line_at(0, 2000.0);
    repair.set_end_pin(Some(500.0));
    let preview = export_preview(&repair, &loaded_song, "T", "0.1.0", "2026-11-16", None, &[]);
    assert!(preview.text.is_empty());
    // The render error is no line's problem: "Leave it out" cannot solve
    // it, so it comes apart from the per-line findings.
    assert!(preview.problems.is_empty());
    assert!(
        preview
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("cannot export")),
        "{:?}",
        preview.error
    );
}

#[test]
fn overview_lines_reports_the_line_spans_and_health_verdicts() {
    // The middle line packs twelve 1-beat notes into the same 12-beat
    // span the neighbours use: its rate is 16 notes/s (racing) against
    // the neighbours' 4 notes/s, so the neighbour factor races too.
    let racing = concat!(
        "#TITLE:T\n#BPM:240\n",
        ": 0 4 0 a \n: 4 4 2 b \n: 8 4 4 c \n- 12\n",
        ": 12 1 0 d1 \n: 13 1 2 d2 \n: 14 1 4 d3 \n: 15 1 0 d4 \n",
        ": 16 1 2 d5 \n: 17 1 4 d6 \n: 18 1 0 d7 \n: 19 1 2 d8 \n",
        ": 20 1 4 d9 \n: 21 1 0 d10 \n: 22 1 2 d11 \n: 23 1 4 d12 \n- 24\n",
        ": 24 4 0 g \n: 28 4 2 h \n: 32 4 4 i \nE\n",
    );
    let (dir, path) = fixture(racing);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    // The spans follow the lines pane's rule: the first note's start to
    // the phrase end or the last note's end (BPM 240: one beat 62.5 ms).
    // The health maps onto the QML convention (0 ok, 1 suspicious,
    // 2 racing); the analysis runs without vocal activity, so the
    // coverage signal stays neutral and rate and factor decide.
    assert_eq!(
        overview_lines(&doc, &song, 0, &HealthBasis::plain()),
        vec![
            OverviewLine {
                line: 0,
                start_ms: 0.0,
                end_ms: 750.0,
                health: 0
            },
            OverviewLine {
                line: 1,
                start_ms: 750.0,
                end_ms: 1500.0,
                health: 2
            },
            OverviewLine {
                line: 2,
                start_ms: 1500.0,
                end_ms: 2250.0,
                health: 0
            },
        ]
    );
    drop(dir);
}

#[test]
fn overview_lines_is_empty_without_a_usable_timing_or_voice() {
    let (_dir, doc, song) = loaded();
    assert!(
        overview_lines(&doc, &song, 7, &HealthBasis::plain()).is_empty(),
        "unknown voice"
    );
    let mut no_bpm = doc.clone();
    if let Some(row) = no_bpm.rows.iter_mut().find(
        |row| matches!(&row.kind, usdx_format::rows::RowKind::Header { key, .. } if key == "BPM"),
    ) {
        row.kind = usdx_format::rows::RowKind::Invalid {
            reason: String::from("no bpm"),
        };
    }
    let song = Song::from_document(&no_bpm);
    assert!(
        overview_lines(&no_bpm, &song, 0, &HealthBasis::plain()).is_empty(),
        "no timing"
    );
}

#[test]
fn racing_ranges_reports_the_racing_sections_as_inclusive_ranges() {
    // Two racing sections with a calm line between them: the ranges
    // stay separate; the strip underlays each with the negative colour.
    let racing = concat!(
        "#TITLE:T\n#BPM:240\n",
        ": 0 1 0 a1 \n: 1 1 2 a2 \n: 2 1 4 a3 \n- 3\n",
        ": 3 4 0 b \n: 7 4 2 c \n: 11 4 4 d \n- 15\n",
        ": 15 1 0 e1 \n: 16 1 2 e2 \n: 17 1 4 e3 \nE\n",
    );
    let (dir, path) = fixture(racing);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    assert_eq!(
        racing_ranges(&doc, &song, 0, &HealthBasis::plain()),
        vec![(0, 0), (2, 2)]
    );
    drop(dir);
}

#[test]
fn repair_section_reports_the_racing_section_in_global_line_numbers() {
    // The second voice's middle line packs twelve 1-beat notes into its
    // 12-beat span: it races against the calm lines around it (16 notes/s
    // against the neighbourhood median 4 notes/s). P1 has two lines, so
    // the section's global numbers are 3 to 3 (0-based).
    let racing = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n: 4 4 2 b \n- 8\n",
        "P2\n: 8 4 0 c \n- 12\n",
        ": 12 1 0 d1 \n: 13 1 2 d2 \n: 14 1 4 d3 \n: 15 1 0 d4 \n",
        ": 16 1 2 d5 \n: 17 1 4 d6 \n: 18 1 0 d7 \n: 19 1 2 d8 \n",
        ": 20 1 4 d9 \n: 21 1 0 d10 \n: 22 1 2 d11 \n: 23 1 4 d12 \n- 24\n",
        ": 24 4 0 e \n: 28 4 2 f \n: 32 4 4 g \nE\n",
    );
    let (dir, path) = fixture(racing);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let section = repair_section(&doc, &song, 1, 1, &HealthBasis::plain());
    assert_eq!((section.first, section.last, section.health), (3, 3, 2));
    assert!((section.factor - 4.0).abs() < 1e-9);
    // The analysis runs without the vocal activity for now, so the
    // coverage stays unknown.
    assert_eq!(section.coverage, None);
    // A calm target line sits in no racing section: the span stays -1
    // and the verdict is the line's own. Its factor is its rate against
    // the neighbourhood median (4 notes/s against the median of 4 and
    // 16, so 0.4).
    let calm = repair_section(&doc, &song, 1, 0, &HealthBasis::plain());
    assert_eq!((calm.first, calm.last, calm.health), (-1, -1, 0));
    assert!((calm.factor - 0.4).abs() < 1e-9);
    let unknown = repair_section(&doc, &song, 1, 7, &HealthBasis::plain());
    assert_eq!((unknown.first, unknown.last, unknown.health), (-1, -1, -1));
    assert!((unknown.factor - 0.0).abs() < 1e-9);
    drop(dir);
}

#[test]
fn repair_section_reports_no_section_without_a_usable_timing_or_voice() {
    let (_dir, doc, song) = loaded();
    let section = repair_section(&doc, &song, 0, 0, &HealthBasis::plain());
    assert_eq!((section.first, section.last, section.health), (-1, -1, 0));
    let none = repair_section(&doc, &song, 7, 0, &HealthBasis::plain());
    assert_eq!((none.first, none.last, none.health), (-1, -1, -1));
}

#[test]
fn overview_anchor_marks_pair_the_pinned_times_with_the_badges() {
    let rows = [
        AnchorRow {
            badge: 0,
            line_number: 1,
            text: String::from("a"),
            old_ms: 500.0,
            target_ms: 500.0,
        },
        AnchorRow {
            badge: 1,
            line_number: 3,
            text: String::from("b"),
            old_ms: 2000.0,
            target_ms: 2000.0,
        },
    ];
    assert_eq!(overview_anchor_marks(&rows), [(500.0, 0), (2000.0, 1)]);
}

#[test]
fn overview_lines_carry_their_voice_local_line_index() {
    let (_dir, doc, song) = loaded();
    let lines = overview_lines(&doc, &song, 0, &HealthBasis::plain());
    assert_eq!(lines.len(), 3);
    for (index, line) in lines.iter().enumerate() {
        assert_eq!(line.line, index, "the lines are in order");
    }
}

#[test]
fn section_end_ms_reports_the_old_and_the_previewed_new_end() {
    // The racing fixture: the section is global line 3 = voice-local
    // line 1, spanning beats 8 to 24 (500 ms to 1500 ms).
    let racing = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n: 4 4 2 b \n- 8\n",
        "P2\n: 8 4 0 c \n- 12\n",
        ": 12 1 0 d1 \n: 13 1 2 d2 \n: 14 1 4 d3 \n: 15 1 0 d4 \n",
        ": 16 1 2 d5 \n: 17 1 4 d6 \n: 18 1 0 d7 \n: 19 1 2 d8 \n",
        ": 20 1 4 d9 \n: 21 1 0 d10 \n: 22 1 2 d11 \n: 23 1 4 d12 \n- 24\n",
        ": 24 4 0 e \n: 28 4 2 f \n: 32 4 4 g \nE\n",
    );
    let (dir, path) = fixture(racing);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let mut repair = RepairController::new();
    // Pin the section's line (voice-local 1) at 500 ms and the line
    // after the section (voice-local 2) at 4000 ms: the section's 12
    // beats stretch into the span between the pins, so its end moves
    // well past the old 1500 ms.
    repair.pin_current_line_at(1, 500.0);
    repair.pin_current_line_at(2, 4000.0);
    let patch = repair
        .preview(&doc, &song, 1)
        .expect("the preview plans")
        .clone();
    let (old_end, new_end) = section_end_ms(&doc, &song, 1, 1, Some(&patch));
    assert_eq!(
        old_end, 1500.0,
        "the section ends after the last 1-beat note"
    );
    assert!(
        new_end > old_end,
        "the previewed end stretches: {old_end} -> {new_end}"
    );
    // Without a preview only the old end is known, so the new end
    // repeats it.
    let (old_end, new_end) = section_end_ms(&doc, &song, 1, 1, None);
    assert_eq!(old_end, 1500.0);
    assert_eq!(new_end, 1500.0);
    drop(dir);
}

#[test]
fn shortened_notes_counts_the_replaces_that_shorten_a_note() {
    // The rigid shift of the anchor preview never shortens: every note
    // keeps its 4-beat length.
    let (_dir, doc, song) = loaded();
    let mut repair = RepairController::new();
    repair.pin_current_line_at(0, 500.0);
    repair.pin_current_line_at(2, 2000.0);
    let patch = repair
        .preview(&doc, &song, 0)
        .expect("the preview plans")
        .clone();
    assert_eq!(shortened_notes(&patch), 0);
    // A patch with two shrunk replaces counts two (the Replace rows carry
    // the before and after notes).
    let mut shrunk = Patch {
        label: String::new(),
        ops: Vec::new(),
    };
    for (row_id, new_length) in [(2, 1), (6, 2)] {
        let before = doc.rows[row_id].clone();
        let RowKind::Note(mut after) = before.kind.clone() else {
            unreachable!()
        };
        after.length = new_length;
        let id = before.id;
        let raw = before.raw.clone();
        let after = usdx_format::rows::Row {
            id,
            raw,
            kind: RowKind::Note(after),
            dirty: false,
        };
        shrunk.ops.push(RowOp::Replace { id, before, after });
    }
    assert_eq!(shortened_notes(&shrunk), 2);
}

#[test]
fn block_span_reports_the_note_span_and_count() {
    let (_dir, doc, song) = loaded();
    // Lines 0..=1: the notes span beats 0..24 (the last note of line 1
    // ends at beat 24); BPM 240 with no #GAP makes 62.5 ms per beat.
    let span = block_span(&doc, &song, 0, 0, 1).expect("the block has a span");
    assert_eq!(
        span,
        BlockSpan {
            start_ms: 0.0,
            end_ms: 1500.0,
            notes: 6,
        }
    );
    // The whole voice: beats 0..36.
    let span = block_span(&doc, &song, 0, 0, 2).expect("the block has a span");
    assert_eq!(span.start_ms, 0.0);
    assert_eq!(span.end_ms, 2250.0);
    assert_eq!(span.notes, 9);
    // One line in the middle: its own notes' span.
    let span = block_span(&doc, &song, 0, 1, 1).expect("the block has a span");
    assert_eq!(span.start_ms, 750.0);
    assert_eq!(span.end_ms, 1500.0);
    // Past the voice's lines, or a voice the song does not have: none.
    assert_eq!(block_span(&doc, &song, 0, 0, 5), None);
    assert_eq!(block_span(&doc, &song, 3, 0, 1), None);
}

#[test]
fn the_follow_toggle_maps_the_i32_both_ways() {
    // The qenum's declaration order: Stay first (the UI_SPEC's button
    // order), MoveAlong second.
    assert_eq!(follow_i32(Follow::Stay), 0);
    assert_eq!(follow_i32(Follow::MoveAlong), 1);
    assert_eq!(follow_of(0), Some(Follow::Stay));
    assert_eq!(follow_of(1), Some(Follow::MoveAlong));
    assert_eq!(follow_of(2), None);
}

#[test]
fn the_ghost_hash_carries_row_times_and_pitch() {
    // The repair sheet places each dashed ghost at its pitch row; a hash
    // without the pitch put every ghost in the ruler.
    let hash = preview_note_hash(&PreviewNote {
        row_id: 7,
        start_ms: 1000.0,
        end_ms: 1250.0,
        pitch: 64,
    });
    let value = |key: &str| hash.get(&cxx_qt_lib::QString::from(key));
    assert_eq!(value("rowId").and_then(|v| v.value::<i64>()), Some(7));
    assert_eq!(value("start").and_then(|v| v.value::<f64>()), Some(1000.0));
    assert_eq!(value("end").and_then(|v| v.value::<f64>()), Some(1250.0));
    assert_eq!(value("pitch").and_then(|v| v.value::<i32>()), Some(64));
}

#[test]
fn racing_note_rows_lists_the_notes_of_the_racing_lines() {
    // The repair sheet draws the notes of racing lines in the negative
    // colour (UI_SPEC 4.6): the rows of lines 0 and 2 of the fixture.
    let racing = concat!(
        "#TITLE:T\n#BPM:240\n",
        ": 0 1 0 a1 \n: 1 1 2 a2 \n: 2 1 4 a3 \n- 3\n",
        ": 3 4 0 b \n: 7 4 2 c \n: 11 4 4 d \n- 15\n",
        ": 15 1 0 e1 \n: 16 1 2 e2 \n: 17 1 4 e3 \nE\n",
    );
    let (dir, path) = fixture(racing);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    assert_eq!(
        racing_note_rows(&doc, &song, 0, &HealthBasis::plain()),
        vec![2, 3, 4, 10, 11, 12]
    );
    assert!(
        racing_note_rows(&doc, &song, 5, &HealthBasis::plain()).is_empty(),
        "unknown voice"
    );
    drop(dir);
}

#[test]
fn tap_lines_lists_the_session_lines_with_syllables_and_note_times() {
    // The tap-along page shows the current line's syllables and the next
    // line, and the lane draws the notes; the lines are the session's
    // global range. BPM 240 without #GAP: beat b sits at 62.5 b ms.
    let text = concat!(
        "#TITLE:T\n#BPM:240\n",
        "P1\n: 0 4 0 a \n- 4\n: 4 4 2 Hel\n: 8 2 2 lo \n- 12\n",
        "P2\n: 12 4 0 Wei\n: 16 4 0 ~ \nE\n",
    );
    let (dir, path) = fixture(text);
    let doc = classify(
        decode(&std::fs::read(&path).expect("the song is read")).expect("the song decodes"),
    );
    let song = Song::from_document(&doc);
    let lines = tap_lines(&doc, &song, 1, 2);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line, 1);
    assert_eq!(lines[0].syllables, vec!["Hel".to_owned(), "lo".to_owned()]);
    assert_eq!(lines[0].starts_ms, vec![250.0, 500.0]);
    assert_eq!(lines[0].ends_ms, vec![500.0, 625.0]);
    // "Hel" continues the word, "lo " ends it: the page puts a space after
    // a word's last syllable only.
    assert_eq!(lines[0].word_ends, vec![false, true]);
    // The global line 2 is the second voice's first line.
    assert_eq!(lines[1].line, 2);
    assert_eq!(lines[1].syllables, vec!["Wei".to_owned(), "~".to_owned()]);
    assert!(tap_lines(&doc, &song, 5, 7).is_empty(), "past the lines");
    drop(dir);
}

#[test]
fn the_default_export_target_is_anchors_txt_next_to_the_song() {
    // UI_SPEC 5.5 / plan 6.11: the save dialog starts at
    // <song folder>/anchors.txt.
    assert_eq!(
        default_anchors_path(std::path::Path::new("/songs/Artist - Title/song.txt")),
        std::path::PathBuf::from("/songs/Artist - Title/anchors.txt")
    );
    assert_eq!(
        default_anchors_path(std::path::Path::new("song.txt")),
        std::path::PathBuf::from("anchors.txt")
    );
}
