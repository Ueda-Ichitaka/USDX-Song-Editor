//! About me: the sheet viewport controller (plan 8.2, task P5-01). It maps
//! beats to sheet x pixels and pitches to pitch-area y pixels, applies the
//! zoom and scroll gestures of UI_SPEC 3.2 (Ctrl+wheel zoom of 1.25 per step
//! around the cursor beat, wheel scrolling of 10 % of the visible width per
//! step, Shift+wheel one pitch row per step, middle-drag panning) and fits a
//! beat range into the view, which is how the follow-line rule of view.rs
//! reaches the beat-based sheet; the FitKey records the range and size a
//! fit was computed from so the bridge refits on either change. It holds
//! no Qt types; the bridge forwards
//! the QML gestures and feeds the fitted range through. Defaults chosen per
//! user decision 2026-11-13: 20 px per beat for a fresh view; C4 (60) on top
//! with the spec's minimum 14 px row height until the pitch rule (P5-02)
//! computes the real range.

/// The viewport state of one note sheet: the leftmost visible beat, the
/// zoom, the top pitch of the pitch area and its row height.
pub struct SheetViewport {
    /// The beat at sheet x 0.
    pub start_beat: f64,
    /// Pixels per beat, clamped to 1.0 to 40.0 (UI_SPEC 3.2).
    pub px_per_beat: f64,
    /// The pitch at pitch-area y 0.
    pub top_pitch: f64,
    /// Pixels per semitone: the height of one pitch row.
    pub px_per_semitone: f64,
    /// The sheet's content width in pixels.
    pub width_px: f64,
    /// The pitch area's height in pixels.
    pub height_px: f64,
}

/// The zoom bounds of UI_SPEC 3.2.
const MIN_PX_PER_BEAT: f64 = 1.0;
const MAX_PX_PER_BEAT: f64 = 40.0;

/// The fit key: the beat range and the item size the current fit was
/// computed from. The bridge compares it against the live inputs and runs
/// fit_range again whenever either changed, so a resize after a fit at a
/// not yet laid out size gets the real fit (user decision 2026-11-14).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FitKey {
    /// The fitted beat range.
    pub range: (f64, f64),
    /// The item size the fit used.
    pub size: (f64, f64),
}

impl FitKey {
    /// new builds the key for one fit.
    pub fn new(range: (f64, f64), size: (f64, f64)) -> FitKey {
        FitKey { range, size }
    }

    /// stale reports whether the live inputs no longer match the key, so
    /// the view needs a new fit_range.
    pub fn stale(self, range: (f64, f64), size: (f64, f64)) -> bool {
        self.range != range || self.size != size
    }
}

/// The wheel scroll share of one time step (UI_SPEC 3.2).
const WHEEL_TIME_SHARE: f64 = 0.1;

/// The fresh-view zoom (user decision 2026-11-13).
const DEFAULT_PX_PER_BEAT: f64 = 20.0;
/// C4 sits on top until the pitch range rule runs.
const DEFAULT_TOP_PITCH: f64 = 60.0;
/// The spec's minimum row height until the pitch range rule runs.
const DEFAULT_PX_PER_SEMITONE: f64 = 14.0;

impl Default for SheetViewport {
    /// A zero-sized viewport with the fresh-view defaults.
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

impl SheetViewport {
    /// new builds the fresh view: beat 0 at x 0, the default zoom, C4 on
    /// top.
    pub fn new(width_px: f64, height_px: f64) -> SheetViewport {
        SheetViewport {
            start_beat: 0.0,
            px_per_beat: DEFAULT_PX_PER_BEAT,
            top_pitch: DEFAULT_TOP_PITCH,
            px_per_semitone: DEFAULT_PX_PER_SEMITONE,
            width_px,
            height_px,
        }
    }

    /// beat_to_x maps a beat to its sheet x position.
    pub fn beat_to_x(&self, beat: f64) -> f64 {
        (beat - self.start_beat) * self.px_per_beat
    }

    /// x_to_beat maps a sheet x position to its beat.
    pub fn x_to_beat(&self, x: f64) -> f64 {
        self.start_beat + x / self.px_per_beat
    }

    /// pitch_to_y maps a pitch to its pitch-area y position; higher pitches
    /// sit higher (smaller y).
    pub fn pitch_to_y(&self, pitch: f64) -> f64 {
        (self.top_pitch - pitch) * self.px_per_semitone
    }

    /// y_to_pitch maps a pitch-area y position to its pitch.
    pub fn y_to_pitch(&self, y: f64) -> f64 {
        self.top_pitch - y / self.px_per_semitone
    }

    /// visible_beats reports how many beats fit into the view width.
    pub fn visible_beats(&self) -> f64 {
        self.width_px / self.px_per_beat
    }

    /// end_beat reports the rightmost visible beat.
    pub fn end_beat(&self) -> f64 {
        self.start_beat + self.visible_beats()
    }

    /// set_size stores a new view size; the mapping follows immediately.
    pub fn set_size(&mut self, width_px: f64, height_px: f64) {
        self.width_px = width_px;
        self.height_px = height_px;
    }

    /// zoom_around multiplies the zoom by the factor (1.25 for one wheel
    /// step in, 0.8 for one out) and keeps the beat under the cursor at the
    /// cursor. When the clamp binds, the beat under the cursor moves.
    pub fn zoom_around(&mut self, factor: f64, cursor_x: f64) {
        let anchor = self.x_to_beat(cursor_x);
        self.px_per_beat = (self.px_per_beat * factor).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
        self.start_beat = anchor - cursor_x / self.px_per_beat;
    }

    /// wheel_time scrolls time by 10 % of the visible width per step;
    /// positive steps move forward.
    pub fn wheel_time(&mut self, steps: f64) {
        self.start_beat += steps * WHEEL_TIME_SHARE * self.visible_beats();
    }

    /// wheel_pitch scrolls the pitch range by one row per step; positive
    /// steps scroll down (the top pitch gets lower).
    pub fn wheel_pitch(&mut self, rows: f64) {
        self.top_pitch -= rows;
    }

    /// pan_time moves the content with the pointer during a middle drag:
    /// dragging right shows earlier beats.
    pub fn pan_time(&mut self, dx_px: f64) {
        self.start_beat -= dx_px / self.px_per_beat;
    }

    /// drag_delta_beats converts a horizontal drag delta into whole beats,
    /// rounded away from zero (notes sit on integer beats).
    pub fn drag_delta_beats(&self, dx_px: f64) -> i64 {
        (dx_px / self.px_per_beat).round() as i64
    }

    /// fit_range shows exactly the given beat range (the follow-line rule's
    /// padded range) by scaling the zoom to it. When the clamp binds, the
    /// view keeps the bound zoom and centres the range.
    pub fn fit_range(&mut self, start_beat: f64, end_beat: f64) {
        let span = end_beat - start_beat;
        let scale = self.width_px / span;
        self.px_per_beat = scale.clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
        if self.px_per_beat == scale {
            self.start_beat = start_beat;
        } else {
            self.start_beat = (start_beat + end_beat) / 2.0 - self.visible_beats() / 2.0;
        }
    }
}
