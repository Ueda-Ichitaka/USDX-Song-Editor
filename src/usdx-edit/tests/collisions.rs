//! About me: tests for the collision pass (task P7-01, plan section 6.6). A
//! retime hands over one voice's lines with new float start and end beats;
//! the pass rounds them to whole beats (length at least 1), shortens a
//! previous note down to 1 beat when the next one starts inside it and
//! pushes the next one to the previous end when that is not enough, puts
//! each phrase end at the old fraction of the new gap between its lines
//! (clamped into the gap) and asks for a #GAP shift when a start would be
//! below 0. Property tests cover every invariant of the plan; a valid song
//! read back unchanged comes out exactly as it is.

use proptest::prelude::*;
use usdx_edit::collisions::*;
use usdx_format::bytes::decode;
use usdx_format::rows::{RowId, RowKind, classify};
use usdx_format::song::Song;

/// note builds a timed note with the row id `id`.
fn note(id: u32, start: f64, end: f64) -> TimedNote {
    TimedNote {
        id: RowId(id),
        start,
        end,
    }
}

/// line builds a timed line without a phrase end.
fn line(notes: Vec<TimedNote>) -> TimedLine {
    TimedLine { notes, end: None }
}

/// placed lists `(id, start, length)` of the placed notes.
fn placed(result: &Placed) -> Vec<(u32, i64, i64)> {
    result
        .notes
        .iter()
        .map(|note| (note.id.0, note.start, note.length))
        .collect()
}

#[test]
fn starts_and_ends_round_to_whole_beats() {
    let result = resolve_collisions(&[line(vec![note(1, 0.4, 3.6), note(2, 5.5, 7.2)])])
        .expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 0, 4), (2, 6, 1)]);
}

#[test]
fn a_length_is_at_least_one_beat() {
    let result = resolve_collisions(&[line(vec![note(1, 2.2, 2.4)])]).expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 2, 1)]);
}

#[test]
fn an_overlap_shortens_the_previous_note() {
    let result = resolve_collisions(&[line(vec![note(1, 0.0, 4.0), note(2, 2.0, 6.0)])])
        .expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 0, 2), (2, 2, 4)]);
}

#[test]
fn a_note_that_cannot_fit_moves_to_the_previous_end() {
    // b would start at 0 like a: a shrinks to 1 beat, b moves to 1 and
    // keeps its 3 beats.
    let result = resolve_collisions(&[line(vec![note(1, 0.0, 4.0), note(2, 0.2, 3.2)])])
        .expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 0, 1), (2, 1, 3)]);
}

#[test]
fn pushes_cascade_along_the_voice() {
    let result = resolve_collisions(&[line(vec![
        note(1, 0.0, 2.0),
        note(2, 0.0, 2.0),
        note(3, 0.0, 2.0),
    ])])
    .expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 0, 1), (2, 1, 1), (3, 2, 2)]);
}

#[test]
fn a_phrase_end_keeps_its_fraction_of_the_gap() {
    let result = resolve_collisions(&[
        TimedLine {
            notes: vec![note(1, 0.0, 4.0)],
            end: Some(TimedBreak {
                id: RowId(9),
                place: BreakPlace::Fraction(0.5),
            }),
        },
        line(vec![note(2, 10.0, 12.0)]),
    ])
    .expect("no shift needed");
    assert_eq!(result.phrase_ends, vec![(RowId(9), 7)]);
}

#[test]
fn a_phrase_end_stays_inside_a_closed_gap() {
    let result = resolve_collisions(&[
        TimedLine {
            notes: vec![note(1, 0.0, 4.0)],
            end: Some(TimedBreak {
                id: RowId(9),
                place: BreakPlace::Fraction(0.5),
            }),
        },
        line(vec![note(2, 3.0, 5.0)]),
    ])
    .expect("no shift needed");
    // The next line's note pushed the first note down to 3 beats: the gap
    // is closed and the marker sits on the boundary.
    assert_eq!(placed(&result), vec![(1, 0, 3), (2, 3, 2)]);
    assert_eq!(result.phrase_ends, vec![(RowId(9), 3)]);
}

#[test]
fn a_phrase_end_after_the_last_line_keeps_its_distance() {
    let result = resolve_collisions(&[TimedLine {
        notes: vec![note(1, 10.0, 14.0)],
        end: Some(TimedBreak {
            id: RowId(9),
            place: BreakPlace::AfterLast(2.0),
        }),
    }])
    .expect("no shift needed");
    assert_eq!(result.phrase_ends, vec![(RowId(9), 16)]);
}

#[test]
fn a_start_below_zero_asks_for_a_gap_shift() {
    let result = resolve_collisions(&[line(vec![note(1, -2.6, 1.0), note(2, 3.0, 4.0)])]);
    assert_eq!(result.unwrap_err(), NeedsGapShift(3));
}

#[test]
fn a_line_without_notes_is_left_out() {
    let result = resolve_collisions(&[line(vec![]), line(vec![note(1, 0.0, 1.0)])])
        .expect("no shift needed");
    assert_eq!(placed(&result), vec![(1, 0, 1)]);
}

#[test]
fn a_valid_song_read_back_comes_out_unchanged() {
    let text = concat!(
        "#TITLE:T\n#BPM:300\n",
        ": 0 4 0 a\n: 5 2 0 b\n- 9\n",
        ": 12 3 0 c\n: 15 1 0 d\n- 16\n",
        ": 20 4 0 e\n- 30\nE\n",
    );
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    let lines = timed_lines(&doc, &song, 0);
    let result = resolve_collisions(&lines).expect("no shift needed");
    let notes: Vec<(u32, i64, i64)> = doc
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some((row.id.0, note.start, note.length)),
            _ => None,
        })
        .collect();
    assert_eq!(placed(&result), notes);
    assert_eq!(
        result.phrase_ends,
        vec![(RowId(4), 9), (RowId(7), 16), (RowId(9), 30)]
    );
}

// ---- properties ----

/// timed_voice generates lines of notes with arbitrary (overlapping,
/// unordered) float times and phrase ends with arbitrary fractions.
fn timed_voice() -> impl Strategy<Value = Vec<TimedLine>> {
    let note = (0.0f64..200.0, 0.0f64..12.0);
    let line = (
        proptest::collection::vec(note, 1..6),
        proptest::option::of(0.0f64..=1.0),
    );
    proptest::collection::vec(line, 1..6).prop_map(|lines| {
        let mut id = 0;
        let count = lines.len();
        lines
            .into_iter()
            .enumerate()
            .map(|(index, (notes, fraction))| {
                let notes = notes
                    .into_iter()
                    .map(|(start, length)| {
                        id += 1;
                        TimedNote {
                            id: RowId(id),
                            start,
                            end: start + length,
                        }
                    })
                    .collect();
                id += 1;
                let end = fraction.map(|fraction| TimedBreak {
                    id: RowId(id),
                    place: if index + 1 < count {
                        BreakPlace::Fraction(fraction)
                    } else {
                        BreakPlace::AfterLast(fraction * 4.0)
                    },
                });
                TimedLine { notes, end }
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn the_placed_voice_keeps_every_invariant(lines in timed_voice()) {
        let result = resolve_collisions(&lines).expect("starts are not negative");
        // Every note comes out once, in the input order.
        let ids: Vec<RowId> = lines.iter().flat_map(|line| line.notes.iter().map(|n| n.id)).collect();
        let placed_ids: Vec<RowId> = result.notes.iter().map(|n| n.id).collect();
        prop_assert_eq!(placed_ids, ids);
        for note in &result.notes {
            prop_assert!(note.start >= 0);
            prop_assert!(note.length >= 1);
        }
        // Starts never decrease and notes never overlap.
        for pair in result.notes.windows(2) {
            prop_assert!(pair[0].start <= pair[1].start);
            prop_assert!(pair[0].start + pair[0].length <= pair[1].start);
        }
        // Every phrase end lies between its line's last note end and the
        // next line's first note start, so none sits inside a note and no
        // two follow each other.
        let mut offset = 0;
        let mut phrase = result.phrase_ends.iter();
        for (index, line) in lines.iter().enumerate() {
            let last = &result.notes[offset + line.notes.len() - 1];
            offset += line.notes.len();
            if line.end.is_some() {
                let (_, beat) = phrase.next().expect("a placed phrase end per break");
                prop_assert!(*beat >= last.start + last.length);
                if index + 1 < lines.len() {
                    prop_assert!(*beat <= result.notes[offset].start);
                }
            }
        }
        prop_assert!(phrase.next().is_none());
    }

    #[test]
    fn a_gap_shift_by_the_asked_beats_resolves(
        lines in timed_voice(),
        shift in 0.0f64..50.0
    ) {
        let moved: Vec<TimedLine> = lines
            .iter()
            .map(|line| TimedLine {
                notes: line
                    .notes
                    .iter()
                    .map(|note| TimedNote { id: note.id, start: note.start - shift, end: note.end - shift })
                    .collect(),
                end: line.end.clone(),
            })
            .collect();
        match resolve_collisions(&moved) {
            Ok(result) => prop_assert!(result.notes.iter().all(|note| note.start >= 0)),
            Err(NeedsGapShift(beats)) => {
                prop_assert!(beats > 0);
                let shifted: Vec<TimedLine> = moved
                    .iter()
                    .map(|line| TimedLine {
                        notes: line
                            .notes
                            .iter()
                            .map(|note| TimedNote {
                                id: note.id,
                                start: note.start + beats as f64,
                                end: note.end + beats as f64,
                            })
                            .collect(),
                        end: line.end.clone(),
                    })
                    .collect();
                let result = resolve_collisions(&shifted).expect("the shift resolves");
                prop_assert_eq!(result.notes.iter().map(|n| n.start).min(), Some(0));
            }
        }
    }
}
