//! About me: tests for the anchors.txt export (task P7-06, plan section
//! 6.11). The export lists every voice-P1 line whose first note's start
//! time changed in the session (decision D-12, via `changes::ChangeLog`)
//! plus the pinned anchors, deduplicated by line and sorted by time, and
//! renders the Factory's file format (`<time> | <text>` lines, an `end`
//! line last, a header comment). The time formatting, the text
//! normalisation and the end-matching are ports of the Factory's
//! `anchors.py` and `outro.py:parse_time` behavior; the matching cases
//! are copied from `test_anchors.py`. The fixtures use BPM 300 (one beat
//! is 50 ms) and GAP 1000 (beat 0 at 1 s).

use usdx_edit::anchors_export::{
    AnchorEntry, AnchorExportError, AnchorMatchError, PinnedAnchor, build_entries, export_problems,
    format_time, match_anchor_units, normalize, render,
};
use usdx_edit::changes::ChangeLog;
use usdx_edit::ops::move_notes;
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, classify};
use usdx_format::song::Song;

/// loaded classifies a song text.
fn loaded(text: &str) -> (Document, Song) {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    (doc, song)
}

/// three_lines builds a three-line song: line 0 at beat 0 ("a1 a2"),
/// line 1 at beat 12 ("b1 b2"), line 2 at beat 24 ("c1 c2" with a hold
/// marker on the last syllable).
fn three_lines() -> String {
    "#TITLE:T\n#BPM:300\n#GAP:1000\n\
     : 0 4 0 a1 \n: 6 4 0 a2 \n- 11\n\
     : 12 4 0 b1 \n: 18 4 0 b2 \n- 23\n\
     : 24 4 0 c1 \n: 30 4 0 ~c2 \nE\n"
        .to_owned()
}

/// moved builds the song with line 1's notes shifted by `d_beat` beats.
fn moved(d_beat: i64) -> (Document, Song) {
    let (doc, song) = loaded(&three_lines());
    let ids: Vec<usdx_format::rows::RowId> = song.voices[0].lines[1].notes.clone();
    let patch = move_notes(&doc, &song, &ids, d_beat, 0).expect("the move plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let song = Song::from_document(&out);
    (out, song)
}

#[test]
fn format_time_writes_minutes_seconds_with_two_decimals_and_hours_past_one() {
    assert_eq!(format_time(66_180.0), "1:06.18");
    assert_eq!(format_time(5_000.0), "0:05.00");
    assert_eq!(format_time(265_000.0), "4:25.00");
    assert_eq!(format_time(3_660_000.0), "1:01:00.00");
    // The centiseconds round half away from zero.
    assert_eq!(format_time(66_184.0), "1:06.18");
    assert_eq!(format_time(66_185.0), "1:06.19");
}

#[test]
fn normalize_ignores_case_punctuation_and_extra_spacing() {
    assert_eq!(normalize("I'm loving You!"), "i m loving you");
    assert_eq!(
        normalize("  Six   feet-underground "),
        "six feet underground"
    );
    assert_eq!(normalize("hello_world 42"), "hello_world 42");
    assert_eq!(normalize("Café"), "café");
}

#[test]
fn build_entries_lists_the_changed_line_with_its_new_time() {
    let (doc, song) = loaded(&three_lines());
    let log = ChangeLog::snapshot(&doc, &song);
    let (doc, song) = moved(2);
    let entries = build_entries(&doc, &song, &log, &[]);
    assert_eq!(
        entries,
        vec![AnchorEntry {
            time_ms: 1000.0 + 14.0 * 50.0,
            text: "b1 b2".to_owned(),
            source_line: 1,
        }]
    );
}

#[test]
fn build_entries_pins_lines_and_deduplicates_by_line() {
    // Line 1 moved AND is pinned at 1800 ms: the pinned time wins. Line 2
    // is pinned without a change, with a hold marker in its text.
    let (doc, song) = loaded(&three_lines());
    let log = ChangeLog::snapshot(&doc, &song);
    let (doc, song) = moved(2);
    let entries = build_entries(
        &doc,
        &song,
        &log,
        &[
            PinnedAnchor {
                line: 1,
                time_ms: 1800.0,
            },
            PinnedAnchor {
                line: 2,
                time_ms: 2500.0,
            },
        ],
    );
    assert_eq!(
        entries,
        vec![
            AnchorEntry {
                time_ms: 1800.0,
                text: "b1 b2".to_owned(),
                source_line: 1,
            },
            AnchorEntry {
                time_ms: 2500.0,
                text: "c1 c2".to_owned(),
                source_line: 2,
            },
        ],
        "the pinned time wins over the moved time, the hold marker is gone"
    );
}

#[test]
fn build_entries_of_an_unchanged_song_is_empty() {
    let (doc, song) = loaded(&three_lines());
    let log = ChangeLog::snapshot(&doc, &song);
    assert!(build_entries(&doc, &song, &log, &[]).is_empty());
}

#[test]
fn render_writes_the_factory_format_with_the_header_and_end_line() {
    let entries = vec![
        AnchorEntry {
            time_ms: 5_000.0,
            text: "first line".to_owned(),
            source_line: 0,
        },
        AnchorEntry {
            time_ms: 66_180.0,
            text: "second line".to_owned(),
            source_line: 1,
        },
    ];
    let file = render("Test Song", "0.1.0", "2026-11-16", &entries, Some(70_000.0))
        .expect("the file renders");
    assert_eq!(
        file,
        "# anchors for Test Song - exported by USDX Song Editor 0.1.0 on 2026-11-16\n\
         0:05.00 | first line\n\
         1:06.18 | second line\n\
         end 1:10.00\n"
    );
    // Without an end pin no end line is written.
    let file = render("T", "0.1.0", "2026-11-16", &entries, None).expect("the file renders");
    assert!(file.ends_with("1:06.18 | second line\n"));
}

#[test]
fn render_refuses_equal_times_and_an_end_at_or_before_the_last_anchor() {
    let entries = vec![
        AnchorEntry {
            time_ms: 1_000.0,
            text: "a".to_owned(),
            source_line: 0,
        },
        AnchorEntry {
            time_ms: 1_000.0,
            text: "b".to_owned(),
            source_line: 1,
        },
    ];
    assert_eq!(
        render("T", "0.1.0", "2026-11-16", &entries, None),
        Err(AnchorExportError::TimesNotIncreasing)
    );
    let entries = vec![AnchorEntry {
        time_ms: 5_000.0,
        text: "a".to_owned(),
        source_line: 0,
    }];
    assert_eq!(
        render("T", "0.1.0", "2026-11-16", &entries, Some(5_000.0)),
        Err(AnchorExportError::EndNotAfterLastAnchor)
    );
    assert_eq!(
        render("T", "0.1.0", "2026-11-16", &entries, Some(4_000.0)),
        Err(AnchorExportError::EndNotAfterLastAnchor)
    );
}

#[test]
fn match_anchor_units_copies_the_factory_behavior() {
    let texts = [
        "Hello there",
        "Chorus line",
        "Verse two",
        "Chorus line",
        "Final words!",
    ];
    // Case, punctuation and spacing do not matter.
    assert_eq!(match_anchor_units(&["final   WORDS"], &texts), Ok(vec![4]));
    // A repeated line means its last occurrence.
    assert_eq!(match_anchor_units(&["chorus line"], &texts), Ok(vec![3]));
    // Earlier anchors search before the later ones (from the end).
    assert_eq!(
        match_anchor_units(&["Chorus line", "Verse two", "Chorus line"], &texts),
        Ok(vec![1, 2, 3])
    );
    // An unknown line is refused, quoting it.
    assert_eq!(
        match_anchor_units(&["not in the song"], &texts),
        Err(AnchorMatchError::NotFound {
            text: "not in the song".to_owned()
        })
    );
    // Anchors in a different order than the lyrics are refused.
    assert!(matches!(
        match_anchor_units(&["Final words", "Hello there"], &texts),
        Err(AnchorMatchError::NotFound { .. })
    ));
    assert_eq!(match_anchor_units(&[], &texts), Ok(vec![]));
}

#[test]
fn export_problems_reports_unfound_and_misbound_anchors() {
    let entry = |time_ms: f64, text: &str, source_line: usize| AnchorEntry {
        time_ms,
        text: text.to_owned(),
        source_line,
    };
    // Without lyrics.txt: a text that appears again on a later song line
    // would bind to the later occurrence.
    let song_lines = vec!["a".to_owned(), "b".to_owned(), "a".to_owned()];
    let problems = export_problems(&[entry(1_000.0, "a", 0)], &song_lines, None);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("later line"), "{}", problems[0]);
    // The last occurrence is fine.
    assert!(export_problems(&[entry(3_000.0, "a", 2)], &song_lines, None).is_empty());

    // With lyrics.txt: the Factory binding runs from the end; an anchor
    // bound to another occurrence than the song line it came from warns.
    let problems = export_problems(&[entry(1_000.0, "a", 0)], &song_lines, Some(&song_lines));
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].contains("different occurrence"),
        "{}",
        problems[0]
    );
    // A text missing from lyrics.txt warns.
    let lyrics = vec!["b".to_owned()];
    let problems = export_problems(&[entry(1_000.0, "a", 0)], &song_lines, Some(&lyrics));
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("not found"), "{}", problems[0]);
    // A binding that agrees with the song line is fine.
    let lyrics = vec!["a".to_owned()];
    assert!(export_problems(&[entry(1_000.0, "a", 0)], &song_lines, Some(&lyrics)).is_empty());
    // A repeated text binds to the last occurrence before the later
    // anchors: the anchor from song line 0 binds lyrics line 1 and warns.
    let lyrics = vec!["a".to_owned(), "a".to_owned(), "a".to_owned()];
    let problems = export_problems(
        &[entry(1_000.0, "a", 0), entry(3_000.0, "a", 2)],
        &song_lines,
        Some(&lyrics),
    );
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].contains("different occurrence"),
        "{}",
        problems[0]
    );
}
