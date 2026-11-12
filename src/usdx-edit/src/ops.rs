//! The note and line operations of the edit operations.
//!
//! About me: this module builds the undoable patches for every note-level
//! edit the UI offers: moving, resizing, retyping, inserting, deleting,
//! splitting and merging notes and moving or toggling phrase ends. Each
//! function takes the document and its song view, validates the edit against
//! the current state and returns one `Patch`; it changes nothing itself. The
//! beat-editing operations refuse relative-mode songs, whose stored relative
//! form cannot express them (editing relative songs is task P9-06 behind
//! gate G-01).

use usdx_format::bytes::{RawLine, Terminator};
use usdx_format::rows::{Document, NoteRow, NoteType, Row, RowId, RowKind};
use usdx_format::serialize::canonical_text;
use usdx_format::song::Song;

use crate::patch::RowOp;

pub use crate::patch::{EditError, Patch};

/// require_absolute refuses relative-mode songs: their stored relative form
/// cannot express the beat edits, and saving a relative-mode song in relative
/// form is behind gate G-01.
fn require_absolute(song: &Song) -> Result<(), EditError> {
    if song.relative {
        return Err(EditError::RelativeModeIsReadOnly);
    }
    Ok(())
}

/// note_data returns the row index and the note data of `id`, or `None` when
/// the row does not exist or is not a note.
fn note_data(doc: &Document, id: RowId) -> Option<(usize, NoteRow)> {
    let index = id.0 as usize;
    let row = doc.rows.get(index)?;
    let RowKind::Note(note) = &row.kind else {
        return None;
    };
    Some((index, note.clone()))
}

/// new_row builds a fresh dirty row with an empty raw line; the serializer
/// gives it the document's dominant terminator and the canonical text.
pub(crate) fn new_row(index: usize, kind: RowKind) -> Row {
    Row {
        id: RowId(index as u32),
        raw: RawLine {
            text: String::new(),
            terminator: Terminator::None,
        },
        kind,
        dirty: true,
    }
}

/// edited builds the replace op for the row at `index` whose kind `change`
/// updates; the row becomes dirty and carries the canonical text.
pub(crate) fn edited(doc: &Document, index: usize, change: impl FnOnce(&mut RowKind)) -> RowOp {
    let before = doc.rows[index].clone();
    let mut after = before.clone();
    change(&mut after.kind);
    after.dirty = true;
    after.raw.text = canonical_text(&after);
    RowOp::Replace {
        id: RowId(index as u32),
        before,
        after,
    }
}

/// body_start_index returns the index of the first body row (the first note,
/// phrase end, voice change or end row), or the row count when the file has
/// no body yet.
pub(crate) fn body_start_index(doc: &Document) -> usize {
    doc.rows
        .iter()
        .position(|row| {
            matches!(
                row.kind,
                RowKind::Note(_) | RowKind::PhraseEnd { .. } | RowKind::Voice(_) | RowKind::End
            )
        })
        .unwrap_or(doc.rows.len())
}

/// move_notes shifts the notes by `d_beat` beats and `d_pitch` pitch steps.
pub fn move_notes(
    doc: &Document,
    song: &Song,
    ids: &[RowId],
    d_beat: i64,
    d_pitch: i64,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let mut ops = Vec::with_capacity(ids.len());
    for id in ids {
        let (index, note) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        if note.start + d_beat < 0 {
            return Err(EditError::BeforeSongStart);
        }
        let (start, pitch) = (note.start, note.pitch);
        ops.push(edited(doc, index, |kind| {
            if let RowKind::Note(note) = kind {
                note.start = start + d_beat;
                note.pitch = pitch + d_pitch;
            }
        }));
    }
    Ok(Patch {
        label: "Move notes".to_owned(),
        ops,
    })
}

/// move_notes_and_following shifts the notes and every later note and phrase
/// end of their voice (the Yass "shift remainder"): the line keeps its shape
/// and the following lines move as a block.
pub fn move_notes_and_following(
    doc: &Document,
    song: &Song,
    ids: &[RowId],
    d_beat: i64,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    // The first id's range covers the later ids of the same voice; the set
    // collects the union so no row shifts twice.
    let mut indices = std::collections::BTreeSet::new();
    for id in ids {
        let (index, _) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        for (later, row) in doc.rows.iter().enumerate().skip(index) {
            match &row.kind {
                RowKind::Note(_) | RowKind::PhraseEnd { .. } => {
                    indices.insert(later);
                }
                RowKind::Voice(_) | RowKind::End => break,
                _ => {}
            }
        }
    }
    for &index in &indices {
        if let RowKind::Note(note) = &doc.rows[index].kind
            && note.start + d_beat < 0
        {
            return Err(EditError::BeforeSongStart);
        }
    }
    let ops = indices
        .into_iter()
        .map(|index| {
            edited(doc, index, |kind| match kind {
                RowKind::Note(note) => note.start += d_beat,
                RowKind::PhraseEnd { beat, .. } => {
                    *beat = beat.map(|value| value + d_beat);
                }
                _ => {}
            })
        })
        .collect();
    Ok(Patch {
        label: "Shift following notes".to_owned(),
        ops,
    })
}

/// resize_start moves the note's start and keeps its end, so the length
/// shrinks or grows by the same amount.
pub fn resize_start(
    doc: &Document,
    song: &Song,
    id: RowId,
    d_beat: i64,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let (index, note) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    let start = note.start + d_beat;
    let length = note.length - d_beat;
    if start < 0 {
        return Err(EditError::BeforeSongStart);
    }
    if length < 1 {
        return Err(EditError::LengthBelowOne);
    }
    let op = edited(doc, index, |kind| {
        if let RowKind::Note(resized) = kind {
            resized.start = start;
            resized.length = length;
        }
    });
    Ok(Patch {
        label: "Resize note start".to_owned(),
        ops: vec![op],
    })
}

/// resize_end moves the note's end and keeps its start. The end may cross the
/// next note; the validation layer reports the overlap.
pub fn resize_end(doc: &Document, song: &Song, id: RowId, d_beat: i64) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let (index, note) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    let length = note.length + d_beat;
    if length < 1 {
        return Err(EditError::LengthBelowOne);
    }
    let op = edited(doc, index, |kind| {
        if let RowKind::Note(resized) = kind {
            resized.length = length;
        }
    });
    Ok(Patch {
        label: "Resize note end".to_owned(),
        ops: vec![op],
    })
}

/// set_pitch sets the pitch of the notes to one value.
pub fn set_pitch(
    doc: &Document,
    _song: &Song,
    ids: &[RowId],
    pitch: i64,
) -> Result<Patch, EditError> {
    let mut ops = Vec::with_capacity(ids.len());
    for id in ids {
        let (index, _) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        ops.push(edited(doc, index, |kind| {
            if let RowKind::Note(note) = kind {
                note.pitch = pitch;
            }
        }));
    }
    Ok(Patch {
        label: "Set pitch".to_owned(),
        ops,
    })
}

/// transpose shifts the pitch of the notes by semitone steps.
pub fn transpose(
    doc: &Document,
    _song: &Song,
    ids: &[RowId],
    semitones: i64,
) -> Result<Patch, EditError> {
    let mut ops = Vec::with_capacity(ids.len());
    for id in ids {
        let (index, note) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        let pitch = note.pitch;
        ops.push(edited(doc, index, |kind| {
            if let RowKind::Note(transposed) = kind {
                transposed.pitch = pitch + semitones;
            }
        }));
    }
    Ok(Patch {
        label: "Transpose".to_owned(),
        ops,
    })
}

/// set_type sets the note type of the notes; the raw row is formatted
/// canonically when it is written back.
pub fn set_type(
    doc: &Document,
    _song: &Song,
    ids: &[RowId],
    note_type: NoteType,
) -> Result<Patch, EditError> {
    let mut ops = Vec::with_capacity(ids.len());
    for id in ids {
        let (index, _) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        ops.push(edited(doc, index, |kind| {
            if let RowKind::Note(note) = kind {
                note.kind = note_type.clone();
            }
        }));
    }
    Ok(Patch {
        label: "Set note type".to_owned(),
        ops,
    })
}

/// toggle_freestyle turns the selected notes all freestyle, or all normal
/// when every one of them already is freestyle (the Factory behaviour).
pub fn toggle_freestyle(doc: &Document, _song: &Song, ids: &[RowId]) -> Result<Patch, EditError> {
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        let (index, note) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        selected.push((index, note.kind == NoteType::Freestyle));
    }
    let target = if selected.iter().all(|(_, freestyle)| *freestyle) {
        NoteType::Normal
    } else {
        NoteType::Freestyle
    };
    let ops = selected
        .into_iter()
        .map(|(index, _)| {
            edited(doc, index, |kind| {
                if let RowKind::Note(note) = kind {
                    note.kind = target.clone();
                }
            })
        })
        .collect();
    Ok(Patch {
        label: "Toggle freestyle".to_owned(),
        ops,
    })
}

/// set_text replaces the syllable text of one note.
pub fn set_text(doc: &Document, _song: &Song, id: RowId, text: &str) -> Result<Patch, EditError> {
    let (index, _) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    let value = text.to_owned();
    let op = edited(doc, index, |kind| {
        if let RowKind::Note(note) = kind {
            note.text = value.clone();
        }
    });
    Ok(Patch {
        label: "Edit syllable".to_owned(),
        ops: vec![op],
    })
}

/// insert_note builds a note after the row `after`, or at the start of the
/// body when `after` is `None`. The default text `"~"` marks a held syllable.
pub fn insert_note(
    doc: &Document,
    song: &Song,
    after: Option<RowId>,
    start: i64,
    length: i64,
    pitch: i64,
    text: Option<String>,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    if start < 0 {
        return Err(EditError::BeforeSongStart);
    }
    if length < 1 {
        return Err(EditError::LengthBelowOne);
    }
    let index = match after {
        Some(predecessor) => {
            if predecessor.0 as usize >= doc.rows.len() {
                return Err(EditError::UnknownRow(predecessor));
            }
            predecessor.0 as usize + 1
        }
        // Without an anchor the note goes to the start of the body: before
        // the first body row, or at the end of the rows when the file has no
        // body yet.
        None => body_start_index(doc),
    };
    let row = new_row(
        index,
        RowKind::Note(NoteRow {
            kind: NoteType::Normal,
            start,
            length,
            pitch,
            text: text.unwrap_or_else(|| "~".to_owned()),
        }),
    );
    let op = RowOp::Insert {
        after: if index == 0 {
            None
        } else {
            Some(RowId(index as u32 - 1))
        },
        row,
    };
    Ok(Patch {
        label: "Insert note".to_owned(),
        ops: vec![op],
    })
}

/// delete_notes removes the notes and every phrase end whose line lost all
/// its notes with them.
pub fn delete_notes(doc: &Document, song: &Song, ids: &[RowId]) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let mut remove = vec![false; doc.rows.len()];
    for id in ids {
        let (index, _) = note_data(doc, *id).ok_or(EditError::UnknownRow(*id))?;
        remove[index] = true;
    }
    // A phrase end with no surviving notes before it since the previous
    // phrase end, the voice change or the start of the body goes away too.
    let mut orphans = Vec::new();
    let mut segment_has_notes = false;
    for (index, row) in doc.rows.iter().enumerate() {
        match &row.kind {
            RowKind::Note(_) => {
                if !remove[index] {
                    segment_has_notes = true;
                }
            }
            RowKind::PhraseEnd { .. } => {
                if !segment_has_notes {
                    orphans.push(index);
                }
                segment_has_notes = false;
            }
            RowKind::Voice(_) | RowKind::End => segment_has_notes = false,
            _ => {}
        }
    }
    for index in orphans {
        remove[index] = true;
    }
    // The removal positions ascend; the k-th removal sits at `index - k`
    // while the earlier ones are gone, which is the position the patch layer
    // checks.
    let indices: Vec<usize> = remove
        .iter()
        .enumerate()
        .filter_map(|(index, flag)| flag.then_some(index))
        .collect();
    let ops = indices
        .iter()
        .enumerate()
        .map(|(k, &index)| {
            let position = index - k;
            // The carried row carries the id of the position it is removed
            // from: the earlier removals of the same patch have renumbered
            // the rows already.
            let mut carried = doc.rows[index].clone();
            carried.id = RowId(position as u32);
            RowOp::Remove {
                id: RowId(position as u32),
                row: carried,
                after: if position == 0 {
                    None
                } else {
                    Some(RowId(position as u32 - 1))
                },
            }
        })
        .collect();
    Ok(Patch {
        label: "Delete notes".to_owned(),
        ops,
    })
}

/// split_note cuts one note at `at_beat` into two: the left part keeps the
/// text, the right part gets the `"~"` placeholder, and the total length is
/// kept.
pub fn split_note(
    doc: &Document,
    song: &Song,
    id: RowId,
    at_beat: i64,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let (index, note) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    let left_length = at_beat - note.start;
    let right_length = note.start + note.length - at_beat;
    if left_length < 1 || right_length < 1 {
        return Err(EditError::LengthBelowOne);
    }
    let left = edited(doc, index, |kind| {
        if let RowKind::Note(left) = kind {
            left.length = left_length;
        }
    });
    let right = new_row(
        index + 1,
        RowKind::Note(NoteRow {
            kind: note.kind.clone(),
            start: at_beat,
            length: right_length,
            pitch: note.pitch,
            text: "~".to_owned(),
        }),
    );
    Ok(Patch {
        label: "Split note".to_owned(),
        ops: vec![
            left,
            RowOp::Insert {
                after: Some(id),
                row: right,
            },
        ],
    })
}

/// merge_with_next joins one note with the note after it: the text joins with
/// the right note's leading `"~"` dropped, the length reaches the right
/// note's end. A phrase end, a voice change or the end row between the notes
/// refuses the merge.
pub fn merge_with_next(doc: &Document, song: &Song, id: RowId) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let (index, note) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    let next_index = index + 1;
    let next = match doc.rows.get(next_index) {
        Some(row) => match &row.kind {
            RowKind::Note(next) => next.clone(),
            _ => return Err(EditError::PhraseEndBetween),
        },
        None => return Err(EditError::PhraseEndBetween),
    };
    let right_text = next.text.strip_prefix('~').unwrap_or(&next.text);
    let merged_text = format!("{}{}", note.text, right_text);
    let merged_length = next.start + next.length - note.start;
    let replace = edited(doc, index, |kind| {
        if let RowKind::Note(merged) = kind {
            merged.length = merged_length;
            merged.text = merged_text.clone();
        }
    });
    let remove = RowOp::Remove {
        id: RowId(next_index as u32),
        row: doc.rows[next_index].clone(),
        after: Some(id),
    };
    Ok(Patch {
        label: "Merge notes".to_owned(),
        ops: vec![replace, remove],
    })
}

/// move_phrase_end moves one end-of-phrase marker. It stays between the end
/// of the previous note and the start of the next one, so no note is cut and
/// no line-break gap opens.
pub fn move_phrase_end(
    doc: &Document,
    song: &Song,
    id: RowId,
    beat: i64,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let index = phrase_end_index(doc, id)?;
    let previous_end = doc.rows[..index]
        .iter()
        .rev()
        .find_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.start + note.length),
            _ => None,
        });
    let next_start = doc.rows[index + 1..]
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.start),
            _ => None,
        });
    let above_previous = previous_end.is_none_or(|end| beat >= end);
    let below_next = next_start.is_none_or(|start| beat <= start);
    if !above_previous || !below_next {
        return Err(EditError::PhraseEndOutOfRange);
    }
    let op = edited(doc, index, |kind| {
        if let RowKind::PhraseEnd { beat: marker, .. } = kind {
            *marker = Some(beat);
        }
    });
    Ok(Patch {
        label: "Move phrase end".to_owned(),
        ops: vec![op],
    })
}

/// toggle_phrase_end_before inserts an end-of-phrase marker before the note,
/// taking the end beat of the previous note (the convention in UltraSinger
/// files), or removes the marker that stands there already.
pub fn toggle_phrase_end_before(
    doc: &Document,
    song: &Song,
    id: RowId,
) -> Result<Patch, EditError> {
    require_absolute(song)?;
    let (index, _) = note_data(doc, id).ok_or(EditError::UnknownRow(id))?;
    // Toggle off: the previous row is the phrase end that closes this line.
    if index > 0 && matches!(doc.rows[index - 1].kind, RowKind::PhraseEnd { .. }) {
        let previous = index - 1;
        let remove = RowOp::Remove {
            id: RowId(previous as u32),
            row: doc.rows[previous].clone(),
            after: if previous == 0 {
                None
            } else {
                Some(RowId(previous as u32 - 1))
            },
        };
        return Ok(Patch {
            label: "Toggle phrase end".to_owned(),
            ops: vec![remove],
        });
    }
    // Toggle on: without a previous note there is no line to break and the
    // marker would stand before the first note.
    let previous_end = doc.rows[..index]
        .iter()
        .rev()
        .find_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.start + note.length),
            _ => None,
        });
    let Some(end) = previous_end else {
        return Err(EditError::PhraseEndOutOfRange);
    };
    let insert = RowOp::Insert {
        after: if index == 0 {
            None
        } else {
            Some(RowId(index as u32 - 1))
        },
        row: new_row(
            index,
            RowKind::PhraseEnd {
                beat: Some(end),
                rel_offset: None,
            },
        ),
    };
    Ok(Patch {
        label: "Toggle phrase end".to_owned(),
        ops: vec![insert],
    })
}

/// phrase_end_index returns the row index of the phrase end `id`, or
/// `UnknownRow` when the row does not exist or is not a phrase end.
fn phrase_end_index(doc: &Document, id: RowId) -> Result<usize, EditError> {
    let index = id.0 as usize;
    if doc
        .rows
        .get(index)
        .is_some_and(|row| matches!(row.kind, RowKind::PhraseEnd { .. }))
    {
        Ok(index)
    } else {
        Err(EditError::UnknownRow(id))
    }
}
