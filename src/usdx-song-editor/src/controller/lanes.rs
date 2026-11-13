//! About me: the analysis state behind the waveform and activity lanes of the
//! sheet background (UI_SPEC 3.7, task P4-04). It decides which audio files
//! need an analysis (the selected track for the waveform, the #VOCALS track
//! for the vocal activity, one job when both are the same file), keeps the
//! progress of the waveform job, accepts results only from jobs it still
//! wants, keeps the finished results of the current song so switching tracks
//! back needs no new analysis, and reports the lane status the QML overlays
//! show. It holds no Qt or GStreamer types, so the bridge's worker threads
//! are the only untested part; the bridge runs the jobs it hands out.

use std::collections::HashMap;
use std::path::PathBuf;

use usdx_audio::analysis::{Activity, Peaks};

/// What the waveform lane shows (UI_SPEC 3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneStatus {
    /// No waveform track, or its analysis failed: "No audio".
    NoAudio,
    /// The waveform track is being analysed: "Reading the waveform… N %".
    Loading,
    /// The waveform is available.
    Ready,
}

/// An analysis the bridge shall run on a worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisJob {
    pub id: u64,
    pub path: PathBuf,
}

/// The jobs a track change starts and the running jobs it no longer needs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TrackChange {
    pub start: Vec<AnalysisJob>,
    pub cancel: Vec<u64>,
}

/// The analysis state of the lanes for the current song.
#[derive(Debug, Default)]
pub struct LaneAnalysis {
    waveform: Option<PathBuf>,
    vocals: Option<PathBuf>,
    results: HashMap<PathBuf, (Peaks, Activity)>,
    failures: HashMap<PathBuf, String>,
    /// The running jobs in start order, with their last reported progress.
    running: Vec<(AnalysisJob, f32)>,
    next_id: u64,
    /// The progress the screenshot harness pins the lane at, if any.
    held: Option<f32>,
}

impl LaneAnalysis {
    /// new builds the state of a song without audio.
    pub fn new() -> LaneAnalysis {
        LaneAnalysis::default()
    }

    /// set_tracks names the waveform track and the vocals track; it returns
    /// the analyses to start and the running ones to cancel. Results and
    /// failures of files no longer needed are dropped.
    pub fn set_tracks(
        &mut self,
        waveform: Option<PathBuf>,
        vocals: Option<PathBuf>,
    ) -> TrackChange {
        if self.waveform == waveform && self.vocals == vocals {
            return TrackChange::default();
        }
        self.waveform = waveform;
        self.vocals = vocals;
        let needed = self.needed();
        let mut change = TrackChange::default();
        self.running.retain(|(job, _)| {
            let keep = needed.contains(&job.path);
            if !keep {
                change.cancel.push(job.id);
            }
            keep
        });
        self.results.retain(|path, _| needed.contains(path));
        self.failures.retain(|path, _| needed.contains(path));
        for path in needed {
            let known = self.results.contains_key(&path)
                || self.failures.contains_key(&path)
                || self.running.iter().any(|(job, _)| job.path == path);
            if !known {
                let job = AnalysisJob {
                    id: self.next_id,
                    path,
                };
                self.next_id += 1;
                self.running.push((job.clone(), 0.0));
                change.start.push(job);
            }
        }
        change
    }

    /// job_progress records the fraction a running job decoded.
    pub fn job_progress(&mut self, id: u64, fraction: f32) {
        if let Some((_, progress)) = self.running.iter_mut().find(|(job, _)| job.id == id) {
            *progress = fraction;
        }
    }

    /// job_finished takes the result of a running job; results of jobs that
    /// were cancelled, or arrive while the lane is held, are ignored.
    pub fn job_finished(&mut self, id: u64, result: Result<(Peaks, Activity), String>) {
        let Some(index) = self.running.iter().position(|(job, _)| job.id == id) else {
            return;
        };
        let (job, _) = self.running.remove(index);
        if self.held.is_some() {
            return;
        }
        match result {
            Ok(data) => {
                self.results.insert(job.path, data);
            }
            Err(reason) => {
                self.failures.insert(job.path, reason);
            }
        }
    }

    /// hold_loading pins the lane in the loading state at the fraction for
    /// the screenshot harness (UI_SPEC 8.1) and returns the running jobs to
    /// cancel.
    pub fn hold_loading(&mut self, fraction: f32) -> Vec<u64> {
        self.held = Some(fraction);
        self.running.drain(..).map(|(job, _)| job.id).collect()
    }

    /// status tells what the waveform lane shows.
    pub fn status(&self) -> LaneStatus {
        if self.held.is_some() {
            return LaneStatus::Loading;
        }
        match &self.waveform {
            None => LaneStatus::NoAudio,
            Some(path) if self.failures.contains_key(path) => LaneStatus::NoAudio,
            Some(path) if self.results.contains_key(path) => LaneStatus::Ready,
            Some(_) => LaneStatus::Loading,
        }
    }

    /// progress returns the decoded fraction of the waveform analysis.
    pub fn progress(&self) -> f32 {
        if let Some(fraction) = self.held {
            return fraction;
        }
        self.running
            .iter()
            .find(|(job, _)| Some(&job.path) == self.waveform.as_ref())
            .map(|(_, progress)| *progress)
            .unwrap_or(0.0)
    }

    /// error returns why the waveform analysis failed, if it did.
    pub fn error(&self) -> Option<&str> {
        self.waveform
            .as_ref()
            .and_then(|path| self.failures.get(path))
            .map(String::as_str)
    }

    /// peaks returns the waveform of the waveform track once analysed.
    pub fn peaks(&self) -> Option<&Peaks> {
        if self.held.is_some() {
            return None;
        }
        let path = self.waveform.as_ref()?;
        self.results.get(path).map(|(peaks, _)| peaks)
    }

    /// activity returns the vocal activity of the vocals track once analysed.
    pub fn activity(&self) -> Option<&Activity> {
        if self.held.is_some() {
            return None;
        }
        let path = self.vocals.as_ref()?;
        self.results.get(path).map(|(_, activity)| activity)
    }

    /// has_vocals tells whether the song has a vocals track for the activity.
    pub fn has_vocals(&self) -> bool {
        self.vocals.is_some()
    }

    /// needed lists the distinct files the current tracks need, waveform first.
    fn needed(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in [&self.waveform, &self.vocals].into_iter().flatten() {
            if !paths.contains(path) {
                paths.push(path.clone());
            }
        }
        paths
    }
}
