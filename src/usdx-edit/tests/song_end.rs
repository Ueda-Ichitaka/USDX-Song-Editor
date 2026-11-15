//! About me: tests for the song end cut (user decision 2026-11-15, "#END at
//! playhead + delete after"): `end_song_at` writes `#END` in whole
//! milliseconds and removes every note that starts at or after that time
//! plus the phrase ends that no longer have notes on both sides, in every
//! voice, as one undoable patch (for example to cut off a YouTube endcard).
//! A note that starts before the cut stays as it is.

use usdx_edit::EditError;
use usdx_edit::ops::end_song_at;
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::serialize::serialize;
use usdx_format::song::Song;

/// loaded classifies a song text.
fn loaded(text: &str) -> (Document, Song) {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    (doc, song)
}

/// cut applies the cut at `ms` and returns the written text.
fn cut(text: &str, ms: f64) -> String {
    let (doc, song) = loaded(text);
    let patch = end_song_at(&doc, &song, ms).expect("the cut plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    String::from_utf8(serialize(&out).expect("the song serializes")).expect("utf-8")
}

// BPM 300 means one beat is 15000 / 300 = 50 ms; GAP 1000 puts beat 0 at
// 1 s, so beat 20 is at 2 s.
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:300\n#GAP:1000\n",
    ": 0 4 0 a \n: 4 4 0 b \n- 10\n",
    ": 12 4 0 c \n: 20 4 0 d \n- 26\n",
    ": 30 4 0 e \nE\n",
);

#[test]
fn the_cut_writes_end_and_removes_the_notes_from_it_on() {
    // 2000 ms is beat 20: "d" starts there and goes, "e" goes, the phrase
    // ends after "c" lose their notes behind them and go too.
    assert_eq!(
        cut(SONG, 2000.0),
        concat!(
            "#TITLE:T\n#BPM:300\n#GAP:1000\n#END:2000\n",
            ": 0 4 0 a \n: 4 4 0 b \n- 10\n",
            ": 12 4 0 c \nE\n",
        )
    );
}

#[test]
fn a_note_that_starts_before_the_cut_stays() {
    // 1700 ms is beat 14, inside "c" (12 to 16): "c" stays whole.
    let text = cut(SONG, 1700.0);
    assert!(text.contains(": 12 4 0 c \nE\n"), "{text}");
    assert!(text.contains("#END:1700\n"), "{text}");
}

#[test]
fn the_end_is_written_in_whole_milliseconds() {
    let text = cut(SONG, 2000.6);
    assert!(text.contains("#END:2001\n"), "{text}");
}

#[test]
fn an_existing_end_header_is_replaced() {
    let with_end = SONG.replace("#GAP:1000\n", "#GAP:1000\n#END:9000\n");
    let text = cut(&with_end, 2000.0);
    assert!(text.contains("#END:2000\n"), "{text}");
    assert!(!text.contains("#END:9000"), "{text}");
}

#[test]
fn a_cut_after_the_last_note_only_writes_end() {
    assert_eq!(
        cut(SONG, 5000.0),
        SONG.replace("#GAP:1000\n", "#GAP:1000\n#END:5000\n")
    );
}

#[test]
fn every_voice_is_cut() {
    let duet = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        "P1\n: 0 4 0 a \n- 10\n: 30 4 0 b \n",
        "P2\n: 2 4 0 c \n- 12\n: 32 4 0 d \nE\n",
    );
    assert_eq!(
        cut(duet, 2000.0),
        concat!(
            "#TITLE:T\n#BPM:300\n#GAP:1000\n#END:2000\n",
            "P1\n: 0 4 0 a \n",
            "P2\n: 2 4 0 c \nE\n",
        )
    );
}

#[test]
fn the_cut_undoes_exactly() {
    let (doc, song) = loaded(SONG);
    let patch = end_song_at(&doc, &song, 2000.0).expect("the cut plans");
    assert_eq!(patch.label, "End song here");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    patch
        .inverse()
        .apply(&mut out)
        .expect("the inverse applies");
    assert_eq!(out, doc);
}

#[test]
fn the_cut_needs_timing_an_absolute_song_and_a_usable_time() {
    let (doc, song) = loaded("#TITLE:T\n: 0 4 0 a \nE\n");
    assert_eq!(
        end_song_at(&doc, &song, 1000.0).unwrap_err(),
        EditError::NoTiming
    );
    let (doc, song) = loaded("#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 a \nE\n");
    assert_eq!(
        end_song_at(&doc, &song, 1000.0).unwrap_err(),
        EditError::RelativeModeIsReadOnly
    );
    let (doc, song) = loaded(SONG);
    assert_eq!(
        end_song_at(&doc, &song, f64::NAN).unwrap_err(),
        EditError::InvalidHeaderValue
    );
    assert_eq!(
        end_song_at(&doc, &song, -5.0).unwrap_err(),
        EditError::InvalidHeaderValue
    );
}

#[test]
fn only_the_removed_rows_and_the_header_change() {
    let (doc, song) = loaded(SONG);
    let patch = end_song_at(&doc, &song, 2000.0).expect("the cut plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let dirty: Vec<String> = out
        .rows
        .iter()
        .filter(|row| row.dirty)
        .map(|row| match &row.kind {
            RowKind::Header { key, .. } => key.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(dirty, vec!["END".to_owned()]);
}

#[test]
fn a_phrase_end_the_cut_does_not_touch_stays_even_when_odd() {
    // A phrase end before the first note is odd but not the cut's business.
    let odd = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        "- 0\n: 2 4 0 a \n- 10\n: 30 4 0 b \nE\n",
    );
    assert_eq!(
        cut(odd, 2000.0),
        concat!(
            "#TITLE:T\n#BPM:300\n#GAP:1000\n#END:2000\n",
            "- 0\n: 2 4 0 a \nE\n",
        )
    );
}
