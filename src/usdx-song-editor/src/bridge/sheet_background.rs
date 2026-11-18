//! About me: the SheetBackground bridge (plan section 8.3), the painted Qt
//! Quick item behind the note sheet. It paints everything in the sheet that
//! is not interactive (UI_SPEC section 3): the ruler with its ticks and
//! song-time labels, the pitch rows with their shading, grid lines and
//! labels, the lyrics lane with its two-row placement, the waveform lane
//! and the activity lane at the item's bottom (task P4-04). In the repair
//! page's overview mode (UI_SPEC 4.6, task P7-08) it paints the whole-song
//! strip instead: the line blocks coloured by the health verdict, the
//! racing underlays, the activity lane, the anchor flags and the time
//! labels, folded by controller::sheet's layout_overview from the list
//! properties QML binds to the Repair bridge's overview data. The draw list
//! comes from controller::sheet's layout(); paint only replays it, eliding
//! the lyric labels through the C++ textmetrics helpers (cxx-qt-lib has no
//! QFontMetrics). The time view lives in the SheetViewport controller
//! (task P5-01): the bridge converts the document's millisecond view to
//! beats and fits the viewport whenever the converted range or the item
//! size changed since the last fit (the fit key), so a resize recomputes
//! the zoom and zoom and pan survive stepping through short neighbouring
//! lines. The navigation gestures of UI_SPEC 3.2 (wheel time, Shift+wheel
//! pitch, Ctrl+wheel zoom around the cursor, middle-drag pan, the
//! scrollbar's scroll-to-beat) arrive as invokables that forward to the
//! viewport and republish its state; the time scroll stays between 0:00
//! and the song or audio end plus a margin (sync_bounds, user decision
//! 2026-11-15). The note data of
//! every line arrives as properties SongDocument
//! computes (user decision 2026-11-15: the sheet paints the whole song);
//! the pitch range runs through pitch_rows here. When the
//! waveform track or the vocals track changes, the item runs the usdx-audio
//! analysis on worker threads (`impl Threading`, cancelled when no longer
//! needed) and reports the lane status, the progress and whether the
//! activity lane has data, so the QML overlays show "Reading the
//! waveform…", "No audio" and the no-vocals note. The theme colours, fonts
//! and lane heights come from QML (Palette.qml, Metrics.qml, the theme), so
//! the item follows the theme.

use cxx_qt_lib::{QHash, QList, QString, QVariant};

use crate::controller::sheet::{OverviewAnchor, OverviewBlock};

/// parse_overview_lines reads the Repair bridge's line hashes (startMs,
/// endMs, health) into the overview strip's blocks; entries missing a key
/// are skipped instead of painted wrong.
pub fn parse_overview_lines(list: &QList<QVariant>) -> Vec<OverviewBlock> {
    let mut blocks = Vec::new();
    for index in 0..list.len() {
        let Some(entry) = list
            .get(index)
            .and_then(|variant| variant.value::<QHash<_>>())
        else {
            continue;
        };
        let (Some(start_ms), Some(end_ms), Some(health)) = (
            entry
                .get(&QString::from("startMs"))
                .and_then(|v| v.value::<f64>()),
            entry
                .get(&QString::from("endMs"))
                .and_then(|v| v.value::<f64>()),
            entry
                .get(&QString::from("health"))
                .and_then(|v| v.value::<i32>()),
        ) else {
            continue;
        };
        blocks.push(OverviewBlock {
            start_ms,
            end_ms,
            health,
        });
    }
    blocks
}

/// parse_overview_racing reads the racing sections' inclusive ranges
/// (first, last) from the Repair bridge's hashes.
pub fn parse_overview_racing(list: &QList<QVariant>) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for index in 0..list.len() {
        let Some(entry) = list
            .get(index)
            .and_then(|variant| variant.value::<QHash<_>>())
        else {
            continue;
        };
        let (Some(first), Some(last)) = (
            entry
                .get(&QString::from("first"))
                .and_then(|v| v.value::<i32>()),
            entry
                .get(&QString::from("last"))
                .and_then(|v| v.value::<i32>()),
        ) else {
            continue;
        };
        ranges.push((first.max(0) as usize, last.max(0) as usize));
    }
    ranges
}

/// parse_overview_anchors reads the anchors' pinned times and badges
/// (timeMs, badge) from the Repair bridge's hashes.
pub fn parse_overview_anchors(list: &QList<QVariant>) -> Vec<OverviewAnchor> {
    let mut anchors = Vec::new();
    for index in 0..list.len() {
        let Some(entry) = list
            .get(index)
            .and_then(|variant| variant.value::<QHash<_>>())
        else {
            continue;
        };
        let (Some(time_ms), Some(badge)) = (
            entry
                .get(&QString::from("timeMs"))
                .and_then(|v| v.value::<f64>()),
            entry
                .get(&QString::from("badge"))
                .and_then(|v| v.value::<i32>()),
        ) else {
            continue;
        };
        anchors.push(OverviewAnchor {
            time_ms,
            badge: badge.max(0) as usize,
        });
    }
    anchors
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qcolor.h");
        type QColor = cxx_qt_lib::QColor;

        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;

        include!("cxx-qt-lib/qsizef.h");
        type QSizeF = cxx_qt_lib::QSizeF;

        include!("cxx-qt-lib/qpainter.h");
        type QPainter = cxx_qt_lib::QPainter;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qfont.h");
        type QFont = cxx_qt_lib::QFont;

        include!("cxx-qt-lib/qpen.h");
        type QPen = cxx_qt_lib::QPen;

        include!("cxx-qt-lib/qlinef.h");
        type QLineF = cxx_qt_lib::QLineF;

        include!("cxx-qt-lib/qpoint.h");
        type QPoint = cxx_qt_lib::QPoint;

        include!("cxx-qt-lib/core/qlist/qlist_QVariant.h");
        type QList_QVariant = cxx_qt_lib::QList<QVariant>;

        include!("textmetrics.h");
        /// The pixel width of the text in the font.
        fn usdx_text_width(font: &QFont, text: &QString) -> f64;
        /// The baseline offset that centres a text line vertically.
        fn usdx_text_baseline_offset(font: &QFont) -> f64;
        /// The text elided on the right so it fits the width.
        fn usdx_elide_text(font: &QFont, text: &QString, max_width: f64) -> QString;
        /// Fills a rounded rectangle with a colour, no outline (the
        /// overview strip's line blocks and anchor flags).
        fn usdx_fill_rounded_rect(
            painter: Pin<&mut QPainter>,
            rect: &QRectF,
            radius: f64,
            color: &QColor,
        );
    }

    unsafe extern "C++" {
        include!(<QtQuick/QQuickPaintedItem>);
        /// The Qt Quick base class of painted items.
        type QQuickPaintedItem;
    }

    /// What the waveform lane shows (UI_SPEC 3.7); the values double as the
    /// i32 of the laneStatus property.
    #[qenum(SheetBackground)]
    enum LaneStatus {
        /// No waveform track, or its analysis failed: "No audio".
        NoAudio,
        /// The waveform is being read: "Reading the waveform… N %".
        Loading,
        /// The waveform is painted.
        Ready,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QQuickPaintedItem]
        #[qproperty(f64, start_ms, cxx_name = "startMs")]
        #[qproperty(f64, end_ms, cxx_name = "endMs")]
        // The song timing the beat axis maps through (UI_SPEC 3.2).
        #[qproperty(f64, bpm)]
        #[qproperty(f64, gap_ms, cxx_name = "gapMs")]
        // The song and audio ends the time scroll is limited by (user
        // decision 2026-11-15): the last note or phrase end beat, #END in
        // ms (0 or less: none) and the player's audio length in ms (0 or
        // less: unknown; the waveform's length stands in).
        #[qproperty(f64, song_end_beat, cxx_name = "songEndBeat")]
        #[qproperty(f64, end_mark_ms, cxx_name = "endMarkMs")]
        #[qproperty(f64, audio_end_ms, cxx_name = "audioEndMs")]
        // The beat range the time scroll is limited to (the scrollbar's
        // range); the bridge computes it from the ends above.
        #[qproperty(f64, min_beat, cxx_name = "minBeat")]
        #[qproperty(f64, max_beat, cxx_name = "maxBeat")]
        // The beat view the viewport computed from the millisecond view.
        #[qproperty(f64, start_beat, cxx_name = "startBeat")]
        #[qproperty(f64, px_per_beat, cxx_name = "pxPerBeat")]
        // The pitch rows' range and row height (UI_SPEC 3.3).
        #[qproperty(f64, pitch_top, cxx_name = "pitchTop")]
        #[qproperty(f64, px_per_semitone, cxx_name = "pxPerSemitone")]
        // The lyric notes of the current line and its neighbours, as
        // parallel lists, and the current line's pitch range (-1 when no
        // line is current); SongDocument computes them.
        #[qproperty(QList_QVariant, sheet_note_starts, cxx_name = "sheetNoteStarts")]
        #[qproperty(QList_QVariant, sheet_note_texts, cxx_name = "sheetNoteTexts")]
        #[qproperty(QList_QVariant, sheet_note_currents, cxx_name = "sheetNoteCurrents")]
        #[qproperty(i32, sheet_pitch_low, cxx_name = "sheetPitchLow")]
        #[qproperty(i32, sheet_pitch_high, cxx_name = "sheetPitchHigh")]
        #[qproperty(QString, audio_path, cxx_name = "audioPath")]
        #[qproperty(QString, vocals_path, cxx_name = "vocalsPath")]
        // The vertical stack (UI_SPEC 3.1); QML supplies the Metrics values.
        #[qproperty(f64, gutter_width, cxx_name = "gutterWidth")]
        #[qproperty(f64, ruler_height, cxx_name = "rulerHeight")]
        #[qproperty(f64, lyrics_height, cxx_name = "lyricsHeight")]
        #[qproperty(f64, gap_height, cxx_name = "gapHeight")]
        #[qproperty(f64, waveform_height, cxx_name = "waveformHeight")]
        #[qproperty(f64, activity_height, cxx_name = "activityHeight")]
        // Repair sheets show the sheet without the lyrics lane; the
        // overview mode of the repair strip arrives with task P7-08.
        #[qproperty(bool, show_lyrics_lane, cxx_name = "showLyricsLane")]
        // The editor sheet hides the lyrics lane's gutter label while its
        // Edit line text button sits there (round R-06).
        #[qproperty(bool, lyrics_label, cxx_name = "lyricsLabel")]
        #[qproperty(bool, overview)]
        // The overview strip's data (UI_SPEC 4.6), QML binds the Repair
        // bridge's lists and the song's audio duration.
        #[qproperty(f64, overview_duration_ms, cxx_name = "overviewDurationMs")]
        #[qproperty(QList_QVariant, overview_lines, cxx_name = "overviewLines")]
        #[qproperty(QList_QVariant, overview_racing, cxx_name = "overviewRacing")]
        #[qproperty(QList_QVariant, overview_anchors, cxx_name = "overviewAnchors")]
        #[qproperty(QString, track_name, cxx_name = "trackName")]
        // The theme colours (UI_SPEC 2.3), passed from QML.
        #[qproperty(QColor, view_background_color, cxx_name = "viewBackgroundColor")]
        #[qproperty(QColor, alternate_color, cxx_name = "alternateColor")]
        #[qproperty(QColor, header_background_color, cxx_name = "headerBackgroundColor")]
        #[qproperty(QColor, window_background_color, cxx_name = "windowBackgroundColor")]
        #[qproperty(QColor, grid_line_color, cxx_name = "gridLineColor")]
        #[qproperty(QColor, strong_grid_line_color, cxx_name = "strongGridLineColor")]
        #[qproperty(QColor, text_color, cxx_name = "textColor")]
        #[qproperty(QColor, disabled_text_color, cxx_name = "disabledTextColor")]
        #[qproperty(QColor, waveform_color, cxx_name = "waveformColor")]
        #[qproperty(QColor, activity_color, cxx_name = "activityColor")]
        // The overview strip's extra colours: the health verdicts (the
        // theme's positive, neutral and negative roles) and the anchor
        // colours (Palette, UI_SPEC 2.3).
        #[qproperty(QColor, positive_color, cxx_name = "positiveColor")]
        #[qproperty(QColor, neutral_color, cxx_name = "neutralColor")]
        #[qproperty(QColor, negative_color, cxx_name = "negativeColor")]
        #[qproperty(QColor, anchor_color, cxx_name = "anchorColor")]
        #[qproperty(QColor, anchor_text_color, cxx_name = "anchorTextColor")]
        // The sheet fonts (UI_SPEC 2.2, 3.3, 3.6), complete from QML
        // (cxx-qt-lib's QFont cannot set point size or weight).
        #[qproperty(QFont, small_font, cxx_name = "smallFont")]
        #[qproperty(QFont, small_demi_font, cxx_name = "smallDemiFont")]
        #[qproperty(QFont, lyrics_font, cxx_name = "lyricsFont")]
        #[qproperty(QFont, lyrics_neighbour_font, cxx_name = "lyricsNeighbourFont")]
        #[qproperty(i32, lane_status, cxx_name = "laneStatus")]
        #[qproperty(bool, peaks_ready, cxx_name = "peaksReady")]
        #[qproperty(f64, analysis_progress, cxx_name = "analysisProgress")]
        #[qproperty(bool, show_activity, cxx_name = "showActivity")]
        #[qproperty(QString, analysis_error, cxx_name = "analysisError")]
        type SheetBackground = super::SheetBackgroundRust;

        /// Paint the lanes; QQuickPaintedItem calls it on every update().
        #[qinvokable]
        #[cxx_override]
        unsafe fn paint(self: Pin<&mut Self>, painter: *mut QPainter);

        /// Pin the waveform lane in the loading state at the fraction; the
        /// screenshot harness uses it for the audio-states-loading demo state
        /// (UI_SPEC 8.1).
        #[qinvokable]
        #[cxx_name = "holdLoading"]
        fn hold_loading(self: Pin<&mut Self>, fraction: f64);

        /// Convert the millisecond view into beats and fit the viewport
        /// when the range changed; QML calls it on view and resize changes.
        #[qinvokable]
        #[cxx_name = "syncView"]
        fn sync_view(self: Pin<&mut Self>);

        /// Scroll time by 10 % of the visible width per wheel step
        /// (UI_SPEC 3.2); positive steps move forward.
        #[qinvokable]
        #[cxx_name = "wheelTime"]
        fn wheel_time(self: Pin<&mut Self>, steps: f64);

        /// Scroll the pitch range by one row per wheel step (UI_SPEC 3.2);
        /// positive steps scroll down.
        #[qinvokable]
        #[cxx_name = "wheelPitch"]
        fn wheel_pitch(self: Pin<&mut Self>, rows: f64);

        /// Zoom by the factor around the sheet x position under the cursor
        /// (UI_SPEC 3.2: Ctrl+wheel, 1.25 per step in, 0.8 per step out);
        /// the beat under the cursor stays at the cursor.
        #[qinvokable]
        #[cxx_name = "zoomAround"]
        fn zoom_around(self: Pin<&mut Self>, factor: f64, cursor_x: f64);

        /// Pan with a middle drag (UI_SPEC 3.2): moving the pointer right
        /// shows earlier beats; dx_px is the delta since the last call.
        #[qinvokable]
        #[cxx_name = "panTime"]
        fn pan_time(self: Pin<&mut Self>, dx_px: f64);

        /// Show the beat at the content's left edge (the horizontal
        /// scrollbar's position).
        #[qinvokable]
        #[cxx_name = "scrollToBeat"]
        fn scroll_to_beat(self: Pin<&mut Self>, beat: f64);

        /// Recompute the time scroll bounds from the song and audio ends
        /// (user decision 2026-11-15); the end properties call it on change.
        #[qinvokable]
        #[cxx_name = "syncBounds"]
        fn sync_bounds(self: Pin<&mut Self>);

        /// Take the note data over, compute the pitch rows from the pitch
        /// area height and repaint; QML calls it when the note data or the
        /// item's size changes.
        #[qinvokable]
        #[cxx_name = "syncLyrics"]
        fn sync_lyrics(self: Pin<&mut Self>);

        /// Convert a horizontal drag delta in pixels into an unrounded beat
        /// delta (plan 8.4, user decision 2026-11-14: the rounding happens
        /// after the snap in the controller's preview functions).
        #[qinvokable]
        #[cxx_name = "dragDeltaBeats"]
        fn drag_delta_beats(self: &SheetBackground, dx_px: f64) -> f64;

        /// Convert a vertical drag delta in pixels into a pitch delta:
        /// dragging down lowers the pitch.
        #[qinvokable]
        #[cxx_name = "dragDeltaPitch"]
        fn drag_delta_pitch(self: &SheetBackground, dy_px: f64) -> i64;

        /// Map a sheet x position to its beat (the rubber band); the beat
        /// view starts at the gutter's right edge.
        #[qinvokable]
        #[cxx_name = "xToBeat"]
        fn x_to_beat(self: &SheetBackground, x: f64) -> f64;

        /// Map a sheet y position to the pitch of the row it falls into.
        #[qinvokable]
        #[cxx_name = "yToPitch"]
        fn y_to_pitch(self: &SheetBackground, y: f64) -> i32;

        #[inherit]
        fn size(self: &Self) -> QSizeF;

        #[inherit]
        fn update(self: Pin<&mut Self>);
    }

    impl cxx_qt::Initialize for SheetBackground {}
    impl cxx_qt::Threading for SheetBackground {}
}

use core::pin::Pin;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QColor, QFont, QLineF, QPen, QPoint, QPointF, QRectF};
use usdx_audio::analysis::{Activity, Peaks, analyze};

use crate::controller::lanes::{AnalysisJob, LaneAnalysis, LaneStatus};
use crate::controller::sheet::{
    ColorRole, LyricNote, OverviewGeometry, OverviewInputs, SheetGeometry, SheetInputs, SheetOp,
    TextFont, layout, layout_overview, overview_duration_ms, pitch_rows,
};
use crate::controller::song_end::scroll_bounds;
use crate::controller::viewport::{FitKey, SheetViewport};

/// The peak bucket length of the analysis (plan section 7.2).
const BUCKET_MS: f64 = 10.0;

/// The Rust side of the SheetBackground: the property storage, the viewport
/// with the beat view, the note data of the current line, the lane analysis
/// state and the cancel flags of the running jobs.
#[derive(Default)]
pub struct SheetBackgroundRust {
    start_ms: f64,
    end_ms: f64,
    bpm: f64,
    gap_ms: f64,
    /// The beat view: the viewport holds the real state, the properties
    /// mirror it for QML (the note delegates of task P5-03 read them).
    viewport: SheetViewport,
    /// The song and audio ends and the scroll bounds (see the properties).
    song_end_beat: f64,
    end_mark_ms: f64,
    audio_end_ms: f64,
    min_beat: f64,
    max_beat: f64,
    /// The fit key of the viewport's last fit: the converted beat range
    /// and the item size; the view refits when either changes.
    fit_key: FitKey,
    /// The mirrored beat view properties (see viewport).
    start_beat: f64,
    px_per_beat: f64,
    pitch_top: f64,
    px_per_semitone: f64,
    /// The mirrored note data properties SongDocument pushed; sync_lyrics
    /// copies them into notes and line_pitch.
    sheet_note_starts: QList<QVariant>,
    sheet_note_texts: QList<QVariant>,
    sheet_note_currents: QList<QVariant>,
    sheet_pitch_low: i32,
    sheet_pitch_high: i32,
    /// The notes of the current line and its global neighbours, sorted by
    /// start, as the lyrics lane shows them.
    notes: Vec<LyricNote>,
    /// The current line's pitch range, none while no line is current.
    line_pitch: Option<(i32, i32)>,
    audio_path: QString,
    vocals_path: QString,
    gutter_width: f64,
    ruler_height: f64,
    lyrics_height: f64,
    gap_height: f64,
    waveform_height: f64,
    activity_height: f64,
    show_lyrics_lane: bool,
    lyrics_label: bool,
    overview: bool,
    overview_duration_ms: f64,
    overview_lines: QList<QVariant>,
    overview_racing: QList<QVariant>,
    overview_anchors: QList<QVariant>,
    /// The parsed overview strip data (see the properties).
    overview_line_blocks: Vec<OverviewBlock>,
    overview_racing_ranges: Vec<(usize, usize)>,
    overview_anchor_marks: Vec<OverviewAnchor>,
    track_name: QString,
    view_background_color: QColor,
    alternate_color: QColor,
    header_background_color: QColor,
    window_background_color: QColor,
    grid_line_color: QColor,
    strong_grid_line_color: QColor,
    text_color: QColor,
    disabled_text_color: QColor,
    waveform_color: QColor,
    activity_color: QColor,
    positive_color: QColor,
    neutral_color: QColor,
    negative_color: QColor,
    anchor_color: QColor,
    anchor_text_color: QColor,
    small_font: QFont,
    small_demi_font: QFont,
    lyrics_font: QFont,
    lyrics_neighbour_font: QFont,
    lane_status: i32,
    peaks_ready: bool,
    analysis_progress: f64,
    show_activity: bool,
    analysis_error: QString,
    lanes: LaneAnalysis,
    cancels: HashMap<u64, Arc<AtomicBool>>,
}

impl Drop for SheetBackgroundRust {
    /// A destroyed item stops its analyses; their results have nowhere to go.
    fn drop(&mut self) {
        for cancel in self.cancels.values() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl SheetBackgroundRust {
    /// color_of resolves a draw-list colour role to the theme colour
    /// property QML supplied (UI_SPEC 2.3).
    fn color_of(&self, role: ColorRole) -> &QColor {
        match role {
            ColorRole::ViewBackground => &self.view_background_color,
            ColorRole::Alternate => &self.alternate_color,
            ColorRole::HeaderBackground => &self.header_background_color,
            ColorRole::WindowBackground => &self.window_background_color,
            ColorRole::GridLine => &self.grid_line_color,
            ColorRole::StrongGridLine => &self.strong_grid_line_color,
            ColorRole::Text => &self.text_color,
            ColorRole::DisabledText => &self.disabled_text_color,
            ColorRole::Waveform => &self.waveform_color,
            ColorRole::Activity => &self.activity_color,
            ColorRole::Positive => &self.positive_color,
            ColorRole::Neutral => &self.neutral_color,
            ColorRole::Negative => &self.negative_color,
            ColorRole::Anchor => &self.anchor_color,
            ColorRole::AnchorText => &self.anchor_text_color,
        }
    }

    /// font_of resolves a draw-list font role to the font property QML
    /// supplied (UI_SPEC 2.2).
    fn font_of(&self, font: TextFont) -> &QFont {
        match font {
            TextFont::Small => &self.small_font,
            TextFont::SmallDemi => &self.small_demi_font,
            TextFont::Lyrics => &self.lyrics_font,
            TextFont::LyricsNeighbour => &self.lyrics_neighbour_font,
        }
    }
}

/// replay draws one op of the sheet's draw list; Rect fills, Line strokes
/// with a 1 px pen and Text draws at its baseline, elided to max_width when
/// one is given (UI_SPEC 3.6).
fn replay(mut painter: Pin<&mut qobject::QPainter>, op: &SheetOp, rust: &SheetBackgroundRust) {
    match op {
        SheetOp::Rect {
            x,
            y,
            width,
            height,
            color,
        } => painter
            .as_mut()
            .fill_rect(&QRectF::new(*x, *y, *width, *height), rust.color_of(*color)),
        SheetOp::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let mut pen = QPen::default();
            pen.set_color(rust.color_of(*color));
            pen.set_width((*width).round() as i32);
            painter.as_mut().set_pen(&pen);
            painter
                .as_mut()
                .draw_linef(&QLineF::new(QPointF::new(*x1, *y1), QPointF::new(*x2, *y2)));
        }
        SheetOp::Text {
            x,
            y,
            text,
            font,
            color,
            max_width,
            opacity,
        } => {
            let font = rust.font_of(*font);
            let shown = if max_width.is_finite() && *max_width > 0.0 {
                qobject::usdx_elide_text(font, &QString::from(text.as_str()), *max_width)
            } else if max_width.is_finite() {
                // No room at all: nothing to show.
                QString::default()
            } else {
                QString::from(text.as_str())
            };
            let mut pen = QPen::default();
            pen.set_color(rust.color_of(*color));
            painter.as_mut().set_pen(&pen);
            painter.as_mut().set_font(font);
            painter.as_mut().set_opacity(*opacity);
            painter.as_mut().draw_text(
                &QPoint::new((*x).round() as i32, (*y).round() as i32),
                &shown,
            );
            painter.as_mut().set_opacity(1.0);
        }
        SheetOp::RoundedRect {
            x,
            y,
            width,
            height,
            radius,
            color,
        } => qobject::usdx_fill_rounded_rect(
            painter.as_mut(),
            &QRectF::new(*x, *y, *width, *height),
            *radius,
            rust.color_of(*color),
        ),
        SheetOp::FaintRect {
            x,
            y,
            width,
            height,
            color,
            opacity,
        } => {
            painter.as_mut().set_opacity(*opacity);
            painter
                .as_mut()
                .fill_rect(&QRectF::new(*x, *y, *width, *height), rust.color_of(*color));
            painter.as_mut().set_opacity(1.0);
        }
    }
}

impl cxx_qt::Initialize for qobject::SheetBackground {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut()
            .on_audio_path_changed(|item| item.sync_tracks())
            .release();
        self.as_mut()
            .on_vocals_path_changed(|item| item.sync_tracks())
            .release();
        self.as_mut()
            .on_start_ms_changed(|item| item.sync_view())
            .release();
        self.as_mut()
            .on_end_ms_changed(|item| item.sync_view())
            .release();
        self.as_mut()
            .on_bpm_changed(|item| item.sync_view())
            .release();
        self.as_mut()
            .on_gap_ms_changed(|item| item.sync_view())
            .release();
        self.as_mut()
            .on_song_end_beat_changed(|item| item.sync_bounds())
            .release();
        self.as_mut()
            .on_end_mark_ms_changed(|item| item.sync_bounds())
            .release();
        self.as_mut()
            .on_audio_end_ms_changed(|item| item.sync_bounds())
            .release();
        self.as_mut()
            .on_sheet_note_starts_changed(|item| item.sync_lyrics())
            .release();
        self.as_mut()
            .on_sheet_note_texts_changed(|item| item.sync_lyrics())
            .release();
        self.as_mut()
            .on_sheet_note_currents_changed(|item| item.sync_lyrics())
            .release();
        self.as_mut()
            .on_sheet_pitch_low_changed(|item| item.sync_lyrics())
            .release();
        self.as_mut()
            .on_sheet_pitch_high_changed(|item| item.sync_lyrics())
            .release();
        self.as_mut()
            .on_view_background_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_alternate_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_header_background_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_window_background_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_grid_line_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_strong_grid_line_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_text_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_disabled_text_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_waveform_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_activity_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_positive_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_neutral_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_negative_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_anchor_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_anchor_text_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_small_font_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_small_demi_font_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_lyrics_font_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_lyrics_neighbour_font_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_gutter_width_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_ruler_height_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_lyrics_height_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_gap_height_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_waveform_height_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_activity_height_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_show_lyrics_lane_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_lyrics_label_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_overview_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_overview_duration_ms_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_overview_lines_changed(|item| item.sync_overview())
            .release();
        self.as_mut()
            .on_overview_racing_changed(|item| item.sync_overview())
            .release();
        self.as_mut()
            .on_overview_anchors_changed(|item| item.sync_overview())
            .release();
        self.as_mut()
            .on_track_name_changed(|item| item.update())
            .release();
        self.as_mut().publish();
    }
}

impl qobject::SheetBackground {
    /// paint replays the sheet's draw list (controller::sheet::layout) over
    /// the full item: ruler, pitch rows, lyrics lane, gap, waveform and
    /// activity lanes (UI_SPEC section 3).
    ///
    /// # Safety
    ///
    /// `painter` is the painter QQuickPaintedItem hands to paint(); it is
    /// valid for the duration of the call.
    pub unsafe fn paint(self: Pin<&mut Self>, painter: *mut qobject::QPainter) {
        // SAFETY: Qt passes a live painter for the duration of paint().
        let Some(painter) = (unsafe { painter.as_mut() }) else {
            return;
        };
        // SAFETY: the painter is not moved while pinned here.
        let mut painter = unsafe { Pin::new_unchecked(painter) };
        let size = self.as_ref().size();
        let (width, height) = (size.width(), size.height());
        let rust = self.rust();
        // The repair page's overview mode paints the whole-song strip
        // (UI_SPEC 4.6) instead of the sheet lanes.
        if rust.overview {
            let geometry = OverviewGeometry { width, height };
            let measure = |font: TextFont, text: &str| {
                qobject::usdx_text_width(rust.font_of(font), &QString::from(text))
            };
            let baseline_offset =
                |font: TextFont| qobject::usdx_text_baseline_offset(rust.font_of(font));
            let inputs = OverviewInputs {
                duration_ms: overview_duration_ms(rust.overview_duration_ms, rust.lanes.activity()),
                lines: &rust.overview_line_blocks,
                racing: &rust.overview_racing_ranges,
                anchors: &rust.overview_anchor_marks,
                activity: rust.lanes.activity(),
                measure: &measure,
                baseline_offset: &baseline_offset,
            };
            for op in layout_overview(&geometry, &inputs) {
                replay(painter.as_mut(), &op, rust);
            }
            return;
        }
        // Without a song (no BPM) or size there is nothing to place.
        if width <= 0.0 || height <= 0.0 || rust.bpm <= 0.0 || rust.viewport.px_per_beat <= 0.0 {
            return;
        }
        let track_name = String::from(&rust.track_name);
        let geometry = SheetGeometry {
            width,
            height,
            gutter_width: rust.gutter_width,
            ruler_height: rust.ruler_height,
            lyrics_height: rust.lyrics_height,
            gap_height: rust.gap_height,
            waveform_height: rust.waveform_height,
            activity_height: rust.activity_height,
            show_lyrics_lane: rust.show_lyrics_lane,
            lyrics_label: rust.lyrics_label,
            start_beat: rust.viewport.start_beat,
            px_per_beat: rust.viewport.px_per_beat,
            top_pitch: rust.viewport.top_pitch.round() as i32,
            px_per_semitone: rust.viewport.px_per_semitone,
            bpm: rust.bpm,
            gap_ms: rust.gap_ms,
        };
        let measure = |font: TextFont, text: &str| {
            qobject::usdx_text_width(rust.font_of(font), &QString::from(text))
        };
        let baseline_offset =
            |font: TextFont| qobject::usdx_text_baseline_offset(rust.font_of(font));
        let inputs = SheetInputs {
            peaks: rust.lanes.peaks(),
            activity: rust.lanes.activity(),
            notes: &rust.notes,
            track_name: &track_name,
            line_pitch: rust.line_pitch,
            measure: &measure,
            baseline_offset: &baseline_offset,
        };
        for op in layout(&geometry, &inputs) {
            replay(painter.as_mut(), &op, rust);
        }
    }

    /// drag_delta_beats converts a horizontal drag delta into beats without
    /// rounding; the preview rounds after the snap (plan 8.4).
    pub fn drag_delta_beats(&self, dx_px: f64) -> f64 {
        let px_per_beat = *self.px_per_beat();
        if px_per_beat <= 0.0 {
            return 0.0;
        }
        dx_px / px_per_beat
    }

    /// drag_delta_pitch converts a vertical drag delta into semitones:
    /// dragging down lowers the pitch.
    pub fn drag_delta_pitch(&self, dy_px: f64) -> i64 {
        let px_per_semitone = *self.px_per_semitone();
        if px_per_semitone <= 0.0 {
            return 0;
        }
        (-dy_px / px_per_semitone).round() as i64
    }

    /// x_to_beat maps a sheet x position to its beat; the beat view starts
    /// at the gutter's right edge.
    pub fn x_to_beat(&self, x: f64) -> f64 {
        let px_per_beat = *self.px_per_beat();
        if px_per_beat <= 0.0 {
            return *self.start_beat();
        }
        *self.start_beat() + (x - *self.gutter_width()) / px_per_beat
    }

    /// y_to_pitch maps a sheet y position to the pitch of the row it falls
    /// into; the pitch area starts below the ruler.
    pub fn y_to_pitch(&self, y: f64) -> i32 {
        let px_per_semitone = *self.px_per_semitone();
        if px_per_semitone <= 0.0 {
            return *self.pitch_top() as i32;
        }
        let rows = ((y - *self.ruler_height()) / px_per_semitone)
            .floor()
            .max(0.0);
        (*self.pitch_top()).round() as i32 - rows as i32
    }

    /// sync_view converts the millisecond view into beats and fits the
    /// viewport when the converted range changed (UI_SPEC 3.2): zoom and
    /// pan survive unchanged ranges and resizes.
    pub fn sync_view(mut self: Pin<&mut Self>) {
        self.as_mut().sync_bounds();
        let bpm = *self.bpm();
        if bpm <= 0.0 {
            return;
        }
        let size = self.as_ref().size();
        // The gutter holds only the labels, so the beat view fits and maps
        // the content width right of the gutter (UI_SPEC 3.1).
        let content_width = (size.width() - *self.gutter_width()).max(0.0);
        // One beat is 15000 / bpm ms (the USDX timing), so a duration in
        // ms maps to beats with bpm / 15000.
        let start_beat = (*self.start_ms() - *self.gap_ms()) * bpm / 15000.0;
        let end_beat = (*self.end_ms() - *self.gap_ms()) * bpm / 15000.0;
        let mut rust = self.as_mut().rust_mut();
        rust.viewport.set_size(content_width, size.height());
        if rust
            .fit_key
            .stale((start_beat, end_beat), (content_width, size.height()))
        {
            rust.fit_key = FitKey::new((start_beat, end_beat), (content_width, size.height()));
            rust.viewport.fit_range(start_beat, end_beat);
        }
        let (start, px_per_beat) = (rust.viewport.start_beat, rust.viewport.px_per_beat);
        self.as_mut().set_start_beat(start);
        self.as_mut().set_px_per_beat(px_per_beat);
        self.as_mut().update();
    }

    /// publish_viewport copies the viewport's live state into the mirrored
    /// properties and repaints; every gesture invokable ends here so the
    /// QML bindings (the delegates' beat axis, the scrollbar) follow.
    fn publish_viewport(mut self: Pin<&mut Self>) {
        let start = self.as_ref().rust().viewport.start_beat;
        let px_per_beat = self.as_ref().rust().viewport.px_per_beat;
        self.as_mut().set_start_beat(start);
        self.as_mut().set_px_per_beat(px_per_beat);
        self.as_mut().update();
    }

    /// wheel_time scrolls time by 10 % of the visible width per step
    /// (UI_SPEC 3.2); positive steps move forward.
    pub fn wheel_time(mut self: Pin<&mut Self>, steps: f64) {
        self.as_mut().rust_mut().viewport.wheel_time(steps);
        self.as_mut().publish_viewport();
    }

    /// wheel_pitch scrolls the pitch range by one row per step (UI_SPEC
    /// 3.2); positive steps scroll down. The pitch rows paint from the
    /// published top pitch, so republishing shifts them.
    pub fn wheel_pitch(mut self: Pin<&mut Self>, rows: f64) {
        self.as_mut().rust_mut().viewport.wheel_pitch(rows);
        let top = self.as_ref().rust().viewport.top_pitch;
        self.as_mut().set_pitch_top(top);
        self.as_mut().update();
    }

    /// zoom_around zooms by the factor around the sheet x position under
    /// the cursor (UI_SPEC 3.2); the gutter holds only the labels, so the
    /// anchor maps right of it like xToBeat does.
    pub fn zoom_around(mut self: Pin<&mut Self>, factor: f64, cursor_x: f64) {
        let gutter = *self.gutter_width();
        self.as_mut()
            .rust_mut()
            .viewport
            .zoom_around(factor, cursor_x - gutter);
        self.as_mut().publish_viewport();
    }

    /// pan_time pans with a middle drag (UI_SPEC 3.2); dx_px is the delta
    /// since the last call.
    pub fn pan_time(mut self: Pin<&mut Self>, dx_px: f64) {
        self.as_mut().rust_mut().viewport.pan_time(dx_px);
        self.as_mut().publish_viewport();
    }

    /// scroll_to_beat shows the beat at the content's left edge (the
    /// horizontal scrollbar's position).
    pub fn scroll_to_beat(mut self: Pin<&mut Self>, beat: f64) {
        self.as_mut().rust_mut().viewport.scroll_to(beat);
        self.as_mut().publish_viewport();
    }

    /// sync_bounds limits the time scroll (user decision 2026-11-15): from
    /// 0:00 to the later of the audio end (#END in its place) and the last
    /// note, plus the margin. The audio end is the player's length, else
    /// the analysed waveform's length. Without a tempo the scroll is free.
    pub fn sync_bounds(mut self: Pin<&mut Self>) {
        let bpm = *self.bpm();
        if bpm <= 0.0 {
            self.as_mut().rust_mut().viewport.set_bounds(None);
            return;
        }
        let audio = Some(*self.audio_end_ms())
            .filter(|ms| *ms > 0.0)
            .or_else(|| {
                self.rust()
                    .lanes
                    .peaks()
                    .map(|peaks| peaks.min.len() as f64 * peaks.bucket_ms)
            });
        let end = Some(*self.end_mark_ms()).filter(|ms| *ms > 0.0);
        let (min, max) = scroll_bounds(*self.gap_ms(), bpm, audio, end, *self.song_end_beat());
        self.as_mut()
            .rust_mut()
            .viewport
            .set_bounds(Some((min, max)));
        self.as_mut().set_min_beat(min);
        self.as_mut().set_max_beat(max);
        self.as_mut().publish_viewport();
    }

    /// sync_lyrics takes the note data over, computes the pitch rows from
    /// the pitch area's height (UI_SPEC 3.3) and repaints.
    pub fn sync_lyrics(mut self: Pin<&mut Self>) {
        let starts = self.sheet_note_starts();
        let texts = self.sheet_note_texts();
        let currents = self.sheet_note_currents();
        let count = starts.len().min(texts.len()).min(currents.len());
        let mut notes = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            notes.push(LyricNote {
                start_beat: starts
                    .get(index)
                    .and_then(QVariant::value::<f64>)
                    .unwrap_or(0.0),
                text: texts
                    .get(index)
                    .and_then(QVariant::value::<QString>)
                    .map(|text| String::from(&text))
                    .unwrap_or_default(),
                current: currents
                    .get(index)
                    .and_then(QVariant::value::<bool>)
                    .unwrap_or(false),
            });
        }
        let low = *self.sheet_pitch_low();
        let high = *self.sheet_pitch_high();
        let pitch = (low >= 0 && high >= 0).then_some((low, high));
        if let Some((low, high)) = pitch {
            let size = self.as_ref().size();
            let pitch_height = (size.height()
                - *self.ruler_height()
                - if *self.show_lyrics_lane() {
                    *self.lyrics_height() + *self.gap_height()
                } else {
                    0.0
                }
                - *self.waveform_height()
                - *self.activity_height())
            .max(0.0);
            if pitch_height > 0.0 {
                let (top, row_height) = pitch_rows(low, high, pitch_height);
                let mut rust = self.as_mut().rust_mut();
                rust.viewport.top_pitch = f64::from(top);
                rust.viewport.px_per_semitone = row_height;
            }
        }
        let mut rust = self.as_mut().rust_mut();
        rust.notes = notes;
        rust.line_pitch = pitch;
        let (top, row_height) = (rust.viewport.top_pitch, rust.viewport.px_per_semitone);
        self.as_mut().set_pitch_top(top);
        self.as_mut().set_px_per_semitone(row_height);
        self.as_mut().update();
    }

    /// sync_overview parses the overview strip's list properties into the
    /// draw data; QML binds them to the Repair bridge's lists.
    pub fn sync_overview(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        rust.overview_line_blocks = parse_overview_lines(&rust.overview_lines);
        rust.overview_racing_ranges = parse_overview_racing(&rust.overview_racing);
        rust.overview_anchor_marks = parse_overview_anchors(&rust.overview_anchors);
        self.as_mut().update();
    }

    /// hold_loading pins the lane at the fraction and stops the analyses.
    pub fn hold_loading(mut self: Pin<&mut Self>, fraction: f64) {
        let cancel = self.as_mut().rust_mut().lanes.hold_loading(fraction as f32);
        self.as_mut().cancel_jobs(&cancel);
        self.as_mut().publish();
    }

    /// sync_tracks hands the current track paths to the lane state, cancels
    /// the analyses it no longer needs and starts the new ones.
    fn sync_tracks(mut self: Pin<&mut Self>) {
        let waveform = path_of(self.as_ref().audio_path());
        let vocals = path_of(self.as_ref().vocals_path());
        let change = self.as_mut().rust_mut().lanes.set_tracks(waveform, vocals);
        self.as_mut().cancel_jobs(&change.cancel);
        for job in change.start {
            self.as_mut().start_job(job);
        }
        self.as_mut().publish();
    }

    /// cancel_jobs raises the cancel flags of the jobs.
    fn cancel_jobs(mut self: Pin<&mut Self>, ids: &[u64]) {
        for id in ids {
            if let Some(cancel) = self.as_mut().rust_mut().cancels.remove(id) {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// start_job runs one analysis on a worker thread; progress (in whole
    /// percent steps) and the result return to the Qt thread through the
    /// queue.
    fn start_job(mut self: Pin<&mut Self>, job: AnalysisJob) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut()
            .rust_mut()
            .cancels
            .insert(job.id, Arc::clone(&cancel));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let id = job.id;
            let last_percent = Cell::new(-1);
            let progress_thread = thread.clone();
            let report = |fraction: f32| {
                let percent = (fraction * 100.0) as i32;
                if percent != last_percent.get() {
                    last_percent.set(percent);
                    let _ = progress_thread.queue(move |item| item.job_progress(id, fraction));
                }
            };
            let result =
                analyze(&job.path, BUCKET_MS, &cancel, &report).map_err(|error| error.to_string());
            let _ = thread.queue(move |item| item.job_finished(id, result));
        });
    }

    /// job_progress records a worker's progress.
    fn job_progress(mut self: Pin<&mut Self>, id: u64, fraction: f32) {
        self.as_mut().rust_mut().lanes.job_progress(id, fraction);
        self.as_mut().publish();
    }

    /// job_finished takes a worker's result.
    fn job_finished(mut self: Pin<&mut Self>, id: u64, result: Result<(Peaks, Activity), String>) {
        self.as_mut().rust_mut().cancels.remove(&id);
        self.as_mut().rust_mut().lanes.job_finished(id, result);
        self.as_mut().publish();
    }

    /// publish mirrors the lane state into the properties and repaints.
    fn publish(mut self: Pin<&mut Self>) {
        let lanes = &self.rust().lanes;
        let status = match lanes.status() {
            LaneStatus::NoAudio => qobject::LaneStatus::NoAudio,
            LaneStatus::Loading => qobject::LaneStatus::Loading,
            LaneStatus::Ready => qobject::LaneStatus::Ready,
        };
        let peaks_ready = lanes.peaks().is_some();
        let progress = f64::from(lanes.progress());
        let show_activity = lanes.has_vocals();
        let error = QString::from(lanes.error().unwrap_or_default());
        self.as_mut().set_lane_status(status.repr);
        self.as_mut().set_peaks_ready(peaks_ready);
        self.as_mut().set_analysis_progress(progress);
        self.as_mut().set_show_activity(show_activity);
        self.as_mut().set_analysis_error(error);
        // A finished waveform tells the audio length the scroll bounds use.
        self.as_mut().sync_bounds();
        self.as_mut().update();
    }
}

/// path_of turns a property path into an optional file path; empty means none.
fn path_of(path: &QString) -> Option<PathBuf> {
    let path = String::from(path);
    (!path.is_empty()).then(|| PathBuf::from(path))
}
