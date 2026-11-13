//! About me: the SheetBackground bridge (plan section 8.3), the painted Qt
//! Quick item behind the note sheet. It paints everything in the sheet that
//! is not interactive; in task P4-04 that is the waveform lane and the
//! activity lane at the item's bottom (UI_SPEC 3.1, 3.7), and task P5-02 adds
//! the ruler, the pitch rows and the lyrics lane to the same item. When the
//! waveform track or the vocals track changes, it runs the usdx-audio
//! analysis on worker threads (`impl Threading`, cancelled when no longer
//! needed) and reports the lane status, the progress and whether the
//! activity lane has data, so the QML overlays show "Reading the waveform…",
//! "No audio" and the no-vocals note. The decisions live in
//! controller::lanes, the column math in controller::waveform; this file
//! only threads and paints. The theme colours and lane heights come from QML
//! (Palette.qml, Metrics.qml), so the item follows the theme.

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
        #[qproperty(QString, audio_path, cxx_name = "audioPath")]
        #[qproperty(QString, vocals_path, cxx_name = "vocalsPath")]
        #[qproperty(f64, waveform_height, cxx_name = "waveformHeight")]
        #[qproperty(f64, activity_height, cxx_name = "activityHeight")]
        #[qproperty(QColor, waveform_color, cxx_name = "waveformColor")]
        #[qproperty(QColor, activity_color, cxx_name = "activityColor")]
        #[qproperty(QColor, lane_color, cxx_name = "laneColor")]
        #[qproperty(QColor, activity_lane_color, cxx_name = "activityLaneColor")]
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
use cxx_qt_lib::{QColor, QRectF, QString};
use usdx_audio::analysis::{Activity, Peaks, analyze};

use crate::controller::lanes::{AnalysisJob, LaneAnalysis, LaneStatus};
use crate::controller::waveform::{DrawOp, LaneLayout, layout};

/// The peak bucket length of the analysis (plan section 7.2).
const BUCKET_MS: f64 = 10.0;
/// The share of the waveform lane height the loudest bucket uses (UI_SPEC 3.7).
const LOUDEST_SHARE: f64 = 0.92;

/// The Rust side of the SheetBackground: the property storage, the lane
/// analysis state and the cancel flags of the running jobs.
#[derive(Default)]
pub struct SheetBackgroundRust {
    start_ms: f64,
    end_ms: f64,
    audio_path: QString,
    vocals_path: QString,
    waveform_height: f64,
    activity_height: f64,
    waveform_color: QColor,
    activity_color: QColor,
    lane_color: QColor,
    activity_lane_color: QColor,
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

impl cxx_qt::Initialize for qobject::SheetBackground {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut()
            .on_audio_path_changed(|item| item.sync_tracks())
            .release();
        self.as_mut()
            .on_vocals_path_changed(|item| item.sync_tracks())
            .release();
        self.as_mut()
            .on_start_ms_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_end_ms_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_waveform_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_activity_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_lane_color_changed(|item| item.update())
            .release();
        self.as_mut()
            .on_activity_lane_color_changed(|item| item.update())
            .release();
        self.as_mut().publish();
    }
}

impl qobject::SheetBackground {
    /// paint fills the waveform lane and the activity lane at the item's
    /// bottom with their backgrounds, then replays the draw list of the
    /// visible time range.
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
        let waveform_top = height - rust.activity_height - rust.waveform_height;
        let activity_top = waveform_top + rust.waveform_height;
        painter.as_mut().fill_rect(
            &QRectF::new(0.0, waveform_top, width, rust.waveform_height),
            &rust.lane_color,
        );
        painter.as_mut().fill_rect(
            &QRectF::new(0.0, activity_top, width, rust.activity_height),
            &rust.activity_lane_color,
        );
        let Some(peaks) = rust.lanes.peaks() else {
            return;
        };
        let lane = LaneLayout {
            width,
            waveform_height: rust.waveform_height,
            activity_height: rust.activity_height,
            start_ms: rust.start_ms,
            end_ms: rust.end_ms,
            scale: LOUDEST_SHARE,
        };
        for op in layout(&lane, peaks, rust.lanes.activity()) {
            match op {
                DrawOp::Waveform { x, top, bottom } => painter.as_mut().fill_rect(
                    &QRectF::new(x, waveform_top + top, 1.0, bottom - top),
                    &rust.waveform_color,
                ),
                DrawOp::Activity { x } => painter.as_mut().fill_rect(
                    &QRectF::new(x, activity_top, 1.0, rust.activity_height),
                    &rust.activity_color,
                ),
            }
        }
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
