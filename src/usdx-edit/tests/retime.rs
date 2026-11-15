//! About me: tests for the anchor stretch (task P7-02, plan section 6.7).
//! Anchors pin lines' starts to target times; the notes between two
//! neighbouring anchors map linearly (with the last anchor line's later
//! notes following the last segment's scale), the lines before shift or
//! keep, the lines after shift, keep or stretch to the end pin, and the
//! result goes through the collision pass (6.6) over the retimed lines
//! plus one line before and after them (user decision 2026-11-15,
//! "Retimed lines and neighbours"), so Factory stacked notes elsewhere
//! stay stacked. The
//! fixtures use BPM 300 (one beat is 50 ms) and GAP 1000 (beat 0 at 1 s),
//! so `ms(beat) = 1000 + beat * 50`.

use usdx_edit::EditError;
use usdx_edit::collisions::NeedsGapShift;
use usdx_edit::retime::{Anchor, Lengths, Outside, StretchOptions, anchor_stretch};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, classify};
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

/// opts builds options without an end pin.
fn opts(before: Outside, after: Outside, lengths: Lengths) -> StretchOptions {
    StretchOptions {
        before,
        after,
        lengths,
        end_pin_ms: None,
    }
}

/// repaired applies the stretch and returns the written text.
fn repaired(text: &str, anchors: &[Anchor], options: &StretchOptions) -> String {
    let (doc, song) = loaded(text);
    let patch = anchor_stretch(&doc, &song, 0, anchors, options).expect("the stretch plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    String::from_utf8(serialize(&out).expect("the song serializes")).expect("utf-8")
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

/// error_of runs the stretch and returns the error it refuses with.
fn error_of(text: &str, anchors: &[Anchor], options: &StretchOptions) -> EditError {
    let (doc, song) = loaded(text);
    anchor_stretch(&doc, &song, 0, anchors, options).expect_err("the stretch refuses")
}

#[test]
fn two_anchors_stretch_the_region_between_with_scaled_lengths() {
    // Anchors at line 0 (start stays at beat 0) and line 2 (start maps from
    // beat 24 to beat 44). The scale between them is (44 - 0) / (24 - 0)
    // = 11/6, so every note time t of lines 0 to 2 maps to t * 11/6, and
    // line 2's later notes follow the same scale (44 + (t - 24) * 11/6).
    let text = standard(3);
    let anchors = [
        Anchor {
            line: 0,
            target_ms: ms_of(0),
        },
        Anchor {
            line: 2,
            target_ms: ms_of(44),
        },
    ];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Scale),
    );
    assert_eq!(
        spans(&out),
        vec![(0, 7), (11, 7), (22, 7), (33, 7), (44, 7), (55, 7)],
        "every note time maps with 11/6 and the lengths scale with it"
    );
    // The phrase ends keep their old fraction of the gap between their
    // line's last note and the next line's first note: both sat at 0.5.
    assert_eq!(
        markers(&out),
        vec![20, 42],
        "0.5 of the new gaps 18..22 and 40..44"
    );
}

#[test]
fn kept_lengths_keep_every_note_length() {
    let text = standard(3);
    let anchors = [
        Anchor {
            line: 0,
            target_ms: ms_of(0),
        },
        Anchor {
            line: 2,
            target_ms: ms_of(44),
        },
    ];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Keep),
    );
    assert_eq!(
        spans(&out),
        vec![(0, 4), (11, 4), (22, 4), (33, 4), (44, 4), (55, 4)],
        "the starts map with 11/6, the lengths stay 4 beats"
    );
    assert_eq!(
        markers(&out),
        vec![19, 41],
        "0.5 of the new gaps 15..22 and 37..44, rounded half away from zero"
    );
}

#[test]
fn a_single_anchor_shifts_the_whole_voice() {
    // One anchor at line 1: its start maps from beat 12 to beat 30, the
    // whole voice follows rigidly (the anchor line has no scale partner),
    // before and after shift by the same 18 beats.
    let text = standard(5);
    let anchors = [Anchor {
        line: 1,
        target_ms: ms_of(30),
    }];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Shift, Outside::Shift, Lengths::Scale),
    );
    assert_eq!(
        spans(&out),
        vec![
            (18, 4),
            (24, 4),
            (30, 4),
            (36, 4),
            (42, 4),
            (48, 4),
            (54, 4),
            (60, 4),
            (66, 4),
            (72, 4),
        ],
        "every note shifts by 18 beats"
    );
    assert_eq!(
        markers(&out),
        vec![29, 41, 53, 65],
        "the markers keep their 0.5 fraction of the shifted gaps"
    );
}

#[test]
fn a_single_anchor_with_keep_outside_moves_its_own_line_rigidly() {
    // One anchor at line 1 with Keep outside: the anchor line moves
    // rigidly from beat 12 to beat 13, everything else keeps its place.
    let text = standard(5);
    let anchors = [Anchor {
        line: 1,
        target_ms: ms_of(13),
    }];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Keep),
    );
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (13, 4),
            (19, 4),
            (24, 4),
            (30, 4),
            (36, 4),
            (42, 4),
            (48, 4),
            (54, 4),
        ],
        "only line 1's notes move, by 1 beat"
    );
    // Line 0's marker keeps its 0.5 fraction of the widened gap 10..13
    // (11.5 rounds to 12); line 1's marker keeps it of the narrowed
    // gap 23..24 (23.5 rounds to 24).
    assert_eq!(markers(&out), vec![12, 24, 35, 47]);
}

#[test]
fn anchor_times_must_increase() {
    let text = standard(3);
    let anchors = [
        Anchor {
            line: 0,
            target_ms: ms_of(10),
        },
        Anchor {
            line: 2,
            target_ms: ms_of(4),
        },
    ];
    assert!(
        matches!(
            error_of(
                &text,
                &anchors,
                &opts(Outside::Keep, Outside::Keep, Lengths::Scale)
            ),
            EditError::AnchorsOutOfOrder
        ),
        "decreasing target times are refused"
    );
    let same_line = [
        Anchor {
            line: 2,
            target_ms: ms_of(10),
        },
        Anchor {
            line: 2,
            target_ms: ms_of(20),
        },
    ];
    assert!(
        matches!(
            error_of(
                &text,
                &same_line,
                &opts(Outside::Keep, Outside::Keep, Lengths::Scale)
            ),
            EditError::AnchorsOutOfOrder
        ),
        "two anchors on one line cannot both pin its start"
    );
}

#[test]
fn unsorted_anchors_are_sorted_by_line() {
    // The same stretch as the scaled two-anchor test, with the anchors
    // handed over in reverse order.
    let text = standard(3);
    let anchors = [
        Anchor {
            line: 2,
            target_ms: ms_of(44),
        },
        Anchor {
            line: 0,
            target_ms: ms_of(0),
        },
    ];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Scale),
    );
    assert_eq!(
        spans(&out),
        vec![(0, 7), (11, 7), (22, 7), (33, 7), (44, 7), (55, 7)]
    );
}

#[test]
fn anchors_must_name_existing_lines_of_the_voice() {
    let text = standard(3);
    let beyond = [Anchor {
        line: 7,
        target_ms: ms_of(10),
    }];
    assert!(
        matches!(
            error_of(
                &text,
                &beyond,
                &opts(Outside::Keep, Outside::Keep, Lengths::Scale)
            ),
            EditError::StalePatch(_)
        ),
        "an anchor beyond the voice's lines is refused"
    );
}

#[test]
fn no_timing_and_relative_songs_are_refused() {
    let no_timing = "#TITLE:T\n: 0 4 0 a \nE\n";
    let anchors = [Anchor {
        line: 0,
        target_ms: ms_of(0),
    }];
    assert!(matches!(
        error_of(
            no_timing,
            &anchors,
            &opts(Outside::Keep, Outside::Keep, Lengths::Scale)
        ),
        EditError::NoTiming
    ));
    let relative = "#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 la\n: 4 4 1 la\n- 8 1\nE\n";
    assert!(matches!(
        error_of(
            relative,
            &anchors,
            &opts(Outside::Keep, Outside::Keep, Lengths::Scale)
        ),
        EditError::RelativeModeIsReadOnly
    ));
}

#[test]
fn the_gap_shift_request_surfaces_as_an_error() {
    // Pinning line 0's start to 0 ms moves its first note to beat -20;
    // the collision pass reports the 20 beats #GAP must go down by.
    let text = standard(3);
    let anchors = [Anchor {
        line: 0,
        target_ms: 0.0,
    }];
    assert!(
        matches!(
            error_of(
                &text,
                &anchors,
                &opts(Outside::Shift, Outside::Shift, Lengths::Scale)
            ),
            EditError::NeedsGapShift(NeedsGapShift(20))
        ),
        "the error carries the beats for the caller"
    );
}

#[test]
fn the_repair_touches_only_its_neighbourhood() {
    // User decision 2026-11-15 "Retimed lines + neighbours": the collision
    // pass runs over the retimed lines plus one line before and after
    // them. Line 4 carries Factory-style stacked notes (two identical
    // rows); the repair of lines 0 and 1 shifts them (Outside::Shift)
    // but must keep them stacked instead of pushing them apart.
    let mut text = String::from("#TITLE:T\n#BPM:300\n#GAP:1000\n");
    for i in 0..6 {
        let letter = char::from(b'a' + i as u8);
        if i == 4 {
            text += ": 48 4 0 x \n: 48 4 0 x \n";
        } else {
            text += &format!(
                ": {} 4 0 {letter}1 \n: {} 4 0 {letter}2 \n",
                i * 12,
                i * 12 + 6
            );
        }
        if i + 1 < 6 {
            text += &format!("- {}\n", i * 12 + 11);
        }
    }
    text += "E\n";
    let anchors = [
        Anchor {
            line: 0,
            target_ms: ms_of(0),
        },
        Anchor {
            line: 1,
            target_ms: ms_of(18),
        },
    ];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Shift, Outside::Shift, Lengths::Scale),
    );
    let all = spans(&out);
    assert_eq!(
        &all[8..],
        &[(54, 4), (54, 4), (66, 4), (72, 4)],
        "the lines beyond the pass window shift rigidly by 6 beats and line 4's stacked notes stay stacked"
    );
    assert_eq!(
        &all[..8],
        &[
            (0, 6),
            (9, 6),
            (18, 6),
            (27, 3),
            (30, 4),
            (36, 4),
            (42, 4),
            (48, 4)
        ],
        "the pass shortens line 1's second note just enough for line 2's shifted start"
    );
}

#[test]
fn the_patch_changes_only_the_moved_rows_and_marks_them_dirty() {
    // The Keep test moves line 1's two notes and the markers of lines 0
    // and 1 (their gaps changed); every other row stays and stays clean.
    let text = standard(5);
    let (doc, song) = loaded(&text);
    let anchors = [Anchor {
        line: 1,
        target_ms: ms_of(13),
    }];
    let patch = anchor_stretch(
        &doc,
        &song,
        0,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Keep),
    )
    .expect("the stretch plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let dirty: Vec<usize> = out
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.dirty.then_some(index))
        .collect();
    assert_eq!(dirty.len(), 4, "two moved notes and two moved markers");
    let (_, view) =
        loaded(&String::from_utf8(serialize(&out).expect("the song serializes")).expect("utf-8"));
    let spans_now: Vec<(i64, i64)> = view.voices[0]
        .lines
        .iter()
        .flat_map(|line| &line.notes)
        .filter_map(|id| view.note(&out, *id))
        .map(|note| (note.start as i64, note.length))
        .collect();
    assert_eq!(
        spans_now,
        vec![
            (0, 4),
            (6, 4),
            (13, 4),
            (19, 4),
            (24, 4),
            (30, 4),
            (36, 4),
            (42, 4),
            (48, 4),
            (54, 4),
        ]
    );
}

#[test]
fn every_anchor_line_starts_within_half_a_beat_of_its_target() {
    // The plan's invariant, checked over the two-anchor stretch.
    let text = standard(3);
    let (doc, song) = loaded(&text);
    let anchors = [
        Anchor {
            line: 0,
            target_ms: ms_of(0),
        },
        Anchor {
            line: 2,
            target_ms: ms_of(44),
        },
    ];
    let options = opts(Outside::Keep, Outside::Keep, Lengths::Scale);
    let patch = anchor_stretch(&doc, &song, 0, &anchors, &options).expect("the stretch plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let view = Song::from_document(&out);
    let timing = view.timing.expect("the timing survives");
    for anchor in &anchors {
        let line = &view.voices[0].lines[anchor.line];
        let id = line.notes[0];
        let start = view.note(&out, id).expect("the note exists").start;
        let reached = timing.beat_to_ms(start);
        assert!(
            (reached - anchor.target_ms).abs() <= 0.5 * 15000.0 / timing.bpm,
            "line {} starts at {} ms, {} ms away from its target",
            anchor.line,
            reached,
            anchor.target_ms
        );
    }
}

#[test]
fn the_end_pin_stretches_the_lines_after_the_last_anchor() {
    // One anchor at line 1 (beat 12 -> beat 30), after: stretch to the end
    // pin at beat 122. The voice's last note ends at beat 58, so the
    // after-region maps t -> 30 + (t - 12) * (122 - 30) / (58 - 12)
    // = 30 + (t - 12) * 2, and the last note's end lands exactly on the
    // pin. Lengths scale in the pin region.
    let text = standard(5);
    let anchors = [Anchor {
        line: 1,
        target_ms: ms_of(30),
    }];
    let options = StretchOptions {
        before: Outside::Shift,
        after: Outside::Shift,
        lengths: Lengths::Scale,
        end_pin_ms: Some(ms_of(122)),
    };
    let out = repaired(&text, &anchors, &options);
    assert_eq!(
        spans(&out),
        vec![
            (18, 4),
            (24, 4),
            (30, 4),
            (36, 4),
            (54, 8),
            (66, 8),
            (78, 8),
            (90, 8),
            (102, 8),
            (114, 8),
        ],
        "line 1 shifts rigidly, lines 2 to 4 stretch with the factor 2"
    );
    // Line 1's marker keeps its 0.5 fraction of the pin-widened gap
    // 40..54; lines 2 and 3 keep theirs (74..78 and 98..102).
    assert_eq!(markers(&out), vec![29, 47, 76, 100]);
}

#[test]
fn a_compressed_section_is_repaired_back_within_one_beat() {
    // The plan's acceptance test (the Factory's lesson 5 method): compress
    // lines 15 to 23 with the linear fault t' = 204 + (t - 180) / 2, which
    // maps every even beat of the fixture exactly (both anchor lines'
    // starts leave their original places: 180 -> 204 and 276 -> 252), then
    // repair with anchors at the original starts of lines 15 and 23. Every
    // note start between the anchors must come back within 1 beat.
    const LINES: usize = 25;
    let fault = |beat: f64| (204.0 + (beat - 180.0) / 2.0).round() as i64;
    let mut text = String::from("#TITLE:T\n#BPM:300\n#GAP:1000\n");
    for i in 0..LINES {
        let letter = char::from(b'a' + i as u8);
        if (15..=23).contains(&i) {
            for start in [i * 12, i * 12 + 6] {
                let moved_start = fault(start as f64);
                let moved_end = fault((start + 4) as f64);
                text += &format!(
                    ": {} {} 0 {letter}x \n",
                    moved_start,
                    moved_end - moved_start
                );
            }
            text += &format!("- {}\n", fault((i * 12 + 11) as f64));
        } else {
            text += &format!(
                ": {} 4 0 {letter}1 \n: {} 4 0 {letter}2 \n",
                i * 12,
                i * 12 + 6
            );
            if i + 1 < LINES {
                text += &format!("- {}\n", i * 12 + 11);
            }
        }
    }
    text += "E\n";
    let anchors = [
        Anchor {
            line: 15,
            target_ms: ms_of(180),
        },
        Anchor {
            line: 23,
            target_ms: ms_of(276),
        },
    ];
    let out = repaired(
        &text,
        &anchors,
        &opts(Outside::Keep, Outside::Keep, Lengths::Scale),
    );
    let (out_doc, view) = loaded(&out);
    let starts: Vec<i64> = view.voices[0]
        .lines
        .iter()
        .enumerate()
        .filter(|(index, _)| (15..=23).contains(index))
        .flat_map(|(_, line)| &line.notes)
        .filter_map(|id| view.note(&out_doc, *id))
        .map(|note| note.start as i64)
        .collect();
    let expected: Vec<i64> = (15..=23).flat_map(|i| [i * 12, i * 12 + 6]).collect();
    assert_eq!(
        starts.len(),
        expected.len(),
        "every note of the section is there"
    );
    for (got, want) in starts.iter().zip(&expected) {
        assert!(
            (got - want).abs() <= 1,
            "the note start {} came back {} beats away from {}",
            got,
            (got - want).abs(),
            want
        );
    }
}
