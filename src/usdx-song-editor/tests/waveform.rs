//! About me: tests for the waveform and activity lane layout of task P4-04
//! (UI_SPEC 3.7). The pure draw-list math (`layout`) is tested without QML
//! on hand-built peaks and activity data: one column per pixel, the 92 %
//! scaling of the loudest bucket, the silence and out-of-data columns, the
//! activity columns and the time mapping.

use usdx_audio::analysis::{Activity, Peaks};
use usdx_song_editor::controller::waveform::{DrawOp, LaneLayout, layout};

/// A lane layout of 4 px width, the sheet's 54 px waveform band and 6 px
/// activity band, showing the full 4 s of 1 s buckets.
fn lane() -> LaneLayout {
    LaneLayout {
        width: 4.0,
        waveform_height: 54.0,
        activity_height: 6.0,
        start_ms: 0.0,
        end_ms: 4000.0,
        scale: 0.92,
    }
}

/// Peaks of 4 one-second buckets with the given amplitudes; the loudest
/// bucket spans -loudest to +loudest.
fn peaks(envelopes: [(f32, f32); 4]) -> Peaks {
    let mut min = Vec::new();
    let mut max = Vec::new();
    for (low, high) in envelopes {
        min.push(low);
        max.push(high);
    }
    Peaks {
        bucket_ms: 1000.0,
        min,
        max,
        rms: vec![0.0; 4],
    }
}

/// layout maps one pixel to one waveform column over the data.
#[test]
fn every_pixel_is_one_waveform_column() {
    let ops = layout(&lane(), &peaks([(-0.5, 0.5); 4]), None);
    assert_eq!(ops.len(), 4, "one column per pixel, got {ops:?}");
    assert!(
        ops.iter().all(|op| matches!(op, DrawOp::Waveform { .. })),
        "every column is a waveform column, got {ops:?}"
    );
}

/// The loudest bucket of the whole song spans 92 % of the lane height,
/// centred vertically.
#[test]
fn the_loudest_bucket_uses_92_percent_of_the_lane_height() {
    let data = peaks([(-0.25, 0.25), (-1.0, 1.0), (-0.5, 0.5), (-0.1, 0.1)]);
    let ops = layout(&lane(), &data, None);
    let DrawOp::Waveform { top, bottom, .. } = ops[1] else {
        panic!("a waveform column, got {:?}", ops[1]);
    };
    let height = bottom - top;
    assert!(
        (height - 0.92 * 54.0).abs() <= 0.01,
        "the loudest bucket spans 92 % of 54 px, measured {height} px"
    );
    let centre = (top + bottom) / 2.0;
    assert!(
        (centre - 27.0).abs() <= 0.01,
        "the envelope is centred in the lane, measured {centre} px"
    );
}

/// Silent pixels produce no column; the time axis maps pixel 2 to the
/// third bucket.
#[test]
fn silence_produces_no_column_and_the_time_axis_maps_the_buckets() {
    let data = peaks([(0.0, 0.0), (0.0, 0.0), (-0.5, 0.5), (0.0, 0.0)]);
    let ops = layout(&lane(), &data, None);
    assert_eq!(
        ops.len(),
        1,
        "only the third pixel has a column, got {ops:?}"
    );
    let DrawOp::Waveform { x, .. } = ops[0] else {
        panic!("a waveform column, got {:?}", ops[0]);
    };
    assert_eq!(x, 2.0, "the column sits at the third pixel");
}

/// Columns past the analysed data produce no column.
#[test]
fn columns_past_the_data_produce_no_column() {
    let data = peaks([(-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5)]);
    let mut only_two = lane();
    only_two.end_ms = 8000.0;
    let ops = layout(&only_two, &data, None);
    assert_eq!(
        ops.len(),
        2,
        "the last two pixels have no data, got {ops:?}"
    );
}

/// Active activity windows become one activity column per pixel, aligned
/// with the pixels.
#[test]
fn active_windows_become_activity_columns() {
    let activity = Activity {
        bucket_ms: 2000.0,
        active: vec![true, false],
        floor_db: -40.0,
    };
    let ops = layout(&lane(), &peaks([(-0.5, 0.5); 4]), Some(&activity));
    let activity_ops: Vec<&DrawOp> = ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Activity { .. }))
        .collect();
    assert_eq!(activity_ops.len(), 2, "the first two pixels are active");
    let DrawOp::Activity { x } = *activity_ops[0] else {
        panic!("an activity column");
    };
    assert_eq!(x, 0.0, "the first activity column starts at pixel 0");
}

/// Without activity data there is no activity column.
#[test]
fn without_activity_data_there_is_no_activity_column() {
    let ops = layout(&lane(), &peaks([(-0.5, 0.5); 4]), None);
    assert!(
        ops.iter().all(|op| matches!(op, DrawOp::Waveform { .. })),
        "no activity columns without data, got {ops:?}"
    );
}

/// A zero-width lane and an empty time range produce nothing.
#[test]
fn degenerate_lanes_produce_nothing() {
    let mut zero = lane();
    zero.width = 0.0;
    assert!(layout(&zero, &peaks([(-0.5, 0.5); 4]), None).is_empty());
    let mut empty = lane();
    empty.end_ms = 0.0;
    assert!(layout(&empty, &peaks([(-0.5, 0.5); 4]), None).is_empty());
}
