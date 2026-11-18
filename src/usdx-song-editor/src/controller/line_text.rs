//! The line text controller (plan 8.3, UI_SPEC 3.6 and 5.4, task P6-02).
//!
//! About me: this module holds the editor side of the text editing. The
//! line text editor in the lyrics lane shows the current line in the line text syntax
//! (`line_text`), previews every keystroke as syllable chips on the notes
//! with their states, the count message of UI_SPEC 5.4 item 4 and the note
//! picks of "Ask me per note" (`preview_line_text`, handed to QML as JSON by
//! `preview_json`), and applies the text as one undo step
//! (`apply_line_text`). The inline syllable edit over a lyric label (user
//! decision 2026-11-15) shows a note's syllable without its word space
//! (`syllable_for_editing`) and keeps that space on commit
//! (`set_note_syllable`); `note_at_lyric_beat` finds the note whose label a
//! double click hits. The plans come from usdx-edit's text module, so the
//! preview shows exactly what Apply commits.

use usdx_edit::text::{
    CountChange, CountFix, NoteStatus, TextPlan, line_notes, line_text_for_editing,
    parse_line_text, plan_line_text, syllables_text, text_change_patch,
};
use usdx_edit::{EditError, ops};
use usdx_format::rows::{NoteRow, RowId, RowKind};

use crate::controller::editor::EditorController;
use crate::controller::shell::edit_failure_reason;

/// How a chip's note relates to the line before the edit (UI_SPEC 5.4
/// item 3: a changed syllable and a new note have their own colours).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipState {
    Same,
    Changed,
    New,
}

/// One syllable chip: the note's text without its spaces, the note's start
/// beat and its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub text: String,
    pub beat: i64,
    pub state: ChipState,
}

/// One entry of a count change's note pick ("Ask me per note"): the line
/// note index and its label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickOption {
    pub note: usize,
    pub label: String,
}

/// One count change with the notes the user may pick and the picked one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeChoice {
    pub kind: CountFix,
    pub options: Vec<PickOption>,
    pub chosen: usize,
}

/// The line text editor's preview of a line text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineTextPreview {
    pub chips: Vec<Chip>,
    pub changes: Vec<ChangeChoice>,
    pub syllable_count: usize,
    pub note_count: usize,
    /// The count message; empty when no count changes.
    pub message: String,
    /// Why the text cannot apply; empty when it can.
    pub error: String,
    /// Whether the text changes any note.
    pub changed: bool,
    pub can_apply: bool,
}

/// count_word writes small counts as words, as the message of UI_SPEC 5.4
/// does ("three syllables on two notes"), larger ones as digits.
fn count_word(count: usize) -> String {
    const WORDS: [&str; 13] = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    WORDS
        .get(count)
        .map_or_else(|| count.to_string(), |word| (*word).to_owned())
}

/// counted writes "<count> <noun>" with the plural "s" when needed.
fn counted(count: String, singular: bool, noun: &str) -> String {
    format!("{count} {noun}{}", if singular { "" } else { "s" })
}

/// listed joins items as "a", "a and b" or "a, b and c".
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// quoted writes a note's text without its spaces in quotes.
fn quoted(note: &NoteRow) -> String {
    format!("\"{}\"", note.text.trim())
}

/// beats writes a beat count with its unit.
fn beats(count: i64) -> String {
    counted(count.to_string(), count == 1, "beat")
}

/// change_sentences explains one count change (UI_SPEC 5.4 item 4).
fn change_sentences(
    change: &CountChange,
    notes: &[NoteRow],
    plan: &TextPlan,
    syllables: &[usdx_edit::text::Syllable],
) -> String {
    let region = syllables_text(&syllables[change.syllables.clone()]);
    let first = format!(
        "\"{region}\" now has {} on {}.",
        counted(
            count_word(change.syllables.len()),
            change.syllables.len() == 1,
            "syllable"
        ),
        counted(
            count_word(change.notes.len()),
            change.notes.len() == 1,
            "note"
        ),
    );
    let chosen = &notes[change.chosen];
    let second = match change.kind {
        CountFix::Split => {
            let lengths: Vec<String> = plan
                .notes
                .iter()
                .filter(|planned| planned.source == change.chosen)
                .map(|planned| planned.note.length.to_string())
                .collect();
            format!(
                "The editor will split {} ({}) into {} notes of {} beats at the same pitch.",
                quoted(chosen),
                beats(chosen.length),
                count_word(lengths.len()),
                listed(&lengths),
            )
        }
        CountFix::Merge => {
            let run = change.notes.len() - change.syllables.len() + 1;
            let names: Vec<String> = notes[change.chosen..change.chosen + run]
                .iter()
                .map(quoted)
                .collect();
            format!(
                "The editor will merge {} ({}) into one note.",
                listed(&names),
                beats(merged_span(notes, change.chosen, run)),
            )
        }
    };
    format!("{first} {second}")
}

/// merged_span is the length of the note a run of `run` notes from `first`
/// merges into.
fn merged_span(notes: &[NoteRow], first: usize, run: usize) -> i64 {
    let last = &notes[first + run - 1];
    last.start + last.length - notes[first].start
}

/// pick_options labels the candidates of a count change: a split names the
/// note and its length, a merge the run of notes and its span.
fn pick_options(change: &CountChange, notes: &[NoteRow]) -> Vec<PickOption> {
    change
        .candidates
        .iter()
        .map(|&note| {
            let label = match change.kind {
                CountFix::Split => {
                    format!("{} ({})", quoted(&notes[note]), beats(notes[note].length))
                }
                CountFix::Merge => {
                    let run = change.notes.len() - change.syllables.len() + 1;
                    let names: Vec<String> = notes[note..note + run].iter().map(quoted).collect();
                    format!(
                        "{} ({})",
                        names.join(" + "),
                        beats(merged_span(notes, note, run))
                    )
                }
            };
            PickOption { note, label }
        })
        .collect()
}

impl EditorController {
    /// line_text returns the line's text in the line text syntax for the
    /// line text editor's field; `None` past the last line.
    pub fn line_text(&self, line: usize) -> Option<String> {
        let loaded = self.loaded()?;
        let notes = line_notes(&loaded.document, &self.line_data(line)?.notes).ok()?;
        Some(line_text_for_editing(&notes))
    }

    /// preview_line_text plans the text for the line and describes the plan
    /// for the dialog. A text that cannot apply carries its reason and no
    /// chips.
    pub fn preview_line_text(
        &self,
        line: usize,
        text: &str,
        picks: &[Option<usize>],
    ) -> LineTextPreview {
        let Some((notes, offset)) = self.line_notes_with_offset(line) else {
            return LineTextPreview::default();
        };
        let syllables = parse_line_text(text);
        let mut preview = LineTextPreview {
            syllable_count: syllables.len(),
            note_count: notes.len(),
            ..LineTextPreview::default()
        };
        let plan = match plan_line_text(&notes, &syllables, picks) {
            Ok(plan) => plan,
            Err(error) => {
                preview.error = edit_failure_reason(&error);
                return preview;
            }
        };
        preview.chips = plan
            .notes
            .iter()
            .map(|planned| Chip {
                text: planned.note.text.trim().to_owned(),
                beat: planned.note.start + offset,
                state: match planned.status {
                    NoteStatus::Unchanged => ChipState::Same,
                    NoteStatus::Changed => ChipState::Changed,
                    NoteStatus::New => ChipState::New,
                },
            })
            .collect();
        preview.changed = plan.notes.len() != notes.len()
            || plan
                .notes
                .iter()
                .any(|planned| planned.status != NoteStatus::Unchanged);
        preview.changes = plan
            .changes
            .iter()
            .map(|change| ChangeChoice {
                kind: change.kind,
                options: pick_options(change, &notes),
                chosen: change.chosen,
            })
            .collect();
        if !plan.changes.is_empty() {
            let mut sentences = vec![format!(
                "{} for {}.",
                counted(
                    syllables.len().to_string(),
                    syllables.len() == 1,
                    "syllable"
                ),
                counted(notes.len().to_string(), notes.len() == 1, "note"),
            )];
            sentences.extend(
                plan.changes
                    .iter()
                    .map(|change| change_sentences(change, &notes, &plan, &syllables)),
            );
            preview.message = sentences.join(" ");
        }
        // A count change edits beats, which a relative-mode song refuses.
        if !plan.changes.is_empty() && self.loaded().is_some_and(|loaded| loaded.song.relative) {
            preview.error = edit_failure_reason(&EditError::RelativeModeIsReadOnly);
        }
        preview.can_apply = preview.error.is_empty();
        preview
    }

    /// apply_line_text applies the text to the line as one undo step and
    /// tells whether anything changed; an unchanged text adds no step. A
    /// count change shifts the rows behind the line, so it clears the
    /// selection.
    pub fn apply_line_text(
        &mut self,
        line: usize,
        text: &str,
        picks: &[Option<usize>],
    ) -> Result<bool, EditError> {
        let (patch, counts_changed) = {
            let loaded = self
                .loaded()
                .ok_or(EditError::StalePatch("no song is loaded"))?;
            let ids = self
                .line_data(line)
                .ok_or(EditError::StalePatch("the line does not exist"))?
                .notes
                .clone();
            let notes = line_notes(&loaded.document, &ids)?;
            let plan = plan_line_text(&notes, &parse_line_text(text), picks)?;
            let patch = text_change_patch(&loaded.document, &loaded.song, &ids, &plan)?;
            (patch, !plan.changes.is_empty())
        };
        if patch.ops.is_empty() {
            return Ok(false);
        }
        self.commit(patch)?;
        if counts_changed {
            self.selection.clear();
            self.anchor = None;
        }
        Ok(true)
    }

    /// syllable_for_editing returns the note's text without its surrounding
    /// spaces for the inline field; `None` when the row is not a note.
    pub fn syllable_for_editing(&self, id: RowId) -> Option<String> {
        match &self.loaded()?.document.rows.get(id.0 as usize)?.kind {
            RowKind::Note(note) => Some(note.text.trim().to_owned()),
            _ => None,
        }
    }

    /// set_note_syllable commits the inline field's text: the typed text
    /// without its surrounding spaces takes the place of the note's text
    /// between the note's own leading and trailing spaces, so the word
    /// boundary stays. An empty or unchanged text adds no step.
    pub fn set_note_syllable(&mut self, id: RowId, typed: &str) -> Result<bool, EditError> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Ok(false);
        }
        let patch = {
            let loaded = self
                .loaded()
                .ok_or(EditError::StalePatch("no song is loaded"))?;
            let Some(RowKind::Note(note)) =
                loaded.document.rows.get(id.0 as usize).map(|row| &row.kind)
            else {
                return Err(EditError::UnknownRow(id));
            };
            let old = note.text.as_str();
            let lead = &old[..old.len() - old.trim_start().len()];
            let trail = if old.trim().is_empty() {
                ""
            } else {
                &old[old.trim_end().len()..]
            };
            let text = format!("{lead}{typed}{trail}");
            if text == old {
                return Ok(false);
            }
            ops::set_text(&loaded.document, &loaded.song, id, &text)?
        };
        self.commit(patch)?;
        Ok(true)
    }

    /// note_at_lyric_beat returns the note of the current voice whose lyric
    /// label covers the beat: the last note starting at or before it.
    pub fn note_at_lyric_beat(&self, beat: f64) -> Option<RowId> {
        let loaded = self.loaded()?;
        let voice = loaded.song.voices.get(self.current_voice_index()?)?;
        voice
            .lines
            .iter()
            .flat_map(|line| line.notes.iter())
            .filter_map(|id| loaded.song.note(&loaded.document, *id))
            .take_while(|note| note.start <= beat)
            .last()
            .map(|note| note.row)
    }

    /// line_notes_with_offset returns the line's note rows and the beat
    /// offset from their stored starts to the absolute starts (non-zero in
    /// relative mode).
    fn line_notes_with_offset(&self, line: usize) -> Option<(Vec<NoteRow>, i64)> {
        let loaded = self.loaded()?;
        let ids = &self.line_data(line)?.notes;
        let notes = line_notes(&loaded.document, ids).ok()?;
        let offset = match (ids.first(), notes.first()) {
            (Some(id), Some(note)) => loaded
                .song
                .note(&loaded.document, *id)
                .map_or(0, |absolute| absolute.start.round() as i64 - note.start),
            _ => 0,
        };
        Some((notes, offset))
    }
}

/// parse_picks reads the dialog's note picks: comma-separated line note
/// indices in change order, where a negative or unreadable entry means
/// "automatic".
pub fn parse_picks(text: &str) -> Vec<Option<usize>> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    text.split(',')
        .map(|entry| {
            entry
                .trim()
                .parse::<i64>()
                .ok()
                .and_then(|value| usize::try_from(value).ok())
        })
        .collect()
}

/// json_string writes a JSON string literal.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// preview_json writes the preview as the JSON object the dialog reads:
/// chips (text, beat, state "same", "changed" or "new"), changes (kind
/// "split" or "merge", options with note and label, chosen),
/// syllableCount, noteCount, message, error, changed and canApply.
pub fn preview_json(preview: &LineTextPreview) -> String {
    let chips: Vec<String> = preview
        .chips
        .iter()
        .map(|chip| {
            let state = match chip.state {
                ChipState::Same => "same",
                ChipState::Changed => "changed",
                ChipState::New => "new",
            };
            format!(
                "{{\"text\":{},\"beat\":{},\"state\":\"{state}\"}}",
                json_string(&chip.text),
                chip.beat
            )
        })
        .collect();
    let changes: Vec<String> = preview
        .changes
        .iter()
        .map(|change| {
            let kind = match change.kind {
                CountFix::Split => "split",
                CountFix::Merge => "merge",
            };
            let options: Vec<String> = change
                .options
                .iter()
                .map(|option| {
                    format!(
                        "{{\"note\":{},\"label\":{}}}",
                        option.note,
                        json_string(&option.label)
                    )
                })
                .collect();
            format!(
                "{{\"kind\":\"{kind}\",\"options\":[{}],\"chosen\":{}}}",
                options.join(","),
                change.chosen
            )
        })
        .collect();
    format!(
        "{{\"chips\":[{}],\"changes\":[{}],\"syllableCount\":{},\"noteCount\":{},\"message\":{},\"error\":{},\"changed\":{},\"canApply\":{}}}",
        chips.join(","),
        changes.join(","),
        preview.syllable_count,
        preview.note_count,
        json_string(&preview.message),
        json_string(&preview.error),
        preview.changed,
        preview.can_apply
    )
}
