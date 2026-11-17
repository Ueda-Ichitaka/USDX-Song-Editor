//! The plain logic of the Repair and TapSession bridges (plan section
//! 8.3, task P7-08).
//!
//! About me: the QObject declarations live in the SongDocument bridge
//! module (same-bridge nested pointers, like the Player); this file
//! holds the pure logic they mirror, unit-testable without Qt (the
//! tests live in tests/repair_bridge.rs), and the QObject impls: the
//! anchor card rows the AnchorPanel shows, the anchors.txt file write
//! behind `exportAnchors`, the export dialog's preview payload
//! (rendered text, changed and pinned counts, Factory-matching
//! problems, excluded lines), the ghost note positions of the preview
//! patch, the lyrics.txt lines the Factory-matching check needs, the
//! click-track WAV writer the latency calibration plays, the UTC date
//! of the export header comment, and the global-to-voice line mappings
//! the current line pin, the block and the tap range need. The repair
//! session state itself lives in the plain-Rust `RepairController`
//! (controller/repair.rs); the QObject impls route and mirror it into
//! the properties.

use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QHash, QHashPair_QString_QVariant, QList, QString, QUrl, QVariant};

use std::collections::BTreeSet;
use usdx_edit::Patch;
use usdx_edit::anchors_export::{AnchorEntry, ExportProblem, export_problems, render};
use usdx_edit::block::Follow;
use usdx_edit::health::{Health, HealthConfig};
use usdx_edit::patch::RowOp;
use usdx_edit::retime::{Anchor, Lengths};
use usdx_format::rows::RowId;
use usdx_format::rows::{Document, RowKind};
use usdx_format::song::Song;

use crate::VERSION;
use crate::bridge::song_document::qobject;
use crate::bridge::song_document::{QVariantList, SongDocumentRust, qurl_to_path};
use crate::controller::EditorController;
use crate::controller::editor::LoadedSong;
use crate::controller::repair::{RepairController, RepairMode, flat_p1_line_index};
use crate::controller::settings::APP_ID;
use crate::controller::shell;
use crate::controller::tap::{TapMode, TapSession, TapState};

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

/// default_anchors_path is the export dialog's starting target: anchors.txt
/// in the song's folder (plan 6.11, UI_SPEC 5.5).
pub fn default_anchors_path(song: &Path) -> std::path::PathBuf {
    song.with_file_name("anchors.txt")
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

/// line_range_target maps a global line range (the lines pane's
/// numbering, the tap page's range) to (voice, first, last), the
/// voice-local inclusive range the tap session runs over. `None` when
/// the range is empty, crosses a voice border or there is no song.
pub fn line_range_target(
    controller: &EditorController,
    first: usize,
    last: usize,
) -> Option<(u8, usize, usize)> {
    let loaded = controller.loaded()?;
    let mut offset = 0;
    for (index, voice) in loaded.song.voices.iter().enumerate() {
        let count = voice.lines.len();
        if (offset..offset + count).contains(&first) && (offset..offset + count).contains(&last) {
            return Some((u8::try_from(index).ok()?, first - offset, last - offset));
        }
        offset += count;
    }
    None
}

/// health_i32 maps a health verdict onto the QML convention the lines
/// pane and the header pill already use (0 ok, 1 suspicious, 2 racing).
fn health_i32(health: Health) -> i32 {
    match health {
        Health::Ok => 0,
        Health::Suspicious => 1,
        Health::Racing => 2,
    }
}

/// line_span_ms reads one voice line's note span in audio milliseconds,
/// the lines pane's rule: the first note's start to the phrase end or
/// the last note's end.
fn line_span_ms(doc: &Document, song: &Song, voice: u8, line: usize) -> (f64, f64) {
    let to_ms = |beat: f64| {
        song.timing
            .map(|timing| timing.beat_to_ms(beat))
            .unwrap_or(beat)
    };
    let Some(voice_data) = song.voices.get(usize::from(voice)) else {
        return (0.0, 0.0);
    };
    let Some(line_data) = voice_data.lines.get(line) else {
        return (0.0, 0.0);
    };
    let first = line_data.notes.first().and_then(|id| song.note(doc, *id));
    let last_end = line_data
        .notes
        .iter()
        .rev()
        .find_map(|id| song.note(doc, *id))
        .map(|note| note.start + note.length as f64);
    let end_beat = line_data.end.and_then(|id| song.phrase_end_beat(doc, id));
    let start_ms = first.map(|note| to_ms(note.start)).unwrap_or(0.0);
    let end_ms = end_beat.or(last_end).map(to_ms).unwrap_or(start_ms);
    (start_ms, end_ms)
}

/// BlockSpan is the block's state the UI shows: the span from the first
/// note's start to the last note's end in audio milliseconds and the
/// note count (the block bar's "Lines 15 - 22 - 57 notes", the
/// selection band's times and the Scale factor's base).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockSpan {
    pub start_ms: f64,
    pub end_ms: f64,
    pub notes: usize,
}

/// block_span reports the note span and count of the voice's lines
/// `first..=last` (the block transform's anchors are the block's first
/// note start and last note end, plan 6.8); None without lines in the
/// range or a voice the song does not have.
pub fn block_span(
    doc: &Document,
    song: &Song,
    voice: u8,
    first: usize,
    last: usize,
) -> Option<BlockSpan> {
    let voice_data = song.voices.get(usize::from(voice))?;
    let to_ms = |beat: f64| {
        song.timing
            .map(|timing| timing.beat_to_ms(beat))
            .unwrap_or(beat)
    };
    let mut span: Option<(f64, f64, usize)> = None;
    for line in voice_data.lines.get(first..=last)?.iter() {
        for row_id in &line.notes {
            let note = song.note(doc, *row_id)?;
            let (start, end, notes) = span.unwrap_or((f64::INFINITY, f64::NEG_INFINITY, 0));
            span = Some((
                start.min(to_ms(note.start)),
                end.max(to_ms(note.start + note.length as f64)),
                notes + 1,
            ));
        }
    }
    span.map(|(start, end, notes)| BlockSpan {
        start_ms: start,
        end_ms: end,
        notes,
    })
}

/// selection_lines maps the editor's selected notes onto the inclusive
/// global line range their lines form (the block mode's seed, user
/// decision 2026-11-17); None for an empty selection or one spanning
/// more than one voice (the block needs one voice).
pub fn selection_lines(song: &Song, selection: &BTreeSet<RowId>) -> Option<(usize, usize)> {
    if selection.is_empty() {
        return None;
    }
    let mut voice_of: Option<u8> = None;
    let mut range: Option<(usize, usize)> = None;
    for (voice, voice_data) in song.voices.iter().enumerate() {
        let offset: usize = song
            .voices
            .iter()
            .take(voice)
            .map(|voice| voice.lines.len())
            .sum();
        for (line, line_data) in voice_data.lines.iter().enumerate() {
            if line_data.notes.iter().any(|id| selection.contains(id)) {
                let voice = voice as u8;
                match voice_of {
                    Some(seen) if seen != voice => return None,
                    None => voice_of = Some(voice),
                    _ => {}
                }
                range = Some(match range {
                    Some((first, last)) => (first.min(offset + line), last.max(offset + line)),
                    None => (offset + line, offset + line),
                });
            }
        }
    }
    range
}

/// follow_of maps the FollowKind qenum's i32 (the enum's declaration
/// order, the UI_SPEC's button order) onto the planner's Follow.
pub fn follow_of(follow: i32) -> Option<Follow> {
    match follow {
        0 => Some(Follow::Stay),
        1 => Some(Follow::MoveAlong),
        _ => None,
    }
}

/// follow_i32 maps the planner's Follow onto the qenum's i32.
pub fn follow_i32(follow: Follow) -> i32 {
    match follow {
        Follow::Stay => 0,
        Follow::MoveAlong => 1,
    }
}

/// block_state reports the block mode's state for the QML properties:
/// the block's global line numbers (-1 and -1 without a block), its old
/// note span (-1 without a span), the note count, the scale factor the
/// new times give against the span (0.0 unknown), the new times (-1
/// without a block) and the follow toggle's i32.
pub fn block_state(
    doc: &Document,
    song: &Song,
    repair: &RepairController,
) -> (i32, i32, f64, f64, i32, f64, f64, f64, i32) {
    let Some(range) = repair.block() else {
        return (
            -1,
            -1,
            -1.0,
            -1.0,
            0,
            0.0,
            -1.0,
            -1.0,
            follow_i32(repair.follow()),
        );
    };
    let voice = repair.block_voice().unwrap_or(0);
    let offset: usize = song
        .voices
        .iter()
        .take(usize::from(voice))
        .map(|voice| voice.lines.len())
        .sum();
    let span = block_span(doc, song, voice, range.start, range.end.saturating_sub(1));
    let (start_ms, end_ms, notes) = match span {
        Some(span) => (span.start_ms, span.end_ms, span.notes as i32),
        None => (-1.0, -1.0, 0),
    };
    let (new_start, new_end) = repair.block_times();
    let (new_start, new_end) = match span {
        Some(_) => (new_start, new_end),
        None => (-1.0, -1.0),
    };
    let scale = match span {
        Some(span) if span.end_ms > span.start_ms && new_end > new_start => {
            (new_end - new_start) / (span.end_ms - span.start_ms)
        }
        _ => 0.0,
    };
    (
        (offset + range.start) as i32,
        (offset + range.end.saturating_sub(1)) as i32,
        start_ms,
        end_ms,
        notes,
        scale,
        new_start,
        new_end,
        follow_i32(repair.follow()),
    )
}

/// OverviewLine is one line block of the overview strip (UI_SPEC 4.6):
/// the line's voice-local index, its note span in audio milliseconds and
/// its health verdict (0 ok, 1 suspicious, 2 racing, the QML convention).
#[derive(Clone, Debug, PartialEq)]
pub struct OverviewLine {
    pub line: usize,
    pub start_ms: f64,
    pub end_ms: f64,
    pub health: i32,
}

/// overview_lines analyzes the voice's lines for the overview strip and
/// the racing warning. The analysis runs without the vocal activity for
/// now, so the coverage signal stays neutral and the rate and the
/// neighbour factor decide.
/// HealthBasis holds what the repair page's health analysis shares with the
/// lines pane (task P7-09): the settings' thresholds and the #VOCALS track's
/// vocal activity from the analysis cache, when there is one.
pub struct HealthBasis<'a> {
    pub cfg: HealthConfig,
    pub activity: Option<&'a usdx_audio::Activity>,
}

impl HealthBasis<'static> {
    /// plain is the analysis with the default thresholds and without the
    /// vocal activity.
    pub fn plain() -> HealthBasis<'static> {
        HealthBasis {
            cfg: HealthConfig::default(),
            activity: None,
        }
    }
}

pub fn overview_lines(
    doc: &Document,
    song: &Song,
    voice: u8,
    basis: &HealthBasis,
) -> Vec<OverviewLine> {
    let healths = usdx_edit::health::analyze(doc, song, voice, basis.activity, &basis.cfg);
    healths
        .iter()
        .map(|report| {
            let (start_ms, end_ms) = line_span_ms(doc, song, voice, report.line);
            OverviewLine {
                line: report.line,
                start_ms,
                end_ms,
                health: health_i32(report.health),
            }
        })
        .collect()
}

/// section_end_ms reports the section's end at its last voice-local
/// line: the old end as the lines pane measures it, and, with a preview
/// patch, the end the preview gives the same line (the status bar's
/// "becomes" time). Without a patch both ends are the old one.
pub fn section_end_ms(
    doc: &Document,
    song: &Song,
    voice: u8,
    line: usize,
    patch: Option<&Patch>,
) -> (f64, f64) {
    let old_end = line_span_ms(doc, song, voice, line).1;
    let new_end = patch.and_then(|patch| {
        let mut clone = doc.clone();
        patch.apply(&mut clone).ok()?;
        let patched = Song::from_document(&clone);
        Some(line_span_ms(&clone, &patched, voice, line).1)
    });
    (old_end, new_end.unwrap_or(old_end))
}

/// racing_ranges reports the racing sections as inclusive voice-local
/// line ranges, the strip's underlay and the warning's data.
pub fn racing_ranges(
    doc: &Document,
    song: &Song,
    voice: u8,
    basis: &HealthBasis,
) -> Vec<(usize, usize)> {
    let healths = usdx_edit::health::analyze(doc, song, voice, basis.activity, &basis.cfg);
    usdx_edit::health::racing_sections(&healths)
        .into_iter()
        .map(|range| (range.start, range.end - 1))
        .collect()
}

/// TapLine is one line of a tap-along session for the page: its global
/// line number (0-based), its syllables (the note texts without their
/// surrounding spaces), whether each syllable ends a word (its text ends
/// with a space, the next starts with one, or it is the line's last) and
/// its notes' start and end in audio milliseconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TapLine {
    pub line: usize,
    pub syllables: Vec<String>,
    pub word_ends: Vec<bool>,
    pub starts_ms: Vec<f64>,
    pub ends_ms: Vec<f64>,
}

/// tap_lines lists the global lines `first..=last` (the lines pane's
/// numbering across the voices) for the tap-along page (UI_SPEC 4.8): the
/// current line's syllables, the next line and the lane's notes. Lines
/// past the song are left out; a song without a usable tempo gives none.
pub fn tap_lines(doc: &Document, song: &Song, first: usize, last: usize) -> Vec<TapLine> {
    let Some(timing) = song.timing else {
        return Vec::new();
    };
    let all_lines: Vec<&usdx_format::song::Line> = song
        .voices
        .iter()
        .flat_map(|voice| voice.lines.iter())
        .collect();
    (first..=last)
        .filter_map(|global| {
            let line = all_lines.get(global)?;
            let mut tap_line = TapLine {
                line: global,
                ..TapLine::default()
            };
            let notes: Vec<_> = line
                .notes
                .iter()
                .filter_map(|id| song.note(doc, *id))
                .collect();
            for (index, note) in notes.iter().enumerate() {
                tap_line.syllables.push(note.text.trim().to_owned());
                let next_starts_word = notes
                    .get(index + 1)
                    .is_none_or(|next| next.text.starts_with(char::is_whitespace));
                tap_line
                    .word_ends
                    .push(note.text.ends_with(char::is_whitespace) || next_starts_word);
                tap_line.starts_ms.push(timing.beat_to_ms(note.start));
                tap_line
                    .ends_ms
                    .push(timing.beat_to_ms(note.start + note.length as f64));
            }
            Some(tap_line)
        })
        .collect()
}

/// tap_line_list writes the tap lines as the TapSession's lines property:
/// one hash per line with line, syllables, wordEnds, starts and ends.
fn tap_line_list(lines: &[TapLine]) -> QList<QVariant> {
    let mut list = QList::<QVariant>::default();
    for line in lines {
        let mut hash = QHash::<QHashPair_QString_QVariant>::default();
        hash.insert(QString::from("line"), QVariant::from(&(line.line as i32)));
        let mut syllables = QList::<QVariant>::default();
        for syllable in &line.syllables {
            syllables.append(QVariant::from(&QString::from(syllable.as_str())));
        }
        hash.insert(QString::from("syllables"), QVariant::from(&syllables));
        let mut word_ends = QList::<QVariant>::default();
        for word_end in &line.word_ends {
            word_ends.append(QVariant::from(word_end));
        }
        hash.insert(QString::from("wordEnds"), QVariant::from(&word_ends));
        let mut starts = QList::<QVariant>::default();
        for start in &line.starts_ms {
            starts.append(QVariant::from(start));
        }
        hash.insert(QString::from("starts"), QVariant::from(&starts));
        let mut ends = QList::<QVariant>::default();
        for end in &line.ends_ms {
            ends.append(QVariant::from(end));
        }
        hash.insert(QString::from("ends"), QVariant::from(&ends));
        list.append(QVariant::from(&hash));
    }
    list
}

/// racing_note_rows lists the row ids of the notes in the voice's racing
/// lines (the lines of every racing section), which the repair sheet draws
/// in the negative colour (UI_SPEC 4.6); empty for an unknown voice.
pub fn racing_note_rows(doc: &Document, song: &Song, voice: u8, basis: &HealthBasis) -> Vec<i64> {
    let Some(lines) = song
        .voices
        .get(usize::from(voice))
        .map(|voice| &voice.lines)
    else {
        return Vec::new();
    };
    racing_ranges(doc, song, voice, basis)
        .into_iter()
        .flat_map(|(first, last)| lines[first..=last].iter())
        .flat_map(|line| line.notes.iter())
        .map(|id| i64::from(id.0))
        .collect()
}

/// health_basis builds the document's health basis: the settings'
/// thresholds and the cached #VOCALS activity, as the lines pane uses them.
fn health_basis(rust: &SongDocumentRust) -> HealthBasis<'_> {
    HealthBasis {
        cfg: crate::controller::line_health::health_config(&rust.editor_settings().health),
        activity: rust.vocal_activity.as_ref(),
    }
}

/// SectionSpan is the repair target's section: the racing section around
/// the target line in global line numbers (0-based; -1 and -1 when the
/// line is calm or unknown), the target line's verdict (0 ok, 1
/// suspicious, 2 racing, -1 unknown), its neighbour factor (0.0 unknown)
/// and its vocal coverage (none while the analysis runs without the
/// activity).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionSpan {
    pub first: i32,
    pub last: i32,
    pub health: i32,
    pub factor: f64,
    pub coverage: Option<f64>,
}

/// repair_section analyzes the target line's voice once and reports the
/// racing section around it (the sheet column header, the warning
/// InlineMessage and the status bar).
pub fn repair_section(
    doc: &Document,
    song: &Song,
    voice: u8,
    line: usize,
    basis: &HealthBasis,
) -> SectionSpan {
    let none = SectionSpan {
        first: -1,
        last: -1,
        health: -1,
        factor: 0.0,
        coverage: None,
    };
    let reports = usdx_edit::health::analyze(doc, song, voice, basis.activity, &basis.cfg);
    let Some(report) = reports.iter().find(|report| report.line == line) else {
        return none;
    };
    let section = usdx_edit::health::racing_sections(&reports)
        .into_iter()
        .find(|range| range.contains(&line));
    let offset: usize = song
        .voices
        .iter()
        .take(usize::from(voice))
        .map(|voice| voice.lines.len())
        .sum();
    let (first, last) = section
        .map(|range| {
            (
                i32::try_from(offset + range.start).unwrap_or(-1),
                i32::try_from(offset + range.end - 1).unwrap_or(-1),
            )
        })
        .unwrap_or((-1, -1));
    SectionSpan {
        first,
        last,
        health: health_i32(report.health),
        factor: report.neighbour_factor,
        coverage: report.vocal_coverage,
    }
}

/// overview_anchor_marks flattens the anchor cards into the (pinned
/// time, badge) pairs the overview strip's flags draw.
pub fn overview_anchor_marks(rows: &[AnchorRow]) -> Vec<(f64, usize)> {
    rows.iter().map(|row| (row.target_ms, row.badge)).collect()
}

/// PreviewNote is one ghost note's data: the row id (the NotesModel and
/// sheet delegates key the rows by it) and the note's new start and end
/// in audio milliseconds.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewNote {
    pub row_id: i64,
    pub start_ms: f64,
    pub end_ms: f64,
    /// The note's pitch (MIDI, C4 = 60); the patch moves notes in time,
    /// so the ghost keeps the pitch.
    pub pitch: i32,
}

/// preview_note_hash writes one ghost for the repair sheet's previewNotes
/// list: rowId, start and end in audio milliseconds, and the pitch that
/// places it on its pitch row.
pub fn preview_note_hash(note: &PreviewNote) -> QHash<QHashPair_QString_QVariant> {
    let mut ghost = QHash::<QHashPair_QString_QVariant>::default();
    ghost.insert(QString::from("rowId"), QVariant::from(&note.row_id));
    ghost.insert(QString::from("start"), QVariant::from(&note.start_ms));
    ghost.insert(QString::from("end"), QVariant::from(&note.end_ms));
    ghost.insert(QString::from("pitch"), QVariant::from(&note.pitch));
    ghost
}

/// preview_notes lists the ghost positions of a preview patch: the
/// patch applies to a clone of the document (plan section 6.2: previews
/// are computed, not committed), so the times include a #GAP change,
/// and every note row the patch replaces with a moved or resized note
/// lands in the list with its new start and end. An empty patch or an
/// unfitting patch (the preview was built against another state) lists
/// nothing.
pub fn preview_notes(doc: &Document, patch: &Patch) -> Vec<PreviewNote> {
    let mut clone = doc.clone();
    if patch.apply(&mut clone).is_err() {
        return Vec::new();
    }
    let Some(timing) = Song::from_document(&clone).timing else {
        return Vec::new();
    };
    patch
        .ops
        .iter()
        .filter_map(|op| match op {
            RowOp::Replace { id, before, after } => {
                let (RowKind::Note(old), RowKind::Note(new)) = (&before.kind, &after.kind) else {
                    return None;
                };
                if old.start == new.start && old.length == new.length {
                    return None;
                }
                Some(PreviewNote {
                    row_id: id.0 as i64,
                    start_ms: timing.beat_to_ms(new.start as f64),
                    end_ms: timing.beat_to_ms((new.start + new.length) as f64),
                    pitch: new.pitch as i32 + crate::controller::notes::PITCH_C4_MIDI,
                })
            }
            RowOp::Insert { .. } | RowOp::Remove { .. } => None,
        })
        .collect()
}

/// shortened_notes counts the patch's replaces that shorten a note: the
/// collisions pill reports "n notes shortened" when the rounding pass
/// had to shrink notes so nothing overlaps.
pub fn shortened_notes(patch: &Patch) -> usize {
    patch
        .ops
        .iter()
        .filter(|op| match op {
            RowOp::Replace { before, after, .. } => {
                let (RowKind::Note(old), RowKind::Note(new)) = (&before.kind, &after.kind) else {
                    return false;
                };
                new.length < old.length
            }
            RowOp::Insert { .. } | RowOp::Remove { .. } => false,
        })
        .count()
}

/// utc_date formats the UTC calendar date of a Unix timestamp as
/// YYYY-MM-DD (Howard Hinnant's civil_from_days), the export header
/// comment's date; no calendar dependency.
pub fn utc_date(unix_secs: u64) -> String {
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// today_date returns the UTC date of now.
pub fn today_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    utc_date(secs)
}

/// ExportPreview is the export dialog's payload (UI_SPEC 5.5): the
/// exact file text, the entries (per entry the source line the "Leave
/// it out" action excludes by), the count of entries that come from a
/// pinned anchor, the end pin and the Factory-matching problems.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPreview {
    pub text: String,
    pub entries: Vec<AnchorEntry>,
    pub pinned: usize,
    pub end: Option<f64>,
    /// The per-line check findings; "Leave it out" drops their line.
    pub problems: Vec<ExportProblem>,
    /// A render rule violation (for example an end pin below the last
    /// anchor), which no line exclusion solves.
    pub error: Option<String>,
}

/// song_p1_lines lists the text of every voice-P1 line in flat order,
/// the song lines the export problems compare against.
fn song_p1_lines(doc: &Document, song: &Song) -> Vec<String> {
    song.voices
        .iter()
        .filter(|voice| voice.number == 1)
        .flat_map(|voice| voice.lines.iter().map(|line| line.text(doc)))
        .collect()
}

/// export_preview builds the export dialog's payload: the export
/// entries (a pin wins over a change) without the excluded source
/// lines, the rendered file text, the pinned count and the
/// Factory-matching problems with their lines. A render rule violation
/// (for example an end pin below the last anchor) leaves the text empty
/// and comes as the error, apart from the problems.
pub fn export_preview(
    repair: &RepairController,
    loaded: &LoadedSong,
    title: &str,
    version: &str,
    date: &str,
    lyrics: Option<&[String]>,
    excluded: &[usize],
) -> ExportPreview {
    let (entries, end) = repair.export_entries(loaded);
    let entries: Vec<AnchorEntry> = entries
        .into_iter()
        .filter(|entry| !excluded.contains(&entry.source_line))
        .collect();
    let pin_lines: HashSet<usize> = repair
        .anchors()
        .iter()
        .filter_map(|anchor| flat_p1_line_index(&loaded.song, repair.voice(), anchor.line))
        .collect();
    let pinned = entries
        .iter()
        .filter(|entry| pin_lines.contains(&entry.source_line))
        .count();
    let (text, problems, error) = match render(title, version, date, &entries, end) {
        Ok(text) => (
            text,
            export_problems(
                &entries,
                &song_p1_lines(&loaded.document, &loaded.song),
                lyrics,
            ),
            None,
        ),
        Err(error) => (
            String::new(),
            Vec::new(),
            Some(format!("cannot export: {error:?}")),
        ),
    };
    ExportPreview {
        text,
        entries,
        pinned,
        end,
        problems,
        error,
    }
}

/// repair_mode_of maps the RepairModeKind qenum's i32 (the enum's
/// declaration order) onto the controller's mode.
fn repair_mode_of(mode: i32) -> Option<RepairMode> {
    match mode {
        0 => Some(RepairMode::Anchor),
        1 => Some(RepairMode::Block),
        2 => Some(RepairMode::Tap),
        _ => None,
    }
}

/// repair_mode_i32 maps the controller's mode onto the qenum's i32.
fn repair_mode_i32(mode: RepairMode) -> i32 {
    match mode {
        RepairMode::Anchor => 0,
        RepairMode::Block => 1,
        RepairMode::Tap => 2,
    }
}

/// tap_mode_of maps the TapModeKind qenum's i32 onto the controller's
/// tap mode.
fn tap_mode_of(mode: i32) -> Option<TapMode> {
    match mode {
        0 => Some(TapMode::Line),
        1 => Some(TapMode::Syllable),
        _ => None,
    }
}

/// tap_mode_i32 maps the controller's tap mode onto the qenum's i32.
fn tap_mode_i32(mode: TapMode) -> i32 {
    match mode {
        TapMode::Line => 0,
        TapMode::Syllable => 1,
    }
}

/// tap_state_i32 maps the controller's tap state onto the
/// TapStateKind qenum's i32.
fn tap_state_i32(state: TapState) -> i32 {
    match state {
        TapState::Ready => 0,
        TapState::PreRoll => 1,
        TapState::Recording => 2,
        TapState::Finished => 3,
    }
}

/// TapStartError is why a tap session could not start: the planner's
/// error or a range that crosses a voice border.
pub(crate) enum TapStartError {
    Edit(usdx_edit::EditError),
    RangeCrossesVoices,
}

/// apply_repair commits the document's repair controller's current mode
/// patch as one history step and refreshes the document; a refused
/// patch reports through the document's `editRefused`. Used by the
/// Repair's apply and the TapSession's useTaps.
fn apply_repair(mut document: Pin<&mut qobject::SongDocument>) -> bool {
    match document.as_mut().rust_mut().get_mut().apply_repair_patch() {
        Some(Ok(())) => {
            document.as_mut().refresh();
            true
        }
        Some(Err(error)) => {
            let message = QString::from(shell::edit_failure_reason(&error));
            document.as_mut().edit_refused(message);
            false
        }
        None => false,
    }
}

impl qobject::Repair {
    /// song_document reads the document pointer QML set once on
    /// startup.
    fn song_document(&self) -> Option<&qobject::SongDocument> {
        // SAFETY: the pointer comes from the document property QML set
        // and the document outlives the repair.
        unsafe { self.document().as_ref() }
    }

    /// pinned_document returns the mutable document behind the pointer.
    fn pinned_document(self: Pin<&mut Self>) -> Option<Pin<&mut qobject::SongDocument>> {
        let pointer = *self.as_ref().document();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the document outlives the repair and Qt runs the
        // invokables on one thread, so the pointer is not aliased.
        Some(unsafe { Pin::new_unchecked(&mut *pointer) })
    }

    /// document_rust returns the mutable document state behind the
    /// pointer; the 'static lifetime is the raw pointer's, not self's
    /// borrow, so the caller can touch its own properties freely.
    fn document_rust(self: Pin<&mut Self>) -> Option<&'static mut SongDocumentRust> {
        let pointer = *self.as_ref().document();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the document outlives the repair and Qt runs the
        // invokables on one thread, so the pointer is not aliased.
        let document = unsafe { Pin::new_unchecked(&mut *pointer) };
        Some(document.rust_mut().get_mut())
    }

    /// sync_racing_rows mirrors the row ids of the notes in the current
    /// voice's racing lines into the racingRows property.
    fn sync_racing_rows(mut self: Pin<&mut Self>) {
        let rows = match self.song_document() {
            Some(document) => {
                let rust = document.rust();
                match (rust.editor().loaded(), current_repair_target(rust.editor())) {
                    (Some(loaded), Some((voice, _))) => {
                        racing_note_rows(&loaded.document, &loaded.song, voice, &health_basis(rust))
                    }
                    _ => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        let mut list = QVariantList::default();
        for row in rows {
            list.append(QVariant::from(&row));
        }
        self.as_mut().set_racing_rows(list);
    }

    /// sync mirrors the repair controller into the properties.
    fn sync(mut self: Pin<&mut Self>) {
        let (
            mode,
            anchors,
            end_pin_ms,
            has_preview,
            ghosts,
            overview,
            racing,
            anchor_marks,
            section,
            section_factor,
            section_coverage,
            section_start,
            section_end,
            section_new_end,
            shortened,
            note_lengths,
            block,
        ) = match self.song_document() {
            Some(document) => {
                let rust = document.rust();
                let repair = &rust.repair;
                let mut anchor_list = QVariantList::default();
                let mut anchor_marks = QVariantList::default();
                if let Some(loaded) = rust.editor().loaded() {
                    let rows = anchor_rows(
                        &loaded.document,
                        &loaded.song,
                        repair.voice(),
                        repair.anchors(),
                    );
                    for row in &rows {
                        let mut card = QHash::<QHashPair_QString_QVariant>::default();
                        card.insert(QString::from("badge"), QVariant::from(&(row.badge as i32)));
                        card.insert(
                            QString::from("line"),
                            QVariant::from(&(row.line_number as i32)),
                        );
                        card.insert(
                            QString::from("text"),
                            QVariant::from(&QString::from(row.text.as_str())),
                        );
                        card.insert(QString::from("old"), QVariant::from(&row.old_ms));
                        card.insert(QString::from("target"), QVariant::from(&row.target_ms));
                        anchor_list.append(QVariant::from(&card));
                    }
                    // The overview strip's anchor flags (timeMs, badge).
                    for (time_ms, badge) in overview_anchor_marks(&rows) {
                        let mut mark = QHash::<QHashPair_QString_QVariant>::default();
                        mark.insert(QString::from("timeMs"), QVariant::from(&time_ms));
                        mark.insert(QString::from("badge"), QVariant::from(&(badge as i32)));
                        anchor_marks.append(QVariant::from(&mark));
                    }
                }
                let mut ghost_list = QVariantList::default();
                if let (Some(loaded), Some(patch)) =
                    (rust.editor().loaded(), repair.preview_patch())
                {
                    for note in preview_notes(&loaded.document, patch) {
                        ghost_list.append(QVariant::from(&preview_note_hash(&note)));
                    }
                }
                // The overview strip's data: the current line's voice's
                // lines with their health verdicts and the racing
                // sections as inclusive ranges.
                let mut overview_list = QVariantList::default();
                let mut racing_list = QVariantList::default();
                if let Some(loaded) = rust.editor().loaded()
                    && let Some((voice, _)) = current_repair_target(rust.editor())
                {
                    // The voice's first line's global index: the sum of
                    // the preceding voices' line counts (the strip's
                    // click maps a block to this global line).
                    let offset: usize = loaded
                        .song
                        .voices
                        .iter()
                        .take(usize::from(voice))
                        .map(|voice| voice.lines.len())
                        .sum();
                    let basis = health_basis(rust);
                    for line in overview_lines(&loaded.document, &loaded.song, voice, &basis) {
                        let mut row = QHash::<QHashPair_QString_QVariant>::default();
                        row.insert(
                            QString::from("line"),
                            QVariant::from(&((offset + line.line) as i32)),
                        );
                        row.insert(QString::from("startMs"), QVariant::from(&line.start_ms));
                        row.insert(QString::from("endMs"), QVariant::from(&line.end_ms));
                        row.insert(QString::from("health"), QVariant::from(&line.health));
                        overview_list.append(QVariant::from(&row));
                    }
                    for (first, last) in
                        racing_ranges(&loaded.document, &loaded.song, voice, &basis)
                    {
                        let mut row = QHash::<QHashPair_QString_QVariant>::default();
                        row.insert(QString::from("first"), QVariant::from(&(first as i32)));
                        row.insert(QString::from("last"), QVariant::from(&(last as i32)));
                        racing_list.append(QVariant::from(&row));
                    }
                }
                // The target's section data (the header, the warning and
                // the status bar): the section's start and old end at its
                // first and last voice-local lines, and the end the
                // pending preview gives the last line.
                let (
                    section,
                    section_factor,
                    section_coverage,
                    section_start,
                    section_end,
                    section_new_end,
                ) = match current_repair_target(rust.editor()) {
                    Some((voice, line)) if rust.editor().loaded().is_some() => {
                        let loaded = rust.editor().loaded().expect("checked above");
                        let section = repair_section(
                            &loaded.document,
                            &loaded.song,
                            voice,
                            line,
                            &health_basis(rust),
                        );
                        let offset: usize = loaded
                            .song
                            .voices
                            .iter()
                            .take(usize::from(voice))
                            .map(|voice| voice.lines.len())
                            .sum();
                        let (start, end, new_end) = if section.first >= 0 {
                            let first_local = section.first as usize - offset;
                            let last_local = section.last as usize - offset;
                            let (end, new_end) = section_end_ms(
                                &loaded.document,
                                &loaded.song,
                                voice,
                                last_local,
                                repair.preview_patch(),
                            );
                            (
                                line_span_ms(&loaded.document, &loaded.song, voice, first_local).0,
                                end,
                                new_end,
                            )
                        } else {
                            (-1.0, -1.0, -1.0)
                        };
                        (
                            section.clone(),
                            section.factor,
                            section.coverage.unwrap_or(-1.0),
                            start,
                            end,
                            new_end,
                        )
                    }
                    _ => (SectionSpan::default(), 0.0, -1.0, -1.0, -1.0, -1.0),
                };
                (
                    repair_mode_i32(repair.mode()),
                    anchor_list,
                    repair.end_pin().unwrap_or(-1.0),
                    repair.has_preview(),
                    ghost_list,
                    overview_list,
                    racing_list,
                    anchor_marks,
                    section,
                    section_factor,
                    section_coverage,
                    section_start,
                    section_end,
                    section_new_end,
                    match repair.preview_patch() {
                        Some(patch) => shortened_notes(patch) as i32,
                        None => 0,
                    },
                    match repair.note_lengths() {
                        Lengths::Scale => 0,
                        Lengths::Keep => 1,
                    },
                    match rust.editor().loaded() {
                        Some(loaded) => block_state(&loaded.document, &loaded.song, repair),
                        None => (
                            -1,
                            -1,
                            -1.0,
                            -1.0,
                            0,
                            0.0,
                            -1.0,
                            -1.0,
                            follow_i32(repair.follow()),
                        ),
                    },
                )
            }
            None => (
                0,
                QVariantList::default(),
                -1.0,
                false,
                QVariantList::default(),
                QVariantList::default(),
                QVariantList::default(),
                QVariantList::default(),
                SectionSpan::default(),
                0.0,
                -1.0,
                -1.0,
                -1.0,
                -1.0,
                0,
                1,
                (-1, -1, -1.0, -1.0, 0, 0.0, -1.0, -1.0, 1),
            ),
        };
        if self.as_ref().mode != mode {
            self.as_mut().rust_mut().mode = mode;
            self.as_mut().mode_changed();
        }
        self.as_mut().set_anchors(anchors);
        self.as_mut().set_end_pin_ms(end_pin_ms);
        self.as_mut().set_has_preview(has_preview);
        self.as_mut().set_preview_notes(ghosts);
        self.as_mut().set_overview_lines(overview);
        self.as_mut().set_racing_sections(racing);
        self.as_mut().sync_racing_rows();
        self.as_mut().set_overview_anchors(anchor_marks);
        self.as_mut().set_section_first(section.first);
        self.as_mut().set_section_last(section.last);
        self.as_mut().set_section_health(section.health);
        self.as_mut().set_section_factor(section_factor);
        self.as_mut().set_section_coverage(section_coverage);
        self.as_mut().set_section_start_ms(section_start);
        self.as_mut().set_section_end_ms(section_end);
        self.as_mut().set_section_new_end_ms(section_new_end);
        self.as_mut().set_shortened_notes(shortened);
        // The WRITE setter of note_lengths is the setNoteLengths
        // invokable, which refreshes the preview; the mirror must not
        // recurse, so the field is written directly (the mode's
        // pattern) and the change notified.
        if self.as_ref().note_lengths != note_lengths {
            self.as_mut().rust_mut().note_lengths = note_lengths;
            self.as_mut().note_lengths_changed();
        }
        self.as_mut().set_block_first(block.0);
        self.as_mut().set_block_last(block.1);
        self.as_mut().set_block_start_ms(block.2);
        self.as_mut().set_block_end_ms(block.3);
        self.as_mut().set_block_notes(block.4);
        self.as_mut().set_block_scale(block.5);
        self.as_mut().set_block_new_start_ms(block.6);
        self.as_mut().set_block_new_end_ms(block.7);
        // The WRITE setter of follow is the setFollow invokable, which
        // refreshes the preview; the mirror writes the field directly.
        if self.as_ref().follow != block.8 {
            self.as_mut().rust_mut().follow = block.8;
            self.as_mut().follow_changed();
        }
    }

    /// refresh_preview recomputes the pending preview after a change:
    /// with the preview on, the current mode's patch is computed (the
    /// ghost notes follow in sync); without it, the pending preview
    /// clears. A refused computation clears the preview and reports the
    /// reason through the previewError property.
    fn refresh_preview(mut self: Pin<&mut Self>) {
        let Some(rust) = self.as_mut().document_rust() else {
            self.as_mut().set_preview_error(QString::default());
            self.as_mut().sync();
            return;
        };
        if !self.as_ref().preview_enabled() {
            rust.repair.cancel();
            self.as_mut().set_preview_error(QString::default());
            self.as_mut().sync();
            return;
        }
        let result = rust.refresh_repair_preview().map(|_| ());
        match result {
            Ok(()) => self.as_mut().set_preview_error(QString::default()),
            Err(error) => self
                .as_mut()
                .set_preview_error(QString::from(shell::edit_failure_reason(&error))),
        }
        self.as_mut().sync();
    }

    /// pin_current_line_at pins the editor's current line at `ms`.
    pub fn pin_current_line_at(mut self: Pin<&mut Self>, ms: f64) {
        let Some(rust) = self.as_mut().document_rust() else {
            return;
        };
        let Some((_, line)) = current_repair_target(rust.editor()) else {
            return;
        };
        rust.repair.pin_current_line_at(line, ms);
        self.as_mut().refresh_preview();
    }

    /// set_mode switches the repair mode; doubles as the WRITE setter
    /// of the mode property.
    pub fn set_mode(mut self: Pin<&mut Self>, mode: i32) {
        let Some(mode) = repair_mode_of(mode) else {
            return;
        };
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.set_mode(mode);
            // Entering the block mode without a block seeds it from the
            // editor's note selection (user decision 2026-11-17); a
            // block chosen earlier keeps.
            if mode == RepairMode::Block
                && rust.repair.block().is_none()
                && let Some(loaded) = rust.editor().loaded()
                && let Some((first, last)) =
                    selection_lines(&loaded.song, rust.editor().selection())
            {
                self.as_mut().set_block(first as i32, last as i32);
            }
            self.as_mut().refresh_preview();
        }
    }

    /// set_anchor_ms changes the pinned time of the anchor `index`.
    pub fn set_anchor_ms(mut self: Pin<&mut Self>, index: i32, ms: f64) {
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.set_anchor_ms(index as usize, ms);
            self.as_mut().refresh_preview();
        }
    }

    /// remove_anchor removes the anchor `index`.
    pub fn remove_anchor(mut self: Pin<&mut Self>, index: i32) {
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.remove_anchor(index as usize);
            self.as_mut().refresh_preview();
        }
    }

    /// set_note_lengths switches the panel's Note lengths toggle
    /// (0 Scale, 1 Keep; other values are a silent no-op) and clears
    /// the pending preview.
    pub fn set_note_lengths(mut self: Pin<&mut Self>, lengths: i32) {
        let lengths = match lengths {
            0 => Lengths::Scale,
            1 => Lengths::Keep,
            _ => return,
        };
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.set_note_lengths(lengths);
            self.as_mut().refresh_preview();
        }
    }

    /// set_end_pin pins the song end at `ms`; a negative value clears
    /// the end pin.
    pub fn set_end_pin(mut self: Pin<&mut Self>, ms: f64) {
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.set_end_pin((ms >= 0.0).then_some(ms));
            self.as_mut().refresh_preview();
        }
    }

    /// set_block selects the block mode's lines (the lines pane's
    /// global numbering); both must belong to one voice. The block's
    /// patch applies against the current line's voice at apply time.
    /// The new times start at the block's old note span, so a fresh
    /// block previews as identity until an edge is dragged.
    pub fn set_block(mut self: Pin<&mut Self>, first: i32, last: i32) {
        let Some(rust) = self.as_mut().document_rust() else {
            return;
        };
        let Some((voice, first, last)) =
            line_range_target(rust.editor(), first.max(0) as usize, last.max(0) as usize)
        else {
            return;
        };
        rust.repair.set_mode(RepairMode::Block);
        rust.repair.set_block(first, last, voice);
        if let Some(loaded) = rust.editor().loaded() {
            let span = block_span(&loaded.document, &loaded.song, voice, first, last);
            if let Some(span) = span {
                rust.repair.set_block_times(span.start_ms, span.end_ms);
            }
        }
        self.as_mut().refresh_preview();
    }

    /// set_block_times sets the block's new start and end.
    pub fn set_block_times(mut self: Pin<&mut Self>, start_ms: f64, end_ms: f64) {
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.set_block_times(start_ms, end_ms);
            self.as_mut().refresh_preview();
        }
    }

    /// set_follow switches what the lines after the block do (the
    /// block bar's "Following lines" toggle); other values are a
    /// silent no-op.
    pub fn set_follow(mut self: Pin<&mut Self>, follow: i32) {
        let Some(follow) = follow_of(follow) else {
            return;
        };
        let Some(rust) = self.as_mut().document_rust() else {
            return;
        };
        rust.repair.set_follow(follow);
        self.as_mut().refresh_preview();
    }

    /// apply commits the current mode's patch as one history step.
    pub fn apply(mut self: Pin<&mut Self>) -> bool {
        let applied = match self.as_mut().pinned_document() {
            Some(document) => apply_repair(document),
            None => false,
        };
        // The page follows the applied song: the preview recomputes (the
        // anchors and the block now sit where they pin, so nothing is
        // pending) and the cards, ghosts and section mirror the result.
        if applied {
            // In the block mode the block's times start over at its new
            // span (setBlock re-reads it), so the next drag starts there.
            let (mode, first, last) = (
                *self.as_ref().mode(),
                *self.as_ref().block_first(),
                *self.as_ref().block_last(),
            );
            if mode == 1 && first >= 0 && last >= first {
                self.as_mut().set_block(first, last);
            } else {
                self.as_mut().refresh_preview();
            }
        }
        applied
    }

    /// refresh re-reads the document: the preview recomputes against the
    /// document as it stands (an edit made in the editor meanwhile would
    /// leave an old preview stale) and the properties mirror the result.
    pub fn refresh(self: Pin<&mut Self>) {
        self.refresh_preview();
    }

    /// cancel clears the pending preview; the document stays unchanged.
    pub fn cancel(mut self: Pin<&mut Self>) {
        if let Some(rust) = self.as_mut().document_rust() {
            rust.repair.cancel();
            self.as_mut().set_preview_error(QString::default());
            self.as_mut().sync();
        }
    }

    /// export_preview_json builds the export dialog's payload.
    pub fn export_preview_json(&self, excluded: QList<i32>) -> QHash<QHashPair_QString_QVariant> {
        let mut hash = QHash::<QHashPair_QString_QVariant>::default();
        let Some(document) = self.song_document() else {
            return hash;
        };
        let rust = document.rust();
        let Some(loaded) = rust.editor().loaded() else {
            return hash;
        };
        let excluded: Vec<usize> = excluded
            .iter()
            .filter_map(|index| usize::try_from(*index).ok())
            .collect();
        let lyrics = lyrics_lines(loaded.path.parent().unwrap_or_else(|| Path::new(".")));
        let preview = export_preview(
            &rust.repair,
            loaded,
            &rust.title().to_string(),
            VERSION,
            &today_date(),
            lyrics.as_deref(),
            &excluded,
        );
        hash.insert(
            QString::from("text"),
            QVariant::from(&QString::from(preview.text.as_str())),
        );
        hash.insert(
            QString::from("lineCount"),
            QVariant::from(&(preview.entries.len() as i32)),
        );
        hash.insert(
            QString::from("changedCount"),
            QVariant::from(&((preview.entries.len() - preview.pinned) as i32)),
        );
        hash.insert(
            QString::from("pinnedCount"),
            QVariant::from(&(preview.pinned as i32)),
        );
        hash.insert(
            QString::from("hasEnd"),
            QVariant::from(&preview.end.is_some()),
        );
        hash.insert(
            QString::from("endMs"),
            QVariant::from(&preview.end.unwrap_or(-1.0)),
        );
        let mut entry_list = QVariantList::default();
        for entry in &preview.entries {
            let mut row = QHash::<QHashPair_QString_QVariant>::default();
            row.insert(QString::from("timeMs"), QVariant::from(&entry.time_ms));
            row.insert(
                QString::from("text"),
                QVariant::from(&QString::from(entry.text.as_str())),
            );
            row.insert(
                QString::from("sourceLine"),
                QVariant::from(&(entry.source_line as i32)),
            );
            entry_list.append(QVariant::from(&row));
        }
        hash.insert(QString::from("entries"), QVariant::from(&entry_list));
        let mut problem_list = QVariantList::default();
        for problem in &preview.problems {
            let mut row = QHash::<QHashPair_QString_QVariant>::default();
            row.insert(
                QString::from("sourceLine"),
                QVariant::from(&(problem.source_line as i32)),
            );
            row.insert(
                QString::from("message"),
                QVariant::from(&QString::from(problem.message.as_str())),
            );
            problem_list.append(QVariant::from(&row));
        }
        hash.insert(QString::from("problems"), QVariant::from(&problem_list));
        hash.insert(
            QString::from("error"),
            QVariant::from(&QString::from(preview.error.as_deref().unwrap_or_default())),
        );
        hash
    }

    /// default_export_url returns anchors.txt in the song's folder as a
    /// file url, empty without a song.
    pub fn default_export_url(&self) -> QUrl {
        let Some(loaded) = self
            .song_document()
            .and_then(|document| document.rust().editor().loaded())
        else {
            return QUrl::default();
        };
        QUrl::from_local_file(&QString::from(
            default_anchors_path(&loaded.path)
                .to_string_lossy()
                .as_ref(),
        ))
    }

    /// file_exists tells whether a file url names an existing file (the
    /// export dialog asks before replacing it).
    pub fn file_exists(&self, url: QUrl) -> bool {
        qurl_to_path(&url).is_file()
    }

    /// export_anchors writes anchors.txt next to the song (with the
    /// same "Leave it out" exclusions the preview showed).
    pub fn export_anchors(&self, url: QUrl, excluded: QList<i32>) -> bool {
        let Some(document) = self.song_document() else {
            return false;
        };
        let rust = document.rust();
        let Some(loaded) = rust.editor().loaded() else {
            return false;
        };
        let excluded: Vec<usize> = excluded
            .iter()
            .filter_map(|index| usize::try_from(*index).ok())
            .collect();
        let preview = export_preview(
            &rust.repair,
            loaded,
            &rust.title().to_string(),
            VERSION,
            &today_date(),
            None,
            &excluded,
        );
        export_anchors(
            &qurl_to_path(&url),
            &rust.title().to_string(),
            VERSION,
            &today_date(),
            &preview.entries,
            preview.end,
        )
        .is_ok()
    }
}

impl qobject::TapSession {
    /// song_document reads the document pointer QML set once on
    /// startup.
    fn song_document(&self) -> Option<&qobject::SongDocument> {
        // SAFETY: the pointer comes from the document property QML set
        // and the document outlives the session.
        unsafe { self.document().as_ref() }
    }

    /// pinned_document returns the mutable document behind the pointer.
    fn pinned_document(self: Pin<&mut Self>) -> Option<Pin<&mut qobject::SongDocument>> {
        let pointer = *self.as_ref().document();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the document outlives the session and Qt runs the
        // invokables on one thread, so the pointer is not aliased.
        Some(unsafe { Pin::new_unchecked(&mut *pointer) })
    }

    /// pinned_player returns the mutable player behind the pointer.
    fn pinned_player(self: Pin<&mut Self>) -> Option<Pin<&mut qobject::Player>> {
        let pointer = *self.as_ref().player();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the player outlives the session (both are window
        // properties) and Qt runs the invokables on one thread, so the
        // pointer is not aliased.
        Some(unsafe { Pin::new_unchecked(&mut *pointer) })
    }

    /// document_rust returns the mutable document state behind the
    /// pointer; the 'static lifetime is the raw pointer's, not self's
    /// borrow, so the caller can touch its own properties freely.
    fn document_rust(self: Pin<&mut Self>) -> Option<&'static mut SongDocumentRust> {
        let pointer = *self.as_ref().document();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the document outlives the session and Qt runs the
        // invokables on one thread, so the pointer is not aliased.
        let document = unsafe { Pin::new_unchecked(&mut *pointer) };
        Some(document.rust_mut().get_mut())
    }

    /// session_mut returns the repair controller's tap session for the
    /// key and playback handling.
    fn session_mut(mut self: Pin<&mut Self>) -> Option<&'static mut TapSession> {
        self.as_mut().document_rust()?.repair.tap_session_mut()
    }

    /// session_of reads the repair controller's tap session.
    fn session_of(&self) -> Option<&TapSession> {
        self.song_document()?.rust().repair.tap_session()
    }

    /// sync mirrors the repair controller's tap session into the
    /// properties.
    fn sync(mut self: Pin<&mut Self>) {
        let (active, mode, state, tap_count, expected_count, current_syllable, pre_roll, taps) =
            match self.session_of() {
                Some(session) => {
                    let mut tap_list = QVariantList::default();
                    for tap in session.taps() {
                        tap_list.append(QVariant::from(&tap.down_ms));
                    }
                    (
                        true,
                        tap_mode_i32(session.mode()),
                        tap_state_i32(session.state()),
                        session.tap_count() as i32,
                        session.expected_count() as i32,
                        session.current_syllable() as i32,
                        session.pre_roll_start().unwrap_or(-1.0),
                        tap_list,
                    )
                }
                None => (false, 0, 0, 0, 0, 0, -1.0, QVariantList::default()),
            };
        let latency_ms = match self.song_document() {
            Some(document) => f64::from(document.rust().editor_settings().latency_ms),
            None => 0.0,
        };
        self.as_mut().set_active(active);
        self.as_mut().set_mode(mode);
        self.as_mut().set_state(state);
        self.as_mut().set_tap_count(tap_count);
        self.as_mut().set_expected_count(expected_count);
        self.as_mut().set_current_syllable(current_syllable);
        self.as_mut().set_latency_ms(latency_ms);
        self.as_mut().set_pre_roll_start_ms(pre_roll);
        self.as_mut().set_taps(taps);
    }

    /// start_session opens a session over the global lines first..=last.
    pub fn start_session(
        mut self: Pin<&mut Self>,
        first: i32,
        last: i32,
        mode: i32,
        rate: f64,
    ) -> bool {
        let Some(mut document) = self.as_mut().pinned_document() else {
            return false;
        };
        let Some(mode) = tap_mode_of(mode) else {
            return false;
        };
        let result = document.as_mut().rust_mut().get_mut().start_tap_session(
            first.max(0) as usize,
            last.max(0) as usize,
            mode,
            rate,
        );
        match result {
            Some(Ok(())) => {
                let lines = match document.rust().editor().loaded() {
                    Some(loaded) => tap_lines(
                        &loaded.document,
                        &loaded.song,
                        first.max(0) as usize,
                        last.max(0) as usize,
                    ),
                    None => Vec::new(),
                };
                let list = tap_line_list(&lines);
                // The playback follows the session's rate for the
                // pre-roll the first Space press starts.
                if let Some(player) = self.as_mut().pinned_player() {
                    player.set_rate(rate);
                }
                self.as_mut().set_lines(list);
                self.as_mut().sync();
                true
            }
            Some(Err(TapStartError::Edit(error))) => {
                let message = QString::from(shell::edit_failure_reason(&error));
                document.as_mut().edit_refused(message);
                false
            }
            Some(Err(TapStartError::RangeCrossesVoices)) => {
                let message = QString::from("The tap range must stay inside one voice.");
                document.as_mut().edit_refused(message);
                false
            }
            None => false,
        }
    }

    /// tick feeds the playback position; the Finished transition pauses
    /// the playback.
    pub fn tick(mut self: Pin<&mut Self>, position_ms: f64) {
        let was_recording = self
            .session_of()
            .is_some_and(|s| s.state() == TapState::Recording);
        if let Some(session) = self.as_mut().session_mut() {
            session.tick(position_ms);
        }
        if was_recording
            && self
                .session_of()
                .is_some_and(|s| s.state() == TapState::Finished)
        {
            self.as_mut().pause_playback();
        }
        self.as_mut().sync();
    }

    /// key_down records a Space press: the first one starts the
    /// pre-roll and plays its range, the later ones record a tap.
    pub fn key_down(mut self: Pin<&mut Self>, position_ms: f64) {
        let was_ready = self
            .session_of()
            .is_some_and(|s| s.state() == TapState::Ready);
        if let Some(session) = self.as_mut().session_mut() {
            if was_ready {
                session.start();
            } else {
                session.key_down(position_ms, false);
            }
        }
        if was_ready {
            self.as_mut().play_pre_roll();
        }
        self.as_mut().sync();
    }

    /// key_up records the release of a held press.
    pub fn key_up(mut self: Pin<&mut Self>, position_ms: f64) {
        if let Some(session) = self.as_mut().session_mut() {
            session.key_up(position_ms);
        }
        self.as_mut().sync();
    }

    /// undo_tap takes the last tap back.
    pub fn undo_tap(mut self: Pin<&mut Self>) {
        if let Some(session) = self.as_mut().session_mut() {
            session.undo_tap();
        }
        self.as_mut().sync();
    }

    /// stop ends the session and pauses the playback.
    pub fn stop(mut self: Pin<&mut Self>) {
        if let Some(session) = self.as_mut().session_mut() {
            session.stop();
        }
        self.as_mut().pause_playback();
        self.as_mut().sync();
    }

    /// use_taps commits the taps as one history step.
    pub fn use_taps(self: Pin<&mut Self>) -> bool {
        match self.pinned_document() {
            Some(document) => apply_repair(document),
            None => false,
        }
    }

    /// calibrate writes the click track into the cache directory.
    pub fn calibrate(&self) -> QUrl {
        match dirs::cache_dir().map(|cache| cache.join(APP_ID).join("calibration-clicks.wav")) {
            Some(path) if click_track_wav(&path, 44_100).is_ok() => {
                QUrl::from_local_file(&QString::from(path.to_string_lossy().into_owned()))
            }
            _ => QUrl::default(),
        }
    }

    /// play_pre_roll seeks the player to the pre-roll and plays into
    /// the session's range.
    fn play_pre_roll(mut self: Pin<&mut Self>) {
        let Some(start) = self.pre_roll_start() else {
            return;
        };
        let Some(end) = self.range_end() else {
            return;
        };
        let Some(player) = self.as_mut().pinned_player() else {
            return;
        };
        player.play_range(start, end);
    }

    /// pause_playback stops the playback while the session is over.
    fn pause_playback(mut self: Pin<&mut Self>) {
        if let Some(player) = self.as_mut().pinned_player() {
            player.pause();
        }
    }

    /// pre_roll_start reads the session's pre-roll seek target.
    fn pre_roll_start(&self) -> Option<f64> {
        self.session_of()?.pre_roll_start()
    }

    /// range_end reads the session's last target end.
    fn range_end(&self) -> Option<f64> {
        self.session_of()?.range_end()
    }
}

impl qobject::Calibration {
    /// document_rust returns the document's Rust side behind the pointer.
    fn calibration_document(self: Pin<&mut Self>) -> Option<&'static mut SongDocumentRust> {
        let pointer = *self.as_ref().document();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the document outlives the calibration and Qt runs the
        // invokables on one thread, so the pointer is not aliased.
        let document = unsafe { Pin::new_unchecked(&mut *pointer) };
        Some(document.rust_mut().get_mut())
    }

    /// sync mirrors the round into the properties.
    fn sync(mut self: Pin<&mut Self>) {
        let round = self.as_ref().rust().round.clone();
        let mut offsets = QVariantList::default();
        for offset in round.offsets() {
            offsets.append(match offset {
                Some(ms) => QVariant::from(&ms),
                None => QVariant::default(),
            });
        }
        self.as_mut().set_offsets(offsets);
        self.as_mut().set_has_median(round.median_ms().is_some());
        self.as_mut()
            .set_median_ms(round.median_ms().unwrap_or(0) as i32);
        self.as_mut().set_complete(round.complete());
    }

    /// start writes the click track, clears the taps and plays it.
    pub fn start(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().stop();
        let Some(path) =
            dirs::cache_dir().map(|cache| cache.join(APP_ID).join("calibration-clicks.wav"))
        else {
            return false;
        };
        if click_track_wav(&path, 44_100).is_err() {
            return false;
        }
        let mut player = match usdx_audio::player::Player::new() {
            Ok(player) => player,
            Err(error) => {
                log::warn!("The calibration clicks cannot play: {error}");
                return false;
            }
        };
        if let Err(error) = player.load(&path) {
            log::warn!("The calibration clicks cannot play: {error}");
            return false;
        }
        player.play();
        self.as_mut().rust_mut().round.clear();
        self.as_mut().rust_mut().player = Some(player);
        self.as_mut().set_click(0);
        self.as_mut().set_running(true);
        self.as_mut().sync();
        true
    }

    /// tick follows the playback: the click now playing, and the end of
    /// the round when the track ends.
    pub fn tick(mut self: Pin<&mut Self>) {
        let mut ended = false;
        let mut position = None;
        if let Some(player) = self.as_mut().rust_mut().player.as_mut() {
            for event in player.poll() {
                if matches!(event, usdx_audio::player::PlayerEvent::EndOfStream) {
                    ended = true;
                }
            }
            position = player.position_ms();
        }
        if let Some(position) = position {
            let click = crate::controller::calibration::Calibration::click_at(position) as i32;
            self.as_mut().set_click(click);
        }
        if ended {
            self.as_mut().stop();
        }
    }

    /// tap records a Space press at the playback position.
    pub fn tap(mut self: Pin<&mut Self>) {
        let position = self
            .as_ref()
            .rust()
            .player
            .as_ref()
            .and_then(usdx_audio::player::Player::position_ms);
        if let Some(position) = position {
            self.as_mut().tap_at(position);
        }
    }

    /// tap_at records a press at the given playback position.
    pub fn tap_at(mut self: Pin<&mut Self>, position_ms: f64) {
        self.as_mut().rust_mut().round.tap(position_ms);
        self.as_mut().sync();
    }

    /// stop ends the clicks; the taps stay for the Use button.
    pub fn stop(mut self: Pin<&mut Self>) {
        if let Some(mut player) = self.as_mut().rust_mut().player.take() {
            player.pause();
        }
        self.as_mut().set_running(false);
    }

    /// use_latency stores the median as the tap latency (negative
    /// medians store 0).
    pub fn use_latency(mut self: Pin<&mut Self>) -> bool {
        let Some(median) = self.as_ref().rust().round.median_ms() else {
            return false;
        };
        let Some(document) = self.as_mut().calibration_document() else {
            return false;
        };
        document.set_latency_ms(median.max(0) as u32);
        true
    }
}
