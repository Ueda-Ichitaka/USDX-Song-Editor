//! The collision pass of the timing repair (plan section 6.6, task P7-01).
//!
//! About me: a retime (anchor stretch, block move, tap mapping) computes new
//! float start and end beats for one voice; this module turns them into
//! whole beats that satisfy the file's invariants. Starts and ends round to
//! the nearest beat with a length of at least 1. Walking the voice in order,
//! a note that starts inside the previous one first shortens the previous
//! note down to 1 beat; when that is not enough, the note moves to the
//! previous note's end and keeps its length, so pushes cascade. Each phrase
//! end keeps its old place as a fraction of the gap between its line's last
//! note and the next line's first note, rounded and clamped into the new
//! gap; a phrase end after the voice's last line keeps its distance. A
//! first start below 0 cannot be written: the pass reports the beats the
//! caller must lower `#GAP` by and shift every note with. `timed_lines`
//! reads a voice as it stands, which a valid song passes through unchanged.

use usdx_format::rows::{Document, RowId};
use usdx_format::song::Song;

/// One note with its new float times in beats.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedNote {
    pub id: RowId,
    pub start: f64,
    pub end: f64,
}

/// Where a line's phrase end goes.
#[derive(Clone, Debug, PartialEq)]
pub enum BreakPlace {
    /// Between this line and the next: the fraction of the gap from the
    /// last note's end (0) to the next line's first start (1).
    Fraction(f64),
    /// After the voice's last line: the distance in beats after its last
    /// note's end.
    AfterLast(f64),
}

/// A line's phrase end row and its place.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedBreak {
    pub id: RowId,
    pub place: BreakPlace,
}

/// One line of the voice: its notes in row order and its phrase end.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedLine {
    pub notes: Vec<TimedNote>,
    pub end: Option<TimedBreak>,
}

/// A note in whole beats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedNote {
    pub id: RowId,
    pub start: i64,
    pub length: i64,
}

/// The resolved voice: the notes in input order and the phrase ends with
/// their beats.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placed {
    pub notes: Vec<PlacedNote>,
    pub phrase_ends: Vec<(RowId, i64)>,
}

/// The voice's first start would be below 0: `#GAP` must go down by this
/// many beats and every note of every voice move up by them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeedsGapShift(pub i64);

impl std::fmt::Display for NeedsGapShift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// resolve_collisions places one voice's notes and phrase ends on whole
/// beats (see the module comment). Lines without notes are left out with
/// their phrase end.
pub fn resolve_collisions(lines: &[TimedLine]) -> Result<Placed, NeedsGapShift> {
    let lines: Vec<&TimedLine> = lines.iter().filter(|line| !line.notes.is_empty()).collect();
    let mut placed = Placed::default();
    // The first and last placed note index of every line.
    let mut spans = Vec::with_capacity(lines.len());
    for line in &lines {
        let first = placed.notes.len();
        for note in &line.notes {
            let start = note.start.round() as i64;
            let length = (note.end.round() as i64 - start).max(1);
            if let Some(previous) = placed.notes.last_mut() {
                let previous_end = previous.start + previous.length;
                if start < previous_end {
                    previous.length = (start - previous.start).max(1);
                }
                let free_from = previous.start + previous.length;
                placed.notes.push(PlacedNote {
                    id: note.id,
                    start: start.max(free_from),
                    length,
                });
            } else {
                placed.notes.push(PlacedNote {
                    id: note.id,
                    start,
                    length,
                });
            }
        }
        spans.push((first, placed.notes.len() - 1));
    }
    // Starts never decrease, so the first note holds the lowest start.
    if let Some(first) = placed.notes.first()
        && first.start < 0
    {
        return Err(NeedsGapShift(-first.start));
    }
    for (index, line) in lines.iter().enumerate() {
        let Some(end) = &line.end else {
            continue;
        };
        let last = &placed.notes[spans[index].1];
        let gap_start = last.start + last.length;
        let beat = match (&end.place, spans.get(index + 1)) {
            (BreakPlace::Fraction(fraction), Some(&(next, _))) => {
                let gap_end = placed.notes[next].start;
                let wanted = gap_start as f64 + fraction * (gap_end - gap_start) as f64;
                (wanted.round() as i64).clamp(gap_start, gap_end.max(gap_start))
            }
            (BreakPlace::AfterLast(distance), None) => gap_start + (distance.round() as i64).max(0),
            // A fraction without a next line or a distance before one:
            // the marker sits on the line's end.
            _ => gap_start,
        };
        placed.phrase_ends.push((end.id, beat));
    }
    Ok(placed)
}

/// timed_lines reads the lines of a voice (an index into the song's
/// voices) with their current times and phrase end places: the input of a
/// retime that has not moved anything yet.
pub fn timed_lines(doc: &Document, song: &Song, voice: usize) -> Vec<TimedLine> {
    let Some(voice) = song.voices.get(voice) else {
        return Vec::new();
    };
    let notes_of = |line: &usdx_format::song::Line| -> Vec<TimedNote> {
        line.notes
            .iter()
            .filter_map(|id| song.note(doc, *id))
            .map(|note| TimedNote {
                id: note.row,
                start: note.start,
                end: note.start + note.length as f64,
            })
            .collect()
    };
    let lines: Vec<Vec<TimedNote>> = voice.lines.iter().map(notes_of).collect();
    voice
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let notes = lines[index].clone();
            let end = line.end.and_then(|id| {
                let beat = song.phrase_end_beat(doc, id)?;
                let last_end = notes.last().map_or(beat, |note| note.end);
                let place = match lines.get(index + 1).and_then(|next| next.first()) {
                    Some(next) => {
                        let gap = next.start - last_end;
                        BreakPlace::Fraction(if gap > 0.0 {
                            ((beat - last_end) / gap).clamp(0.0, 1.0)
                        } else {
                            0.0
                        })
                    }
                    None => BreakPlace::AfterLast(beat - last_end),
                };
                Some(TimedBreak { id, place })
            });
            TimedLine { notes, end }
        })
        .collect()
}
