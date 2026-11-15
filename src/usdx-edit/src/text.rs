//! The line text operations of the edit operations.
//!
//! About me: this module turns a whole line's text, written the way people
//! write syllabified lyrics, into note texts (plan section 6.5, task P6-01).
//! The syntax: white space separates words, `|` splits a word into
//! syllables, and `~` starts a new syllable that continues the previous one
//! on a new note (a held note); the `~` stays in that note's text. Every
//! word's last syllable carries the trailing space (user decision
//! 2026-11-15, "Always trailing space").
//!
//! `line_text_for_editing` writes the notes of a line in that syntax for
//! the edit field. Not every note text can be written so (a leading space,
//! two words on one note, a `~` inside a note, an empty text), so the plan
//! never maps a text back blindly: `plan_line_text` aligns the old
//! syllables with the new ones (a longest common subsequence on the
//! syllable texts without spaces) and changes only the stretches of notes
//! whose syllables changed (user decision 2026-11-15, "Local to the edit").
//! Unchanged notes keep their exact text, and moving a space is no change.
//! A stretch with more syllables than notes splits its longest note in
//! halves (rounded down on the left) until the counts match; a stretch with
//! fewer merges its run of neighbouring notes with the shortest span. A
//! word added between two notes goes to the notes without text between
//! them, else into the note before it (the first note at the line start);
//! a word deleted from a note's stretch merges that note into its
//! neighbour. The `Ask` policy returns the plan as `NeedsDecision` so the
//! dialog can let the user pick the note per count change (user decision
//! 2026-11-15, "Pick note per change"); `plan_line_text` takes those picks.

use std::ops::Range;

use usdx_format::rows::{Document, NoteRow, RowId, RowKind};
use usdx_format::song::Song;

use crate::ops::{edited, new_row};
use crate::patch::RowOp;

pub use crate::patch::{EditError, Patch};

/// The undo label of a line text edit.
const LABEL: &str = "Edit line text";

/// One syllable of a parsed line text: the text as it goes into the note,
/// the trailing space of a word's last syllable included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Syllable {
    pub text: String,
}

/// What the plan does when a stretch has a different syllable count than
/// note count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountPolicy {
    /// Split the longest notes or merge the shortest pairs automatically.
    SplitMerge,
    /// Return the proposal as `EditError::NeedsDecision` when any count
    /// differs.
    Ask,
}

/// The kind of a count fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountFix {
    /// More syllables than notes: one note is split.
    Split,
    /// Fewer syllables than notes: a run of neighbouring notes is merged.
    Merge,
}

/// How a planned note relates to the line's notes before the edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteStatus {
    /// The note keeps its row exactly.
    Unchanged,
    /// The note keeps its row with a new text, start or length.
    Changed,
    /// The note is a new row from a split.
    New,
}

/// One note of the planned line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedNote {
    /// The index into the line's notes of the note this one comes from.
    pub source: usize,
    pub note: NoteRow,
    pub status: NoteStatus,
}

/// One stretch whose syllable count differs from its note count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountChange {
    pub kind: CountFix,
    /// The line's notes of the stretch (indices into the line's notes).
    pub notes: Range<usize>,
    /// The new syllables of the stretch (indices into the parsed syllables).
    pub syllables: Range<usize>,
    /// The notes the user may pick: for a split the notes long enough to
    /// take the extra syllables, for a merge the first note of each run
    /// that can merge.
    pub candidates: Vec<usize>,
    /// The picked candidate: the user's pick or the automatic choice.
    pub chosen: usize,
}

/// The planned line: its notes after the edit and the count changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextPlan {
    pub notes: Vec<PlannedNote>,
    pub changes: Vec<CountChange>,
}

/// parse_line_text splits a line text into syllables. White space of any
/// length separates words, `|` separates syllables and `~` starts a new
/// syllable; empty pieces give no syllable.
pub fn parse_line_text(input: &str) -> Vec<Syllable> {
    let mut syllables = Vec::new();
    for word in input.split_whitespace() {
        let mut pieces: Vec<String> = Vec::new();
        for part in word.split('|') {
            let mut current = String::new();
            for ch in part.chars() {
                if ch == '~' && !current.is_empty() {
                    pieces.push(std::mem::take(&mut current));
                }
                current.push(ch);
            }
            if !current.is_empty() {
                pieces.push(current);
            }
        }
        if let Some(last) = pieces.last_mut() {
            last.push(' ');
        }
        syllables.extend(pieces.into_iter().map(|text| Syllable { text }));
    }
    syllables
}

/// line_text_for_editing writes the line's notes in the line text syntax:
/// the note texts joined, a `|` where two notes meet inside a word (not
/// before a `~`, which starts a syllable by itself), white space runs
/// collapsed to one space and the ends trimmed.
pub fn line_text_for_editing(notes: &[NoteRow]) -> String {
    join_texts(notes.iter().map(|note| note.text.as_str()))
}

/// syllables_text writes syllables in the line text syntax, the way
/// `line_text_for_editing` writes note texts.
pub fn syllables_text(syllables: &[Syllable]) -> String {
    join_texts(syllables.iter().map(|syllable| syllable.text.as_str()))
}

/// join_texts joins note or syllable texts in the line text syntax.
fn join_texts<'a>(texts: impl Iterator<Item = &'a str>) -> String {
    let mut joined = String::new();
    for text in texts {
        if let (Some(last), Some(first)) = (joined.chars().last(), text.chars().next())
            && !last.is_whitespace()
            && !first.is_whitespace()
            && first != '~'
        {
            joined.push('|');
        }
        joined.push_str(text);
    }
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// key is the comparison form of a syllable: its text without spaces, so
/// moving a space is no change.
fn key(text: &str) -> &str {
    text.trim()
}

/// matched_pairs aligns two key lists by a longest common subsequence and
/// returns the matched index pairs in increasing order.
fn matched_pairs(old: &[&str], new: &[&str]) -> Vec<(usize, usize)> {
    // suffix[i][j] is the LCS length of old[i..] and new[j..].
    let mut suffix = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            suffix[i][j] = if old[i] == new[j] {
                suffix[i + 1][j + 1] + 1
            } else {
                suffix[i + 1][j].max(suffix[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pairs = Vec::new();
    while i < old.len() && j < new.len() {
        if old[i] == new[j] && suffix[i][j] == suffix[i + 1][j + 1] + 1 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if suffix[i + 1][j] >= suffix[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// The old syllables of the line with the note that owns each.
struct OldSyllables {
    keys: Vec<String>,
    owner: Vec<usize>,
    /// The first old syllable of each note; `first[n]` is the count.
    first: Vec<usize>,
}

impl OldSyllables {
    /// of parses each note text by itself: a note boundary is always a
    /// syllable boundary of the display text, so the keys equal the keys of
    /// the parsed display text.
    fn of(notes: &[NoteRow]) -> OldSyllables {
        let mut keys = Vec::new();
        let mut owner = Vec::new();
        let mut first = Vec::with_capacity(notes.len() + 1);
        for (index, note) in notes.iter().enumerate() {
            first.push(keys.len());
            for syllable in parse_line_text(&note.text) {
                keys.push(key(&syllable.text).to_owned());
                owner.push(index);
            }
        }
        first.push(keys.len());
        OldSyllables { keys, owner, first }
    }

    /// range returns the old syllables owned by the notes `a..=b`.
    fn range(&self, a: usize, b: usize) -> Range<usize> {
        self.first[a]..self.first[b + 1]
    }
}

/// regions finds the stretches of notes whose syllables changed, as
/// inclusive note ranges with their new syllable ranges, in line order.
fn regions(
    notes: &[NoteRow],
    old: &OldSyllables,
    new_keys: &[&str],
) -> Vec<((usize, usize), Range<usize>)> {
    let old_keys: Vec<&str> = old.keys.iter().map(String::as_str).collect();
    let pairs = matched_pairs(&old_keys, new_keys);
    let mut partner_of_old = vec![None; old_keys.len()];
    for &(i, j) in &pairs {
        partner_of_old[i] = Some(j);
    }
    let mut dirty = vec![false; notes.len()];
    // The owners of the unmatched old syllables changed.
    for (i, partner) in partner_of_old.iter().enumerate() {
        if partner.is_none() {
            dirty[old.owner[i]] = true;
        }
    }
    // New syllables between two matched pairs with no old syllable between
    // them go to the notes without text between the two owners, else to
    // the owner before them (the owner after them at the line start).
    let mut previous: Option<(usize, usize)> = None;
    for next in pairs.iter().copied().map(Some).chain(std::iter::once(None)) {
        let new_from = previous.map_or(0, |(_, j)| j + 1);
        let new_to = next.map_or(new_keys.len(), |(_, j)| j);
        let old_from = previous.map_or(0, |(i, _)| i + 1);
        let old_to = next.map_or(old_keys.len(), |(i, _)| i);
        if new_from < new_to && old_from == old_to {
            let low = previous.map(|(i, _)| old.owner[i]);
            let high = next.map(|(i, _)| old.owner[i]);
            let between_from = low.map_or(0, |owner| owner + 1);
            let between_to = high.unwrap_or(notes.len());
            if between_from < between_to {
                for flag in &mut dirty[between_from..between_to] {
                    *flag = true;
                }
            } else if let Some(owner) = low.or(high) {
                dirty[owner] = true;
            }
        }
        previous = next;
    }
    // Grow the dirty runs until each is bounded by matched syllables on
    // both sides and holds at least one new syllable.
    loop {
        let runs = dirty_runs(&dirty);
        let mut grown = false;
        for &(a, b) in &runs {
            let range = old.range(a, b);
            if range.start > 0 && partner_of_old[range.start - 1].is_none() {
                for flag in &mut dirty[old.owner[range.start - 1]..a] {
                    *flag = true;
                }
                grown = true;
            }
            if range.end < old_keys.len() && partner_of_old[range.end].is_none() {
                for flag in &mut dirty[b + 1..=old.owner[range.end]] {
                    *flag = true;
                }
                grown = true;
            }
            if !grown && new_range(old, &partner_of_old, new_keys.len(), a, b).is_empty() {
                if a > 0 {
                    dirty[a - 1] = true;
                    grown = true;
                } else if b + 1 < notes.len() {
                    dirty[b + 1] = true;
                    grown = true;
                }
            }
            if grown {
                break;
            }
        }
        if !grown {
            return runs
                .into_iter()
                .map(|(a, b)| {
                    (
                        (a, b),
                        new_range(old, &partner_of_old, new_keys.len(), a, b),
                    )
                })
                .collect();
        }
    }
}

/// dirty_runs lists the maximal runs of dirty notes as inclusive ranges.
fn dirty_runs(dirty: &[bool]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, flag) in dirty.iter().enumerate() {
        match (*flag, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                runs.push((from, index - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        runs.push((from, dirty.len() - 1));
    }
    runs
}

/// new_range maps the notes `a..=b` to their new syllables: the new
/// syllables between the partners of the matched old syllables that bound
/// the notes' old syllables.
fn new_range(
    old: &OldSyllables,
    partner_of_old: &[Option<usize>],
    new_count: usize,
    a: usize,
    b: usize,
) -> Range<usize> {
    let range = old.range(a, b);
    let from = if range.start == 0 {
        0
    } else {
        partner_of_old[range.start - 1].map_or(0, |j| j + 1)
    };
    let to = if range.end == partner_of_old.len() {
        new_count
    } else {
        partner_of_old[range.end].unwrap_or(new_count)
    };
    from..to.max(from)
}

/// halves splits a note into `count` pieces: the longest piece (the first
/// of equal ones) is split in halves, the left half rounded down, until the
/// count is reached.
fn halves(note: &NoteRow, count: usize) -> Vec<(i64, i64)> {
    let mut pieces = vec![(note.start, note.length)];
    while pieces.len() < count {
        let (longest, &(start, length)) = pieces
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(_, (_, length))| *length)
            .expect("the pieces are never empty");
        let left = length / 2;
        pieces.splice(
            longest..=longest,
            [(start, left), (start + left, length - left)],
        );
    }
    pieces
}

/// span is the merged length of the notes `from..=to`.
fn span(notes: &[NoteRow], from: usize, to: usize) -> i64 {
    notes[to].start + notes[to].length - notes[from].start
}

/// text_for keeps the old text when the new one differs only in its
/// surrounding spaces.
fn text_for(old: &str, new: &str) -> String {
    if old.trim() == new.trim() {
        old.to_owned()
    } else {
        new.to_owned()
    }
}

/// derived builds a planned note from an old note with a new text and
/// timing; it is unchanged when nothing differs.
fn derived(source: usize, old: &NoteRow, text: &str, start: i64, length: i64) -> PlannedNote {
    let note = NoteRow {
        text: text_for(&old.text, text),
        start,
        length,
        ..old.clone()
    };
    let status = if &note == old {
        NoteStatus::Unchanged
    } else {
        NoteStatus::Changed
    };
    PlannedNote {
        source,
        note,
        status,
    }
}

/// plan_line_text plans the line's notes for the new syllables. `picks`
/// holds the user's pick per count change in line order; a missing pick or
/// one outside the candidates takes the automatic choice.
pub fn plan_line_text(
    notes: &[NoteRow],
    syllables: &[Syllable],
    picks: &[Option<usize>],
) -> Result<TextPlan, EditError> {
    if syllables.is_empty() {
        return Err(EditError::EmptyLineText);
    }
    let old = OldSyllables::of(notes);
    let new_keys: Vec<&str> = syllables.iter().map(|s| key(&s.text)).collect();
    let mut plan = TextPlan::default();
    let mut next_note = 0;
    for ((a, b), new) in regions(notes, &old, &new_keys) {
        for (index, note) in notes.iter().enumerate().take(a).skip(next_note) {
            plan.notes.push(PlannedNote {
                source: index,
                note: note.clone(),
                status: NoteStatus::Unchanged,
            });
        }
        next_note = b + 1;
        let texts: Vec<&str> = syllables[new.clone()]
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        let note_count = b + 1 - a;
        let old_count = old.range(a, b).len();
        if texts.len() == old_count {
            // Same syllable count: each note takes as many syllables as it
            // had, so a note with two words keeps both.
            let mut taken = 0;
            for (index, note) in notes.iter().enumerate().take(b + 1).skip(a) {
                let owned = old.range(index, index).len();
                let text = texts[taken..taken + owned].concat();
                taken += owned;
                plan.notes
                    .push(derived(index, note, &text, note.start, note.length));
            }
        } else if texts.len() == note_count {
            for (index, text) in (a..=b).zip(&texts) {
                let note = &notes[index];
                plan.notes
                    .push(derived(index, note, text, note.start, note.length));
            }
        } else if texts.len() > note_count {
            let pieces = texts.len() - note_count + 1;
            let candidates: Vec<usize> = (a..=b)
                .filter(|&index| notes[index].length >= pieces as i64)
                .collect();
            // The longest note, the first of equal ones.
            let automatic = candidates
                .iter()
                .copied()
                .rev()
                .max_by_key(|&index| notes[index].length)
                .ok_or(EditError::NotesTooShort)?;
            let chosen = pick(picks, plan.changes.len(), &candidates, automatic);
            let mut text = texts.iter();
            for (index, note) in notes.iter().enumerate().take(b + 1).skip(a) {
                let count = if index == chosen { pieces } else { 1 };
                for (piece, (start, length)) in halves(note, count).into_iter().enumerate() {
                    let syllable = text.next().copied().unwrap_or_default();
                    if piece == 0 {
                        plan.notes
                            .push(derived(index, note, syllable, start, length));
                    } else {
                        plan.notes.push(PlannedNote {
                            source: index,
                            note: NoteRow {
                                start,
                                length,
                                text: syllable.to_owned(),
                                ..note.clone()
                            },
                            status: NoteStatus::New,
                        });
                    }
                }
            }
            plan.changes.push(CountChange {
                kind: CountFix::Split,
                notes: a..b + 1,
                syllables: new,
                candidates,
                chosen,
            });
        } else {
            let run = note_count - texts.len() + 1;
            let candidates: Vec<usize> = (a..=b + 1 - run).collect();
            // The run with the shortest merged span, the first of equal ones.
            let automatic = candidates
                .iter()
                .copied()
                .min_by_key(|&from| span(notes, from, from + run - 1))
                .expect("a stretch with fewer syllables than notes has a run");
            let chosen = pick(picks, plan.changes.len(), &candidates, automatic);
            let mut text = texts.iter();
            let mut index = a;
            while index <= b {
                let note = &notes[index];
                let syllable = text.next().copied().unwrap_or_default();
                if index == chosen {
                    let length = span(notes, index, index + run - 1);
                    plan.notes
                        .push(derived(index, note, syllable, note.start, length));
                    index += run;
                } else {
                    plan.notes
                        .push(derived(index, note, syllable, note.start, note.length));
                    index += 1;
                }
            }
            plan.changes.push(CountChange {
                kind: CountFix::Merge,
                notes: a..b + 1,
                syllables: new,
                candidates,
                chosen,
            });
        }
    }
    for (index, note) in notes.iter().enumerate().skip(next_note) {
        plan.notes.push(PlannedNote {
            source: index,
            note: note.clone(),
            status: NoteStatus::Unchanged,
        });
    }
    Ok(plan)
}

/// pick returns the user's pick for the count change when it is a
/// candidate, else the automatic choice.
fn pick(picks: &[Option<usize>], change: usize, candidates: &[usize], automatic: usize) -> usize {
    match picks.get(change).copied().flatten() {
        Some(picked) if candidates.contains(&picked) => picked,
        _ => automatic,
    }
}

/// line_notes returns the note rows of the line's note ids.
pub fn line_notes(doc: &Document, line: &[RowId]) -> Result<Vec<NoteRow>, EditError> {
    line.iter()
        .map(
            |id| match doc.rows.get(id.0 as usize).map(|row| &row.kind) {
                Some(RowKind::Note(note)) => Ok(note.clone()),
                _ => Err(EditError::UnknownRow(*id)),
            },
        )
        .collect()
}

/// text_change_patch builds the patch that turns the line's notes into the
/// planned ones. A plan with count changes edits beats, which a
/// relative-mode song refuses.
pub fn text_change_patch(
    doc: &Document,
    song: &Song,
    line: &[RowId],
    plan: &TextPlan,
) -> Result<Patch, EditError> {
    if !plan.changes.is_empty() && song.relative {
        return Err(EditError::RelativeModeIsReadOnly);
    }
    let notes = line_notes(doc, line)?;
    let mut planned: Vec<Vec<&NoteRow>> = vec![Vec::new(); notes.len()];
    for note in &plan.notes {
        planned
            .get_mut(note.source)
            .ok_or(EditError::StalePatch(
                "the plan names a note the line lacks",
            ))?
            .push(&note.note);
    }
    // The rows change from the last to the first, so every op names the
    // index the row has in the document as it is.
    let mut ops = Vec::new();
    for (index, (id, pieces)) in line.iter().zip(&planned).enumerate().rev() {
        let row = id.0 as usize;
        let Some((first, rest)) = pieces.split_first() else {
            ops.push(RowOp::Remove {
                id: *id,
                row: doc.rows[row].clone(),
                after: row.checked_sub(1).map(|before| RowId(before as u32)),
            });
            continue;
        };
        if **first != notes[index] {
            let wanted = (*first).clone();
            ops.push(edited(doc, row, |kind| *kind = RowKind::Note(wanted)));
        }
        for (offset, piece) in rest.iter().enumerate() {
            let at = row + 1 + offset;
            ops.push(RowOp::Insert {
                after: Some(RowId(at as u32 - 1)),
                row: new_row(at, RowKind::Note((*piece).clone())),
            });
        }
    }
    Ok(Patch {
        label: LABEL.to_owned(),
        ops,
    })
}

/// plan_text_change plans the line text edit and builds its patch. With the
/// `Ask` policy, a plan with count changes returns `NeedsDecision` with the
/// automatic proposal instead.
pub fn plan_text_change(
    doc: &Document,
    song: &Song,
    line: &[RowId],
    syllables: &[Syllable],
    policy: CountPolicy,
) -> Result<Patch, EditError> {
    let notes = line_notes(doc, line)?;
    let plan = plan_line_text(&notes, syllables, &[])?;
    if policy == CountPolicy::Ask && !plan.changes.is_empty() {
        return Err(EditError::NeedsDecision(plan));
    }
    text_change_patch(doc, song, line, &plan)
}
