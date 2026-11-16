//! The tap-along session controller (plan section 8.2, UI_SPEC 4.8,
//! task P7-07).
//!
//! About me: a TapSession collects the Space key presses of one
//! tap-along run over a line range of one voice. A key event carries the
//! player's playback position in audio milliseconds; the session
//! subtracts the latency, so a tap records the time the user heard. The
//! first Space press starts the pre-roll (the settings' pre-roll before
//! the first line, the playback seek target the UI plays); recording
//! begins when the playback reaches the first target and the session
//! finishes when the playback passes the last target. Line mode taps
//! once per line (the line moves rigidly onto the tap, `usdx-edit`'s
//! `apply_line_taps`), syllable mode once per note
//! (`apply_syllable_taps`). Auto-repeat key events are ignored;
//! Backspace undoes the last tap; `use_taps` builds the same patch the
//! mapping functions plan. The session is a plain struct: the bridge
//! forwards the player position and the key events, the repair
//! controller's tap mode holds the session and previews its result.

use std::ops::Range;

use usdx_edit::patch::{EditError, Patch};
use usdx_edit::tap::{Tap, apply_line_taps, apply_syllable_taps};
use usdx_format::rows::{Document, RowId};
use usdx_format::song::Song;

/// TapMode says what one tap pins: the line start or one syllable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapMode {
    /// One tap per line; the line moves rigidly onto the tap.
    Line,
    /// One tap per note of the range's lines.
    Syllable,
}

/// TapState is the session state shown in the status line (UI_SPEC 4.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapState {
    /// Before the first Space press.
    Ready,
    /// The pre-roll plays; presses record nothing yet.
    PreRoll,
    /// The playback is inside the range; presses record taps.
    Recording,
    /// The playback passed the last target; the session is done.
    Finished,
}

/// TapSession holds the state of one tap-along run: the targets (start
/// and end in audio milliseconds) of the tapped lines or notes, the taps
/// collected so far and the session state.
pub struct TapSession {
    mode: TapMode,
    /// The playback rate the UI plays the session at.
    rate: f64,
    latency_ms: f64,
    /// The tapped lines of the voice (half open), for the line mode's
    /// patch.
    lines: Range<usize>,
    /// The tapped note rows, for the syllable mode's patch.
    notes: Vec<RowId>,
    /// The pre-roll before the first target in audio milliseconds, from
    /// the settings (UI_SPEC 4.10: 0.0-10.0 s).
    preroll_ms: f64,
    /// One (start, end) pair in audio milliseconds per tap: the line
    /// span in line mode, the note span in syllable mode.
    targets: Vec<(f64, f64)>,
    state: TapState,
    taps: Vec<Tap>,
}

impl TapSession {
    /// new builds a session over `first_line..=last_line` of `voice`.
    /// The targets come from the song's current times; a song without a
    /// usable tempo is refused. The pre-roll comes from the settings
    /// (UI_SPEC 4.10); the arguments mirror the plan's
    /// `startSession(firstLine, lastLine, mode, rate)` invokable plus
    /// the voice, the latency and the pre-roll, so they stay one call.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        doc: &Document,
        song: &Song,
        voice: u8,
        first_line: usize,
        last_line: usize,
        mode: TapMode,
        rate: f64,
        latency_ms: f64,
        preroll_ms: f64,
    ) -> Result<TapSession, EditError> {
        if song.timing.is_none() {
            return Err(EditError::NoTiming);
        }
        let timing = song.timing.expect("checked above");
        let lines_range = first_line..last_line + 1;
        let mut targets = Vec::new();
        let mut notes = Vec::new();
        if let Some(voice_data) = song.voices.get(usize::from(voice)) {
            for line in voice_data.lines.iter().take(last_line + 1).skip(first_line) {
                let line_notes: Vec<_> = line
                    .notes
                    .iter()
                    .filter_map(|id| song.note(doc, *id).map(|note| (*id, note)))
                    .collect();
                match mode {
                    TapMode::Line => {
                        if let (Some(first), Some(last)) = (line_notes.first(), line_notes.last()) {
                            targets.push((
                                timing.beat_to_ms(first.1.start),
                                timing.beat_to_ms(last.1.start + last.1.length as f64),
                            ));
                        }
                    }
                    TapMode::Syllable => {
                        for (id, note) in line_notes {
                            targets.push((
                                timing.beat_to_ms(note.start),
                                timing.beat_to_ms(note.start + note.length as f64),
                            ));
                            notes.push(id);
                        }
                    }
                }
            }
        }
        Ok(TapSession {
            mode,
            rate,
            latency_ms,
            preroll_ms,
            lines: lines_range,
            notes,
            targets,
            state: TapState::Ready,
            taps: Vec::new(),
        })
    }

    /// The session's tap mode.
    pub fn mode(&self) -> TapMode {
        self.mode
    }

    /// The playback rate the UI plays the session at.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The key latency in milliseconds, subtracted from every key time.
    pub fn latency_ms(&self) -> f64 {
        self.latency_ms
    }

    /// state returns the session state for the status line.
    pub fn state(&self) -> TapState {
        self.state
    }

    /// pre_roll_start returns the playback seek target: the settings'
    /// pre-roll before the first target, so the user can settle into the
    /// tempo. `None` without a target.
    pub fn pre_roll_start(&self) -> Option<f64> {
        self.targets
            .first()
            .map(|&(start, _)| start - self.preroll_ms)
    }

    /// start begins the session: the first Space press starts the
    /// pre-roll and records nothing.
    pub fn start(&mut self) {
        if self.state == TapState::Ready {
            self.state = TapState::PreRoll;
        }
    }

    /// tick feeds the player's playback position in audio milliseconds:
    /// the pre-roll turns into recording at the first target and the
    /// session finishes past the last target's end.
    pub fn tick(&mut self, position: f64) {
        match self.state {
            TapState::PreRoll
                if self
                    .targets
                    .first()
                    .is_some_and(|&(start, _)| position >= start) =>
            {
                self.state = TapState::Recording;
            }
            TapState::Recording if self.targets.last().is_some_and(|&(_, end)| position > end) => {
                self.state = TapState::Finished;
            }
            _ => {}
        }
    }

    /// key_down records a Space press at the playback position. An
    /// auto-repeat event is ignored, and only a press during the
    /// recording records a tap (the time the user heard: the position
    /// minus the latency).
    pub fn key_down(&mut self, position: f64, auto_repeat: bool) {
        if auto_repeat || self.state != TapState::Recording {
            return;
        }
        self.taps.push(Tap {
            down_ms: position - self.latency_ms,
            up_ms: None,
        });
    }

    /// key_up records the release of a held press.
    pub fn key_up(&mut self, position: f64) {
        if let Some(last) = self.taps.last_mut()
            && last.up_ms.is_none()
        {
            last.up_ms = Some(position - self.latency_ms);
        }
    }

    /// undo_tap takes the last tap back; undoing past the first tap is
    /// safe.
    pub fn undo_tap(&mut self) {
        self.taps.pop();
    }

    /// stop ends the session.
    pub fn stop(&mut self) {
        self.state = TapState::Finished;
    }

    /// taps returns the taps collected so far, in press order.
    pub fn taps(&self) -> &[Tap] {
        &self.taps
    }

    /// tap_count returns how many taps were recorded.
    pub fn tap_count(&self) -> usize {
        self.taps.len()
    }

    /// expected_count returns how many taps the range wants: one per
    /// line in line mode, one per note in syllable mode.
    pub fn expected_count(&self) -> usize {
        self.targets.len()
    }

    /// current_syllable returns the index of the next target to tap; it
    /// clamps at the expected count when the taps run past it.
    pub fn current_syllable(&self) -> usize {
        self.taps.len().min(self.targets.len())
    }

    /// use_taps builds the patch the mapping functions plan from the
    /// collected taps: the line mode's lines move rigidly onto their
    /// taps, the syllable mode's notes onto theirs.
    pub fn use_taps(&self, doc: &Document, song: &Song, voice: u8) -> Result<Patch, EditError> {
        match self.mode {
            TapMode::Line => apply_line_taps(doc, song, voice, self.lines.clone(), &self.taps),
            TapMode::Syllable => apply_syllable_taps(doc, song, voice, &self.notes, &self.taps),
        }
    }
}
