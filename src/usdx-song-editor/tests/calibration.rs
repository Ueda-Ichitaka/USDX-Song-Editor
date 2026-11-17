//! About me: tests for the tap latency calibration (UI_SPEC 5.7, task
//! P7-08). Eight clicks play one second apart, the first at 0 ms; each Space
//! press records the playback position. A tap counts for the click it is
//! nearest to when it lies within 300 ms of it, the first such tap per click
//! wins; its offset is the tap time minus the click time. The proposed
//! latency is the median of the counted offsets, rounded to whole
//! milliseconds; the round is complete when every click has a tap. The
//! click now playing follows the playback position.

use usdx_song_editor::controller::calibration::{CLICKS, Calibration};

#[test]
fn a_tap_counts_for_its_nearest_click() {
    let mut round = Calibration::new();
    round.tap(34.0);
    round.tap(2038.0);
    assert_eq!(
        round.offsets(),
        vec![Some(34.0), None, Some(38.0), None, None, None, None, None]
    );
}

#[test]
fn a_tap_far_from_every_click_is_ignored() {
    let mut round = Calibration::new();
    round.tap(1450.0);
    round.tap(-350.0);
    round.tap(7400.0);
    assert!(round.offsets().iter().all(Option::is_none));
}

#[test]
fn an_early_tap_counts_with_a_negative_offset() {
    let mut round = Calibration::new();
    round.tap(2980.0);
    assert_eq!(round.offsets()[3], Some(-20.0));
}

#[test]
fn the_first_tap_of_a_click_wins() {
    let mut round = Calibration::new();
    round.tap(1040.0);
    round.tap(1100.0);
    assert_eq!(round.offsets()[1], Some(40.0));
}

#[test]
fn the_median_rounds_to_whole_milliseconds() {
    let mut round = Calibration::new();
    assert_eq!(round.median_ms(), None, "no tap, no median");
    for (click, offset) in [34.0, 38.0, 36.0, 31.0, 37.0].iter().enumerate() {
        round.tap(click as f64 * 1000.0 + offset);
    }
    assert_eq!(round.median_ms(), Some(36));
    // An even count takes the mean of the two middle offsets.
    round.tap(5000.0 + 40.0);
    assert_eq!(round.median_ms(), Some(37));
}

#[test]
fn the_round_is_complete_when_every_click_has_a_tap() {
    let mut round = Calibration::new();
    for click in 0..CLICKS - 1 {
        round.tap(click as f64 * 1000.0 + 30.0);
    }
    assert!(!round.complete());
    round.tap((CLICKS - 1) as f64 * 1000.0 + 30.0);
    assert!(round.complete());
    assert_eq!(CLICKS, 8);
}

#[test]
fn the_click_now_playing_follows_the_position() {
    assert_eq!(Calibration::click_at(0.0), 0);
    assert_eq!(Calibration::click_at(999.0), 0);
    assert_eq!(Calibration::click_at(1000.0), 1);
    assert_eq!(Calibration::click_at(7600.0), 7);
    assert_eq!(Calibration::click_at(9000.0), 7);
    assert_eq!(Calibration::click_at(-5.0), 0);
}

#[test]
fn start_again_clears_the_taps() {
    let mut round = Calibration::new();
    round.tap(30.0);
    round.clear();
    assert!(round.offsets().iter().all(Option::is_none));
}
