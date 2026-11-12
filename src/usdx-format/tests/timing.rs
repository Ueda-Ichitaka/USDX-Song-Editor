//! About me: tests for the beat-to-millisecond timing (task P1-04, plan
//! section 5.5). They assert the values of the plan's sample song "Realize The
//! Real Lies" (`#BPM:369.14`, `#GAP:14937`), the round trip
//! `ms_to_beat(beat_to_ms(b)) == b` within 1e-9 and the None cases (no `#BPM`
//! header, a `#BPM` of 0).

use usdx_format::bytes::decode;
use usdx_format::headers::Headers;
use usdx_format::rows::classify;
use usdx_format::timing::Timing;

/// timing builds the timing of a song text.
fn timing(text: &str) -> Option<Timing> {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    Timing::from_headers(&Headers::from_document(&doc))
}

/// assert_close checks that two values are within the tolerance.
fn assert_close(left: f64, right: f64, tolerance: f64, what: &str) {
    assert!(
        (left - right).abs() <= tolerance,
        "{what}: {left} is not within {tolerance} of {right}"
    );
}

#[test]
fn the_timing_comes_from_the_sample_song_headers() {
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    assert_eq!(t.bpm, 369.14);
    assert_eq!(t.gap_ms, 14_937.0);
}

#[test]
fn beat_zero_is_at_the_gap() {
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    assert_close(t.beat_to_ms(0.0), 14_937.0, 1e-9, "beat 0");
}

#[test]
fn one_beat_is_about_40_635_ms() {
    // The plan's prose says 40.634 ms; the plan's own formula
    // 60000 / (369.14 * 4) gives 40.63499 ms, which rounds to 40.635.
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    assert_close(t.ms_per_beat(), 40.635, 5e-4, "one beat");
}

#[test]
fn beat_161_is_at_about_21479_2_ms() {
    // The plan's prose says 21 479.1 ms (computed from a truncated ms-per-beat);
    // the exact formula gives 14937 + 161 * 40.63499 = 21 479.23 ms.
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    assert_close(t.beat_to_ms(161.0), 21_479.23, 0.1, "beat 161");
}

#[test]
fn the_round_trip_holds_within_1e_9() {
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    for beat in [-10.0, -0.5, 0.0, 0.25, 1.0, 161.0, 161.5, 1e6] {
        let back = t.ms_to_beat(t.beat_to_ms(beat));
        assert_close(back, beat, 1e-9, "the beat round trip");
    }
}

#[test]
fn the_millisecond_round_trip_holds_from_the_gap_onwards() {
    let t = timing("#BPM:369.14\n#GAP:14937\nE\n").expect("the sample song has a valid BPM");
    for ms in [14_937.0, 15_000.0, 21_479.1, 1e6] {
        let back = t.beat_to_ms(t.ms_to_beat(ms));
        assert_close(back, ms, 1e-9, "the millisecond round trip");
    }
}

#[test]
fn a_song_without_a_bpm_header_has_no_timing() {
    assert_eq!(timing("#TITLE:x\nE\n"), None);
}

#[test]
fn a_bpm_of_zero_has_no_timing() {
    assert_eq!(timing("#BPM:0\nE\n"), None);
}

#[test]
fn a_bpm_that_does_not_parse_has_no_timing() {
    // The time header syntax has no sign, so a negative BPM is no valid BPM.
    assert_eq!(timing("#BPM:-120\nE\n"), None);
    assert_eq!(timing("#BPM:120,5x\nE\n"), None);
}

#[test]
fn a_missing_gap_means_zero() {
    let t = timing("#BPM:120\nE\n").expect("a plain BPM builds a timing");
    assert_eq!(t.gap_ms, 0.0);
    assert_close(t.beat_to_ms(0.0), 0.0, 1e-9, "beat 0 without a gap");
}

#[test]
fn a_bpm_with_a_decimal_comma_builds_the_timing() {
    // The v1 format allows the decimal comma; a value of 120.5 is quadrupled
    // implicitly, so one beat is 60000 / (120.5 * 4) ms.
    let t = timing("#BPM:120,5\nE\n").expect("the decimal comma parses");
    assert_eq!(t.bpm, 120.5);
    assert_close(t.ms_per_beat(), 60000.0 / (120.5 * 4.0), 1e-9, "one beat");
}
