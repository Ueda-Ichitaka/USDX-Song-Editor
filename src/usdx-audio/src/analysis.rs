//! About me: the waveform peak extraction and vocal activity analysis of the
//! usdx-audio library (plan section 7.2, task P4-03). It decodes an audio
//! file once through GStreamer into 22050 Hz mono samples, folds them into
//! min/max/rms peak buckets for the waveform view, and derives a vocal
//! activity curve (RMS dBFS per window, noise floor at the 10th percentile,
//! active above floor + 12 dB, gaps shorter than 100 ms closed) for the
//! sheet background. Results go through the versioned cache in cache.rs, so
//! a song is decoded only once per file version.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

use crate::cache;
use crate::player::AudioError;

/// The sample rate the analysis pipeline resamples to.
const ANALYSIS_RATE: i32 = 22050;
/// Levels below this dBFS count as digital silence (0.0 rms).
const SILENCE_DB: f32 = -140.0;
/// A window is active when its dBFS level is above the noise floor plus
/// this many dB.
const ACTIVE_OVER_FLOOR_DB: f32 = 12.0;

/// The waveform peaks of one audio file: min, max and rms level per bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct Peaks {
    pub bucket_ms: f64,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    pub rms: Vec<f32>,
}

/// The vocal activity of one audio file: one flag per activity window
/// (twice the peak bucket length) and the noise floor in dBFS.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub bucket_ms: f64,
    pub active: Vec<bool>,
    pub floor_db: f32,
}

/// Accumulates samples into min/max/rms buckets of `bucket_ms` milliseconds.
struct BucketFold {
    bucket_ms: f64,
    min: Vec<f32>,
    max: Vec<f32>,
    rms: Vec<f32>,
    // The index of the bucket currently being filled.
    current: Option<usize>,
    current_min: f32,
    current_max: f32,
    current_sum_sq: f64,
    current_count: u64,
    // Samples seen so far, for the position when a buffer has no timestamp.
    samples_seen: u64,
}

impl BucketFold {
    fn new(bucket_ms: f64) -> BucketFold {
        BucketFold {
            bucket_ms,
            min: Vec::new(),
            max: Vec::new(),
            rms: Vec::new(),
            current: None,
            current_min: f32::MAX,
            current_max: f32::MIN,
            current_sum_sq: 0.0,
            current_count: 0,
            samples_seen: 0,
        }
    }

    /// Adds one sample at stream position `position_ms` (when known).
    fn push(&mut self, sample: f32, position_ms: Option<f64>) {
        let fallback = self.samples_seen as f64 * 1000.0 / f64::from(ANALYSIS_RATE);
        let position = position_ms.unwrap_or(fallback);
        self.samples_seen += 1;
        let index = (position / self.bucket_ms).floor() as usize;
        if self.current == Some(index) {
            self.accumulate(sample);
            return;
        }
        // Finish the bucket being filled and create any buckets the stream
        // jumped over (a discont) as silent.
        self.finish_current();
        while self.min.len() < index {
            self.min.push(0.0);
            self.max.push(0.0);
            self.rms.push(0.0);
        }
        self.current = Some(index);
        self.current_min = f32::MAX;
        self.current_max = f32::MIN;
        self.current_sum_sq = 0.0;
        self.current_count = 0;
        self.accumulate(sample);
    }

    fn accumulate(&mut self, sample: f32) {
        self.current_min = self.current_min.min(sample);
        self.current_max = self.current_max.max(sample);
        self.current_sum_sq += f64::from(sample) * f64::from(sample);
        self.current_count += 1;
    }

    fn finish_current(&mut self) {
        if let Some(_index) = self.current {
            let count = self.current_count.max(1) as f64;
            self.min.push(if self.current_count == 0 {
                0.0
            } else {
                self.current_min
            });
            self.max.push(if self.current_count == 0 {
                0.0
            } else {
                self.current_max
            });
            self.rms.push((self.current_sum_sq / count).sqrt() as f32);
        }
        self.current = None;
    }

    fn finish(mut self) -> Peaks {
        self.finish_current();
        Peaks {
            bucket_ms: self.bucket_ms,
            min: self.min,
            max: self.max,
            rms: self.rms,
        }
    }
}

/// Computes the dBFS level of one rms value.
fn rms_to_db(rms: f32) -> f32 {
    if rms <= 0.0 {
        SILENCE_DB
    } else {
        (20.0 * rms.log10()).max(SILENCE_DB)
    }
}

/// Derives the vocal activity from the per-bucket rms levels: one window per
/// two buckets, the noise floor at the 10th percentile, active above the
/// floor plus 12 dB, and gaps shorter than 100 ms between active passages
/// closed.
fn compute_activity(rms: &[f32], bucket_ms: f64) -> Activity {
    let window_ms = 2.0 * bucket_ms;
    let mut window_db = Vec::with_capacity(rms.len().div_ceil(2));
    for window in rms.chunks(2) {
        let mean_sq = window
            .iter()
            .map(|value| f64::from(*value) * f64::from(*value))
            .sum::<f64>()
            / window.len() as f64;
        window_db.push(rms_to_db((mean_sq.sqrt()) as f32));
    }
    let mut sorted = window_db.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor_db = if sorted.is_empty() {
        SILENCE_DB
    } else {
        sorted[sorted.len() * 10 / 100]
    };
    let mut active: Vec<bool> = window_db
        .iter()
        .map(|db| *db > floor_db + ACTIVE_OVER_FLOOR_DB)
        .collect();
    // Close inactive runs shorter than 100 ms that sit between two active
    // passages; leading and trailing silence stays inactive.
    let gap_windows = (100.0 / window_ms).floor() as usize;
    let mut index = 0;
    while index < active.len() {
        if active[index] {
            index += 1;
            continue;
        }
        let start = index;
        while index < active.len() && !active[index] {
            index += 1;
        }
        let surrounded = start > 0 && index < active.len();
        let short = index - start < gap_windows.max(1);
        if surrounded && short {
            for flag in &mut active[start..index] {
                *flag = true;
            }
        }
    }
    Activity {
        bucket_ms: window_ms,
        active,
        floor_db,
    }
}

/// Sets the pipeline to NULL when the analysis ends, however it ends, so
/// GStreamer never disposes elements that are still playing.
struct StoppedPipeline(gst::Pipeline);

impl Drop for StoppedPipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

/// Analyzes an audio file into peaks and activity, using the cache when the
/// file version matches; `cancel` stops the analysis and `progress` reports
/// the fraction decoded (0.0 to 1.0).
pub fn analyze(
    path: &Path,
    bucket_ms: f64,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(Peaks, Activity), AudioError> {
    if !path.is_file() {
        return Err(AudioError::File(path.display().to_string()));
    }
    if let Some(cached) = cache::load(path, bucket_ms) {
        return Ok(cached);
    }
    gst::init()
        .map_err(|error| AudioError::Gst(format!("gstreamer could not initialize: {error}")))?;
    let uri = gst::glib::filename_to_uri(path, None).map_err(|error| {
        AudioError::Gst(format!("the audio path does not become a uri: {error}"))
    })?;
    let uridecodebin = gst::ElementFactory::make("uridecodebin")
        .property("uri", uri.as_str())
        .build()
        .map_err(|error| AudioError::Gst(format!("uridecodebin does not build: {error}")))?;
    let audioconvert = gst::ElementFactory::make("audioconvert")
        .build()
        .map_err(|error| AudioError::Gst(format!("audioconvert does not build: {error}")))?;
    let audioresample = gst::ElementFactory::make("audioresample")
        .build()
        .map_err(|error| AudioError::Gst(format!("audioresample does not build: {error}")))?;
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "F32LE")
        .field("channels", 1i32)
        .field("rate", ANALYSIS_RATE)
        .build();
    let appsink = gst_app::AppSink::builder().caps(&caps).sync(false).build();
    let pipeline = StoppedPipeline(gst::Pipeline::default());
    pipeline.0.add(&uridecodebin).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not assemble: {error}"))
    })?;
    pipeline.0.add(&audioconvert).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not assemble: {error}"))
    })?;
    pipeline.0.add(&audioresample).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not assemble: {error}"))
    })?;
    pipeline.0.add(&appsink).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not assemble: {error}"))
    })?;
    gst::Element::link(&audioconvert, &audioresample).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not link: {error}"))
    })?;
    gst::Element::link(&audioresample, appsink.upcast_ref::<gst::Element>()).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not link: {error}"))
    })?;
    // uridecodebin creates its source pad only when the file is opened, so
    // the decode side links to the fixed analysis chain then.
    let convert_for_link = audioconvert.clone();
    uridecodebin.connect_pad_added(move |_bin, pad| {
        let Some(sink_pad) = convert_for_link.static_pad("sink") else {
            return;
        };
        if sink_pad.is_linked() {
            return;
        }
        let _ = pad.link(&sink_pad);
    });
    pipeline.0.set_state(gst::State::Playing).map_err(|error| {
        AudioError::Gst(format!("the analysis pipeline does not start: {error}"))
    })?;
    let (state_result, _state, _pending) = pipeline.0.state(Some(gst::ClockTime::from_seconds(10)));
    if state_result.is_err() {
        return Err(AudioError::Gst(String::from(
            "the analysis pipeline does not reach PLAYING within 10 s",
        )));
    }
    // The duration powers the progress callback; the mp3 demuxer announces
    // it late, so poll briefly.
    let mut duration = None;
    for _ in 0..50 {
        duration = pipeline.0.query_duration::<gst::ClockTime>();
        if duration.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    let duration_ms = duration.map(|time| time.mseconds() as f64);
    let bus = pipeline
        .0
        .bus()
        .ok_or_else(|| AudioError::Gst(String::from("the analysis pipeline has no bus")))?;
    let mut fold = BucketFold::new(bucket_ms);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AudioError::Cancelled);
        }
        match appsink.try_pull_sample(Some(gst::ClockTime::from_mseconds(100))) {
            Some(sample) => {
                let buffer = sample.buffer().ok_or_else(|| {
                    AudioError::Gst(String::from(
                        "the analysis received a sample without a buffer",
                    ))
                })?;
                // The buffer's timestamp is the time of its first sample;
                // each further sample lies one sample period later, so a
                // buffer longer than a bucket spreads over its buckets.
                let start_ms = buffer
                    .pts()
                    .map(|time| time.nseconds() as f64 / 1_000_000.0);
                let map = buffer.map_readable().map_err(|error| {
                    AudioError::Gst(format!("the analysis cannot read a buffer: {error}"))
                })?;
                let sample_ms = 1000.0 / f64::from(ANALYSIS_RATE);
                for (index, chunk) in map.as_slice().as_chunks::<4>().0.iter().enumerate() {
                    let position_ms = start_ms.map(|start| start + index as f64 * sample_ms);
                    fold.push(f32::from_le_bytes(*chunk), position_ms);
                }
                if let Some(duration) = duration_ms
                    && let Some(time) = buffer.pts()
                {
                    progress((time.mseconds() as f64 / duration).clamp(0.0, 1.0) as f32);
                }
            }
            None => {
                if appsink.is_eos() {
                    break;
                }
                // A timeout without EOS: report a decode failure if the bus
                // has one, otherwise keep waiting.
                if let Some(message) = bus.timed_pop_filtered(
                    Some(gst::ClockTime::from_mseconds(0)),
                    &[gst::MessageType::Error],
                ) && let gst::MessageView::Error(error) = message.view()
                {
                    return Err(AudioError::Gst(format!(
                        "the analysis pipeline failed: {} ({})",
                        error.error(),
                        error.debug().unwrap_or_default()
                    )));
                }
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioError::Cancelled);
    }
    let peaks = fold.finish();
    let activity = compute_activity(&peaks.rms, bucket_ms);
    cache::store(path, bucket_ms, &peaks, &activity);
    Ok((peaks, activity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testmedia;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// Generates the tone in both formats and returns the paths with the
    /// generator functions; the caller gets the temporary directory so the
    /// files live as long as the test.
    fn tone_in_both_formats() -> Vec<(std::path::PathBuf, tempfile::TempDir)> {
        let mut files = Vec::new();
        for (generate, name) in [
            (
                testmedia::tone_opus as fn(&Path) -> Result<(), String>,
                "tone.opus",
            ),
            (testmedia::tone_mp3, "tone.mp3"),
        ] {
            let dir = tempfile::tempdir().expect("the temporary directory is created");
            let path = dir.path().join(name);
            generate(&path).expect("the test tone is generated");
            files.push((path, dir));
        }
        files
    }

    /// Plan test 1: the bucket count equals duration / bucket_ms plus or
    /// minus 1, for opus and mp3. The mp3 codec's encoder delay and padding
    /// make the playbin duration differ from the decoded stream by up to two
    /// buckets, so the mp3 tolerance is 2 (user decision 2026-11-13 gave the
    /// codec the 150 ms duration tolerance for the same reason).
    #[test]
    fn bucket_count_matches_duration() {
        for (path, tolerance, _dir) in tone_files_with_bucket_tolerance() {
            let duration = testmedia::probe_duration_ms(&path).expect("the duration is measured");
            let cancel = AtomicBool::new(false);
            let (peaks, activity) =
                analyze(&path, 10.0, &cancel, &|_| {}).expect("the analysis succeeds");
            let expected = (duration / peaks.bucket_ms).round();
            assert!(
                (peaks.min.len() as f64 - expected).abs() <= f64::from(tolerance),
                "the bucket count matches duration / bucket_ms plus or minus {tolerance}, expected {expected}, got {}",
                peaks.min.len()
            );
            assert_eq!(peaks.min.len(), peaks.max.len(), "min and max align");
            assert_eq!(peaks.min.len(), peaks.rms.len(), "min and rms align");
            // The activity windows cover the same duration with twice the
            // bucket length.
            assert_eq!(
                activity.active.len(),
                peaks.rms.len().div_ceil(2),
                "one activity window spans two peak buckets"
            );
            assert_eq!(activity.bucket_ms, 2.0 * peaks.bucket_ms);
        }
    }

    /// Every bucket inside the first tone (0.2 s to 1.8 s, away from the
    /// codec's start and the silence) carries signal: each sample lands in
    /// the bucket of its own time, not in the bucket of its buffer's start,
    /// so decoder buffers longer than a bucket leave no silent gaps.
    #[test]
    fn every_bucket_of_a_continuous_tone_carries_signal() {
        for (path, _dir) in tone_in_both_formats() {
            let cancel = AtomicBool::new(false);
            let (peaks, _) = analyze(&path, 10.0, &cancel, &|_| {}).expect("the analysis succeeds");
            let silent: Vec<usize> = (20..180)
                .filter(|&index| peaks.max[index] <= peaks.min[index])
                .collect();
            assert!(
                silent.is_empty(),
                "{}: {} of 160 tone buckets are silent, first {:?}",
                path.display(),
                silent.len(),
                silent.iter().take(5).collect::<Vec<_>>()
            );
        }
    }

    /// The generated tones with the bucket count tolerance their codec
    /// allows: opus is exact, mp3 drifts by up to two buckets.
    fn tone_files_with_bucket_tolerance() -> Vec<(std::path::PathBuf, u32, tempfile::TempDir)> {
        let mut files = Vec::new();
        for (generate, name, tolerance) in [
            (
                testmedia::tone_opus as fn(&Path) -> Result<(), String>,
                "tone.opus",
                1,
            ),
            (testmedia::tone_mp3, "tone.mp3", 2),
        ] {
            let dir = tempfile::tempdir().expect("the temporary directory is created");
            let path = dir.path().join(name);
            generate(&path).expect("the test tone is generated");
            files.push((path, tolerance, dir));
        }
        files
    }

    /// Plan test 2: the silent second is inactive and the tone is active,
    /// for opus and mp3 (the mp3 codec delays the content by about 25 ms,
    /// which the mid-second windows do not touch).
    #[test]
    fn silent_second_is_inactive_and_tone_is_active() {
        for (path, _dir) in tone_in_both_formats() {
            let cancel = AtomicBool::new(false);
            let (_, activity) =
                analyze(&path, 10.0, &cancel, &|_| {}).expect("the analysis succeeds");
            let silence = (2500.0 / activity.bucket_ms) as usize;
            assert!(
                !activity.active[silence],
                "the window at 2500 ms is inside the silent second"
            );
            let tone = (1000.0 / activity.bucket_ms) as usize;
            assert!(
                activity.active[tone],
                "the window at 1000 ms is inside the first tone"
            );
            assert!(
                activity.floor_db.is_finite(),
                "the noise floor is a finite dBFS value"
            );
        }
    }

    /// Plan test 3: cancel stops within 200 ms - once before the analysis
    /// starts, once while it is running on a long file.
    #[test]
    fn cancel_stops_analysis_within_200_ms() {
        // Pre-cancelled: the analysis must refuse to start.
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let short = dir.path().join("tone.opus");
        testmedia::tone_opus(&short).expect("the test tone is generated");
        let cancel = AtomicBool::new(true);
        let begin = Instant::now();
        let result = analyze(&short, 10.0, &cancel, &|_| {});
        assert!(
            begin.elapsed() <= Duration::from_millis(200),
            "a pre-cancelled analysis stops within 200 ms"
        );
        assert!(
            matches!(result, Err(AudioError::Cancelled)),
            "a pre-cancelled analysis reports cancelled, got {result:?}"
        );

        // Mid-flight: the analysis runs on a 180 s file, the cancel flag is
        // set while it decodes, and the result arrives within 200 ms as
        // cancelled. Without a cancel check the remaining decode takes
        // longer than that.
        let long = dir.path().join("long.opus");
        testmedia::tone_opus_repeated(&long, 36).expect("the long tone is generated");
        let cancel = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let thread_cancel = Arc::clone(&cancel);
        let thread_started = Arc::clone(&started);
        let thread_path = long.clone();
        std::thread::spawn(move || {
            let result = analyze(&thread_path, 10.0, &thread_cancel, &|_| {
                thread_started.store(true, Ordering::Relaxed);
            });
            let _ = sender.send(result);
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !started.load(Ordering::Relaxed) {
            assert!(
                Instant::now() < deadline,
                "the analysis does not start within 10 s"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(50));
        cancel.store(true, Ordering::Relaxed);
        let begin = Instant::now();
        let result = receiver
            .recv_timeout(Duration::from_millis(200))
            .expect("the analysis thread stops within 200 ms of the cancel");
        assert!(
            begin.elapsed() <= Duration::from_millis(200),
            "the cancelled result arrives within 200 ms"
        );
        assert!(
            matches!(result, Err(AudioError::Cancelled)),
            "a mid-flight cancel reports cancelled, got {result:?}"
        );
    }
}
