//! About me: the test media helper for the audio tests of every crate (plan
//! section 9.4, task P4-01). It generates the test audio the player and
//! analysis tests run on: a 5 s 440 Hz tone with a 1 s silence in the middle
//! (2 s tone, 1 s silence, 2 s tone), as Ogg/Opus and as MP3, into a
//! per-test temporary directory - no audio fixtures are stored in the
//! repository. The player bridge tests of the app crate use it through the
//! public module. The encoder pipelines were verified with gst-launch-1.0
//! first (plan section 9.4); they differ from the plan's one-liner because
//! audiotestsrc adapts to the negotiated rate, so each segment pins its
//! source rate to 44100 with a capsfilter (num-buffers x samplesperbuffer is
//! rate-sensitive) and the Opus branch resamples to the 48000 Hz Opus
//! requires.

use std::path::Path;

use gst::prelude::*;
use gstreamer as gst;

/// How long the tone and the silence last, in audiotestsrc segments: two
/// buffers of 44100 samples (1 s each at the pinned 44100 Hz source rate)
/// of 440 Hz tone, one buffer of silence, two more of tone.
const SEGMENTS: &[(bool, u32)] = &[(true, 2), (false, 1), (true, 2)];

/// Generates the tone-with-silence media at `path`, with `encoder_chain`
/// describing everything between the concat element and the filesink; the
/// 3 segment pattern (2 s tone, 1 s silence, 2 s tone) repeats `repeats`
/// times.
fn generate(path: &Path, encoder_chain: &str, repeats: usize) -> Result<(), String> {
    gst::init().map_err(|error| format!("gstreamer could not initialize: {error}"))?;
    // The verified pipeline shape: a rate-pinned source per segment into a
    // concat element, then the encoder chain to the filesink. The location
    // is quoted, so temp paths with spaces stay one value.
    let mut description = format!(
        "concat name=c ! {encoder_chain} location=\"{}\"",
        path.display()
    );
    for _ in 0..repeats {
        for (tone, buffers) in SEGMENTS {
            let wave = if *tone { "" } else { " wave=silence" };
            description.push_str(&format!(
                " audiotestsrc{wave} freq=440 samplesperbuffer=44100 num-buffers={buffers}"
            ));
            description.push_str(" ! audio/x-raw,rate=44100 ! c.");
        }
    }
    let pipeline = gst::parse::launch_full(&description, None, gst::ParseFlags::empty())
        .map_err(|error| format!("the media pipeline does not parse: {error}"))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| format!("the media pipeline does not start: {error}"))?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| String::from("the media pipeline has no bus"))?;
    let message = bus.timed_pop_filtered(
        Some(gst::ClockTime::from_seconds(30)),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    );
    pipeline
        .set_state(gst::State::Null)
        .map_err(|error| format!("the media pipeline does not stop: {error}"))?;
    match message {
        Some(message) => match message.view() {
            gst::MessageView::Error(error) => Err(format!(
                "the media pipeline failed: {} ({})",
                error.error(),
                error.debug().unwrap_or_default()
            )),
            _ => Ok(()),
        },
        None => Err(String::from(
            "the media pipeline did not reach EOS within 30 s",
        )),
    }
}

/// Generates the 5 s tone-with-silence media as Ogg/Opus at `path`.
pub fn tone_opus(path: &Path) -> Result<(), String> {
    tone_opus_repeated(path, 1)
}

/// Generates the tone-with-silence media as Ogg/Opus at `path`, `repeats`
/// times 5 s long; the cancel test uses a long file so the cancel arrives
/// while the analysis is still running.
pub fn tone_opus_repeated(path: &Path, repeats: usize) -> Result<(), String> {
    // audioresample to the 48000 Hz Opus requires; without it the encoder
    // relabels 44100 Hz samples as 48000 and the tone is pitch-shifted.
    generate(
        path,
        "audioconvert ! audioresample ! audio/x-raw,rate=48000 ! opusenc ! oggmux ! filesink",
        repeats,
    )
}

/// Generates the 5 s tone-with-silence media as MP3 at `path`.
pub fn tone_mp3(path: &Path) -> Result<(), String> {
    generate(path, "audioconvert ! lamemp3enc ! id3v2mux ! filesink", 1)
}

/// Reports the duration of an audio file in milliseconds, measured through
/// playbin3 (the same decoder chain the player uses) at the PLAYING state;
/// at PAUSED the MP3 decoder reports no duration yet.
pub fn probe_duration_ms(path: &Path) -> Result<f64, String> {
    gst::init().map_err(|error| format!("gstreamer could not initialize: {error}"))?;
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|error| format!("playbin3 does not build: {error}"))?;
    let uri = gst::glib::filename_to_uri(path, None)
        .map_err(|error| format!("the media path does not become a uri: {error}"))?;
    playbin.set_property("uri", uri.as_str());
    playbin
        .set_state(gst::State::Playing)
        .map_err(|error| format!("the probe pipeline does not start: {error}"))?;
    let (result, state, _) = playbin.state(Some(gst::ClockTime::from_seconds(10)));
    // The duration is not ready the instant PLAYING is reached: the MP3
    // demuxer reports it a moment later, so poll briefly before giving up.
    let mut duration = None;
    for _ in 0..50 {
        duration = playbin.query_duration::<gst::ClockTime>();
        if duration.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    playbin
        .set_state(gst::State::Null)
        .map_err(|error| format!("the probe pipeline does not stop: {error}"))?;
    if result.is_err() || state != gst::State::Playing {
        return Err(String::from(
            "the probe pipeline did not reach PLAYING within 10 s",
        ));
    }
    let milliseconds = duration
        .ok_or_else(|| String::from("the probe pipeline reports no duration"))?
        .mseconds();
    Ok(f64::from(u32::try_from(milliseconds).map_err(|error| {
        format!("the probe pipeline reports an implausible duration: {error}")
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Opus file lasts 5 s within 50 ms (plan task P4-01); measured on
    /// this machine: 4.9934 s, the Opus pre-skip accounts for the rest.
    #[test]
    fn the_generated_opus_exists_and_last_five_seconds() {
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path = dir.path().join("tone.opus");
        tone_opus(&path).expect("the opus tone is generated");
        assert!(path.exists(), "the generated opus file exists");
        let duration = probe_duration_ms(&path).expect("the opus duration is measured");
        assert!(
            (duration - 5000.0).abs() <= 50.0,
            "the opus tone lasts 5 s within 50 ms, measured {duration} ms"
        );
    }

    /// The MP3 file lasts 5 s within 150 ms (user decision 2026-11-13: the
    /// LAME encoder delay shifts the content by about 25 ms and pads the
    /// stream, so the file duration measures 5.066-5.120 s; the content
    /// itself is exactly 5 s). Measured on this machine: 5.1008 s.
    #[test]
    fn the_generated_mp3_exists_and_last_five_seconds() {
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path = dir.path().join("tone.mp3");
        tone_mp3(&path).expect("the mp3 tone is generated");
        assert!(path.exists(), "the generated mp3 file exists");
        let duration = probe_duration_ms(&path).expect("the mp3 duration is measured");
        assert!(
            (duration - 5000.0).abs() <= 150.0,
            "the mp3 tone lasts 5 s within 150 ms, measured {duration} ms"
        );
    }
}
