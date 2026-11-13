//! About me: tests for the default time view of the note sheet (UI_SPEC 3.2,
//! task P4-04, taken over by the SheetViewport of P5-01). They assert the
//! padding of max(1.5 s, 50 % of the line's duration) on both sides, that the
//! view stays put while the new current line is fully visible, and that it
//! moves to the new line once any part of it is outside.

use usdx_song_editor::controller::view::{TimeView, follow_line};

#[test]
fn a_short_line_gets_one_and_a_half_seconds_on_both_sides() {
    let view = follow_line(None, 10_000.0, 12_000.0);
    assert_eq!(
        view,
        TimeView {
            start_ms: 8_500.0,
            end_ms: 13_500.0
        }
    );
}

#[test]
fn a_long_line_gets_half_its_duration_on_both_sides() {
    let view = follow_line(None, 10_000.0, 16_000.0);
    assert_eq!(
        view,
        TimeView {
            start_ms: 7_000.0,
            end_ms: 19_000.0
        }
    );
}

#[test]
fn a_line_inside_the_view_keeps_the_view() {
    let previous = TimeView {
        start_ms: 8_500.0,
        end_ms: 13_500.0,
    };
    let view = follow_line(Some(previous), 12_100.0, 13_400.0);
    assert_eq!(
        view, previous,
        "stepping to a visible neighbour does not jump"
    );
}

#[test]
fn a_line_partly_outside_the_view_moves_the_view() {
    let previous = TimeView {
        start_ms: 8_500.0,
        end_ms: 13_500.0,
    };
    let view = follow_line(Some(previous), 13_000.0, 14_000.0);
    assert_eq!(
        view,
        TimeView {
            start_ms: 11_500.0,
            end_ms: 15_500.0
        }
    );
}

#[test]
fn an_empty_line_still_gets_the_padding() {
    let view = follow_line(None, 5_000.0, 5_000.0);
    assert_eq!(
        view,
        TimeView {
            start_ms: 3_500.0,
            end_ms: 6_500.0
        }
    );
}
