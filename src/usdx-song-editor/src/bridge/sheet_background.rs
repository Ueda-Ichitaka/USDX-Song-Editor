//! About me: the SheetBackground bridge (plan section 8.3), the painted Qt
//! Quick item behind the note sheet. It paints everything in the sheet that
//! is not interactive (UI_SPEC section 3): the ruler with its ticks and
//! song-time labels, the pitch rows with their shading, grid lines and
//! labels, the lyrics lane with its two-row placement, the waveform lane
//! and the activity lane at the item's bottom (task P4-04). The draw list
//! comes from controller::sheet's layout(); paint only replays it, eliding
//! the lyric labels through the C++ textmetrics helpers (cxx-qt-lib has no
//! QFontMetrics). The time view lives in the SheetViewport controller
//! (task P5-01): the bridge converts the document's millisecond view to
//! beats and fits the viewport whenever the converted range or the item
//! size changed since the last fit (the fit key), so a resize recomputes
//! the zoom and zoom and pan survive stepping through short neighbouring
//! lines. The note data of
//! the current line and its neighbours arrives as properties SongDocument
//! computes (user decision 2026-11-13: data push instead of a cross-bridge
//! pointer); the pitch range runs through pitch_rows here. When the
//! waveform track or the vocals track changes, the item runs the usdx-audio
//! analysis on worker threads (`impl Threading`, cancelled when no longer
//! needed) and reports the lane status, the progress and whether the
//! activity lane has data, so the QML overlays show "Reading the
//! waveform…", "No audio" and the no-vocals note. The theme colours, fonts
//! and lane heights come from QML (Palette.qml, Metrics.qml, the theme), so
//! the item follows the theme.

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
        #[qproperty(bool, overview)]
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

        /// Take the note data over, compute the pitch rows from the pitch
        /// area height and repaint; QML calls it when the note data or the
        /// item's size changes.
        #[qinvokable]
        #[cxx_name = "syncLyrics"]
        fn sync_lyrics(self: Pin<&mut Self>);

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
use cxx_qt_lib::{QColor, QFont, QLineF, QList, QPen, QPoint, QPointF, QRectF, QString, QVariant};
use usdx_audio::analysis::{Activity, Peaks, analyze};

use crate::controller::lanes::{AnalysisJob, LaneAnalysis, LaneStatus};
use crate::controller::sheet::{
    ColorRole, LyricNote, SheetGeometry, SheetInputs, SheetOp, TextFont, layout, pitch_rows,
};
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
    overview: bool,
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

    /// sync_view converts the millisecond view into beats and fits the
    /// viewport when the converted range changed (UI_SPEC 3.2): zoom and
    /// pan survive unchanged ranges and resizes.
    pub fn sync_view(mut self: Pin<&mut Self>) {
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
        self.as_mut().update();
    }
}

/// path_of turns a property path into an optional file path; empty means none.
fn path_of(path: &QString) -> Option<PathBuf> {
    let path = String::from(path);
    (!path.is_empty()).then(|| PathBuf::from(path))
}
