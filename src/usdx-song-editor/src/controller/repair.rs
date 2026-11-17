//! The repair controller (plan section 8.2, task P7-07).
//!
//! About me: the controller holds the state of the repair mode the
//! Repair QObject mirrors: the mode (anchor, block, tap), the anchors
//! pinned in this session with their target times, the end pin, the
//! block range and times with the follow behaviour, and the tap session
//! of the tap mode. `preview` computes the patch the UI ghosts;
//! `apply` commits exactly that computation to the loaded song as one
//! history step (so preview equals what apply commits) and rebuilds the
//! song view; `cancel` clears the pending preview and leaves the
//! document unchanged; `export_entries` builds the anchors.txt content
//! (decision D-12) from the applied changes, the pinned anchors and the
//! end pin. The planning itself lives in `usdx-edit` (anchor stretch,
//! block transform, tap mapping, anchors export); the controller only
//! holds the state and forwards. It is a plain struct: its methods take
//! the `LoadedSong` (or the document and song) as parameters, the
//! bridge forwards from the QObject invokables.

use std::ops::Range;

use usdx_edit::anchors_export::{AnchorEntry, PinnedAnchor, build_entries};
use usdx_edit::block::{Follow, block_transform};
use usdx_edit::patch::{EditError, Patch};
use usdx_edit::retime::{Anchor, Lengths, Outside, Pin, StretchOptions, anchor_stretch};
use usdx_format::rows::Document;
use usdx_format::song::Song;

use super::editor::LoadedSong;
use super::tap::{TapMode, TapSession};

/// RepairMode is the repair page's mode (the Repair QObject's mode
/// enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairMode {
    /// Anchor and stretch (plan 6.7).
    Anchor,
    /// Block move and scale (plan 6.8).
    Block,
    /// Tap-along (plan 6.9, UI_SPEC 4.8).
    Tap,
}

/// RepairController holds the repair mode's pending state.
pub struct RepairController {
    mode: RepairMode,
    /// The voice the repair acts on, updated with every preview, apply
    /// and tap session; the export maps the anchors' voice line indexes
    /// through it.
    voice: u8,
    /// The anchors pinned in this session, in pin order.
    anchors: Vec<Anchor>,
    /// What the mapped lines' note lengths do (UI_SPEC 4.6, the panel's
    /// "Note lengths" toggle; Keep is the default, user decision
    /// 2026-11-17).
    lengths: Lengths,
    /// The end pin in audio milliseconds, `None` while unset.
    end_pin_ms: Option<f64>,
    /// The block's line range (half open), set by setBlock.
    block: Option<Range<usize>>,
    /// The block's new start and end in audio milliseconds.
    block_times: (f64, f64),
    /// What the lines after the block do.
    follow: Follow,
    /// The tap session of the tap mode.
    tap: Option<TapSession>,
    /// The previewed patch the UI ghosts, until apply or cancel.
    preview: Option<Patch>,
}

impl Default for RepairController {
    fn default() -> Self {
        Self::new()
    }
}

impl RepairController {
    /// new builds an empty controller in the anchor mode.
    pub fn new() -> RepairController {
        RepairController {
            mode: RepairMode::Anchor,
            voice: 0,
            anchors: Vec::new(),
            lengths: Lengths::Keep,
            end_pin_ms: None,
            block: None,
            block_times: (0.0, 0.0),
            follow: Follow::MoveAlong,
            tap: None,
            preview: None,
        }
    }

    /// mode returns the current repair mode.
    pub fn mode(&self) -> RepairMode {
        self.mode
    }

    /// set_mode switches the repair mode and clears the pending
    /// preview.
    pub fn set_mode(&mut self, mode: RepairMode) {
        self.mode = mode;
        self.preview = None;
    }

    /// anchors returns the pinned anchors in pin order.
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// note_lengths returns what the mapped lines' note lengths do.
    pub fn note_lengths(&self) -> Lengths {
        self.lengths
    }

    /// set_note_lengths switches the panel's Note lengths toggle and
    /// clears the pending preview.
    pub fn set_note_lengths(&mut self, lengths: Lengths) {
        self.lengths = lengths;
        self.preview = None;
    }

    /// voice returns the voice the repair acts on; the export maps the
    /// anchors' voice line indexes through it.
    pub fn voice(&self) -> u8 {
        self.voice
    }

    /// pin_current_line_at pins `line`'s first note start to `ms`: an
    /// existing anchor of the line updates its target, otherwise an
    /// anchor is added.
    pub fn pin_current_line_at(&mut self, line: usize, ms: f64) {
        match self.anchors.iter_mut().find(|anchor| anchor.line == line) {
            Some(anchor) => anchor.target_ms = ms,
            None => self.anchors.push(Anchor {
                line,
                target_ms: ms,
                pin: Pin::Start,
            }),
        }
        self.preview = None;
    }

    /// set_anchor_ms changes the target time of the anchor at `index`.
    pub fn set_anchor_ms(&mut self, index: usize, ms: f64) {
        if let Some(anchor) = self.anchors.get_mut(index) {
            anchor.target_ms = ms;
        }
        self.preview = None;
    }

    /// remove_anchor removes the anchor at `index`.
    pub fn remove_anchor(&mut self, index: usize) {
        if index < self.anchors.len() {
            self.anchors.remove(index);
        }
        self.preview = None;
    }

    /// end_pin returns the end pin in audio milliseconds.
    pub fn end_pin(&self) -> Option<f64> {
        self.end_pin_ms
    }

    /// set_end_pin sets or clears the end pin.
    pub fn set_end_pin(&mut self, ms: Option<f64>) {
        self.end_pin_ms = ms;
        self.preview = None;
    }

    /// set_block sets the block's line range (first and last line,
    /// inclusive).
    pub fn set_block(&mut self, first: usize, last: usize) {
        self.block = Some(first..last + 1);
        self.preview = None;
    }

    /// set_block_times sets the block's new start and end in audio
    /// milliseconds.
    pub fn set_block_times(&mut self, start_ms: f64, end_ms: f64) {
        self.block_times = (start_ms, end_ms);
        self.preview = None;
    }

    /// set_follow sets what the lines after the block do.
    pub fn set_follow(&mut self, follow: Follow) {
        self.follow = follow;
        self.preview = None;
    }

    /// start_tap_session opens the tap mode's session over
    /// `first_line..=last_line`. The arguments mirror the plan's
    /// `startSession(firstLine, lastLine, mode, rate)` invokable plus
    /// the latency and the pre-roll, so they stay one call.
    #[allow(clippy::too_many_arguments)]
    pub fn start_tap_session(
        &mut self,
        doc: &Document,
        song: &Song,
        voice: u8,
        first_line: usize,
        last_line: usize,
        mode: TapMode,
        rate: f64,
        latency_ms: f64,
        preroll_ms: f64,
    ) -> Result<(), EditError> {
        self.voice = voice;
        self.tap = Some(TapSession::new(
            doc, song, voice, first_line, last_line, mode, rate, latency_ms, preroll_ms,
        )?);
        self.preview = None;
        Ok(())
    }

    /// tap_session returns the tap mode's session while one runs.
    pub fn tap_session(&self) -> Option<&TapSession> {
        self.tap.as_ref()
    }

    /// tap_session_mut returns the tap session for the bridge's key and
    /// playback handling.
    pub fn tap_session_mut(&mut self) -> Option<&mut TapSession> {
        self.tap.as_mut()
    }

    /// preview_patch returns the patch the UI ghosts, until apply or
    /// cancel.
    pub fn preview_patch(&self) -> Option<&Patch> {
        self.preview.as_ref()
    }

    /// compute_patch plans the patch of the current mode against the
    /// passed document and song.
    fn compute_patch(&self, doc: &Document, song: &Song, voice: u8) -> Result<Patch, EditError> {
        match self.mode {
            RepairMode::Anchor => {
                let options = StretchOptions {
                    before: Outside::Keep,
                    after: Outside::Keep,
                    lengths: self.lengths,
                    end_pin_ms: self.end_pin_ms,
                };
                anchor_stretch(doc, song, voice, &self.anchors, &options)
            }
            RepairMode::Block => {
                let range = self
                    .block
                    .clone()
                    .ok_or(EditError::StalePatch("the block has no lines yet"))?;
                let (start_ms, end_ms) = self.block_times;
                block_transform(doc, song, voice, range, start_ms, end_ms, self.follow)
            }
            RepairMode::Tap => {
                let session = self
                    .tap
                    .as_ref()
                    .ok_or(EditError::StalePatch("the tap mode has no session"))?;
                session.use_taps(doc, song, voice)
            }
        }
    }

    /// preview computes the patch the UI ghosts, stores it and returns
    /// it. A stale state (an anchor of a line the voice lost) is
    /// refused with the planner's error.
    pub fn preview(&mut self, doc: &Document, song: &Song, voice: u8) -> Result<&Patch, EditError> {
        self.voice = voice;
        let patch = self.compute_patch(doc, song, voice)?;
        self.preview = Some(patch);
        Ok(self.preview.as_ref().expect("the patch was just stored"))
    }

    /// apply commits the current mode's patch (the same computation the
    /// preview shows) to the loaded song as one history step and
    /// rebuilds the song view. The anchors, the end pin and the block
    /// stay for further edits; the pending preview is cleared.
    pub fn apply(&mut self, loaded: &mut LoadedSong, voice: u8) -> Result<(), EditError> {
        self.voice = voice;
        let patch = self.compute_patch(&loaded.document, &loaded.song, voice)?;
        loaded.history.commit(&mut loaded.document, patch)?;
        loaded.song = Song::from_document(&loaded.document);
        self.preview = None;
        Ok(())
    }

    /// cancel clears the pending preview; the document stays unchanged.
    pub fn cancel(&mut self) {
        self.preview = None;
    }

    /// has_preview says whether a preview is pending.
    pub fn has_preview(&self) -> bool {
        self.preview.is_some()
    }

    /// export_entries builds the anchors.txt content (decision D-12):
    /// the voice-P1 lines whose first note's start time differs from the
    /// load snapshot, plus the pinned anchors (a pin wins over a change)
    /// and the end pin. The anchors' voice line indexes map to the flat
    /// voice-P1 indexes the export compares; a voice other than P1
    /// exports no pins.
    pub fn export_entries(&self, loaded: &LoadedSong) -> (Vec<AnchorEntry>, Option<f64>) {
        let pinned: Vec<PinnedAnchor> = self
            .anchors
            .iter()
            .filter_map(|anchor| {
                flat_p1_line_index(&loaded.song, self.voice, anchor.line).map(|line| PinnedAnchor {
                    line,
                    time_ms: anchor.target_ms,
                })
            })
            .collect();
        let entries = build_entries(&loaded.document, &loaded.song, &loaded.changes, &pinned);
        (entries, self.end_pin_ms)
    }
}

/// flat_p1_line_index maps a voice's line index to the flat index over
/// all voice-P1 lines, which the anchors export compares. `None` for a
/// voice that is not numbered 1 or a line the voice does not have. The
/// bridge's export preview reuses it for the pinned-count breakdown.
pub(crate) fn flat_p1_line_index(song: &Song, voice: u8, line: usize) -> Option<usize> {
    let voice_index = usize::from(voice);
    let mut flat = 0;
    for (index, voice_data) in song.voices.iter().enumerate() {
        if index == voice_index {
            return if voice_data.number == 1 && line < voice_data.lines.len() {
                Some(flat + line)
            } else {
                None
            };
        }
        if voice_data.number == 1 {
            flat += voice_data.lines.len();
        }
    }
    None
}
