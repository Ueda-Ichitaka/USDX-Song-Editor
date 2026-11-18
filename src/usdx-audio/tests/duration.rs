//! About me: tests for the audio length probe (task P7-10, user decision
//! 2026-11-18: the editor reads the #AUDIO length when a song opens, so the
//! health analysis can mark lines that start after the audio end). The
//! probe reads the length without playing: the generated 5 s tones measure
//! 5 s (Opus within 50 ms, MP3 within 150 ms for the LAME padding, the
//! same bounds as the test media helper's own tests), and a missing or
//! non-audio file is an error.

use usdx_audio::{AudioError, duration_ms, testmedia};

#[test]
fn the_opus_tone_measures_five_seconds() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("tone.opus");
    testmedia::tone_opus(&path).expect("the opus tone is generated");
    let duration = duration_ms(&path).expect("the length is read");
    assert!((duration - 5000.0).abs() <= 50.0, "measured {duration} ms");
}

#[test]
fn the_mp3_tone_measures_five_seconds() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("tone.mp3");
    testmedia::tone_mp3(&path).expect("the mp3 tone is generated");
    let duration = duration_ms(&path).expect("the length is read");
    assert!((duration - 5000.0).abs() <= 150.0, "measured {duration} ms");
}

#[test]
fn a_missing_file_is_a_file_error() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let result = duration_ms(&dir.path().join("missing.opus"));
    assert!(matches!(result, Err(AudioError::File(_))), "{result:?}");
}

#[test]
fn a_file_without_audio_is_an_error() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("notes.opus");
    std::fs::write(&path, "this is no audio").expect("the file is written");
    assert!(duration_ms(&path).is_err());
}
