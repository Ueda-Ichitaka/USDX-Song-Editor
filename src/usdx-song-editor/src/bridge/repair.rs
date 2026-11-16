//! The plain logic of the Repair and TapSession bridges (plan section
//! 8.3, task P7-08).
//!
//! About me: the QObject declarations live in the SongDocument bridge
//! module (same-bridge nested pointers, like the Player); this file
//! holds the pure logic they mirror, unit-testable without Qt (the
//! tests live in tests/repair_bridge.rs): the anchor card rows the
//! AnchorPanel shows, the anchors.txt file write behind
//! `exportAnchors`, the lyrics.txt lines the export dialog's
//! Factory-matching check needs, the click-track WAV writer the latency
//! calibration plays, and the current line's repair target (the voice
//! and voice-local line `pinCurrentLineAt` pins). The repair session
//! state itself lives in the plain-Rust `RepairController`
//! (controller/repair.rs); this file only converts and routes.

use std::path::Path;

use usdx_edit::anchors_export::{AnchorEntry, render};
use usdx_edit::retime::Anchor;
use usdx_format::rows::Document;
use usdx_format::song::Song;

use crate::controller::EditorController;

/// AnchorRow is one anchor card's data in the AnchorPanel (UI_SPEC 4.6):
/// the badge index (A1, A2, ...), the 1-based global line number, the
/// line text and the line's old and pinned start times in audio
/// milliseconds.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorRow {
    pub badge: usize,
    pub line_number: usize,
    pub text: String,
    pub old_ms: f64,
    pub target_ms: f64,
}

/// anchor_rows lists the anchor cards of the voice's anchors. An anchor
/// at a line the voice lost (a stale patch) is skipped; the voice-local
/// line indexes map to the global 1-based numbers the lines pane shows.
pub fn anchor_rows(doc: &Document, song: &Song, voice: u8, anchors: &[Anchor]) -> Vec<AnchorRow> {
    let voice_index = usize::from(voice);
    let Some(voice_data) = song.voices.get(voice_index) else {
        return Vec::new();
    };
    let offset: usize = song
        .voices
        .iter()
        .take(voice_index)
        .map(|voice| voice.lines.len())
        .sum();
    anchors
        .iter()
        .enumerate()
        .filter_map(|(badge, anchor)| {
            let line = voice_data.lines.get(anchor.line)?;
            let first = line.notes.first()?;
            let note = song.note(doc, *first)?;
            let old_ms = song
                .timing
                .map(|timing| timing.beat_to_ms(note.start))
                .unwrap_or(0.0);
            Some(AnchorRow {
                badge,
                line_number: offset + anchor.line + 1,
                text: line.text(doc),
                old_ms,
                target_ms: anchor.target_ms,
            })
        })
        .collect()
}

/// export_anchors renders the anchors.txt content and writes it to
/// `path`. An invalid anchor set (the render's rules) is refused and
/// leaves an existing file untouched.
pub fn export_anchors(
    path: &Path,
    title: &str,
    version: &str,
    date: &str,
    entries: &[AnchorEntry],
    end: Option<f64>,
) -> Result<(), String> {
    let file = render(title, version, date, entries, end).map_err(|error| format!("{error:?}"))?;
    std::fs::write(path, file).map_err(|error| error.to_string())
}

/// lyrics_lines reads the lyrics.txt lines next to the song, trimmed and
/// without empty lines (the Factory matching compares the sung lines).
/// `None` when the song folder has no lyrics.txt - the export dialog
/// then skips the Factory-matching check.
pub fn lyrics_lines(folder: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(folder.join("lyrics.txt")).ok()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The calibration click track: 8 clicks at 1 s spacing (plan task
/// P7-08; UI_SPEC 4.8's Calibrate dialog), 16-bit mono PCM with the
/// canonical 44-byte header, no new dependency.
const CALIBRATION_CLICKS: usize = 8;

/// click_track_wav writes the calibration click track: each click is a
/// 60 ms burst of 880 Hz with an 8 ms exponential decay, 0.5 amplitude.
pub fn click_track_wav(path: &Path, sample_rate: u32) -> std::io::Result<()> {
    let rate = sample_rate as usize;
    let mut samples = vec![0_i16; rate * CALIBRATION_CLICKS];
    let burst = rate / 16;
    for click in 0..CALIBRATION_CLICKS {
        for (index, sample) in samples[click * rate..click * rate + burst]
            .iter_mut()
            .enumerate()
        {
            let t = index as f64 / f64::from(sample_rate);
            let envelope = (-t / 0.008).exp();
            let value =
                (std::f64::consts::TAU * 880.0 * t).sin() * 0.5 * envelope * f64::from(i16::MAX);
            *sample = value as i16;
        }
    }
    write_wav(path, sample_rate, &samples)
}

/// write_wav writes mono 16-bit PCM samples as a canonical 44-byte
/// header WAV file.
fn write_wav(path: &Path, sample_rate: u32, samples: &[i16]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes)
}

/// current_repair_target maps the editor's current global line to the
/// (voice, voice-local line) the repair controller pins: the voice
/// index the current line belongs to and the line's index inside that
/// voice. `None` without a loaded song.
pub fn current_repair_target(controller: &EditorController) -> Option<(u8, usize)> {
    let voice = u8::try_from(controller.current_voice_index()?).ok()?;
    let loaded = controller.loaded()?;
    let offset: usize = loaded
        .song
        .voices
        .iter()
        .take(usize::from(voice))
        .map(|voice| voice.lines.len())
        .sum();
    let line = controller.current_line();
    (line >= offset).then_some((voice, line - offset))
}
