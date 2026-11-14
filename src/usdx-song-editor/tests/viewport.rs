//! About me: tests for the sheet viewport of task P5-01 (plan 8.2, UI_SPEC
//! 3.2). The pure beat/pixel and pitch/pixel mapping is tested without QML:
//! the round trips, the zoom around the cursor with its 1.0 to 40.0 clamp,
//! the wheel scrolling rules, the middle-drag pan, the rounded drag deltas
//! and the fit of a beat range (the follow-line rule of view.rs lands here).

use usdx_song_editor::controller::viewport::{FitKey, SheetViewport};

/// A viewport of 1000 px width and 500 px pitch-area height.
fn viewport() -> SheetViewport {
    SheetViewport::new(1000.0, 500.0)
}

/// Two f64 values equal within 1e-9.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn defaults_are_twenty_px_per_beat_with_c4_on_top() {
    let v = viewport();
    assert_eq!(v.px_per_beat, 20.0, "user decision 2026-11-13: 20 px/beat");
    assert_eq!(v.start_beat, 0.0);
    assert_eq!(v.top_pitch, 60.0, "C4 on top until the pitch rule runs");
    assert_eq!(
        v.px_per_semitone, 14.0,
        "the spec's minimum row height until the pitch rule runs"
    );
}

#[test]
fn beat_and_pitch_mappings_round_trip() {
    let v = viewport();
    for beat in [-10.0, 0.0, 7.5, 123.4] {
        assert!(
            close(v.x_to_beat(v.beat_to_x(beat)), beat),
            "beat {beat} round trips"
        );
    }
    for pitch in [48.0, 55.0, 60.0, 72.5] {
        assert!(
            close(v.y_to_pitch(v.pitch_to_y(pitch)), pitch),
            "pitch {pitch} round trips"
        );
    }
}

#[test]
fn pixel_mapping_starts_at_the_view_origin() {
    let mut v = viewport();
    v.start_beat = 100.0;
    assert_eq!(v.beat_to_x(100.0), 0.0, "the view start sits at x 0");
    assert_eq!(v.beat_to_x(110.0), 200.0, "10 beats at 20 px/beat");
    assert_eq!(v.pitch_to_y(60.0), 0.0, "the top pitch sits at y 0");
    assert_eq!(v.pitch_to_y(55.0), 70.0, "5 semitones at 14 px each");
}

#[test]
fn end_beat_reports_the_visible_range() {
    let v = viewport();
    assert_eq!(v.visible_beats(), 50.0, "1000 px at 20 px/beat");
    assert_eq!(v.end_beat(), 50.0);
}

#[test]
fn zoom_around_the_cursor_keeps_the_beat_under_the_cursor() {
    let mut v = viewport();
    // x 400 shows beat 20.0; zooming in around it keeps it at x 400.
    v.zoom_around(1.25, 400.0);
    assert_eq!(v.px_per_beat, 25.0, "one wheel step multiplies by 1.25");
    assert!(
        close(v.x_to_beat(400.0), 20.0),
        "the beat under the cursor stays, got {}",
        v.x_to_beat(400.0)
    );
    assert!(close(v.start_beat, 4.0));
}

#[test]
fn zoom_out_keeps_the_beat_under_the_cursor_too() {
    let mut v = viewport();
    v.px_per_beat = 25.0;
    v.start_beat = 4.0;
    v.zoom_around(0.8, 400.0);
    assert!(close(v.px_per_beat, 20.0));
    assert!(close(v.x_to_beat(400.0), 20.0));
}

#[test]
fn zoom_clamps_px_per_beat() {
    let mut v = viewport();
    v.px_per_beat = 40.0;
    v.zoom_around(1.25, 0.0);
    assert_eq!(v.px_per_beat, 40.0, "the maximum is 40.0");
    v.px_per_beat = 1.0;
    v.zoom_around(0.8, 0.0);
    assert_eq!(v.px_per_beat, 1.0, "the minimum is 1.0");
}

#[test]
fn wheel_scrolls_time_by_ten_percent_of_the_visible_width_per_step() {
    let mut v = viewport();
    v.wheel_time(1.0);
    assert!(
        close(v.start_beat, 5.0),
        "one step is 10 % of 50 visible beats"
    );
    v.wheel_time(-2.0);
    assert!(close(v.start_beat, -5.0), "negative steps scroll back");
}

#[test]
fn wheel_scrolls_the_pitch_range_by_one_row_per_step() {
    let mut v = viewport();
    v.wheel_pitch(1.0);
    assert_eq!(v.top_pitch, 59.0, "one row down shows one semitone lower");
    v.wheel_pitch(-2.0);
    assert_eq!(v.top_pitch, 61.0);
}

#[test]
fn middle_drag_pans_the_content_with_the_pointer() {
    let mut v = viewport();
    v.pan_time(100.0);
    assert!(
        close(v.start_beat, -5.0),
        "dragging right shows earlier beats"
    );
    v.pan_time(-50.0);
    assert!(close(v.start_beat, -2.5));
}

#[test]
fn drag_delta_rounds_to_whole_beats() {
    let v = viewport();
    assert_eq!(
        v.drag_delta_beats(10.0),
        1,
        "0.5 beats rounds away from zero"
    );
    assert_eq!(v.drag_delta_beats(9.0), 0, "0.45 beats rounds to none");
    assert_eq!(
        v.drag_delta_beats(30.0),
        2,
        "1.5 beats rounds away from zero"
    );
    assert_eq!(v.drag_delta_beats(-10.0), -1, "rounding works left too");
    assert_eq!(v.drag_delta_beats(-9.0), 0);
}

#[test]
fn fit_range_scales_the_zoom_to_the_range() {
    let mut v = viewport();
    v.fit_range(100.0, 140.0);
    assert!(
        close(v.px_per_beat, 25.0),
        "1000 px over 40 beats, got {}",
        v.px_per_beat
    );
    assert!(close(v.start_beat, 100.0));
    assert!(close(v.end_beat(), 140.0));
}

#[test]
fn fit_range_centers_when_the_maximum_zoom_clamps() {
    let mut v = viewport();
    v.fit_range(0.0, 10.0);
    assert_eq!(
        v.px_per_beat, 40.0,
        "10 beats would need 100 px/beat, clamped"
    );
    assert!(close(v.start_beat, -7.5), "the range stays centred");
    assert!(close(v.end_beat(), 17.5));
    assert!(close(v.x_to_beat(0.0) + v.x_to_beat(1000.0), 10.0));
}

#[test]
fn fit_range_centers_when_the_minimum_zoom_clamps() {
    let mut v = viewport();
    v.fit_range(0.0, 5000.0);
    assert_eq!(v.px_per_beat, 1.0, "5000 beats would need 0.2 px/beat");
    assert!(close(v.start_beat, 2000.0), "the range stays centred");
    assert!(close(v.end_beat(), 3000.0));
}

#[test]
fn set_size_changes_the_visible_range() {
    let mut v = viewport();
    v.set_size(500.0, 300.0);
    assert_eq!(v.width_px, 500.0);
    assert_eq!(v.height_px, 300.0);
    assert_eq!(v.visible_beats(), 25.0, "500 px at 20 px/beat");
}

#[test]
fn fit_key_is_current_while_the_range_and_the_size_stay() {
    let key = FitKey::new((-36.9, 93.0), (979.0, 674.0));
    assert!(
        !key.stale((-36.9, 93.0), (979.0, 674.0)),
        "the same range and size need no new fit"
    );
}

#[test]
fn fit_key_stales_when_the_range_moves() {
    let key = FitKey::new((0.0, 10.0), (979.0, 674.0));
    assert!(
        key.stale((5.0, 15.0), (979.0, 674.0)),
        "a moved beat range needs a new fit (the follow-line rule)"
    );
}

#[test]
fn fit_key_stales_when_the_size_changes() {
    // The acceptance bug: a fit that ran at size 0x0 survived the resize
    // because only the range was compared, leaving the 1 px/beat clamp.
    let key = FitKey::new((-36.9, 93.0), (0.0, 0.0));
    assert!(
        key.stale((-36.9, 93.0), (979.0, 674.0)),
        "a resize needs a new fit even when the range stays"
    );
}

#[test]
fn fit_key_defaults_to_the_zero_range_and_size() {
    let key = FitKey::default();
    assert!(!key.stale((0.0, 0.0), (0.0, 0.0)));
    assert!(
        key.stale((-36.9, 93.0), (0.0, 0.0)),
        "the first real range always fits"
    );
}
