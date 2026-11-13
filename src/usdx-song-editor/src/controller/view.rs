//! About me: the default time view of the note sheet (UI_SPEC 3.2). When a
//! line becomes current, the sheet shows the line plus padding of
//! max(1.5 s, 50 % of the line's duration) on both sides, and it only moves
//! when the new line is not fully visible, so stepping through short
//! neighbouring lines does not jump. It exists so the waveform lane (task
//! P4-04) shows the same range the finished sheet will; the SheetViewport of
//! task P5-01 takes this rule over and adds zoom and scrolling.

/// A visible time range of the sheet in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeView {
    pub start_ms: f64,
    pub end_ms: f64,
}

/// The minimum padding on each side of the current line.
const MIN_PADDING_MS: f64 = 1_500.0;
/// The padding as a share of the line's duration when that is larger.
const PADDING_SHARE: f64 = 0.5;

/// follow_line returns the view for a line that became current: the previous
/// view when it shows the whole line, otherwise the line with its padding.
pub fn follow_line(previous: Option<TimeView>, line_start_ms: f64, line_end_ms: f64) -> TimeView {
    if let Some(view) = previous
        && view.start_ms <= line_start_ms
        && line_end_ms <= view.end_ms
    {
        return view;
    }
    let padding = MIN_PADDING_MS.max(PADDING_SHARE * (line_end_ms - line_start_ms));
    TimeView {
        start_ms: line_start_ms - padding,
        end_ms: line_end_ms + padding,
    }
}
