//! About me: the draw-list layout of the full note sheet (UI_SPEC section 3,
//! task P5-02). Plain Rust so the sheet math is unit-testable without QML:
//! layout() folds the sheet geometry, the analysis peaks and vocal activity,
//! the visible lyric notes and the measured text metrics into draw ops that
//! the SheetBackground bridge only replays. The vertical stack follows
//! UI_SPEC 3.1 (ruler 22 px, pitch rows, lyrics lane 34 px, 8 px gap,
//! waveform 54 px, activity 6 px, gutter 46 px), the pitch rows follow 3.3,
//! the ruler 3.4 and the lyrics lane 3.6; the lane column math lives in
//! controller::waveform and is reused.

use usdx_audio::analysis::{Activity, Peaks};
use usdx_format::rows::Document;
use usdx_format::song::{Line, Song};

use crate::controller::waveform::{DrawOp, LaneLayout, layout as lane_layout};

/// A colour of the sheet, mapped to the theme properties the bridge holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorRole {
    /// The view background (pitch rows base, gap strip, activity lane).
    ViewBackground,
    /// The alternate row colour of shaded pitch rows.
    Alternate,
    /// The header background of the ruler.
    HeaderBackground,
    /// The window background (lyrics lane, waveform lane).
    WindowBackground,
    /// The 1 px grid line under a pitch row.
    GridLine,
    /// The strong grid line under a C row.
    StrongGridLine,
    /// Emphasised label text (C pitch labels).
    Text,
    /// Secondary label text.
    DisabledText,
    /// A waveform envelope column.
    Waveform,
    /// A vocal activity column.
    Activity,
}

/// A font of the sheet, mapped to the font properties the bridge holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFont {
    /// The theme small font (ruler labels, gutter labels, pitch labels).
    Small,
    /// The small font in demi-bold (C pitch labels).
    SmallDemi,
    /// The 9 pt font of the current line's syllables.
    Lyrics,
    /// The 9 pt font of the faded neighbour lines' syllables.
    LyricsNeighbour,
}

/// One draw op of the sheet.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetOp {
    /// A filled rectangle.
    Rect {
        /// Left edge.
        x: f64,
        /// Top edge.
        y: f64,
        /// Width.
        width: f64,
        /// Height.
        height: f64,
        /// Fill colour.
        color: ColorRole,
    },
    /// A straight line.
    Line {
        /// Start x.
        x1: f64,
        /// Start y.
        y1: f64,
        /// End x.
        x2: f64,
        /// End y.
        y2: f64,
        /// Pen colour.
        color: ColorRole,
        /// Pen width.
        width: f64,
    },
    /// A text label drawn at its baseline.
    Text {
        /// Left edge.
        x: f64,
        /// Baseline y.
        y: f64,
        /// The text.
        text: String,
        /// The font.
        font: TextFont,
        /// The colour.
        color: ColorRole,
        /// The width the bridge may elide the text to.
        max_width: f64,
        /// The opacity.
        opacity: f64,
    },
}

/// The sheet geometry in pixels and beats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetGeometry {
    /// The sheet width in pixels.
    pub width: f64,
    /// The sheet height in pixels.
    pub height: f64,
    /// The gutter width in pixels (46).
    pub gutter_width: f64,
    /// The ruler height in pixels (22).
    pub ruler_height: f64,
    /// The lyrics lane height in pixels (34).
    pub lyrics_height: f64,
    /// The gap strip height in pixels (8).
    pub gap_height: f64,
    /// The waveform lane height in pixels (54).
    pub waveform_height: f64,
    /// The activity lane height in pixels (6).
    pub activity_height: f64,
    /// Whether the lyrics lane is shown (repair sheets leave it out).
    pub show_lyrics_lane: bool,
    /// The beat at sheet x 0.
    pub start_beat: f64,
    /// Pixels per beat.
    pub px_per_beat: f64,
    /// The pitch of the topmost pitch row.
    pub top_pitch: i32,
    /// Pixels per semitone: the pitch row height.
    pub px_per_semitone: f64,
    /// The song's BPM.
    pub bpm: f64,
    /// The song's GAP in milliseconds.
    pub gap_ms: f64,
}

/// One lyric label of the lyrics lane.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricNote {
    /// The note's absolute start beat.
    pub start_beat: f64,
    /// The note's text, trimmed.
    pub text: String,
    /// Whether the note belongs to the current line (neighbours fade).
    pub current: bool,
}

/// What layout folds into draw ops.
pub struct SheetInputs<'a> {
    /// The waveform peaks of the selected track, once analysed.
    pub peaks: Option<&'a Peaks>,
    /// The vocal activity of the vocals track, once analysed.
    pub activity: Option<&'a Activity>,
    /// The lyric notes of the current line and its neighbours, sorted by
    /// start.
    pub notes: &'a [LyricNote],
    /// The track name in the waveform lane's gutter.
    pub track_name: &'a str,
    /// The lowest and highest pitch of the current line; none while no song
    /// or no line is current.
    pub line_pitch: Option<(i32, i32)>,
    /// Measures a text in the given font: the bridge supplies the real font
    /// metrics, tests supply synthetic widths.
    pub measure: &'a (dyn Fn(TextFont, &str) -> f64 + 'a),
    /// The baseline offset of a font for vertically centred text: half the
    /// difference of its ascent and descent.
    pub baseline_offset: &'a (dyn Fn(TextFont) -> f64 + 'a),
}

/// The smallest pitch row height (UI_SPEC 3.3).
const MIN_ROW_PX: f64 = 14.0;
/// The row height from which every row is labelled (UI_SPEC 3.3).
const LABEL_ROW_PX: f64 = 18.0;
/// The minimum number of pitch rows (UI_SPEC 3.3).
const MIN_ROWS: i32 = 14;
/// The semitone extension on both sides of the line's pitch range.
const PITCH_EXTENSION: i32 = 3;
/// The pitch classes shaded with the alternate colour: C and the black keys.
const SHADED_CLASSES: [i32; 6] = [0, 1, 3, 6, 8, 10];
/// The smallest distance between ruler ticks in pixels (UI_SPEC 3.4).
const TICK_MIN_PX: f64 = 70.0;
/// The candidate ruler tick intervals in seconds (UI_SPEC 3.4).
const TICK_INTERVALS_S: [f64; 7] = [0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0];
/// The ruler label baseline from the ruler top (UI_SPEC 3.4).
const RULER_LABEL_BASELINE: f64 = 14.0;
/// The tick line starts this far above the ruler's bottom (UI_SPEC 3.4).
const TICK_LINE_ABOVE: f64 = 6.0;
/// The lyric baselines from the lane top (UI_SPEC 3.6).
const LYRICS_BASELINES: [f64; 2] = [14.0, 29.0];
/// The gutter labels' left margin (UI_SPEC 3.1).
const GUTTER_LABEL_MARGIN: f64 = 4.0;
/// The pitch labels' distance from the gutter's right edge (UI_SPEC 3.3).
const PITCH_LABEL_MARGIN: f64 = 6.0;
/// The opacity of the faded neighbour lines' syllables (UI_SPEC 3.6).
const NEIGHBOUR_OPACITY: f64 = 0.7;
/// The share of the waveform lane height the loudest bucket uses (3.7).
const LOUDEST_SHARE: f64 = 0.92;

/// The note names of the twelve pitch classes.
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// pitch_rows computes the visible pitch range of a line (UI_SPEC 3.3): the
/// line's lowest to highest pitch, extended by 3 semitones on both sides,
/// then extended symmetrically to at least 14 rows. The row height is the
/// pitch area height over the rows, at least 14 px; when the rows do not fit
/// at 14 px the pitch area scrolls vertically and starts centred on the
/// range (user decision 2026-11-13). Returns the top pitch and the row
/// height.
pub fn pitch_rows(line_min: i32, line_max: i32, pitch_height: f64) -> (i32, f64) {
    let mut low = line_min - PITCH_EXTENSION;
    let mut high = line_max + PITCH_EXTENSION;
    let rows = high - low + 1;
    if rows < MIN_ROWS {
        let pad = MIN_ROWS - rows;
        low -= pad / 2;
        high += pad - pad / 2;
    }
    let rows = high - low + 1;
    let height = pitch_height / f64::from(rows);
    if height >= MIN_ROW_PX {
        (high, height)
    } else {
        let visible = ((pitch_height / MIN_ROW_PX).floor() as i32).max(1);
        let offset = (rows - visible) / 2;
        (high - offset, MIN_ROW_PX)
    }
}

/// tick_interval_s chooses the ruler tick interval (UI_SPEC 3.4): the
/// smallest of the candidates that keeps the ticks at least 70 px apart.
pub fn tick_interval_s(px_per_second: f64) -> f64 {
    TICK_INTERVALS_S
        .iter()
        .copied()
        .find(|interval| interval * px_per_second >= TICK_MIN_PX)
        .unwrap_or(60.0)
}

/// ruler_label formats a song position in milliseconds as `m:ss.s`.
pub fn ruler_label(t_ms: f64) -> String {
    let t = t_ms.max(0.0) / 1000.0;
    let minutes = (t / 60.0).floor();
    let seconds = t - minutes * 60.0;
    format!("{minutes}:{seconds:04.1}")
}

impl SheetGeometry {
    /// pitch_height returns the pitch rows' height: everything below the
    /// ruler except the lyrics lane (with the gap, when shown) and the
    /// lanes.
    fn pitch_height(&self) -> f64 {
        let below = self.ruler_height
            + if self.show_lyrics_lane {
                self.lyrics_height + self.gap_height
            } else {
                0.0
            }
            + self.waveform_height
            + self.activity_height;
        (self.height - below).max(0.0)
    }

    /// pitch_bottom returns the first y past the pitch rows.
    fn pitch_bottom(&self) -> f64 {
        self.ruler_height + self.pitch_height()
    }

    /// lyrics_top returns the top of the lyrics lane (the pitch bottom).
    fn lyrics_top(&self) -> f64 {
        self.pitch_bottom()
    }

    /// gap_top returns the top of the gap strip.
    fn gap_top(&self) -> f64 {
        self.lyrics_top() + self.lyrics_height
    }

    /// wave_top returns the top of the waveform lane.
    fn wave_top(&self) -> f64 {
        self.gap_top() + self.gap_height
    }

    /// activity_top returns the top of the activity lane.
    fn activity_top(&self) -> f64 {
        self.wave_top() + self.waveform_height
    }

    /// beat_to_x maps a beat to its sheet x position; the gutter holds only
    /// the labels, so the time content starts at the gutter's right edge.
    pub fn beat_to_x(&self, beat: f64) -> f64 {
        self.gutter_width + (beat - self.start_beat) * self.px_per_beat
    }

    /// end_beat returns the rightmost visible beat.
    fn end_beat(&self) -> f64 {
        self.start_beat + self.width / self.px_per_beat
    }

    /// ms_per_beat returns the milliseconds per beat of the song's BPM
    /// (60000 over four times the BPM, the format's timing rule).
    fn ms_per_beat(&self) -> f64 {
        15000.0 / self.bpm
    }

    /// ms_of_beat maps a beat to its song position in milliseconds.
    fn ms_of_beat(&self, beat: f64) -> f64 {
        self.gap_ms + beat * self.ms_per_beat()
    }

    /// beat_of_ms maps a song position in milliseconds to its beat.
    fn beat_of_ms(&self, t_ms: f64) -> f64 {
        (t_ms - self.gap_ms) / self.ms_per_beat()
    }

    /// px_per_second returns the horizontal scale in pixels per second.
    fn px_per_second(&self) -> f64 {
        self.px_per_beat * self.bpm / 15.0
    }
}

/// shaded tells whether a pitch row carries the alternate colour: the C rows
/// and the black keys (UI_SPEC 3.3).
fn shaded(pitch: i32) -> bool {
    SHADED_CLASSES.contains(&pitch.rem_euclid(12))
}

/// pitch_name formats a pitch as a note name with octave (`C4`, `F#4`).
fn pitch_name(pitch: i32) -> String {
    let class = pitch.rem_euclid(12) as usize;
    let octave = pitch.div_euclid(12) - 1;
    format!("{}{octave}", NOTE_NAMES[class])
}

/// The lyric notes and pitch range the sheet needs for a current line and
/// its neighbours.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineNotes {
    /// All notes of the given lines sorted by start beat; the current
    /// line's notes carry `current`.
    pub notes: Vec<LyricNote>,
    /// The lowest and highest pitch of the current line, or `None` when it
    /// has no notes.
    pub pitch: Option<(i32, i32)>,
}

/// The MIDI pitch of USDX relative pitch 0: the USDX format stores pitches
/// relative to C4, and the sheet's pitch rows and note names use MIDI
/// numbers (C4 = 60, UI_SPEC 3.3).
const PITCH_C4_MIDI: i32 = 60;

/// line_notes collects the lyric notes of the current line and its global
/// neighbours (UI_SPEC 3.5, 3.6; user decision 2026-11-13: the neighbours
/// are the global previous and next line) and the current line's pitch
/// range. A note's text reads a tab as a space and is trimmed.
pub fn line_notes(doc: &Document, song: &Song, lines: &[&Line], current: usize) -> LineNotes {
    let mut notes = Vec::new();
    let mut pitch: Option<(i32, i32)> = None;
    for (index, line) in lines.iter().enumerate() {
        let is_current = index == current;
        for row_id in &line.notes {
            let Some(note) = song.note(doc, *row_id) else {
                continue;
            };
            if is_current {
                let value = note.pitch as i32 + PITCH_C4_MIDI;
                let (min, max) = pitch.unwrap_or((value, value));
                pitch = Some((min.min(value), max.max(value)));
            }
            notes.push(LyricNote {
                start_beat: note.start,
                text: note.text.replace('\t', " ").trim().to_owned(),
                current: is_current,
            });
        }
    }
    notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    LineNotes { notes, pitch }
}

/// rect pushes a filled rectangle op.
fn rect(ops: &mut Vec<SheetOp>, x: f64, y: f64, width: f64, height: f64, color: ColorRole) {
    ops.push(SheetOp::Rect {
        x,
        y,
        width,
        height,
        color,
    });
}

/// line pushes a straight line op.
fn line(ops: &mut Vec<SheetOp>, x1: f64, y1: f64, x2: f64, y2: f64, color: ColorRole, width: f64) {
    ops.push(SheetOp::Line {
        x1,
        y1,
        x2,
        y2,
        color,
        width,
    });
}

/// A text label to push: the fields of a text op.
struct TextLabel<'a> {
    x: f64,
    y: f64,
    content: &'a str,
    font: TextFont,
    color: ColorRole,
    max_width: f64,
    opacity: f64,
}

/// text pushes a text op drawn at its baseline.
fn text(ops: &mut Vec<SheetOp>, label: TextLabel) {
    ops.push(SheetOp::Text {
        x: label.x,
        y: label.y,
        text: label.content.to_owned(),
        font: label.font,
        color: label.color,
        max_width: label.max_width,
        opacity: label.opacity,
    });
}

/// gutter_label pushes a lane label in the gutter: left aligned 4 px from
/// the left edge, vertically centred in the lane.
fn gutter_label(
    ops: &mut Vec<SheetOp>,
    content: &str,
    lane_top: f64,
    lane_height: f64,
    baseline_offset: f64,
) {
    text(
        ops,
        TextLabel {
            x: GUTTER_LABEL_MARGIN,
            y: lane_top + lane_height / 2.0 + baseline_offset,
            content,
            font: TextFont::Small,
            color: ColorRole::DisabledText,
            max_width: f64::INFINITY,
            opacity: 1.0,
        },
    );
}

/// layout folds the sheet geometry and inputs into the draw ops (see the
/// module comment). The ops paint top to bottom: backgrounds, pitch rows
/// with their grid lines and labels, ruler ticks and labels, the lyrics
/// lane, the gap strip, the waveform and activity lanes with their columns
/// and the gutter labels.
pub fn layout(geometry: &SheetGeometry, inputs: &SheetInputs) -> Vec<SheetOp> {
    let mut ops = Vec::new();
    if geometry.width <= 0.0
        || geometry.height <= 0.0
        || geometry.px_per_beat <= 0.0
        || geometry.bpm <= 0.0
    {
        return ops;
    }
    let pitch_height = geometry.pitch_height();
    let pitch_bottom = geometry.pitch_bottom();
    let lyrics_top = geometry.lyrics_top();
    let wave_top = geometry.wave_top();
    let activity_top = geometry.activity_top();

    // The ruler and the pitch rows' base.
    rect(
        &mut ops,
        0.0,
        0.0,
        geometry.width,
        geometry.ruler_height,
        ColorRole::HeaderBackground,
    );
    rect(
        &mut ops,
        0.0,
        geometry.ruler_height,
        geometry.width,
        pitch_height,
        ColorRole::ViewBackground,
    );

    // The pitch rows from the top pitch down (UI_SPEC 3.3): alternate
    // shading for C and the black keys, a 1 px grid line under each row,
    // strong under a C row, labels right aligned in the gutter.
    if inputs.line_pitch.is_some() && geometry.px_per_semitone > 0.0 {
        let row_height = geometry.px_per_semitone;
        let mut pitch = geometry.top_pitch;
        let mut row_top = geometry.ruler_height;
        while row_top < pitch_bottom {
            let clipped = (pitch_bottom - row_top).min(row_height);
            if shaded(pitch) {
                rect(
                    &mut ops,
                    0.0,
                    row_top,
                    geometry.width,
                    clipped,
                    ColorRole::Alternate,
                );
            }
            if row_top + row_height <= pitch_bottom + f64::EPSILON {
                let grid = if pitch.rem_euclid(12) == 0 {
                    ColorRole::StrongGridLine
                } else {
                    ColorRole::GridLine
                };
                rect(
                    &mut ops,
                    0.0,
                    row_top + row_height - 1.0,
                    geometry.width,
                    1.0,
                    grid,
                );
                // The label rules: every row when the rows are tall enough,
                // otherwise only the C rows and the even pitches.
                let is_c = pitch.rem_euclid(12) == 0;
                if row_height >= LABEL_ROW_PX || is_c || pitch.rem_euclid(2) == 0 {
                    let (font, color) = if is_c {
                        (TextFont::SmallDemi, ColorRole::Text)
                    } else {
                        (TextFont::Small, ColorRole::DisabledText)
                    };
                    let name = pitch_name(pitch);
                    let width = (inputs.measure)(font, &name);
                    let baseline = row_top + row_height / 2.0 + (inputs.baseline_offset)(font);
                    text(
                        &mut ops,
                        TextLabel {
                            x: geometry.gutter_width - PITCH_LABEL_MARGIN - width,
                            y: baseline,
                            content: &name,
                            font,
                            color,
                            max_width: f64::INFINITY,
                            opacity: 1.0,
                        },
                    );
                }
            }
            pitch -= 1;
            row_top += row_height;
        }
    }

    // The ruler ticks and labels (UI_SPEC 3.4): song-time multiples of the
    // interval, each with a grid line through the pitch rows and a label
    // 3 px right of the tick.
    let interval_ms = tick_interval_s(geometry.px_per_second()) * 1000.0;
    let start_ms = geometry.ms_of_beat(geometry.start_beat);
    let end_ms = geometry.ms_of_beat(geometry.end_beat());
    let mut tick_ms = ((start_ms / interval_ms).ceil().max(0.0)) * interval_ms;
    while tick_ms <= end_ms {
        let x = geometry.beat_to_x(geometry.beat_of_ms(tick_ms));
        line(
            &mut ops,
            x,
            geometry.ruler_height - TICK_LINE_ABOVE,
            x,
            pitch_bottom,
            ColorRole::GridLine,
            1.0,
        );
        text(
            &mut ops,
            TextLabel {
                x: x + 3.0,
                y: RULER_LABEL_BASELINE,
                content: &ruler_label(tick_ms),
                font: TextFont::Small,
                color: ColorRole::DisabledText,
                max_width: f64::INFINITY,
                opacity: 1.0,
            },
        );
        tick_ms += interval_ms;
    }

    // The lyrics lane (UI_SPEC 3.6): one label per note of the current line
    // and the faded neighbours, on two rows by the placement rule.
    if geometry.show_lyrics_lane {
        rect(
            &mut ops,
            0.0,
            lyrics_top,
            geometry.width,
            geometry.lyrics_height,
            ColorRole::WindowBackground,
        );
        let mut sorted: Vec<&LyricNote> = inputs.notes.iter().collect();
        sorted.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        let mut row_ends = [f64::NEG_INFINITY; 2];
        for (index, note) in sorted.iter().enumerate() {
            let x = geometry.beat_to_x(note.start_beat);
            let next_x = sorted
                .get(index + 1)
                .map(|next| geometry.beat_to_x(next.start_beat))
                .unwrap_or(f64::INFINITY);
            let (font, color, opacity) = if note.current {
                (TextFont::Lyrics, ColorRole::Text, 1.0)
            } else {
                (
                    TextFont::LyricsNeighbour,
                    ColorRole::DisabledText,
                    NEIGHBOUR_OPACITY,
                )
            };
            let label_width = (inputs.measure)(font, &note.text);
            // Row 1 when the label starts after the last row-1 label's end,
            // otherwise row 2, otherwise row 1 again (and overlap is elided
            // at the next label's start).
            let (row, baseline) = if x > row_ends[0] {
                (0, LYRICS_BASELINES[0])
            } else if x > row_ends[1] {
                (1, LYRICS_BASELINES[1])
            } else {
                (0, LYRICS_BASELINES[0])
            };
            row_ends[row] = x + label_width;
            text(
                &mut ops,
                TextLabel {
                    x,
                    y: lyrics_top + baseline,
                    content: &note.text,
                    font,
                    color,
                    max_width: (next_x - x).max(0.0),
                    opacity,
                },
            );
        }
        gutter_label(
            &mut ops,
            "Lyrics",
            lyrics_top,
            geometry.lyrics_height,
            (inputs.baseline_offset)(TextFont::Small),
        );
        // The gap strip between the lyrics and the waveform lane.
        rect(
            &mut ops,
            0.0,
            geometry.gap_top(),
            geometry.width,
            geometry.gap_height,
            ColorRole::ViewBackground,
        );
    }

    // The waveform and activity lanes (UI_SPEC 3.7): backgrounds, then the
    // columns of the lane math translated into the sheet.
    rect(
        &mut ops,
        0.0,
        wave_top,
        geometry.width,
        geometry.waveform_height,
        ColorRole::WindowBackground,
    );
    rect(
        &mut ops,
        0.0,
        activity_top,
        geometry.width,
        geometry.activity_height,
        ColorRole::ViewBackground,
    );
    if let Some(peaks) = inputs.peaks {
        let lane = LaneLayout {
            width: geometry.width,
            waveform_height: geometry.waveform_height,
            activity_height: geometry.activity_height,
            start_ms,
            end_ms,
            scale: LOUDEST_SHARE,
        };
        for op in lane_layout(&lane, peaks, inputs.activity) {
            match op {
                DrawOp::Waveform { x, top, bottom } => {
                    rect(
                        &mut ops,
                        x,
                        wave_top + top,
                        1.0,
                        bottom - top,
                        ColorRole::Waveform,
                    );
                }
                DrawOp::Activity { x } => {
                    rect(
                        &mut ops,
                        x,
                        activity_top,
                        1.0,
                        geometry.activity_height,
                        ColorRole::Activity,
                    );
                }
            }
        }
    }
    gutter_label(
        &mut ops,
        inputs.track_name,
        wave_top,
        geometry.waveform_height,
        (inputs.baseline_offset)(TextFont::Small),
    );
    ops
}
