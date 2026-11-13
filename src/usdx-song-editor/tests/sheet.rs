//! About me: tests for the full-sheet draw-list layout of task P5-02
//! (plan section 8.3, UI_SPEC section 3). The pure `layout` math is tested
//! without QML: the vertical stack of backgrounds (ruler 22 px, pitch rows,
//! lyrics lane 34 px, 8 px gap, waveform 54 px, activity 6 px), the pitch
//! range and row height rules with the alternate shading and grid lines,
//! the pitch label rules, the ruler tick interval choice and its song-time
//! labels, the two-row lyrics placement (the "or"/"just" case of the
//! reference song lands on different rows), the gutter labels and the
//! waveform and activity columns reused from the lane math.

use usdx_audio::analysis::{Activity, Peaks};
use usdx_song_editor::controller::sheet::{
    ColorRole, LyricNote, SheetGeometry, SheetInputs, SheetOp, TextFont, layout, line_notes,
    pitch_rows, ruler_label, tick_interval_s,
};

/// The pitch area height of the 300 px geometry.
const PITCH_AREA: f64 = 300.0 - 22.0 - 34.0 - 8.0 - 54.0 - 6.0;
/// The top of the lyrics lane (below ruler and pitch rows).
const LYRICS_TOP: f64 = 22.0 + PITCH_AREA;
/// The top of the waveform lane.
const WAVE_TOP: f64 = LYRICS_TOP + 34.0 + 8.0;
/// The top of the activity lane.
const ACTIVITY_TOP: f64 = WAVE_TOP + 54.0;

/// A geometry of 500 px width and 300 px height with the sheet heights of
/// UI_SPEC 3.1; the pitch values come from `pitch_rows` like the bridge does.
fn geom(line_pitch: Option<(i32, i32)>) -> SheetGeometry {
    let (top_pitch, px_per_semitone) = match line_pitch {
        Some((low, high)) => pitch_rows(low, high, PITCH_AREA),
        None => (67, 14.0),
    };
    SheetGeometry {
        width: 500.0,
        height: 300.0,
        gutter_width: 46.0,
        ruler_height: 22.0,
        lyrics_height: 34.0,
        gap_height: 8.0,
        waveform_height: 54.0,
        activity_height: 6.0,
        show_lyrics_lane: true,
        start_beat: 0.0,
        px_per_beat: 20.0,
        top_pitch,
        px_per_semitone,
        bpm: 369.14,
        gap_ms: 14937.0,
    }
}

/// Width of a 10 px per character font, 3 px baseline offset.
type MeasureFn = Box<dyn Fn(TextFont, &str) -> f64>;
type BaselineFn = Box<dyn Fn(TextFont) -> f64>;

fn metrics() -> (MeasureFn, BaselineFn) {
    (
        Box::new(|_font: TextFont, text: &str| 10.0 * text.chars().count() as f64),
        Box::new(|_font: TextFont| 3.0),
    )
}

/// Inputs without audio and without notes.
fn inputs<'a>(
    notes: &'a [LyricNote],
    line_pitch: Option<(i32, i32)>,
    measure: &'a (dyn Fn(TextFont, &str) -> f64 + 'a),
    baseline: &'a (dyn Fn(TextFont) -> f64 + 'a),
) -> SheetInputs<'a> {
    SheetInputs {
        peaks: None,
        activity: None,
        notes,
        track_name: "Mix",
        line_pitch,
        measure,
        baseline_offset: baseline,
    }
}

/// All ops matching the predicate.
fn all(ops: &[SheetOp], pred: impl Fn(&SheetOp) -> bool) -> Vec<SheetOp> {
    ops.iter().filter(|op| pred(op)).cloned().collect()
}

/// The single op matching the predicate.
fn one(ops: &[SheetOp], pred: impl Fn(&SheetOp) -> bool) -> SheetOp {
    let found = all(ops, pred);
    assert_eq!(found.len(), 1, "exactly one op expected, got {found:?}");
    found[0].clone()
}

/// The background rect of the given colour at the given y.
fn rect_at(ops: &[SheetOp], y: f64, color: ColorRole) -> SheetOp {
    one(ops, |op| match op {
        SheetOp::Rect {
            x,
            y: top,
            width,
            height,
            color: rect_color,
        } => *x == 0.0 && *top == y && *width == 500.0 && *height > 0.0 && *rect_color == color,
        _ => false,
    })
}

/// The text op carrying the given text.
fn text(ops: &[SheetOp], content: &str) -> SheetOp {
    one(ops, |op| match op {
        SheetOp::Text { text, .. } => text == content,
        _ => false,
    })
}

#[test]
fn the_stack_paints_the_six_backgrounds_in_order() {
    let geometry = geom(None);
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], None, measure.as_ref(), baseline.as_ref()),
    );
    rect_at(&ops, 0.0, ColorRole::HeaderBackground);
    rect_at(&ops, 22.0, ColorRole::ViewBackground);
    rect_at(&ops, LYRICS_TOP, ColorRole::WindowBackground);
    rect_at(&ops, LYRICS_TOP + 34.0, ColorRole::ViewBackground);
    rect_at(&ops, WAVE_TOP, ColorRole::WindowBackground);
    rect_at(&ops, ACTIVITY_TOP, ColorRole::ViewBackground);
    // The ruler background comes first so everything paints over it.
    assert!(
        matches!(
            &ops[0],
            SheetOp::Rect {
                y: 0.0,
                color: ColorRole::HeaderBackground,
                ..
            }
        ),
        "first op is the ruler background, got {:?}",
        ops[0]
    );
}

#[test]
fn pitch_rows_extends_by_three_and_symmetrically_to_fourteen_rows() {
    // One note at pitch 60: 57..63 is 7 rows, extended by pad 7 (3 down,
    // 4 up) to 54..67, and 200 px over 14 rows gives a row height above 14.
    let (top, row_height) = pitch_rows(60, 60, 200.0);
    assert_eq!(top, 67);
    assert!((row_height - 200.0 / 14.0).abs() < 1e-9);

    // An 11-semitone line: 52..68 is 17 rows, no symmetric extension needed.
    let (top, row_height) = pitch_rows(55, 65, 300.0);
    assert_eq!(top, 68);
    assert!((row_height - 300.0 / 17.0).abs() < 1e-9);
}

#[test]
fn pitch_rows_clamps_the_row_height_at_14_and_centers_the_range() {
    // 17 rows need 238 px at 14 px; over 200 px the height clamps to 14 px,
    // 14 rows are visible and the range is centred (1 row hidden above,
    // 2 below).
    let (top, row_height) = pitch_rows(55, 65, 200.0);
    assert_eq!(row_height, 14.0);
    assert_eq!(top, 67);

    // Over 100 px only 7 rows are visible; 5 of the 17 are cut below.
    let (top, _row_height) = pitch_rows(55, 65, 100.0);
    assert_eq!(top, 63);
}

#[test]
fn rows_shade_c_and_the_black_keys_with_grid_lines() {
    // One note at C4 (60) over 176 px: pitch_rows clamps the 14 extended
    // rows to 14 px each, 12 rows visible, so rows 66 down to 55 paint.
    let geometry = geom(Some((60, 60)));
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], Some((60, 60)), measure.as_ref(), baseline.as_ref()),
    );

    let row_y = |pitch: i32| 22.0 + (geometry.top_pitch - pitch) as f64 * 14.0;
    // Alternate colour for C (60) and the black keys C# (61), D# (63),
    // F# (66), G# (56), A# (58); view background for the rest.
    for pitch in [60, 61, 63, 66, 56, 58] {
        rect_at(&ops, row_y(pitch), ColorRole::Alternate);
    }
    for pitch in [65, 64, 62, 59, 57, 55] {
        let shaded = all(&ops, |op| match op {
            SheetOp::Rect { y, color, .. } => *y == row_y(pitch) && *color == ColorRole::Alternate,
            _ => false,
        });
        assert!(shaded.is_empty(), "pitch {pitch} must not be shaded");
    }

    // A 1 px grid line under every row; the line under a C row is strong.
    for pitch in [66, 60, 59] {
        let expected_color = if pitch % 12 == 0 {
            ColorRole::StrongGridLine
        } else {
            ColorRole::GridLine
        };
        one(&ops, |op| match op {
            SheetOp::Rect {
                y, height, color, ..
            } => *height == 1.0 && *y == row_y(pitch) + 13.0 && *color == expected_color,
            _ => false,
        });
    }
}

#[test]
fn pitch_labels_are_right_aligned_and_centered() {
    // Row height 14 px is below 18: only C rows and even pitches labelled.
    let geometry = geom(Some((60, 60)));
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], Some((60, 60)), measure.as_ref(), baseline.as_ref()),
    );

    for present in ["F#4", "E4", "D4", "C4", "A#3", "G#3"] {
        text(&ops, present);
    }
    for absent in ["C#4", "D#4", "F4", "G4", "A4", "B3", "A#4", "G#4"] {
        let found = all(&ops, |op| match op {
            SheetOp::Text { text, .. } => text == absent,
            _ => false,
        });
        assert!(found.is_empty(), "{absent} must not be labelled");
    }

    // Right aligned 6 px from the gutter's right edge, vertically centred:
    // C4 sits 6 rows below the top pitch, its label is 2 characters wide.
    let c4 = text(&ops, "C4");
    match &c4 {
        SheetOp::Text {
            x, y, font, color, ..
        } => {
            assert!((*x - (46.0 - 6.0 - 20.0)).abs() < 1e-9, "x {x}");
            let expected_y = 22.0 + 6.0 * 14.0 + 7.0 + 3.0;
            assert!(
                (*y - expected_y).abs() < 1e-9,
                "y {y} expected {expected_y}"
            );
            assert_eq!(*font, TextFont::SmallDemi);
            assert_eq!(*color, ColorRole::Text);
        }
        _ => panic!("a text op"),
    }
    // Non-C labels use the small font in the disabled text colour.
    match &text(&ops, "F#4") {
        SheetOp::Text { font, color, .. } => {
            assert_eq!(*font, TextFont::Small);
            assert_eq!(*color, ColorRole::DisabledText);
        }
        _ => panic!("a text op"),
    }
}

#[test]
fn pitch_labels_name_every_row_when_the_row_height_reaches_18_px() {
    // Over 276 px the 14 rows are 19.7 px tall: every row is labelled.
    let pitch_height = 400.0 - 22.0 - 34.0 - 8.0 - 54.0 - 6.0;
    let (top_pitch, px_per_semitone) = pitch_rows(60, 60, pitch_height);
    let mut geometry = geom(Some((60, 60)));
    geometry.height = 400.0;
    geometry.top_pitch = top_pitch;
    geometry.px_per_semitone = px_per_semitone;
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], Some((60, 60)), measure.as_ref(), baseline.as_ref()),
    );
    for present in ["C#4", "D#4", "F4", "G4", "B3"] {
        text(&ops, present);
    }
}

#[test]
fn tick_interval_is_the_smallest_with_70_px_between_ticks() {
    assert_eq!(tick_interval_s(492.2), 0.5);
    assert_eq!(tick_interval_s(24.6), 5.0);
    assert_eq!(tick_interval_s(1.0), 60.0);
    assert_eq!(tick_interval_s(70.0), 1.0, "70 px apart allows 1 s");
}

#[test]
fn ruler_labels_format_m_ss_s() {
    assert_eq!(ruler_label(0.0), "0:00.0");
    assert_eq!(ruler_label(20000.0), "0:20.0");
    assert_eq!(ruler_label(61340.0), "1:01.3");
}

#[test]
fn ruler_ticks_and_labels_lay_on_song_time() {
    // Beats 100 to 150 at 20 px per beat (a 1000 px view), bpm 369.14,
    // gap 14937 ms: the view spans 19000.5 to 21032.25 ms, so the ticks sit
    // at 19.5, 20, 20.5 and 21 s.
    let mut geometry = geom(None);
    geometry.start_beat = 100.0;
    geometry.width = 1000.0;
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], None, measure.as_ref(), baseline.as_ref()),
    );

    let ms_per_beat = 15000.0 / 369.14;
    for (tick_ms, label) in [
        (19500.0, "0:19.5"),
        (20000.0, "0:20.0"),
        (20500.0, "0:20.5"),
        (21000.0, "0:21.0"),
    ] {
        let expected_x =
            (tick_ms - geometry.gap_ms) / ms_per_beat * 20.0 - geometry.start_beat * 20.0;
        match &text(&ops, label) {
            SheetOp::Text {
                x, y, font, color, ..
            } => {
                // The label sits 3 px right of its tick.
                assert!(
                    (*x - expected_x - 3.0).abs() < 0.1,
                    "x {x} expected {expected_x}"
                );
                assert_eq!(*y, 14.0);
                assert_eq!(*font, TextFont::Small);
                assert_eq!(*color, ColorRole::DisabledText);
            }
            _ => panic!("a text op"),
        }
        // Each tick draws a grid line from 6 px above the ruler's bottom
        // through the pitch rows.
        one(&ops, |op| match op {
            SheetOp::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => {
                (*x1 - expected_x).abs() < 0.1
                    && *y1 == 16.0
                    && *x2 == *x1
                    && *y2 == LYRICS_TOP
                    && *color == ColorRole::GridLine
                    && *width == 1.0
            }
            _ => false,
        });
    }
}

/// The notes of the reference line "Are you free or just a slave of the
/// authorities" with the measured label widths the real font produces.
fn reference_line() -> Vec<LyricNote> {
    vec![
        LyricNote {
            start_beat: 161.0,
            text: "Are".into(),
            current: true,
        },
        LyricNote {
            start_beat: 165.0,
            text: "you".into(),
            current: true,
        },
        LyricNote {
            start_beat: 172.0,
            text: "free".into(),
            current: true,
        },
        LyricNote {
            start_beat: 189.0,
            text: "or".into(),
            current: true,
        },
        LyricNote {
            start_beat: 192.0,
            text: "just".into(),
            current: true,
        },
        LyricNote {
            start_beat: 206.0,
            text: "a".into(),
            current: true,
        },
        LyricNote {
            start_beat: 209.0,
            text: "slave".into(),
            current: true,
        },
        LyricNote {
            start_beat: 228.0,
            text: "of".into(),
            current: true,
        },
        LyricNote {
            start_beat: 232.0,
            text: "the".into(),
            current: true,
        },
        LyricNote {
            start_beat: 237.0,
            text: "~".into(),
            current: true,
        },
        LyricNote {
            start_beat: 246.0,
            text: "au".into(),
            current: true,
        },
        LyricNote {
            start_beat: 254.0,
            text: "thorities?".into(),
            current: true,
        },
    ]
}

#[test]
fn or_and_just_of_the_reference_song_land_in_different_rows() {
    // The reference view: the line with its padding needs about 4.3 px per
    // beat; "or" starts 3 beats before "just", so at that zoom the start of
    // "just" is not after the end of the "or" label.
    let mut geometry = geom(Some((60, 60)));
    geometry.start_beat = 140.0;
    geometry.px_per_beat = 4.3;
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(
            &reference_line(),
            Some((60, 60)),
            measure.as_ref(),
            baseline.as_ref(),
        ),
    );

    let row_of = |content: &str| match &text(&ops, content) {
        SheetOp::Text { y, .. } => *y - LYRICS_TOP,
        _ => panic!("a text op"),
    };
    assert!((row_of("or") - 14.0).abs() < 1e-9, "or on the first row");
    assert!(
        (row_of("just") - 29.0).abs() < 1e-9,
        "just on the second row"
    );
    assert!((row_of("Are") - 14.0).abs() < 1e-9);
    assert!((row_of("you") - 29.0).abs() < 1e-9);
    assert!((row_of("free") - 14.0).abs() < 1e-9);
}

#[test]
fn lyrics_labels_elide_at_the_next_label_start() {
    let mut geometry = geom(Some((60, 60)));
    geometry.start_beat = 140.0;
    geometry.px_per_beat = 4.3;
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(
            &reference_line(),
            Some((60, 60)),
            measure.as_ref(),
            baseline.as_ref(),
        ),
    );
    // "just" is elided at the start of "a": 14 beats later at 4.3 px.
    match &text(&ops, "just") {
        SheetOp::Text { x, max_width, .. } => {
            let expected = (206.0 - 192.0) * 4.3;
            assert!(
                (*max_width - expected).abs() < 1e-9,
                "max width {max_width}"
            );
            assert!(*x > 0.0);
        }
        _ => panic!("a text op"),
    }
}

#[test]
fn lyrics_labels_use_the_current_font_and_the_faded_neighbour_style() {
    let mut geometry = geom(Some((60, 60)));
    geometry.start_beat = 140.0;
    geometry.px_per_beat = 4.3;
    let mut notes = reference_line();
    notes.push(LyricNote {
        start_beat: 270.0,
        text: "Don't".into(),
        current: false,
    });
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&notes, Some((60, 60)), measure.as_ref(), baseline.as_ref()),
    );
    match &text(&ops, "Are") {
        SheetOp::Text {
            font,
            color,
            opacity,
            ..
        } => {
            assert_eq!(*font, TextFont::Lyrics);
            assert_eq!(*color, ColorRole::Text);
            assert!((*opacity - 1.0).abs() < 1e-9);
        }
        _ => panic!("a text op"),
    }
    match &text(&ops, "Don't") {
        SheetOp::Text {
            font,
            color,
            opacity,
            ..
        } => {
            assert_eq!(*font, TextFont::LyricsNeighbour);
            assert_eq!(*color, ColorRole::DisabledText);
            assert!((*opacity - 0.7).abs() < 1e-9);
        }
        _ => panic!("a text op"),
    }
}

fn peaks() -> Peaks {
    Peaks {
        bucket_ms: 1000.0,
        min: vec![-0.5, -0.5, -0.5],
        max: vec![0.5, 0.5, 0.5],
        rms: vec![0.0; 3],
    }
}

fn activity() -> Activity {
    Activity {
        bucket_ms: 1000.0,
        active: vec![true, false, true],
        floor_db: -30.0,
    }
}

#[test]
fn waveform_and_activity_columns_fill_their_lanes() {
    // The peaks cover the first 3 s, so this view starts at song time 0;
    // a 1000 px view shows 50 beats = about 2032 ms of song time.
    let mut geometry = geom(None);
    geometry.gap_ms = 0.0;
    geometry.width = 1000.0;
    let (measure, baseline) = metrics();
    let peaks = peaks();
    let activity = activity();
    let mut sheet_inputs = inputs(&[], None, measure.as_ref(), baseline.as_ref());
    sheet_inputs.peaks = Some(&peaks);
    sheet_inputs.activity = Some(&activity);
    let ops = layout(&geometry, &sheet_inputs);

    // The waveform lane spans y 240 to 294; every column is 1 px wide and
    // inside the lane.
    let wave = all(&ops, |op| {
        matches!(
            op,
            SheetOp::Rect {
                color: ColorRole::Waveform,
                ..
            }
        )
    });
    assert!(!wave.is_empty(), "waveform columns exist");
    for op in &wave {
        match op {
            SheetOp::Rect {
                x,
                y,
                width,
                height,
                ..
            } => {
                assert_eq!(*width, 1.0);
                assert!(
                    *y >= WAVE_TOP && *y + *height <= ACTIVITY_TOP + 1e-9,
                    "y {y} h {height}"
                );
                assert!(*x >= 0.0 && *x < geometry.width);
            }
            _ => panic!("a rect"),
        }
    }
    // The envelope column of bucket 0 at x 0: centred, the loudest bucket
    // (envelope 1.0) spans 92 % of the 54 px lane.
    one(&ops, |op| match op {
        SheetOp::Rect {
            x,
            y,
            height,
            color,
            ..
        } => {
            *x == 0.0
                && *color == ColorRole::Waveform
                && (*y - (WAVE_TOP + 27.0 - 0.5 * 0.92 * 54.0)).abs() < 1e-9
                && (*height - 0.92 * 54.0).abs() < 1e-9
        }
        _ => false,
    });

    // The activity lane: 1 px columns of the 6 px band. One pixel spans
    // about 2.03 ms; bucket 0 (active, ms 0..1000) lights the pixels
    // 0..492 (the boundary pixel straddles into bucket 1), bucket 1
    // (inactive, ms 1000..2000) the pixels 493..983 and bucket 2 (active)
    // from pixel 984, whose window reaches into it.
    let active = all(&ops, |op| match op {
        SheetOp::Rect {
            x: _,
            y,
            height,
            color,
            ..
        } => *color == ColorRole::Activity && *y == ACTIVITY_TOP && *height == 6.0,
        _ => false,
    });
    assert!(!active.is_empty(), "activity columns exist");
    let at = |x: f64| {
        active.iter().any(|op| match op {
            SheetOp::Rect { x: column, .. } => *column == x,
            _ => false,
        })
    };
    assert!(at(0.0), "bucket 0 is active at pixel 0");
    assert!(at(492.0), "bucket 0 reaches pixel 492");
    assert!(!at(493.0), "bucket 1 is inactive at pixel 493");
    assert!(!at(983.0), "bucket 1 is inactive at pixel 983");
    assert!(at(984.0), "the boundary pixel reaches bucket 2");
    assert!(at(985.0), "bucket 2 is active from pixel 985");
    assert!(!at(800.0), "pixel 800 belongs to the inactive bucket 1");
}

#[test]
fn gutter_labels_sit_left_in_their_lanes() {
    let geometry = geom(None);
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], None, measure.as_ref(), baseline.as_ref()),
    );
    match &text(&ops, "Lyrics") {
        SheetOp::Text {
            x, y, font, color, ..
        } => {
            assert_eq!(*x, 4.0);
            assert!((*y - (LYRICS_TOP + 17.0 + 3.0)).abs() < 1e-9);
            assert_eq!(*font, TextFont::Small);
            assert_eq!(*color, ColorRole::DisabledText);
        }
        _ => panic!("a text op"),
    }
    match &text(&ops, "Mix") {
        SheetOp::Text { x, y, .. } => {
            assert_eq!(*x, 4.0);
            assert!((*y - (WAVE_TOP + 27.0 + 3.0)).abs() < 1e-9);
        }
        _ => panic!("a text op"),
    }
}

#[test]
fn without_lyrics_lane_the_pitch_area_grows_and_no_labels_appear() {
    let mut geometry = geom(None);
    geometry.show_lyrics_lane = false;
    let (measure, baseline) = metrics();
    let notes = reference_line();
    let ops = layout(
        &geometry,
        &inputs(&notes, None, measure.as_ref(), baseline.as_ref()),
    );

    // The pitch area spans from the ruler to the waveform lane: 218 px.
    match &rect_at(&ops, 22.0, ColorRole::ViewBackground) {
        SheetOp::Rect { height, .. } => assert!((*height - 218.0).abs() < 1e-9, "h {height}"),
        _ => panic!("a rect"),
    }
    // No lyrics background, no gap strip, no gutter or note labels.
    for missing in ["Lyrics", "Are", "just"] {
        let found = all(&ops, |op| match op {
            SheetOp::Text { text, .. } => text == missing,
            _ => false,
        });
        assert!(found.is_empty(), "{missing} must not be painted");
    }
    let gap = all(&ops, |op| match op {
        SheetOp::Rect {
            y, height, color, ..
        } => *y == 240.0 && *height == 8.0 && *color == ColorRole::ViewBackground,
        _ => false,
    });
    assert!(gap.is_empty(), "no gap strip without the lyrics lane");
}

#[test]
fn without_a_line_no_rows_or_labels_are_painted() {
    let geometry = geom(None);
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], None, measure.as_ref(), baseline.as_ref()),
    );
    let shaded = all(&ops, |op| {
        matches!(
            op,
            SheetOp::Rect {
                color: ColorRole::Alternate,
                ..
            }
        )
    });
    assert!(shaded.is_empty(), "no rows without a line");
    let labels = all(&ops, |op| {
        matches!(
            op,
            SheetOp::Text {
                font: TextFont::SmallDemi,
                ..
            }
        )
    });
    assert!(labels.is_empty(), "no pitch labels without a line");
}

#[test]
fn zero_width_emits_nothing() {
    let mut geometry = geom(None);
    geometry.width = 0.0;
    let (measure, baseline) = metrics();
    let ops = layout(
        &geometry,
        &inputs(&[], None, measure.as_ref(), baseline.as_ref()),
    );
    assert!(ops.is_empty(), "got {ops:?}");
}

/// song decodes a song text into the document and song view.
fn song(text: &str) -> (usdx_format::rows::Document, usdx_format::song::Song) {
    let decoded = usdx_format::bytes::decode(text.as_bytes()).expect("the song decodes");
    let document = usdx_format::rows::classify(decoded);
    let song = usdx_format::song::Song::from_document(&document);
    (document, song)
}

/// the lines of a decoded song, flattened in global order.
fn lines_of(song: &usdx_format::song::Song) -> Vec<&usdx_format::song::Line> {
    song.voices.iter().flat_map(|voice| &voice.lines).collect()
}

const NOTE_SONG: &str = "#TITLE:T\n#BPM:240\n: 0 4 60 la\n: 4 4 62 la\tla\n- 8\n: 8 4 59 la\nE\n";

#[test]
fn line_notes_collects_the_current_line_sorted_with_its_pitch_range() {
    let (doc, view) = song(NOTE_SONG);
    let lines = lines_of(&view);
    let result = line_notes(&doc, &view, &lines, 0);
    assert_eq!(
        result.notes,
        vec![
            LyricNote {
                start_beat: 0.0,
                text: "la".into(),
                current: true
            },
            LyricNote {
                start_beat: 4.0,
                text: "la la".into(),
                current: true
            },
            LyricNote {
                start_beat: 8.0,
                text: "la".into(),
                current: false
            },
        ],
        "sorted by start, tab reads as a space, neighbours faded"
    );
    assert_eq!(
        result.pitch,
        Some((60, 62)),
        "the current line's pitch range"
    );
}

#[test]
fn line_notes_include_the_faded_global_neighbours() {
    let (doc, view) = song(NOTE_SONG);
    let lines = lines_of(&view);
    let result = line_notes(&doc, &view, &lines, 1);
    assert_eq!(
        result.notes,
        vec![
            LyricNote {
                start_beat: 0.0,
                text: "la".into(),
                current: false
            },
            LyricNote {
                start_beat: 4.0,
                text: "la la".into(),
                current: false
            },
            LyricNote {
                start_beat: 8.0,
                text: "la".into(),
                current: true
            },
        ]
    );
    assert_eq!(result.pitch, Some((59, 59)));
}

#[test]
fn line_notes_of_an_empty_line_have_no_pitch() {
    let (doc, view) = song("#TITLE:T\n#BPM:240\n- 4\nE\n");
    let lines = lines_of(&view);
    let result = line_notes(&doc, &view, &lines, 0);
    assert!(result.notes.is_empty());
    assert_eq!(result.pitch, None);
}
