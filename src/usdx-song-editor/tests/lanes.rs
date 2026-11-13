//! About me: tests for the lane analysis state of the sheet background (task
//! P4-04, UI_SPEC 3.7). The state decides which audio files to analyse (the
//! waveform track and the #VOCALS track for the activity), tracks the
//! progress of the waveform analysis, takes results only from the jobs it
//! still wants, reuses finished results when the track switches back, and
//! reports the lane status the QML overlays show: no audio, loading or ready.
//! It is tested without GStreamer on hand-built peaks and activity.

use std::path::PathBuf;

use usdx_audio::analysis::{Activity, Peaks};
use usdx_song_editor::controller::lanes::{LaneAnalysis, LaneStatus};

fn mix() -> Option<PathBuf> {
    Some(PathBuf::from("/songs/a/song.opus"))
}

fn vocals() -> Option<PathBuf> {
    Some(PathBuf::from("/songs/a/vocals.opus"))
}

/// result builds an analysis result with one bucket of the given level.
fn result(level: f32) -> Result<(Peaks, Activity), String> {
    Ok((
        Peaks {
            bucket_ms: 10.0,
            min: vec![-level],
            max: vec![level],
            rms: vec![level],
        },
        Activity {
            bucket_ms: 20.0,
            active: vec![level > 0.5],
            floor_db: -60.0,
        },
    ))
}

#[test]
fn without_a_waveform_track_the_lane_has_no_audio() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(None, None);
    assert!(change.start.is_empty());
    assert_eq!(lanes.status(), LaneStatus::NoAudio);
    assert!(!lanes.has_vocals());
}

#[test]
fn mix_and_vocals_start_two_jobs_and_the_lane_loads() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), vocals());
    let paths: Vec<_> = change.start.iter().map(|job| job.path.clone()).collect();
    assert_eq!(paths, vec![mix().unwrap(), vocals().unwrap()]);
    assert_eq!(lanes.status(), LaneStatus::Loading);
    assert_eq!(lanes.progress(), 0.0);
    assert!(lanes.has_vocals());
}

#[test]
fn only_the_waveform_job_drives_the_progress() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), vocals());
    let (waveform, activity) = (change.start[0].id, change.start[1].id);
    lanes.job_progress(activity, 0.9);
    assert_eq!(lanes.progress(), 0.0);
    lanes.job_progress(waveform, 0.42);
    assert_eq!(lanes.progress(), 0.42);
}

#[test]
fn the_waveform_and_the_activity_arrive_from_their_own_jobs() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), vocals());
    let (waveform, activity) = (change.start[0].id, change.start[1].id);
    lanes.job_finished(waveform, result(0.3));
    assert_eq!(lanes.status(), LaneStatus::Ready);
    assert_eq!(lanes.peaks().map(|peaks| peaks.max[0]), Some(0.3));
    assert!(
        lanes.activity().is_none(),
        "the vocals job has not finished"
    );
    lanes.job_finished(activity, result(0.8));
    assert_eq!(
        lanes.activity().map(|activity| activity.active[0]),
        Some(true)
    );
    assert_eq!(
        lanes.peaks().map(|peaks| peaks.max[0]),
        Some(0.3),
        "the waveform stays the mix"
    );
}

#[test]
fn the_vocals_track_as_waveform_needs_one_job() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(vocals(), vocals());
    assert_eq!(change.start.len(), 1);
    lanes.job_finished(change.start[0].id, result(0.8));
    assert_eq!(lanes.status(), LaneStatus::Ready);
    assert!(lanes.peaks().is_some() && lanes.activity().is_some());
}

#[test]
fn a_track_change_cancels_the_jobs_it_no_longer_needs_and_ignores_their_results() {
    let mut lanes = LaneAnalysis::new();
    let first = lanes.set_tracks(mix(), vocals());
    let other = Some(PathBuf::from("/songs/b/song.opus"));
    let second = lanes.set_tracks(other.clone(), None);
    assert_eq!(second.cancel, vec![first.start[0].id, first.start[1].id]);
    assert_eq!(second.start.len(), 1);
    lanes.job_finished(first.start[0].id, result(0.3));
    assert_eq!(
        lanes.status(),
        LaneStatus::Loading,
        "the old song's result is ignored"
    );
    assert!(!lanes.has_vocals());
}

#[test]
fn switching_back_to_an_analysed_track_needs_no_job() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), vocals());
    lanes.job_finished(change.start[0].id, result(0.3));
    lanes.job_finished(change.start[1].id, result(0.8));
    let to_vocals = lanes.set_tracks(vocals(), vocals());
    assert!(to_vocals.start.is_empty());
    assert_eq!(lanes.peaks().map(|peaks| peaks.max[0]), Some(0.8));
    assert_eq!(lanes.status(), LaneStatus::Ready);
}

#[test]
fn unchanged_tracks_start_nothing() {
    let mut lanes = LaneAnalysis::new();
    lanes.set_tracks(mix(), vocals());
    let again = lanes.set_tracks(mix(), vocals());
    assert!(again.start.is_empty() && again.cancel.is_empty());
}

#[test]
fn a_failed_waveform_analysis_shows_no_audio_and_keeps_the_reason() {
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), None);
    lanes.job_finished(change.start[0].id, Err(String::from("not an audio file")));
    assert_eq!(lanes.status(), LaneStatus::NoAudio);
    assert_eq!(lanes.error(), Some("not an audio file"));
}

#[test]
fn a_held_loading_state_ignores_results_and_cancels_the_jobs() {
    // The screenshot harness pins the lane at a fixed progress (UI_SPEC 8.1).
    let mut lanes = LaneAnalysis::new();
    let change = lanes.set_tracks(mix(), None);
    let cancel = lanes.hold_loading(0.42);
    assert_eq!(cancel, vec![change.start[0].id]);
    lanes.job_finished(change.start[0].id, result(0.3));
    assert_eq!(lanes.status(), LaneStatus::Loading);
    assert_eq!(lanes.progress(), 0.42);
}
