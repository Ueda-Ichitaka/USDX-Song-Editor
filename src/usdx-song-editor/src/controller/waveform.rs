//! About me: the draw-list layout of the waveform and activity lanes
//! (UI_SPEC 3.7, task P4-04). Plain Rust so the column math is unit-testable
//! without QML: layout() folds the analysis peaks and the vocal activity of
//! usdx-audio into one draw op per pixel column, scaled so the loudest
//! bucket of the whole song uses 92 % of the lane height (UI_SPEC 2.3 gives
//! the colours). The WaveformLane bridge paints the list; the sheet task
//! P5-02 extends the same design to the full note sheet.

use usdx_audio::analysis::{Activity, Peaks};

/// One draw op: a filled rectangle of the lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawOp {
    /// A waveform envelope column between the top and bottom y positions.
    Waveform { x: f64, top: f64, bottom: f64 },
    /// An activity column filling the activity band at x.
    Activity { x: f64 },
}

/// The geometry and time range of the lanes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneLayout {
    /// The lane width in px; every pixel gets one column.
    pub width: f64,
    /// The waveform band height in px (54 in the sheet).
    pub waveform_height: f64,
    /// The activity band height in px (6 in the sheet).
    pub activity_height: f64,
    /// The first millisecond the lane shows.
    pub start_ms: f64,
    /// The millisecond just past the end of the shown range.
    pub end_ms: f64,
    /// The share of the lane height the loudest bucket uses (0.92).
    pub scale: f64,
}

/// layout folds the peaks and the vocal activity into the draw ops of the
/// lanes: one waveform column per pixel from the min/max envelope, one
/// activity column per pixel from the activity windows.
pub fn layout(lane: &LaneLayout, peaks: &Peaks, activity: Option<&Activity>) -> Vec<DrawOp> {
    let mut ops = Vec::new();
    let range = lane.end_ms - lane.start_ms;
    if lane.width <= 0.0 || range <= 0.0 {
        return ops;
    }
    // The vertical scale: the loudest bucket's envelope (max - min) spans
    // scale times the lane height, centred.
    let loudest = peaks
        .min
        .iter()
        .zip(&peaks.max)
        .map(|(min, max)| (*max - *min).max(0.0))
        .fold(0.0_f32, f32::max);
    let centre = lane.waveform_height / 2.0;
    let factor = if loudest > 0.0 {
        lane.scale * lane.waveform_height / loudest as f64
    } else {
        0.0
    };
    let ms_per_pixel = range / lane.width;
    for pixel in 0..lane.width.max(0.0) as usize {
        let x = pixel as f64;
        let t0 = lane.start_ms + x * ms_per_pixel;
        let t1 = t0 + ms_per_pixel;
        // The buckets the pixel covers; buckets past the analysed data end
        // the lane.
        let bucket = peaks.bucket_ms;
        let first = (t0 / bucket).floor().max(0.0) as usize;
        let last = ((t1 / bucket).ceil() as usize).min(peaks.min.len());
        if first >= last {
            continue;
        }
        let mut low = f32::INFINITY;
        let mut high = f32::NEG_INFINITY;
        for index in first..last {
            low = low.min(peaks.min[index]);
            high = high.max(peaks.max[index]);
        }
        let top = centre - high as f64 * factor;
        let bottom = centre - low as f64 * factor;
        if bottom > top {
            ops.push(DrawOp::Waveform { x, top, bottom });
        }
        // The activity windows the pixel covers; the columns need the
        // activity data of the vocals stem.
        if let Some(activity) = activity {
            let window = activity.bucket_ms;
            let first = (t0 / window).floor().max(0.0) as usize;
            let last = ((t1 / window).ceil() as usize).min(activity.active.len());
            if first < last && activity.active[first..last].iter().any(|active| *active) {
                ops.push(DrawOp::Activity { x });
            }
        }
    }
    ops
}
