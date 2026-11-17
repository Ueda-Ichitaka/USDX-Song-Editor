//! The tap latency calibration (UI_SPEC 5.7, task P7-08).
//!
//! About me: the "Measure tap latency" dialog plays eight clicks one second
//! apart (the first at 0 ms; the click track the TapSession writes) and the
//! user presses Space on each click. This module turns the recorded press
//! positions into the latency: a tap counts for the click it is nearest to
//! when it lies within 300 ms of it, the first such tap per click wins, and
//! its offset is the tap time minus the click time. The proposed latency is
//! the median of the counted offsets in whole milliseconds; the round is
//! complete when every click has a tap. It holds no Qt types; the
//! Calibration bridge feeds it the playback positions.

/// The number of clicks of a round.
pub const CLICKS: usize = 8;

/// The time between two clicks in milliseconds.
const CLICK_SPACING_MS: f64 = 1000.0;

/// The farthest a tap may lie from its click and still count.
const TAP_WINDOW_MS: f64 = 300.0;

/// One calibration round: the offset per click, none until a tap counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calibration {
    offsets: [Option<f64>; CLICKS],
}

impl Calibration {
    /// new starts a round without taps.
    pub fn new() -> Calibration {
        Calibration::default()
    }

    /// tap records a press at the playback position: it counts for its
    /// nearest click within the window, unless that click has a tap.
    pub fn tap(&mut self, position_ms: f64) {
        let nearest = (position_ms / CLICK_SPACING_MS).round();
        if nearest < 0.0 || nearest >= CLICKS as f64 {
            return;
        }
        let click = nearest as usize;
        let offset = position_ms - click as f64 * CLICK_SPACING_MS;
        if offset.abs() <= TAP_WINDOW_MS && self.offsets[click].is_none() {
            self.offsets[click] = Some(offset);
        }
    }

    /// clear drops every tap (Start again).
    pub fn clear(&mut self) {
        self.offsets = [None; CLICKS];
    }

    /// offsets returns the offset per click.
    pub fn offsets(&self) -> Vec<Option<f64>> {
        self.offsets.to_vec()
    }

    /// median_ms returns the median of the counted offsets in whole
    /// milliseconds (the mean of the two middle ones for an even count),
    /// none without a counted tap.
    pub fn median_ms(&self) -> Option<i64> {
        let mut values: Vec<f64> = self.offsets.iter().flatten().copied().collect();
        if values.is_empty() {
            return None;
        }
        values.sort_by(f64::total_cmp);
        let middle = values.len() / 2;
        let median = if values.len().is_multiple_of(2) {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        Some(median.round() as i64)
    }

    /// complete tells whether every click has a counted tap.
    pub fn complete(&self) -> bool {
        self.offsets.iter().all(Option::is_some)
    }

    /// click_at returns the click now playing at the playback position:
    /// the last click at or before it, clamped to the round.
    pub fn click_at(position_ms: f64) -> usize {
        ((position_ms / CLICK_SPACING_MS).floor().max(0.0) as usize).min(CLICKS - 1)
    }
}
