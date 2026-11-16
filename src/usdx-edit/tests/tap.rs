//! About me: tests for the tap mapping (task P7-04, plan section 6.9).
//! Line mode pins every line in the range to its tap's time as a rigid
//! shift (user decision 2026-11-16 "Keep outside, keep lengths": the lines
//! before the first and after the last tapped line keep their place, a
//! line's notes follow its start rigidly, so only the line starts move).
//! Syllable mode sets every tapped note's start to its tap's time, with
//! the held length `up - down` and the unheld length
//! `min(old length * local factor, next tap - 1 beat)` (the local factor is
//! the ratio of the tapped span to the old span of the same notes); with
//! fewer taps than notes the notes after the last tap stretch between the
//! last tap and the next unchanged note. The fixtures use BPM 300 (one
//! beat is 50 ms) and GAP 1000 (beat 0 at 1 s), so
//! `ms(beat) = 1000 + beat * 50`.

use std::ops::Range;

use usdx_edit::EditError;
use usdx_edit::tap::{Tap, apply_line_taps, apply_syllable_taps};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowId, RowKind, classify};
use usdx_format::serialize::serialize;
use usdx_format::song::Song;

/// loaded classifies a song text.
fn loaded(text: &str) -> (Document, Song) {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    (doc, song)
}

/// ms_of returns the millisecond position of a beat in the fixtures.
fn ms_of(beat: i64) -> f64 {
    1000.0 + beat as f64 * 50.0
}

/// tap_at builds an unheld tap at a beat's time.
fn tap_at(beat: i64) -> Tap {
    Tap {
        down_ms: ms_of(beat),
        up_ms: None,
    }
}

/// held_at builds a tap held from one beat to another.
fn held_at(down: i64, up: i64) -> Tap {
    Tap {
        down_ms: ms_of(down),
        up_ms: Some(ms_of(up)),
    }
}

/// standard builds the standard fixture: `lines` lines, line i with two
/// 4-beat notes at beats i*12 and i*12+6 (texts a1/a2, b1/b2, ...) and a
/// phrase end at i*12+11 for every line but the last. Line i's first note
/// starts at ms(12 i).
fn standard(lines: usize) -> String {
    let mut text = String::from("#TITLE:T\n#BPM:300\n#GAP:1000\n");
    for i in 0..lines {
        let letter = char::from(b'a' + i as u8);
        text += &format!(
            ": {} 4 0 {letter}1 \n: {} 4 0 {letter}2 \n",
            i * 12,
            i * 12 + 6
        );
        if i + 1 < lines {
            text += &format!("- {}\n", i * 12 + 11);
        }
    }
    text += "E\n";
    text
}

/// syllable builds a one-line fixture with 4-beat notes at beats 0, 4, 8
/// and 12 (texts a to d) and no phrase end.
fn syllable() -> String {
    "#TITLE:T\n#BPM:300\n#GAP:1000\n\
     : 0 4 0 a \n: 4 4 0 b \n: 8 4 0 c \n: 12 4 0 d \nE\n"
        .to_owned()
}

/// tail_fixture builds a two-line fixture: line 0 holds 4-beat notes at
/// beats 0, 4, 8 and 12 with the phrase end at 18, line 1 notes at 20 and
/// 24 (the next unchanged note for a syllable tail).
fn tail_fixture() -> String {
    "#TITLE:T\n#BPM:300\n#GAP:1000\n\
     : 0 4 0 a \n: 4 4 0 b \n: 8 4 0 c \n: 12 4 0 d \n- 18\n\
     : 20 4 0 e \n: 24 4 0 f \nE\n"
        .to_owned()
}

/// applied runs a line tap mapping and returns the written text.
fn line_applied(text: &str, lines: Range<usize>, taps: &[Tap]) -> String {
    let (doc, song) = loaded(text);
    let patch = apply_line_taps(&doc, &song, 0, lines, taps).expect("the taps plan");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    String::from_utf8(serialize(&out).expect("the song serializes")).expect("utf-8")
}

/// syllable_applied runs a syllable tap mapping over the voice's first
/// `count` notes and returns the written text.
fn syllable_applied(text: &str, count: usize, taps: &[Tap]) -> String {
    let (doc, song) = loaded(text);
    let notes = note_ids(&doc);
    let patch = apply_syllable_taps(&doc, &song, 0, &notes[..count], taps).expect("the taps plan");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    String::from_utf8(serialize(&out).expect("the song serializes")).expect("utf-8")
}

/// note_ids lists every note row id in row order.
fn note_ids(doc: &Document) -> Vec<RowId> {
    doc.rows
        .iter()
        .enumerate()
        .filter(|(_, row)| matches!(row.kind, RowKind::Note(_)))
        .map(|(index, _)| RowId(index as u32))
        .collect()
}

/// spans lists `(start, length)` of every note of voice 0 in row order.
fn spans(text: &str) -> Vec<(i64, i64)> {
    let (doc, song) = loaded(text);
    song.voices[0]
        .lines
        .iter()
        .flat_map(|line| &line.notes)
        .filter_map(|id| song.note(&doc, *id))
        .map(|note| (note.start as i64, note.length))
        .collect()
}

/// markers lists every phrase end beat of voice 0 in row order.
fn markers(text: &str) -> Vec<i64> {
    let (doc, song) = loaded(text);
    song.voices[0]
        .lines
        .iter()
        .filter_map(|line| line.end)
        .filter_map(|id| song.phrase_end_beat(&doc, id))
        .map(|beat| beat as i64)
        .collect()
}

/// line_error_of runs a line tap mapping and returns the error it refuses
/// with.
fn line_error_of(text: &str, lines: Range<usize>, taps: &[Tap]) -> EditError {
    let (doc, song) = loaded(text);
    apply_line_taps(&doc, &song, 0, lines, taps).expect_err("the taps refuse")
}

/// syllable_error_of runs a syllable tap mapping and returns the error it
/// refuses with.
fn syllable_error_of(text: &str, notes: &[RowId], taps: &[Tap]) -> EditError {
    let (doc, song) = loaded(text);
    apply_syllable_taps(&doc, &song, 0, notes, taps).expect_err("the taps refuse")
}

#[test]
fn line_taps_shift_every_tapped_line_rigidly_and_keep_the_rest() {
    // Two taps for lines 1 and 2 at beats 13 and 27 (old starts 12 and
    // 24): the lines shift rigidly by +1 and +3 beats, the lines before
    // and after keep their place, the lengths keep their beat count. Line
    // 2's last note then ends at 37, inside line 3's untouched first note
    // at 36, so the pass shortens it to 3 beats.
    let text = standard(5);
    let out = line_applied(&text, 1..3, &[tap_at(13), tap_at(27)]);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (13, 4),
            (19, 4),
            (27, 4),
            (33, 3),
            (36, 4),
            (42, 4),
            (48, 4),
            (54, 4),
        ],
        "lines 1 and 2 shift rigidly, line 2's last note shortens to beat 36"
    );
    // The recomputed markers keep their 0.5 fractions of the new gaps.
    assert_eq!(markers(&out), vec![12, 25, 36, 47]);
}

#[test]
fn line_taps_below_beat_zero_ask_for_a_gap_shift() {
    // A tap that pulls the first tapped line below beat 0 cannot be
    // written: the pass reports the #GAP shift.
    let text = standard(2);
    let error = line_error_of(
        &text,
        0..1,
        &[Tap {
            down_ms: 1000.0 - 2.0 * 50.0,
            up_ms: None,
        }],
    );
    assert!(matches!(error, EditError::NeedsGapShift(_)));
}

#[test]
fn line_taps_refuse_a_count_mismatch_and_bad_times() {
    let text = standard(3);
    assert!(matches!(
        line_error_of(&text, 1..3, &[tap_at(13)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        line_error_of(&text, 1..3, &[tap_at(13), tap_at(27), tap_at(39)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        line_error_of(&text, 1..1, &[tap_at(13)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        line_error_of(&text, 0..9, &[tap_at(13)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        line_error_of(
            &text,
            1..2,
            &[Tap {
                down_ms: f64::NAN,
                up_ms: None,
            }]
        ),
        EditError::InvalidHeaderValue
    ));
    assert!(matches!(
        line_error_of(
            &text,
            1..2,
            &[Tap {
                down_ms: ms_of(13),
                up_ms: Some(f64::INFINITY),
            }]
        ),
        EditError::InvalidHeaderValue
    ));
    assert!(matches!(
        line_error_of(
            &text,
            1..2,
            &[Tap {
                down_ms: ms_of(13),
                up_ms: Some(ms_of(12)),
            }]
        ),
        EditError::InvalidHeaderValue
    ));
}

#[test]
fn line_taps_without_timing_or_relative_are_refused() {
    let no_timing = "#TITLE:T\n: 0 4 0 a \nE\n";
    assert!(matches!(
        line_error_of(no_timing, 0..1, &[tap_at(0)]),
        EditError::NoTiming
    ));
    let relative = "#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 la\n: 4 4 1 la\n- 8 1\nE\n";
    assert!(matches!(
        line_error_of(relative, 0..1, &[tap_at(0)]),
        EditError::RelativeModeIsReadOnly
    ));
}

#[test]
fn syllable_taps_set_starts_with_held_and_unheld_lengths() {
    // Four taps at beats 2, 5, 9 and 14 (the old starts span 12 beats, the
    // taps 12 beats, so the local factor is 1): the first two notes are
    // unheld and take the bound to the next tap minus 1 beat, the third is
    // held from beat 9 to 13, the last is unheld with no bound after it.
    let text = syllable();
    let out = syllable_applied(
        &text,
        4,
        &[tap_at(2), tap_at(5), held_at(9, 13), tap_at(14)],
    );
    assert_eq!(
        spans(&out),
        vec![(2, 2), (5, 3), (9, 4), (14, 4)],
        "held length up - down, unheld length min(old, next tap - 1 beat)"
    );
    assert_eq!(markers(&out), Vec::<i64>::new());
}

#[test]
fn syllable_taps_scale_unheld_lengths_with_the_local_factor() {
    // Four taps at beats 0, 2, 4 and 6 over old starts 0, 4, 8, 12: the
    // local factor is 0.5, so every unheld length is min(2, next tap - 1
    // beat) = 1 and the last note's length is 2.
    let text = syllable();
    let out = syllable_applied(&text, 4, &[tap_at(0), tap_at(2), tap_at(4), tap_at(6)]);
    assert_eq!(
        spans(&out),
        vec![(0, 1), (2, 1), (4, 1), (6, 2)],
        "the unheld lengths scale with the 0.5 factor"
    );
}

#[test]
fn fewer_taps_stretch_the_tail_between_the_last_tap_and_the_next_note() {
    // Two taps for the first two notes at beats 0 and 6 (old span 4 beats,
    // so the local factor is 1.5); notes 2 and 3 stretch between the last
    // tap (6) and the next unchanged note (line 1's first note at 20):
    // their starts map from the old span 4..20 onto 6..20. The last
    // tapped note's length is bounded by the tail's first start.
    let text = tail_fixture();
    let out = syllable_applied(&text, 4, &[tap_at(0), tap_at(6)]);
    assert_eq!(
        spans(&out),
        vec![(0, 5), (6, 3), (10, 3), (13, 4), (20, 4), (24, 4)],
        "the tail stretches between the last tap and beat 20"
    );
    // The phrase end keeps its 0.5 fraction of the new gap 17..20.
    assert_eq!(markers(&out), vec![19]);
}

#[test]
fn syllable_taps_refuse_mismatched_counts_and_bad_times() {
    let text = syllable();
    let notes = note_ids(&loaded(&text).0);
    assert!(matches!(
        syllable_error_of(
            &text,
            &notes,
            &[tap_at(0), tap_at(2), tap_at(4), tap_at(6), tap_at(8)]
        ),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        syllable_error_of(&text, &notes, &[]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        syllable_error_of(&text, &notes[..2], &[tap_at(0), tap_at(2), tap_at(4)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        syllable_error_of(
            &text,
            &notes,
            &[Tap {
                down_ms: f64::NAN,
                up_ms: None,
            }]
        ),
        EditError::InvalidHeaderValue
    ));
    assert!(matches!(
        syllable_error_of(&text, &notes, &[held_at(4, 2)]),
        EditError::InvalidHeaderValue
    ));
    assert!(matches!(
        syllable_error_of(&text, &[RowId(9999)], &[tap_at(0)]),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        syllable_error_of(&text, &[], &[tap_at(0)]),
        EditError::StalePatch(_)
    ));
}

#[test]
fn syllable_taps_without_timing_or_relative_are_refused() {
    let notes = note_ids(&loaded(&syllable()).0);
    let no_timing = "#TITLE:T\n: 0 4 0 a \nE\n";
    assert!(matches!(
        syllable_error_of(no_timing, &notes, &[tap_at(0)]),
        EditError::NoTiming
    ));
    let relative = "#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 la\n: 4 4 1 la\n- 8 1\nE\n";
    let relative_notes = note_ids(&loaded(relative).0);
    assert!(matches!(
        syllable_error_of(relative, &relative_notes, &[tap_at(0)]),
        EditError::RelativeModeIsReadOnly
    ));
}
