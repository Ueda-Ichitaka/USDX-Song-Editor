//! About me: tests for the repair bridge logic of task P7-08 (plan
//! sections 8.3, 6.11; UI_SPEC 4.6, 4.8). The QObject declarations are
//! thin wrappers; the logic they mirror lives in `bridge/repair.rs` and
//! is tested here without Qt: the anchor card rows the AnchorPanel
//! shows (badge, line number, line text, old and pinned time), the
//! anchors.txt file write behind `exportAnchors`, the lyrics.txt lines
//! the export dialog's Factory-matching check needs, the click-track
//! WAV writer the latency calibration plays, and the current line's
//! repair target (the voice and voice-local line `pinCurrentLineAt`
//! pins). The calibration WAV is 8 clicks at 1 s spacing: 16-bit mono
//! PCM with a 44-byte header, no new dependency.

use tempfile::TempDir;
use usdx_edit::anchors_export::AnchorEntry;
use usdx_edit::retime::{Anchor, Pin};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, classify};
use usdx_format::song::Song;
use usdx_song_editor::bridge::repair::{
    AnchorRow, anchor_rows, click_track_wav, current_repair_target, export_anchors, lyrics_lines,
};
use usdx_song_editor::controller::EditorController;

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
