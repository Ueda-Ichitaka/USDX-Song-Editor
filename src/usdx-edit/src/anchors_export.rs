//! The anchors.txt export for the Factory (plan section 6.11, task
//! P7-06).
//!
//! About me: the export turns a timing-repair session into the Factory's
//! anchors.txt: every voice-P1 line whose first note's start time changed
//! in the session (decision D-12, compared by `changes::ChangeLog`
//! against the state as loaded), plus the anchors pinned in the session
//! and the end pin. Entries are deduplicated by line (a pinned time wins
//! over a changed time) and sorted by time. The text of an entry is the
//! line's syllables joined with the hold marker `~` deleted and the
//! whitespace collapsed, so it compares equal to the Factory's
//! lyrics.txt text of the same line. `format_time`, `normalize` and
//! `match_anchor_units` are ports of the Factory's `outro.py:parse_time`,
//! `anchors._normalise` and `anchors.match_anchor_units` (end matching:
//! the last anchor binds the last lyric line with that text, each earlier
//! anchor the last match before it). The module is pure: the caller reads
//! lyrics.txt and passes its lines (user decision 2026-11-16); the save
//! dialog lives in the app layer.

use std::collections::HashMap;

use usdx_format::rows::{Document, RowId};
use usdx_format::song::Song;

use crate::changes::{ChangeLog, LineChange};

/// AnchorEntry is one exported anchor: the time in milliseconds from the
/// audio start, the cleaned line text and the flat voice-P1 line index
/// the anchor came from (the export dialog's checks compare it with the
/// Factory's binding).
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorEntry {
    pub time_ms: f64,
    pub text: String,
    pub source_line: usize,
}

/// PinnedAnchor is an anchor pinned in the session: the flat voice-P1
/// line index it pins and the pinned time in milliseconds from the audio
/// start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinnedAnchor {
    pub line: usize,
    pub time_ms: f64,
}

/// AnchorExportError refuses an unrepresentable anchor set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorExportError {
    /// Two anchors share a time (the format needs strictly increasing
    /// times).
    TimesNotIncreasing,
    /// The end pin is at or before the last anchor.
    EndNotAfterLastAnchor,
}

/// AnchorMatchError refuses an anchor text the lyrics do not have.
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorMatchError {
    /// No lyric line with that text at or before the previous anchor's
    /// binding.
    NotFound { text: String },
}

/// format_time writes milliseconds as `M:SS.ss`, or `H:MM:SS.ss` from one
/// hour on, with two decimals that round half away from zero. Times
/// below 0 clamp to `0:00.00`.
pub fn format_time(ms: f64) -> String {
    let centis = (ms / 10.0).round().max(0.0) as i64;
    let hours = centis / 360_000;
    let rest = centis % 360_000;
    let minutes = rest / 6_000;
    let seconds = (rest % 6_000) / 100;
    let hundredths = rest % 100;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{hundredths:02}")
    } else {
        format!("{minutes}:{seconds:02}.{hundredths:02}")
    }
}

/// normalize ports the Factory's `_normalise`: lower case, every non-word
/// non-space character replaced by a space, whitespace collapsed. A word
/// character is a letter, a digit or an underscore, as in Python's `\w`.
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// line_text returns the export text of a line: the syllables joined
/// (song.rs `Line::text`), the hold marker `~` deleted and the
/// whitespace collapsed, matching how the Factory strips hold markers
/// from lyrics.txt.
fn line_text(doc: &Document, line: &usdx_format::song::Line) -> String {
    line.text(doc)
        .replace('~', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// p1_lines walks the lines of every voice numbered 1 with their flat
/// index over all P1 lines.
fn p1_lines(song: &Song) -> impl Iterator<Item = (usize, &usdx_format::song::Line)> {
    song.voices
        .iter()
        .filter(|voice| voice.number == 1)
        .flat_map(|voice| &voice.lines)
        .enumerate()
}

/// build_entries lists the export anchors (decision D-12): every
/// voice-P1 line whose first note's start time differs from the
/// snapshot, plus the pinned lines with their pinned times (a pin wins
/// over a change of the same line), deduplicated by line and sorted by
/// time. A pin at a line index without a noteable line is stale and is
/// skipped.
pub fn build_entries(
    doc: &Document,
    song: &Song,
    changes: &ChangeLog,
    pinned: &[PinnedAnchor],
) -> Vec<AnchorEntry> {
    let lines: Vec<(usize, RowId, String)> = p1_lines(song)
        .filter_map(|(index, line)| {
            line.notes
                .first()
                .map(|first| (index, *first, line_text(doc, line)))
        })
        .collect();

    // The wanted time per line index: the pinned times first, so a pin
    // wins over a change (or_insert keeps the pin).
    let mut times: HashMap<usize, f64> = HashMap::new();
    for pin in pinned {
        times.insert(pin.line, pin.time_ms);
    }
    for LineChange {
        first_note,
        time_ms,
    } in changes.changed_lines(doc, song)
    {
        if let Some(&(index, _, _)) = lines.iter().find(|&(_, first, _)| *first == first_note) {
            times.entry(index).or_insert(time_ms);
        }
    }

    let mut entries: Vec<AnchorEntry> = lines
        .into_iter()
        .filter_map(|(index, _, text)| {
            times.get(&index).map(|&time_ms| AnchorEntry {
                time_ms,
                text,
                source_line: index,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    entries
}

/// render writes the anchors.txt file: a header comment naming the song,
/// the editor version and the date, then the anchor lines
/// `<time> | <text>` and last the optional `end <time>` line. The times
/// must strictly increase and the end must come after the last anchor
/// (the Factory's reading rules).
pub fn render(
    title: &str,
    version: &str,
    date: &str,
    entries: &[AnchorEntry],
    end: Option<f64>,
) -> Result<String, AnchorExportError> {
    for pair in entries.windows(2) {
        if pair[0].time_ms >= pair[1].time_ms {
            return Err(AnchorExportError::TimesNotIncreasing);
        }
    }
    if let Some(end_ms) = end
        && entries.last().is_some_and(|last| end_ms <= last.time_ms)
    {
        return Err(AnchorExportError::EndNotAfterLastAnchor);
    }
    let mut file =
        format!("# anchors for {title} - exported by USDX Song Editor {version} on {date}\n");
    for entry in entries {
        file += &format!("{} | {}\n", format_time(entry.time_ms), entry.text);
    }
    if let Some(end_ms) = end {
        file += &format!("end {}\n", format_time(end_ms));
    }
    Ok(file)
}

/// match_anchor_units ports the Factory's `match_anchor_units`: which
/// lyric line each anchor pins. Text comparison ignores case,
/// punctuation and spacing (`normalize`). Matching runs from the end:
/// the last anchor pins the last lyric line with that text, each earlier
/// anchor the last matching line before the one pinned after it - so a
/// repeated chorus means its latest occurrence. An anchor text that is
/// not found in this order is refused.
pub fn match_anchor_units(
    anchor_texts: &[&str],
    unit_texts: &[&str],
) -> Result<Vec<usize>, AnchorMatchError> {
    let normalised: Vec<String> = unit_texts.iter().map(|text| normalize(text)).collect();
    let mut bound = Vec::with_capacity(anchor_texts.len());
    let mut limit = unit_texts.len();
    for text in anchor_texts.iter().rev() {
        let wanted = normalize(text);
        let index = (0..limit).rev().find(|&i| normalised[i] == wanted);
        match index {
            Some(i) => {
                bound.push(i);
                limit = i;
            }
            None => {
                return Err(AnchorMatchError::NotFound {
                    text: text.to_string(),
                });
            }
        }
    }
    bound.reverse();
    Ok(bound)
}

/// export_problems runs the export dialog's checks and returns one
/// message per problem. With `lyrics` (the lyrics.txt lines) the
/// Factory's matching decides: an anchor text that is not found, or that
/// binds to a different occurrence than the song line it came from,
/// warns. Without lyrics.txt a text that appears again on a later song
/// line warns (the Factory would bind it to the later occurrence).
pub fn export_problems(
    entries: &[AnchorEntry],
    song_lines: &[String],
    lyrics: Option<&[String]>,
) -> Vec<String> {
    let Some(lyrics) = lyrics else {
        let mut problems = Vec::new();
        for entry in entries {
            let wanted = normalize(&entry.text);
            let repeats_later = song_lines
                .iter()
                .enumerate()
                .skip(entry.source_line + 1)
                .any(|(_, text)| normalize(text) == wanted);
            if repeats_later {
                problems.push(format!(
                    "{} | {}: the Factory would bind this text to a later line",
                    format_time(entry.time_ms),
                    entry.text
                ));
            }
        }
        return problems;
    };

    // The Factory binding, from the end, with the anchor chain.
    let normalised: Vec<String> = lyrics.iter().map(|text| normalize(text)).collect();
    let mut problems = Vec::new();
    let mut limit = lyrics.len();
    for entry in entries.iter().rev() {
        let wanted = normalize(&entry.text);
        match (0..limit).rev().find(|&i| normalised[i] == wanted) {
            Some(index) => {
                if index != entry.source_line {
                    problems.push(format!(
                        "{} | {}: binds to lyrics line {}, a different occurrence than the song line {} it came from",
                        format_time(entry.time_ms),
                        entry.text,
                        index,
                        entry.source_line
                    ));
                }
                limit = index;
            }
            None => {
                problems.push(format!(
                    "{} | {}: the text is not found in lyrics.txt (in this order)",
                    format_time(entry.time_ms),
                    entry.text
                ));
            }
        }
    }
    problems.reverse();
    problems
}
