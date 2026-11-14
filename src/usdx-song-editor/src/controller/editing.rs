//! The mouse-editing controller: selection and drag commits (plan 8.3,
//! 8.4, task P5-04).
//!
//! About me: this module holds the selection commands the note sheet's
//! delegates call (click selects, Ctrl adds, Shift extends from the anchor
//! to the click, the rubber band selects every note intersecting the band)
//! and the drag commits (move the selection, shift the following notes,
//! resize an edge), each as exactly one history step so one drag undoes as
//! one step. The pure snapping functions place a dragged note edge onto a
//! neighbour's edge when the edge lands within 8 px of it (Alt turns the
//! snapping off); they exist so the ghost preview and the committed delta
//! agree, and because the notes sit on integer beats the rounding of the
//! committed delta happens after the snap. The methods live on the
//! EditorController because they own its selection and history state.

use usdx_edit::ops;
use usdx_edit::patch::RowOp;
use usdx_edit::{EditError, Patch};
use usdx_format::rows::{RowId, RowKind};
use usdx_format::song::Song;

use crate::controller::editor::{EditorController, LoadedSong};
use crate::controller::notes::PITCH_C4_MIDI;

/// How a click on a note changes the selection (plan 8.4: a click selects,
/// Ctrl adds, Shift extends in time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    /// The clicked note becomes the whole selection.
    Set,
    /// The clicked note joins the selection.
    Add,
    /// The notes between the anchor (the last selected note) and the
    /// clicked note join the selection, in row order. Without an anchor
    /// the clicked note becomes the whole selection.
    Extend,
}

/// The edge of a note a drag or resize works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The note's start beat.
    Start,
    /// The note's end beat (start + length).
    End,
}

/// The snap threshold in pixels (user decision 2026-11-14): a dragged edge
/// that lands within this distance of a neighbour edge snaps onto it.
const SNAP_PX: f64 = 8.0;

/// snapped_move rounds a body-drag delta into committed whole beats and,
/// when snapping is on, places the note's start edge onto the previous
/// note's end or its end edge onto the next note's start, whichever edge
/// lands closer to its neighbour edge. The pitch delta passes through. An
/// unknown id rounds plainly.
pub fn snapped_move(
    doc: &usdx_format::rows::Document,
    song: &Song,
    id: RowId,
    d_beat: f64,
    d_pitch: i64,
    snap: bool,
    px_per_beat: f64,
) -> (i64, i64) {
    let Some((index, start, length)) = note_data(doc, song, id) else {
        return (d_beat.round() as i64, d_pitch);
    };
    let threshold = SNAP_PX / px_per_beat;
    // Each candidate names the committed delta that would place one edge
    // exactly onto its neighbour edge: the start edge onto the previous
    // note's end, the end edge onto the next note's start. A candidate
    // fires when the dragged edge lands within the threshold of that
    // neighbour edge, and the closer one wins (ties go to the start,
    // evaluated first).
    let start_candidate = index
        .checked_sub(1)
        .and_then(|previous| doc.rows.get(previous))
        .and_then(|row| song.note(doc, row.id))
        .map(|note| note.start + note.length as f64 - start)
        .map(|adjustment| ((d_beat - adjustment).abs(), adjustment as i64));
    let end_candidate = doc
        .rows
        .get(index + 1)
        .and_then(|row| song.note(doc, row.id))
        .map(|note| note.start - start - length as f64)
        .map(|adjustment| ((d_beat - adjustment).abs(), adjustment as i64));
    let mut best: Option<(f64, i64)> = None;
    if snap {
        for (distance, adjustment) in [start_candidate, end_candidate].into_iter().flatten() {
            if distance <= threshold
                && best.is_none_or(|(best_distance, _)| distance < best_distance)
            {
                best = Some((distance, adjustment));
            }
        }
    }
    let committed = best.map_or_else(|| d_beat.round() as i64, |(_, adjustment)| adjustment);
    (committed, d_pitch)
}

/// snapped_resize rounds an edge-drag delta into committed whole beats
/// and, when snapping is on, places the resized edge onto the neighbour
/// edge (the start edge onto the previous note's end, the end edge onto
/// the next note's start). An unknown id rounds plainly.
pub fn snapped_resize(
    doc: &usdx_format::rows::Document,
    song: &Song,
    id: RowId,
    edge: Edge,
    d_beat: f64,
    snap: bool,
    px_per_beat: f64,
) -> i64 {
    let Some((index, start, length)) = note_data(doc, song, id) else {
        return d_beat.round() as i64;
    };
    let threshold = SNAP_PX / px_per_beat;
    let (edge_position, target) = match edge {
        Edge::Start => {
            let target = index
                .checked_sub(1)
                .and_then(|previous| doc.rows.get(previous))
                .and_then(|row| song.note(doc, row.id))
                .map(|note| note.start + note.length as f64);
            (start + d_beat, target)
        }
        Edge::End => {
            let target = doc
                .rows
                .get(index + 1)
                .and_then(|row| song.note(doc, row.id))
                .map(|note| note.start);
            (start + length as f64 + d_beat, target)
        }
    };
    if snap
        && let Some(target) = target
        && (edge_position - target).abs() <= threshold
    {
        // The committed delta places the edge exactly onto the neighbour.
        return (target - edge_position + d_beat).round() as i64;
    }
    d_beat.round() as i64
}

/// note_data locates a note: its document index, absolute start and
/// length, or `None` when the id is unknown or not a note.
fn note_data(
    doc: &usdx_format::rows::Document,
    song: &Song,
    id: RowId,
) -> Option<(usize, f64, i64)> {
    let note = song.note(doc, id)?;
    Some((id.0 as usize, note.start, note.length))
}

impl EditorController {
    /// select_note applies one click's selection change (plan 8.4).
    pub fn select_note(&mut self, id: RowId, mode: SelectionMode) {
        match mode {
            SelectionMode::Set => {
                self.selection.clear();
                self.selection.insert(id);
                self.anchor = Some(id);
            }
            SelectionMode::Add => {
                self.selection.insert(id);
                self.anchor = Some(id);
            }
            SelectionMode::Extend => match self.anchor {
                Some(anchor) => self.extend_between(anchor, id),
                None => {
                    self.selection.clear();
                    self.selection.insert(id);
                    self.anchor = Some(id);
                }
            },
        }
    }

    /// clear_selection empties the selection and drops the anchor.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.anchor = None;
    }

    /// select_range replaces the selection with every note intersecting
    /// the band rectangle (plan 8.4, user decision 2026-11-14): the band
    /// covers beats and MIDI pitches, the bounds may arrive swapped. The
    /// anchor moves to the last selected note in row order, so a
    /// Shift+click extends from the band.
    pub fn select_range(
        &mut self,
        start_beat: f64,
        end_beat: f64,
        low_pitch: i32,
        high_pitch: i32,
    ) {
        self.selection.clear();
        self.anchor = None;
        let Some(loaded) = self.loaded.as_ref() else {
            return;
        };
        let (from_beat, to_beat) = ordered(start_beat, end_beat);
        let (from_pitch, to_pitch) = ordered(low_pitch as f64, high_pitch as f64);
        for row in &loaded.document.rows {
            let RowKind::Note(_) = &row.kind else {
                continue;
            };
            let Some(note) = loaded.song.note(&loaded.document, row.id) else {
                continue;
            };
            let midi = note.pitch as i32 + PITCH_C4_MIDI;
            let overlaps = note.start < to_beat
                && note.start + note.length as f64 > from_beat
                && midi as f64 >= from_pitch
                && midi as f64 <= to_pitch;
            if overlaps {
                self.selection.insert(row.id);
            }
        }
        self.anchor = self.selection.last().copied();
    }

    /// extend_between selects the note rows between the anchor and the
    /// clicked note, inclusive, in row order (row order is time order
    /// within a voice, user decision 2026-11-14).
    fn extend_between(&mut self, anchor: RowId, clicked: RowId) {
        let Some(loaded) = self.loaded.as_ref() else {
            return;
        };
        let (from, to) = if anchor <= clicked {
            (anchor, clicked)
        } else {
            (clicked, anchor)
        };
        self.selection.clear();
        for row in &loaded.document.rows[from.0 as usize..=to.0 as usize] {
            if matches!(row.kind, RowKind::Note(_)) {
                self.selection.insert(row.id);
            }
        }
        self.anchor = Some(clicked);
    }

    /// move_selection moves every selected note by the committed drag
    /// deltas (plan 8.4: one undo step per drag). An empty selection is a
    /// no-op; a refused move changes nothing.
    pub fn move_selection(&mut self, d_beat: i64, d_pitch: i64) -> Result<(), EditError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::move_notes(&loaded.document, &loaded.song, &ids, d_beat, d_pitch)?
        };
        self.commit(patch)
    }

    /// move_selection_and_following shifts the selected notes and every
    /// later note and phrase end of their voice as a block (the Ctrl+Shift
    /// arrow keys and the block semantics of plan 8.4).
    pub fn move_selection_and_following(&mut self, d_beat: i64) -> Result<(), EditError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::move_notes_and_following(&loaded.document, &loaded.song, &ids, d_beat)?
        };
        self.commit(patch)
    }

    /// resize_note moves one note's edge by the committed drag delta
    /// (plan 8.4: at least 1 beat, enforced by the operation).
    pub fn resize_note(&mut self, id: RowId, edge: Edge, d_beat: i64) -> Result<(), EditError> {
        let patch = {
            let loaded = self.require_loaded()?;
            match edge {
                Edge::Start => ops::resize_start(&loaded.document, &loaded.song, id, d_beat)?,
                Edge::End => ops::resize_end(&loaded.document, &loaded.song, id, d_beat)?,
            }
        };
        self.commit(patch)
    }

    /// undo takes the last edit step back and rebuilds the song view.
    pub fn undo(&mut self) -> Option<String> {
        let label = {
            let loaded = self.loaded.as_mut()?;
            loaded.history.undo(&mut loaded.document)?
        };
        self.rebuild_song();
        Some(label)
    }

    /// redo re-applies the last undone edit step and rebuilds the song
    /// view.
    pub fn redo(&mut self) -> Option<String> {
        let label = {
            let loaded = self.loaded.as_mut()?;
            loaded.history.redo(&mut loaded.document)?
        };
        self.rebuild_song();
        Some(label)
    }

    /// commit applies a built patch as one history step and rebuilds the
    /// song view, so the lines, notes and times follow the edit.
    fn commit(&mut self, patch: Patch) -> Result<(), EditError> {
        let Some(loaded) = self.loaded.as_mut() else {
            return Ok(());
        };
        loaded.history.commit(&mut loaded.document, patch)?;
        self.rebuild_song();
        Ok(())
    }

    /// rebuild_song rebuilds the derived song view from the document.
    fn rebuild_song(&mut self) {
        if let Some(loaded) = self.loaded.as_mut() {
            loaded.song = Song::from_document(&loaded.document);
        }
    }

    /// require_loaded returns the loaded song or a stale-patch error, so
    /// the edit methods can use `?` without unwrapping.
    fn require_loaded(&self) -> Result<&LoadedSong, EditError> {
        self.loaded
            .as_ref()
            .ok_or(EditError::StalePatch("no song is loaded"))
    }

    /// insert_note_after inserts one note after the last selected note, or
    /// after the last note of the current line without a selection (the
    /// Insert key, user decision 2026-11-14): it inherits the length and
    /// the pitch of its predecessor, starts at the predecessor's end and
    /// carries the default text "~". The new note becomes the selection,
    /// so pressing Insert again chains notes.
    pub fn insert_note_after(&mut self) -> Result<(), EditError> {
        let (after, start, length, pitch) = {
            let loaded = self.require_loaded()?;
            let predecessor = match self.selection.last() {
                Some(id) => *id,
                None => current_line_last_note(loaded, self.current_line())?,
            };
            let note = loaded
                .song
                .note(&loaded.document, predecessor)
                .ok_or(EditError::UnknownRow(predecessor))?;
            (
                Some(predecessor),
                note.start + note.length as f64,
                note.length,
                note.pitch,
            )
        };
        let patch = {
            let loaded = self.require_loaded()?;
            ops::insert_note(
                &loaded.document,
                &loaded.song,
                after,
                start.round() as i64,
                length,
                pitch,
                None,
            )?
        };
        // The inserted row sits one after the predecessor; the predecessor
        // is unchanged, so its id names the new note. The new note becomes
        // the whole selection, so pressing Insert again chains notes.
        if let Some(predecessor) = after {
            let inserted = RowId(predecessor.0 + 1);
            self.selection.clear();
            self.selection.insert(inserted);
            self.anchor = Some(inserted);
        }
        self.commit(patch)
    }

    /// delete_selection removes every selected note (the Delete key);
    /// a phrase end whose line loses all its notes goes away too. The
    /// selection empties because its ids no longer name notes.
    pub fn delete_selection(&mut self) -> Result<(), EditError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::delete_notes(&loaded.document, &loaded.song, &ids)?
        };
        self.commit(patch)?;
        self.selection.clear();
        self.anchor = None;
        Ok(())
    }

    /// split_selection_at splits the single selected note at the beat
    /// (the S key at the playhead). With no note or several selected it is
    /// a no-op; a beat outside the note is refused by the operation.
    pub fn split_selection_at(&mut self, at_beat: i64) -> Result<(), EditError> {
        let Some(id) = self.single_selection() else {
            return Ok(());
        };
        let patch = {
            let loaded = self.require_loaded()?;
            ops::split_note(&loaded.document, &loaded.song, id, at_beat)?
        };
        self.commit(patch)
    }

    /// merge_selection_with_next joins the single selected note with the
    /// note after it (the M key). With no note or several selected it is a
    /// no-op; a phrase end between the notes is refused by the operation.
    pub fn merge_selection_with_next(&mut self) -> Result<(), EditError> {
        let Some(id) = self.single_selection() else {
            return Ok(());
        };
        let patch = {
            let loaded = self.require_loaded()?;
            ops::merge_with_next(&loaded.document, &loaded.song, id)?
        };
        self.commit(patch)
    }

    /// set_selection_type sets the type of every selected note (the N, R
    /// and Shift+R keys and the inspector's type buttons). The code is one
    /// of the type characters `:`, `*`, `R`, `G`; the freestyle key uses
    /// `toggle_freestyle_selection` instead (it toggles).
    pub fn set_selection_type(&mut self, code: char) -> Result<(), EditError> {
        let Some(note_type) = type_of_code(code) else {
            return Ok(());
        };
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::set_type(&loaded.document, &loaded.song, &ids, note_type)?
        };
        self.commit(patch)
    }

    /// toggle_freestyle_selection turns every selected note freestyle, or
    /// every one of them back to normal when all are freestyle already
    /// (the F key, the Factory behaviour of the operation).
    pub fn toggle_freestyle_selection(&mut self) -> Result<(), EditError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::toggle_freestyle(&loaded.document, &loaded.song, &ids)?
        };
        self.commit(patch)
    }

    /// set_note_text sets one note's syllable (the inspector's Text field,
    /// a double-click on a note or label). One history step.
    pub fn set_note_text(&mut self, id: RowId, text: &str) -> Result<(), EditError> {
        let patch = {
            let loaded = self.require_loaded()?;
            ops::set_text(&loaded.document, &loaded.song, id, text)?
        };
        self.commit(patch)
    }

    /// toggle_phrase_end_before_selection inserts an end-of-phrase marker
    /// before the selected note carrying the previous note's end beat, or
    /// removes the marker that stands there (the B key). The selected
    /// note keeps being selected: the marker insert or remove shifts its
    /// row id, so the selection follows it. One history step.
    pub fn toggle_phrase_end_before_selection(&mut self) -> Result<(), EditError> {
        let Some(id) = self.single_selection() else {
            return Ok(());
        };
        let patch = {
            let loaded = self.require_loaded()?;
            ops::toggle_phrase_end_before(&loaded.document, &loaded.song, id)?
        };
        // The op inserts or removes the marker row before the note: the
        // rows from the marker on shift by one, so the selected id moves
        // with its note.
        let inserted = patch
            .ops
            .iter()
            .any(|op| matches!(op, RowOp::Insert { .. }));
        self.commit(patch)?;
        self.anchor = Some(if inserted {
            RowId(id.0 + 1)
        } else {
            RowId(id.0 - 1)
        });
        self.selection.clear();
        self.selection.insert(self.anchor.expect("just set"));
        Ok(())
    }

    /// move_phrase_end moves one end-of-phrase marker to the beat (the
    /// phrase end handle's drag commit); it stays between the previous
    /// note's end and the next note's start. One history step.
    pub fn move_phrase_end(&mut self, id: RowId, beat: i64) -> Result<(), EditError> {
        let patch = {
            let loaded = self.require_loaded()?;
            ops::move_phrase_end(&loaded.document, &loaded.song, id, beat)?
        };
        self.commit(patch)
    }

    /// transpose_selection shifts the pitch of every selected note by
    /// semitone steps (the inspector's Pitch field on a multi selection).
    /// One history step.
    pub fn transpose_selection(&mut self, semitones: i64) -> Result<(), EditError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let ids: Vec<RowId> = self.selection.iter().copied().collect();
        let patch = {
            let loaded = self.require_loaded()?;
            ops::transpose(&loaded.document, &loaded.song, &ids, semitones)?
        };
        self.commit(patch)
    }

    /// single_selection returns the one selected note, or `None` when the
    /// selection is empty or holds several notes (S, M and B need exactly
    /// one, user decision 2026-11-14).
    fn single_selection(&self) -> Option<RowId> {
        if self.selection.len() == 1 {
            self.selection.iter().copied().next()
        } else {
            None
        }
    }
}

/// type_of_code maps a type character onto its note type; the freestyle
/// key toggles instead of setting, so `F` names no type here.
fn type_of_code(code: char) -> Option<usdx_format::rows::NoteType> {
    use usdx_format::rows::NoteType;
    match code {
        ':' => Some(NoteType::Normal),
        '*' => Some(NoteType::Golden),
        'R' => Some(NoteType::Rap),
        'G' => Some(NoteType::RapGolden),
        _ => None,
    }
}

/// current_line_last_note returns the last note of the global line at
/// `current_line`, for the insert without a selection.
fn current_line_last_note(loaded: &LoadedSong, current_line: usize) -> Result<RowId, EditError> {
    let mut offset = 0;
    for voice in &loaded.song.voices {
        if current_line < offset + voice.lines.len() {
            return voice.lines[current_line - offset]
                .notes
                .last()
                .copied()
                .ok_or(EditError::StalePatch("the current line has no notes"));
        }
        offset += voice.lines.len();
    }
    Err(EditError::StalePatch("the current line is out of range"))
}

/// ordered returns its two arguments as (smaller, larger); the band bounds
/// may arrive swapped.
fn ordered(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}
