//! The tap mapping of the timing repair (plan section 6.9, task P7-04).
//!
//! About me: a tap-along session's key presses arrive as `Tap`s (audio
//! times, latency already removed). Line mode pins every line in the range
//! to its tap's time as a rigid shift: the lines before the first and
//! after the last tapped line keep their place and a line's notes follow
//! their start rigidly, so only the line starts move (user decision
//! 2026-11-16 "Keep outside, keep lengths"). Syllable mode sets every
//! tapped note's start to its tap's time, with the held length `up - down`
//! and the unheld length `min(old length * local factor, next tap - 1
//! beat)`; the local factor is the ratio of the tapped span to the old
//! span of the same notes. Fewer taps than notes: the notes after the last
//! tap stretch between the last tap and the next unchanged note (the
//! voice's first note after the edited set; without one they follow the
//! last tap rigidly), and the last tapped note's unheld length is bounded
//! by the tail's first start. Both modes write through the shared commit
//! core (retime.rs): the affected lines plus one line before and after
//! them go through the collision pass, and every phrase end keeps its old
//! fraction of the new gap.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use crate::collisions::{self, TimedLine, TimedNote};
use crate::patch::{EditError, Patch};
use crate::retime::{LineMap, commit_times, mapped_window, rigid_shifts};
use usdx_format::rows::{Document, RowId};
use usdx_format::song::Song;

/// Tap is one key press along the song: `down_ms` is the press time and
/// `up_ms` the release time when the key was held. Both are audio times
/// with the latency already removed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tap {
    pub down_ms: f64,
    pub up_ms: Option<f64>,
}

/// sorted_taps validates the taps (finite times, a held release does not
/// precede its press) and sorts them by press time.
fn sorted_taps(taps: &[Tap]) -> Result<Vec<(f64, Option<f64>)>, EditError> {
    let mut sorted: Vec<(f64, Option<f64>)> = Vec::with_capacity(taps.len());
    for tap in taps {
        if !tap.down_ms.is_finite() {
            return Err(EditError::InvalidHeaderValue);
        }
        if let Some(up) = tap.up_ms
            && (!up.is_finite() || up < tap.down_ms)
        {
            return Err(EditError::InvalidHeaderValue);
        }
        sorted.push((tap.down_ms, tap.up_ms));
    }
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(sorted)
}

/// checked_voice refuses a relative song, a song without a tempo and a
/// voice outside the song.
fn checked_voice(song: &Song, voice: u8) -> Result<usize, EditError> {
    if song.relative {
        return Err(EditError::RelativeModeIsReadOnly);
    }
    song.timing.ok_or(EditError::NoTiming)?;
    usize::from(voice)
        .lt(&song.voices.len())
        .then_some(usize::from(voice))
        .ok_or(EditError::StalePatch("the voice is not in the song"))
}

/// apply_line_taps pins every line of `voice` in `lines` to its tap's time
/// and returns the undoable patch: each tap is the next line's new start,
/// the line's notes follow their start rigidly, the lines outside the
/// range keep their place.
pub fn apply_line_taps(
    doc: &Document,
    song: &Song,
    voice: u8,
    lines: Range<usize>,
    taps: &[Tap],
) -> Result<Patch, EditError> {
    let voice = checked_voice(song, voice)?;
    if taps.is_empty() {
        return Err(EditError::StalePatch("the taps are empty"));
    }
    let taps = sorted_taps(taps)?;
    let known = lines.start < lines.end && lines.end <= song.voices[voice].lines.len();
    if !known {
        return Err(EditError::StalePatch(
            "the tapped lines are not lines of the voice",
        ));
    }
    if taps.len() != lines.len() {
        return Err(EditError::StalePatch("every tapped line needs one tap"));
    }
    let timing = song.timing.expect("the timing is checked");
    let all = collisions::timed_lines(doc, song, voice);
    // Each tapped line shifts rigidly by its tap's offset; only the line
    // starts move (user decision 2026-11-16).
    let mut maps = Vec::with_capacity(taps.len());
    for (tap, line) in taps.iter().zip(lines.clone()) {
        let old = all[line]
            .notes
            .first()
            .map(|note| note.start)
            .ok_or(EditError::StalePatch("the tapped line has no notes"))?;
        let delta = timing.ms_to_beat(tap.0) - old;
        maps.push((line, LineMap::rigid(delta, false)));
    }
    let first = lines.start;
    let after_end = lines.end - 1;
    let window = mapped_window(&all, &maps, first, after_end);
    let shifted = rigid_shifts(doc, &all, &maps, first, after_end);
    commit_times(doc, &all, &window, first, after_end, &shifted, "Tap lines")
}

/// apply_syllable_taps maps `voice`'s `notes` onto `taps` and returns the
/// undoable patch: each tap sets the next note's start, a held tap also
/// its end, the notes after the last tap stretch between the last tap and
/// the next unchanged note.
pub fn apply_syllable_taps(
    doc: &Document,
    song: &Song,
    voice: u8,
    notes: &[RowId],
    taps: &[Tap],
) -> Result<Patch, EditError> {
    let voice = checked_voice(song, voice)?;
    if taps.is_empty() {
        return Err(EditError::StalePatch("the taps are empty"));
    }
    if notes.is_empty() {
        return Err(EditError::StalePatch("the notes are empty"));
    }
    if taps.len() > notes.len() {
        return Err(EditError::StalePatch("more taps than notes"));
    }
    let taps = sorted_taps(taps)?;
    let timing = song.timing.expect("the timing is checked");
    let all = collisions::timed_lines(doc, song, voice);

    // The given notes in the voice's row order with their old times and
    // line indexes; an id the voice does not have is stale input.
    let wanted: HashSet<u32> = notes.iter().map(|id| id.0).collect();
    let mut edited: Vec<(usize, RowId, f64, f64)> = Vec::new(); // line, id, start, end
    for (index, line) in all.iter().enumerate() {
        for note in &line.notes {
            if wanted.contains(&note.id.0) {
                edited.push((index, note.id, note.start, note.end));
            }
        }
    }
    if edited.len() != wanted.len() {
        return Err(EditError::StalePatch(
            "the taps name notes the voice does not have",
        ));
    }

    // The tapped notes take the tap times in beats.
    let down: Vec<f64> = taps
        .iter()
        .map(|(down, _)| timing.ms_to_beat(*down))
        .collect();
    let up: Vec<Option<f64>> = taps
        .iter()
        .map(|(_, up)| up.map(|ms| timing.ms_to_beat(ms)))
        .collect();
    let tapped = taps.len();
    let first_old = edited[0].2;
    let last_old = edited[tapped - 1].2;
    let old_span = last_old - first_old;
    let factor = if tapped > 1 && old_span > 0.0 {
        (down[tapped - 1] - down[0]) / old_span
    } else {
        1.0
    };

    // The tail (the notes after the last tapped note) stretches between
    // the last tap and the next unchanged note, the voice's first note
    // after the edited set; without one it follows the last tap rigidly.
    let flat: Vec<(RowId, f64)> = all
        .iter()
        .flat_map(|line| line.notes.iter().map(|note| (note.id, note.start)))
        .collect();
    let last_id = edited[edited.len() - 1].1;
    let next_unchanged = flat
        .iter()
        .position(|(id, _)| *id == last_id)
        .and_then(|position| flat.get(position + 1))
        .map(|(_, start)| *start);
    let (tail_k, tail_a_old, tail_a_new) = match next_unchanged {
        Some(next_old) if next_old > last_old && next_old > down[tapped - 1] => (
            (next_old - down[tapped - 1]) / (next_old - last_old),
            last_old,
            down[tapped - 1],
        ),
        _ => (1.0, last_old, down[tapped - 1]),
    };
    // The first tail note's mapped start bounds the last tapped note's
    // unheld length.
    let tail_first_new = edited
        .get(tapped)
        .map(|&(_, _, start, _)| tail_a_new + (start - tail_a_old) * tail_k);

    // The new times in beats: the tapped notes take the tap times, the
    // tail maps linearly from the last tapped note's start.
    let mut times: HashMap<RowId, (f64, f64)> = HashMap::with_capacity(edited.len());
    for (index, &(_, id, old_start, old_end)) in edited.iter().enumerate() {
        let (start, end) = if index < tapped {
            let old_length = old_end - old_start;
            let bound = if index + 1 < tapped {
                down[index + 1] - 1.0
            } else if let Some(tail_start) = tail_first_new {
                tail_start - 1.0
            } else {
                next_unchanged.map_or(f64::INFINITY, |next_old| next_old - 1.0)
            };
            let length = match up[index] {
                Some(up_beat) => up_beat - down[index],
                None => (old_length * factor).min(bound - down[index]),
            };
            (down[index], down[index] + length)
        } else {
            let start = tail_a_new + (old_start - tail_a_old) * tail_k;
            let old_length = old_end - old_start;
            (start, start + old_length * tail_k)
        };
        times.insert(id, (start, end));
    }

    // The pass window: the edited notes' lines plus one line before and
    // after them; the edited notes carry their new times, the rest keep.
    let first = edited[0].0;
    let after_end = edited[edited.len() - 1].0;
    let in_window = |line: usize| line + 1 >= first && line <= after_end + 1;
    let window: Vec<TimedLine> = all
        .iter()
        .enumerate()
        .filter(|(index, _)| in_window(*index))
        .map(|(_, line)| TimedLine {
            notes: line
                .notes
                .iter()
                .map(|note| match times.get(&note.id) {
                    Some(&(start, end)) => TimedNote {
                        id: note.id,
                        start,
                        end,
                    },
                    None => note.clone(),
                })
                .collect(),
            end: None,
        })
        .collect();
    commit_times(doc, &all, &window, first, after_end, &[], "Tap syllables")
}
