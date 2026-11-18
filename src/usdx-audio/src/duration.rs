//! The audio length probe (task P7-10).
//!
//! About me: `duration_ms` reads the length of an audio file without playing
//! it, through GStreamer's Discoverer. It exists so the editor knows where a
//! song's audio ends as soon as the song opens (user decision 2026-11-18),
//! before the player has loaded anything: the health analysis marks the
//! lines that start after that end.

use std::path::Path;

use gstreamer as gst;
use gstreamer_pbutils as pbutils;

use crate::player::AudioError;

/// How long the Discoverer may take for one file.
const PROBE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(5);

/// duration_ms returns the length of the audio file at `path` in
/// milliseconds. A missing file is `AudioError::File`; a file GStreamer
/// cannot read or that reports no length is `AudioError::Gst`.
pub fn duration_ms(path: &Path) -> Result<f64, AudioError> {
    let absolute = path
        .canonicalize()
        .map_err(|_| AudioError::File(path.display().to_string()))?;
    gst::init().map_err(|error| AudioError::Gst(error.to_string()))?;
    let uri = gst::glib::filename_to_uri(&absolute, None)
        .map_err(|error| AudioError::Gst(error.to_string()))?;
    let discoverer = pbutils::Discoverer::new(PROBE_TIMEOUT)
        .map_err(|error| AudioError::Gst(error.to_string()))?;
    let info = discoverer
        .discover_uri(uri.as_str())
        .map_err(|error| AudioError::Gst(error.to_string()))?;
    let duration = info
        .duration()
        .ok_or_else(|| AudioError::Gst(format!("{} reports no length", path.display())))?;
    if info.audio_streams().is_empty() {
        return Err(AudioError::Gst(format!(
            "{} holds no audio",
            path.display()
        )));
    }
    Ok(duration.nseconds() as f64 / 1_000_000.0)
}
