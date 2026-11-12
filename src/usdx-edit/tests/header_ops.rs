//! About me: tests for the header operations (task P2-04, plan section 6.4).
//! They assert that set_header rewrites a value case-insensitively and adds
//! missing headers before the body, that remove_header is idempotent, that
//! set_gap_ms changes only the GAP row and that set_bpm without keep_times
//! touches only the BPM row while keep_times rescales every beat so the
//! notes stay at their audio time within half a beat.

use usdx_edit::header_ops::{remove_header, set_bpm, set_gap_ms, set_header};
use usdx_edit::{EditError, Patch};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;

/// doc classifies a decoded text.
fn doc(text: &str) -> Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

/// song builds the song view of a document.
fn song(d: &Document) -> Song {
    Song::from_document(d)
}

/// applied builds the patch, applies it to a copy and returns the copy.
fn applied(result: Result<Patch, EditError>, d: &Document) -> Document {
    let patch = result.expect("the operation builds a patch");
    let mut copy = d.clone();
    patch
        .apply(&mut copy)
        .unwrap_or_else(|error| panic!("the patch does not apply: {error}"));
    copy
}

/// header_value reads the value of a header row by its upper-case key.
fn header_value(d: &Document, key: &str) -> Option<String> {
    d.rows.iter().find_map(|row| match &row.kind {
        RowKind::Header { key: found, value } if found.as_str() == key => Some(value.clone()),
        _ => None,
    })
}

/// header_raw reads the raw text of a header row by its upper-case key.
fn header_raw(d: &Document, key: &str) -> Option<String> {
    d.rows.iter().find_map(|row| match &row.kind {
        RowKind::Header { key: found, .. } if found.as_str() == key => Some(row.raw.text.clone()),
        _ => None,
    })
}

/// dirty_rows lists the ids of the dirty rows.
fn dirty_rows(d: &Document) -> Vec<u32> {
    d.rows
        .iter()
        .filter(|row| row.dirty)
        .map(|row| row.id.0)
        .collect()
}

/// note_starts lists the start beats of the note rows.
fn note_starts(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.start),
            _ => None,
        })
        .collect()
}

/// note_lengths lists the lengths of the note rows.
fn note_lengths(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.length),
            _ => None,
        })
        .collect()
}

/// phrase_ends lists the beats of the phrase-end rows.
fn phrase_ends(d: &Document) -> Vec<i64> {
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::PhraseEnd {
                beat: Some(beat), ..
            } => Some(*beat),
            _ => None,
        })
        .collect()
}

/// audio_times lists the millisecond positions of all notes and phrase ends.
fn audio_times(d: &Document) -> Vec<f64> {
    let timing = song(d).timing.expect("the song has a timing");
    d.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some(timing.beat_to_ms(note.start as f64)),
            RowKind::PhraseEnd {
                beat: Some(beat), ..
            } => Some(timing.beat_to_ms(*beat as f64)),
            _ => None,
        })
        .collect()
}

/// a song with a title, a tempo, a gap and two lines.
const SONG: &str = "#TITLE:T\n#BPM:240\n#GAP:500\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

// ---- set_header ----

#[test]
fn set_header_rewrites_the_value_of_an_existing_header() {
    let d = doc(SONG);
    let out = applied(set_header(&d, "title", "New"), &d);
    assert_eq!(header_value(&out, "TITLE").as_deref(), Some("New"));
    // The raw key spelling before the colon is kept.
    assert_eq!(header_raw(&out, "TITLE").as_deref(), Some("#TITLE:New"));
    assert_eq!(dirty_rows(&out), vec![0]);
}

#[test]
fn set_header_matches_the_key_case_insensitively() {
    let d = doc(SONG);
    let out = applied(set_header(&d, "bpm", "120"), &d);
    assert_eq!(header_value(&out, "BPM").as_deref(), Some("120"));
    assert_eq!(header_raw(&out, "BPM").as_deref(), Some("#BPM:120"));
    assert_eq!(dirty_rows(&out), vec![1]);
}

#[test]
fn set_header_adds_a_missing_header_before_the_body() {
    let d = doc(SONG);
    let out = applied(set_header(&d, "language", "German"), &d);
    assert_eq!(header_value(&out, "LANGUAGE").as_deref(), Some("German"));
    // A new row carries no raw line yet; the serializer writes the canonical
    // text for dirty rows and gives the row the dominant terminator.
    let language = out
        .rows
        .iter()
        .find(|row| row.id.0 == 3)
        .expect("the new row");
    assert_eq!(language.raw.text, "");
    assert_eq!(
        dirty_rows(&out),
        vec![3],
        "the new row sits before the notes"
    );
}

// ---- remove_header ----

#[test]
fn remove_header_takes_the_row_out() {
    let d = doc(SONG);
    let out = applied(remove_header(&d, "gap"), &d);
    assert_eq!(header_value(&out, "GAP"), None);
    assert_eq!(out.rows.len(), d.rows.len() - 1);
    assert_eq!(dirty_rows(&out), vec![]);
}

#[test]
fn remove_header_of_a_missing_key_does_nothing() {
    let d = doc(SONG);
    let patch = remove_header(&d, "language").expect("removing a missing key is not an error");
    assert!(patch.ops.is_empty());
    let mut copy = d.clone();
    patch.apply(&mut copy).expect("the empty patch applies");
    assert_eq!(copy, d);
}

// ---- set_gap_ms ----

#[test]
fn set_gap_ms_changes_only_the_gap_row() {
    let d = doc(SONG);
    let out = applied(set_gap_ms(&d, 1500.0), &d);
    assert_eq!(header_value(&out, "GAP").as_deref(), Some("1500"));
    assert_eq!(header_raw(&out, "GAP").as_deref(), Some("#GAP:1500"));
    assert_eq!(dirty_rows(&out), vec![2]);
}

#[test]
fn set_gap_ms_writes_a_negative_fractional_value() {
    let d = doc(SONG);
    let out = applied(set_gap_ms(&d, -250.5), &d);
    assert_eq!(header_value(&out, "GAP").as_deref(), Some("-250.5"));
}

#[test]
fn set_gap_ms_refuses_a_non_finite_value() {
    let d = doc(SONG);
    assert_eq!(set_gap_ms(&d, f64::NAN), Err(EditError::InvalidHeaderValue));
    assert_eq!(
        set_gap_ms(&d, f64::INFINITY),
        Err(EditError::InvalidHeaderValue)
    );
}

// ---- set_bpm ----

#[test]
fn set_bpm_without_keep_times_changes_only_the_bpm_row() {
    let d = doc(SONG);
    let out = applied(set_bpm(&d, 480.0, false), &d);
    assert_eq!(header_value(&out, "BPM").as_deref(), Some("480"));
    assert_eq!(note_starts(&out), vec![0, 4, 8]);
    assert_eq!(note_lengths(&out), vec![4, 4, 4]);
    assert_eq!(dirty_rows(&out), vec![1]);
}

#[test]
fn set_bpm_with_keep_times_rescales_every_beat() {
    let d = doc(SONG);
    let out = applied(set_bpm(&d, 480.0, true), &d);
    assert_eq!(header_value(&out, "BPM").as_deref(), Some("480"));
    // The factor 480 / 240 = 2 doubles every beat.
    assert_eq!(note_starts(&out), vec![0, 8, 16]);
    assert_eq!(note_lengths(&out), vec![8, 8, 8]);
    assert_eq!(phrase_ends(&out), vec![16]);
    assert_eq!(dirty_rows(&out), vec![1, 3, 4, 5, 6]);
    // Every note and phrase end stays at its audio time within half a beat.
    let old_times = audio_times(&d);
    let new_times = audio_times(&out);
    let half_beat = song(&out)
        .timing
        .expect("the song has a timing")
        .ms_per_beat()
        / 2.0;
    for (before, after) in old_times.iter().zip(&new_times) {
        assert!(
            (after - before).abs() <= half_beat,
            "the note moved from {before} ms to {after} ms"
        );
    }
}

#[test]
fn set_bpm_with_keep_times_keeps_lengths_at_least_one() {
    let d = doc("#TITLE:T\n#BPM:240\n: 0 1 0 la\n: 4 1 1 la\n: 8 4 2 la\nE\n");
    // The factor 96 / 240 = 0.4 rounds 1.6 to 2 and 0.4 to 0, and a length
    // never drops below one beat.
    let out = applied(set_bpm(&d, 96.0, true), &d);
    assert_eq!(note_starts(&out), vec![0, 2, 3]);
    assert_eq!(note_lengths(&out), vec![1, 1, 2]);
}

#[test]
fn set_bpm_with_keep_times_refuses_a_relative_song_but_the_header_alone_is_fine() {
    let d = doc("#TITLE:T\n#BPM:240\n#RELATIVE:yes\n: 0 4 0 la\n- 8 4\nE\n");
    assert_eq!(
        set_bpm(&d, 480.0, true),
        Err(EditError::RelativeModeIsReadOnly)
    );
    let out = applied(set_bpm(&d, 480.0, false), &d);
    assert_eq!(header_value(&out, "BPM").as_deref(), Some("480"));
}

#[test]
fn set_bpm_with_keep_times_needs_a_usable_old_timing() {
    let d = doc("#TITLE:T\n: 0 4 0 la\nE\n");
    assert_eq!(set_bpm(&d, 480.0, true), Err(EditError::NoTiming));
}

#[test]
fn set_bpm_refuses_an_unusable_new_value() {
    let d = doc(SONG);
    assert_eq!(set_bpm(&d, 0.0, false), Err(EditError::InvalidHeaderValue));
    assert_eq!(set_bpm(&d, -1.0, true), Err(EditError::InvalidHeaderValue));
}
