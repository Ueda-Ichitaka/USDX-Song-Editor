//! About me: tests for the block transform (task P7-03, plan section 6.8).
//! A block of lines maps rigidly when its new span keeps its old length
//! (move only) and linearly otherwise (scale only), the lines before keep
//! their place, and the lines after follow the block (MoveAlong shifts them
//! by the end offset) or stay (Stay keeps them; the collision pass resolves
//! the overlap at the boundary within the retimed lines plus one line
//! before and after them, user decision 2026-11-15). The block is an
//! anchor stretch with two anchors: the first line's first note start pins
//! the new start, the last line's last note end pins the new end. The
//! fixtures use BPM 300 (one beat is 50 ms) and GAP 1000 (beat 0 at 1 s),
//! so `ms(beat) = 1000 + beat * 50`.

use usdx_edit::EditError;
use usdx_edit::block::{Follow, block_transform};
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

/// transformed applies the block transform and returns the written text.
fn transformed(
    text: &str,
    from: usize,
    to: usize,
    new_start_ms: f64,
    new_end_ms: f64,
    follow: Follow,
) -> String {
    let (doc, song) = loaded(text);
    let patch = block_transform(&doc, &song, 0, from..to, new_start_ms, new_end_ms, follow)
        .expect("the block plans");
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

/// error_of runs the block transform and returns the error it refuses with.
fn error_of(
    text: &str,
    from: usize,
    to: usize,
    new_start_ms: f64,
    new_end_ms: f64,
    follow: Follow,
) -> EditError {
    let (doc, song) = loaded(text);
    block_transform(&doc, &song, 0, from..to, new_start_ms, new_end_ms, follow)
        .expect_err("the block refuses")
}

#[test]
fn a_moved_block_shifts_rigidly_and_move_along_carries_the_rest() {
    // The block's span keeps its length (24 beats -> 24 beats), so the map
    // is the rigid shift by 10 beats. The lines before keep their place,
    // the block moves, and MoveAlong shifts line 4 by the same end offset.
    let text = standard(5);
    let out = transformed(&text, 2, 4, ms_of(34), ms_of(56), Follow::MoveAlong);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (12, 4),
            (18, 4),
            (34, 4),
            (40, 4),
            (46, 4),
            (52, 4),
            (58, 4),
            (64, 4),
        ],
        "lines 0 and 1 stay, the block and line 4 shift by 10 beats"
    );
    // Line 1 sits in the pass window (the block plus one line before and
    // after), so its marker keeps its 0.5 fraction of the widened gap
    // 22..34; the block's markers keep theirs.
    assert_eq!(markers(&out), vec![11, 28, 45, 57]);
}

#[test]
fn a_scaled_block_stretches_and_move_along_shifts_by_the_end_offset() {
    // The block's span halves (22 beats -> 11 beats), so every note time of
    // the block maps to 24 + (t - 24) * 0.5 and the lengths halve. MoveAlong
    // shifts line 4 by the end offset -11 beats.
    let text = standard(5);
    let out = transformed(&text, 2, 4, ms_of(24), ms_of(35), Follow::MoveAlong);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (12, 4),
            (18, 4),
            (24, 2),
            (27, 2),
            (30, 2),
            (33, 2),
            (37, 4),
            (43, 4),
        ],
        "the block halves, line 4 shifts by the end offset -11"
    );
    // The block's markers keep their 0.5 fractions of the new gaps; the
    // marker of line 2 rounds 29.5 up onto the next note's start.
    assert_eq!(markers(&out), vec![11, 23, 30, 36]);
}

#[test]
fn stay_keeps_the_following_lines_and_resolves_the_boundary_overlap() {
    // The block grows by 1.2 into line 4's first note (the block's last
    // note maps to 46..50, line 4 starts at 48): the pass shortens the
    // block's last note to end at 48, and line 4 keeps its place. The
    // block's end target yields to the following notes here - the pass
    // resolves the boundary overlap, not the end pin.
    let text = standard(5);
    let out = transformed(&text, 2, 4, ms_of(24), 1000.0 + 50.4 * 50.0, Follow::Stay);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (12, 4),
            (18, 4),
            (24, 5),
            (31, 5),
            (38, 5),
            (46, 2),
            (48, 4),
            (54, 4),
        ],
        "the block grows by 1.2, its last note shortens to beat 48, line 4 stays"
    );
    // The block's markers keep their 0.5 fractions of the new gaps; line
    // 3's marker sits on the shrunken gap 48..48.
    assert_eq!(markers(&out), vec![11, 23, 37, 48]);
}

#[test]
fn a_deep_stay_overlap_crushes_the_boundary_notes_and_pushes() {
    // The block doubles (22 beats -> 44 beats), so its last note ends at
    // beat 68, right of line 4's untouched notes at 48 and 54. No
    // shortening can reach that far back, so the pass crushes the
    // boundary notes to 1 beat and pushes line 4's notes behind them
    // (the pass's rule, applied at the boundary).
    let text = standard(5);
    let out = transformed(&text, 2, 4, ms_of(24), ms_of(68), Follow::Stay);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (12, 4),
            (18, 4),
            (24, 8),
            (36, 8),
            (48, 8),
            (60, 1),
            (61, 1),
            (62, 4),
        ],
        "the block doubles, the boundary notes crush to 1 beat, line 4 pushes"
    );
    // Line 3's marker sits on the crushed gap 61..61.
    assert_eq!(markers(&out), vec![11, 23, 46, 61]);
}

#[test]
fn move_along_shifts_the_following_lines_so_no_overlap_arises() {
    // The same doubled block with MoveAlong: line 4 shifts by the end
    // offset +22 beats, so no note overlaps and the pass changes nothing
    // beyond the markers.
    let text = standard(5);
    let out = transformed(&text, 2, 4, ms_of(24), ms_of(68), Follow::MoveAlong);
    assert_eq!(
        spans(&out),
        vec![
            (0, 4),
            (6, 4),
            (12, 4),
            (18, 4),
            (24, 8),
            (36, 8),
            (48, 8),
            (60, 8),
            (70, 4),
            (76, 4),
        ],
        "the block doubles and line 4 follows by 22 beats"
    );
    assert_eq!(markers(&out), vec![11, 23, 46, 69]);
}

#[test]
fn a_single_line_block_scales_within_the_line() {
    // One line as the block: the first note's start pins the new start and
    // the last note's end pins the new end, so the line's own times scale
    // with 2/10 (its second note is pushed to the 1-beat minimum by the
    // pass's length rule) and MoveAlong shifts line 2 by the end offset -8.
    let text = standard(3);
    let out = transformed(&text, 1, 2, ms_of(12), ms_of(14), Follow::MoveAlong);
    assert_eq!(
        spans(&out),
        vec![(0, 4), (6, 4), (12, 1), (13, 1), (16, 4), (22, 4)],
        "line 1 compresses onto beats 12 to 14, line 2 follows by -8"
    );
    assert_eq!(markers(&out), vec![11, 15]);
}

#[test]
fn the_block_lands_on_its_targets_within_half_a_beat() {
    // The plan's anchor invariant, read from the written song: the block's
    // first note starts within 0.5 beat of the new start and its last note
    // ends within 0.5 beat of the new end.
    let text = standard(5);
    let (doc, song) = loaded(&text);
    let patch = block_transform(
        &doc,
        &song,
        0,
        2..4,
        ms_of(24),
        ms_of(35),
        Follow::MoveAlong,
    )
    .expect("the block plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let view = Song::from_document(&out);
    let timing = view.timing.expect("the timing survives");
    let first = view
        .note(&out, view.voices[0].lines[2].notes[0])
        .expect("the note exists");
    let last = view
        .note(&out, view.voices[0].lines[3].notes[1])
        .expect("the note exists");
    assert!(
        (timing.beat_to_ms(first.start) - ms_of(24)).abs() <= 0.5 * 15000.0 / timing.bpm,
        "the block starts at {} ms",
        timing.beat_to_ms(first.start)
    );
    assert!(
        (timing.beat_to_ms(last.start + last.length as f64) - ms_of(35)).abs()
            <= 0.5 * 15000.0 / timing.bpm,
        "the block ends at {} ms",
        timing.beat_to_ms(last.start + last.length as f64)
    );
    assert_eq!(view.voices[0].lines.len(), 5, "every line kept its notes");
}

#[test]
fn an_empty_or_beyond_range_and_reversed_times_are_refused() {
    let text = standard(3);
    assert!(matches!(
        error_of(&text, 1, 1, ms_of(12), ms_of(22), Follow::Stay),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        error_of(&text, 0, 9, ms_of(0), ms_of(46), Follow::Stay),
        EditError::StalePatch(_)
    ));
    assert!(matches!(
        error_of(&text, 1, 2, ms_of(14), ms_of(12), Follow::Stay),
        EditError::AnchorsOutOfOrder
    ));
    assert!(matches!(
        error_of(&text, 1, 2, ms_of(14), ms_of(14), Follow::Stay),
        EditError::AnchorsOutOfOrder
    ));
}

#[test]
fn a_non_finite_time_is_refused() {
    let text = standard(3);
    assert!(matches!(
        error_of(&text, 1, 2, f64::NAN, ms_of(14), Follow::Stay),
        EditError::InvalidHeaderValue
    ));
    assert!(matches!(
        error_of(&text, 1, 2, ms_of(12), f64::INFINITY, Follow::Stay),
        EditError::InvalidHeaderValue
    ));
}

#[test]
fn no_timing_and_relative_songs_are_refused() {
    let no_timing = "#TITLE:T\n: 0 4 0 a \nE\n";
    assert!(matches!(
        error_of(no_timing, 0, 1, ms_of(0), ms_of(4), Follow::Stay),
        EditError::NoTiming
    ));
    let relative = "#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 la\n: 4 4 1 la\n- 8 1\nE\n";
    assert!(matches!(
        error_of(relative, 0, 1, ms_of(0), ms_of(4), Follow::Stay),
        EditError::RelativeModeIsReadOnly
    ));
}
