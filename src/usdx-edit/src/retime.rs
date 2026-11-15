//! The anchor stretch of the timing repair (plan section 6.7, task P7-02).
//!
//! About me: anchors pin the first note of chosen lines to target times;
//! between two neighbouring anchors every note time maps linearly from the
//! old to the new anchor times (a line maps with the segment that starts at
//! its anchor, the last anchor's line with the last segment), the lines
//! before shift by the first anchor's offset or keep, the lines after
//! shift, keep or stretch to the end pin, and note lengths scale with
//! their line's stretch factor or keep their beat count. The mapped lines
//! plus one line before and after them (user decision 2026-11-15
//! "Retimed lines + neighbours") go through the collision pass, which
//! rounds and resolves overlaps; the lines beyond that window shift
//! rigidly, so Factory quirks far from the repair stay untouched. Every
//! phrase end keeps its old fraction of the gap to the next line's first
//! note (the pass's placement rule) computed from the final note
//! positions. A first note below beat 0 is refused with the #GAP shift
//! the caller must apply first.

use std::collections::HashMap;

use crate::collisions::{self, BreakPlace, NeedsGapShift, TimedLine};
use crate::ops::edited;
use crate::patch::{EditError, Patch};
use usdx_format::rows::{Document, RowKind};
use usdx_format::song::Song;

/// Anchor pins the first note of `line` (an index into the voice's lines)
/// to the time `target_ms`.
pub struct Anchor {
    pub line: usize,
    pub target_ms: f64,
}

/// Outside says what happens to the lines before the first anchor.
pub enum Outside {
    /// The lines shift by the first anchor's offset.
    Shift,
    /// The lines keep their times.
    Keep,
}

/// Lengths says what happens to the note lengths of the mapped lines.
pub enum Lengths {
    /// The lengths scale with the line's stretch factor.
    Scale,
    /// The lengths keep their beat count.
    Keep,
}

/// StretchOptions collects the outside and length behaviour and the
/// optional end pin.
pub struct StretchOptions {
    pub before: Outside,
    pub after: Outside,
    pub lengths: Lengths,
    /// When set, the lines after the last anchor stretch so the voice's
    /// last note ends at this time (this overrides `after` for them).
    pub end_pin_ms: Option<f64>,
}

/// LineMap maps the float beat times of one line's region: a time `t`
/// becomes `a_new + (t - a_old) * k`.
#[derive(Clone, Copy)]
struct LineMap {
    a_old: f64,
    a_new: f64,
    k: f64,
    scale: bool,
}

impl LineMap {
    /// point maps one note time.
    fn point(&self, t: f64) -> f64 {
        self.a_new + (t - self.a_old) * self.k
    }

    /// end maps a note end: with scaled lengths the end maps like a point,
    /// otherwise the length in beats stays.
    fn end(&self, old_start: f64, old_end: f64) -> f64 {
        if self.scale {
            self.point(old_end)
        } else {
            self.point(old_start) + (old_end - old_start)
        }
    }

    /// rigid builds the map of a rigid shift by `delta` beats.
    fn rigid(delta: f64, scale: bool) -> Self {
        LineMap {
            a_old: 0.0,
            a_new: delta,
            k: 1.0,
            scale,
        }
    }
}

/// anchor_stretch plans the stretch of `voice`'s notes onto the anchors and
/// returns the undoable patch.
pub fn anchor_stretch(
    doc: &Document,
    song: &Song,
    voice: u8,
    anchors: &[Anchor],
    options: &StretchOptions,
) -> Result<Patch, EditError> {
    if song.relative {
        return Err(EditError::RelativeModeIsReadOnly);
    }
    let timing = song.timing.ok_or(EditError::NoTiming)?;
    let lines = collisions::timed_lines(doc, song, usize::from(voice));

    // The anchors are sorted by line and must pin different lines with
    // strictly increasing target times.
    let mut sorted: Vec<&Anchor> = anchors.iter().collect();
    sorted.sort_by_key(|anchor| anchor.line);
    for pair in sorted.windows(2) {
        if pair[0].line == pair[1].line || pair[1].target_ms <= pair[0].target_ms {
            return Err(EditError::AnchorsOutOfOrder);
        }
    }
    // Every anchor names a line of the voice that has notes; its old beat
    // is that line's first note start and its new beat the target time.
    let mut pinned: Vec<(usize, f64, f64)> = Vec::with_capacity(sorted.len());
    for anchor in &sorted {
        let line = lines.get(anchor.line).ok_or(EditError::StalePatch(
            "the anchor names a line the voice does not have",
        ))?;
        let old = line
            .notes
            .first()
            .map(|note| note.start)
            .ok_or(EditError::StalePatch(
                "the anchor names a line without notes",
            ))?;
        pinned.push((anchor.line, old, timing.ms_to_beat(anchor.target_ms)));
    }
    if pinned.is_empty() {
        return Ok(Patch {
            label: "Anchor stretch".to_owned(),
            ops: Vec::new(),
        });
    }
    let first = pinned[0].0;
    let last_line = pinned[pinned.len() - 1].0;
    let scale = matches!(options.lengths, Lengths::Scale);

    // The map of every mapped line: between two neighbouring anchors the
    // line maps with the segment that starts at its anchor; the last
    // anchor's line maps with the last segment.
    let mut segment_maps: Vec<LineMap> = Vec::with_capacity(pinned.len() - 1);
    for pair in pinned.windows(2) {
        if pair[1].1 <= pair[0].1 {
            return Err(EditError::AnchorsOutOfOrder);
        }
        segment_maps.push(LineMap {
            a_old: pair[0].1,
            a_new: pair[0].2,
            k: (pair[1].2 - pair[0].2) / (pair[1].1 - pair[0].1),
            scale,
        });
    }
    let mut maps: Vec<(usize, LineMap)> = Vec::new();
    if pinned.len() == 1 {
        // One anchor has no scale partner: its line follows rigidly.
        let (line, old, new) = pinned[0];
        maps.push((line, LineMap::rigid(new - old, scale)));
    } else {
        for line in first..=last_line {
            let mut segment = 0;
            for (index, (a_line, _, _)) in pinned.iter().enumerate() {
                if *a_line <= line {
                    segment = index;
                }
            }
            if segment == pinned.len() - 1 {
                segment -= 1;
            }
            maps.push((line, segment_maps[segment]));
        }
    }
    // With an end pin the lines after the last anchor stretch so the
    // voice's last note ends at the pin; when they hold no note after the
    // anchor there is nothing to stretch and the plain after policy stays.
    let mut after_end = last_line;
    if let Some(pin_ms) = options.end_pin_ms {
        let last_index = lines.len() - 1;
        if last_index > last_line {
            let (_, a_old, a_new) = pinned[pinned.len() - 1];
            if let Some(last_end) = lines[last_index].notes.last().map(|note| note.end)
                && last_end > a_old
            {
                let map = LineMap {
                    a_old,
                    a_new,
                    k: (timing.ms_to_beat(pin_ms) - a_new) / (last_end - a_old),
                    scale,
                };
                for line in last_line + 1..=last_index {
                    maps.push((line, map));
                }
                after_end = last_index;
            }
        }
    }
    let last_index = lines.len() - 1;
    let d_first = pinned[0].2 - pinned[0].1;
    let d_last = pinned[pinned.len() - 1].2 - pinned[pinned.len() - 1].1;
    if let Outside::Shift = options.before {
        for line in 0..first {
            maps.push((line, LineMap::rigid(d_first, scale)));
        }
    }
    if let Outside::Shift = options.after {
        for line in after_end + 1..=last_index {
            maps.push((line, LineMap::rigid(d_last, scale)));
        }
    }

    // The collision pass sees the mapped lines plus one line before and
    // after them; a line of the window without a map keeps its times. The
    // markers are handed over without places: they are placed below from
    // the final positions, where the pass would only know the window.
    let in_window = |line: usize| line + 1 >= first && line <= after_end + 1;
    let mut window: Vec<TimedLine> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !in_window(index) {
            continue;
        }
        let map = maps
            .iter()
            .find_map(|(mapped, map)| (*mapped == index).then_some(*map));
        let notes = line
            .notes
            .iter()
            .map(|note| match map {
                Some(map) => crate::collisions::TimedNote {
                    id: note.id,
                    start: map.point(note.start),
                    end: map.end(note.start, note.end),
                },
                None => note.clone(),
            })
            .collect();
        window.push(TimedLine { notes, end: None });
    }
    let placed = collisions::resolve_collisions(&window)
        .map_err(|NeedsGapShift(beats)| EditError::NeedsGapShift(NeedsGapShift(beats)))?;

    // The patch: every placed note whose row differs, every note of the
    // mapped lines outside the window (they are rigid shifts, so only the
    // start moves) and every recomputed phrase end.
    let mut ops = Vec::new();
    for note in &placed.notes {
        let index = note.id.0 as usize;
        let RowKind::Note(row) = &doc.rows[index].kind else {
            continue;
        };
        if row.start != note.start || row.length != note.length {
            ops.push(edited(doc, index, |kind| {
                if let RowKind::Note(row) = kind {
                    row.start = note.start;
                    row.length = note.length;
                }
            }));
        }
    }
    for (line, map) in &maps {
        if in_window(*line) {
            continue;
        }
        for note in &lines[*line].notes {
            let new_start = map.point(note.start).round() as i64;
            let index = note.id.0 as usize;
            let RowKind::Note(row) = &doc.rows[index].kind else {
                continue;
            };
            if row.start != new_start {
                ops.push(edited(doc, index, |kind| {
                    if let RowKind::Note(row) = kind {
                        row.start = new_start;
                    }
                }));
            }
        }
    }

    // The final position of every note of the voice: the current values,
    // overlaid with the pass's placed notes and the shifted lines.
    let mut final_pos: HashMap<u32, (i64, i64)> = HashMap::new();
    for line in &lines {
        for note in &line.notes {
            let RowKind::Note(row) = &doc.rows[note.id.0 as usize].kind else {
                continue;
            };
            final_pos.insert(note.id.0, (row.start, row.start + row.length));
        }
    }
    for note in &placed.notes {
        final_pos.insert(note.id.0, (note.start, note.start + note.length));
    }
    for (line, map) in &maps {
        if in_window(*line) {
            continue;
        }
        for note in &lines[*line].notes {
            let RowKind::Note(row) = &doc.rows[note.id.0 as usize].kind else {
                continue;
            };
            let start = map.point(note.start).round() as i64;
            final_pos.insert(note.id.0, (start, start + row.length));
        }
    }

    // Every phrase end whose line or next line is affected keeps its old
    // fraction of the new gap (the pass's placement rule, computed from
    // the final note positions).
    let affected = |line: usize| {
        (line < lines.len() && in_window(line)) || maps.iter().any(|(mapped, _)| *mapped == line)
    };
    for (index, line) in lines.iter().enumerate() {
        let Some(break_row) = &line.end else {
            continue;
        };
        // A marker needs notes on its own side and an affected line pair.
        let Some(last) = line.notes.last() else {
            continue;
        };
        if !affected(index) && !affected(index + 1) {
            continue;
        }
        let (_, gap_start) = final_pos[&last.id.0];
        let next_start = lines
            .get(index + 1)
            .and_then(|next| next.notes.first())
            .map(|note| final_pos[&note.id.0].0);
        let beat = match (&break_row.place, next_start) {
            (BreakPlace::Fraction(fraction), Some(next)) => {
                let wanted = gap_start as f64 + fraction * (next - gap_start) as f64;
                (wanted.round() as i64).clamp(gap_start, next.max(gap_start))
            }
            (BreakPlace::AfterLast(distance), None) => gap_start + (distance.round() as i64).max(0),
            // A fraction without a next line or a distance before one:
            // the marker sits on the line's end.
            _ => gap_start,
        };
        let row_index = break_row.id.0 as usize;
        let RowKind::PhraseEnd { beat: current, .. } = &doc.rows[row_index].kind else {
            continue;
        };
        if *current != Some(beat) {
            ops.push(edited(doc, row_index, |kind| {
                if let RowKind::PhraseEnd { beat: marker, .. } = kind {
                    *marker = Some(beat);
                }
            }));
        }
    }

    Ok(Patch {
        label: "Anchor stretch".to_owned(),
        ops,
    })
}
