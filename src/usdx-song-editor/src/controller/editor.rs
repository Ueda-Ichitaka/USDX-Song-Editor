//! The editor controller: load, navigate and (in later tasks) edit.
//!
//! About me: this controller holds the loaded document with its song view,
//! undo history and load-time change log, plus the navigation state (the
//! current line as a global index across all voices) and the selection. It
//! exists as the plain-Rust core of the editor: every user action lands here
//! as a method, the cxx-qt bridge forwards it, and it is testable without
//! Qt. Line navigation is global - the lines pane numbers all lines of all
//! voices from 1, so moving past the last line of one voice continues into
//! the next voice - and clamps at both ends. While the whole song plays, the
//! playback follow moves the current line to the line being sung within the
//! current voice (`line_at_ms`, `follow_playback`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use usdx_edit::History;
use usdx_edit::changes::ChangeLog;
use usdx_format::bytes::decode;
use usdx_format::headers::Version;
use usdx_format::rows::{Document, RowId, classify};
use usdx_format::serialize::serialize;
use usdx_format::song::{Line, Song};

use crate::controller::fingerprint::FileFingerprint;

/// The line padding of the autoplay play range in ms (plan 8.6: 0.3 s,
/// the same padding the Play line button uses).
pub const PLAY_PADDING_MS: f64 = 300.0;

/// The song a session has loaded: the document, its views, the undo history
/// and the file facts the save service needs.
pub struct LoadedSong {
    pub path: PathBuf,
    pub fingerprint: FileFingerprint,
    pub document: Document,
    pub song: Song,
    pub history: History,
    /// The change log taken at load, compared against the current song for
    /// the anchors export.
    pub changes: ChangeLog,
    /// The song loads read-only when the format version is not editable or
    /// the serializer self-check fails.
    pub read_only: bool,
}

/// The state of one editing session.
pub struct EditorController {
    /// The loaded session or `None` of the start screen; the editing
    /// methods in controller/editing.rs read and write it.
    pub(crate) loaded: Option<LoadedSong>,
    /// The current line as a global index across all voices.
    current_line: usize,
    /// The selected row ids; the editing methods in controller/editing.rs
    /// read and write it.
    pub(crate) selection: BTreeSet<RowId>,
    /// The anchor of a Shift-extend selection: the last note a click or a
    /// rubber band selected (task P5-04).
    pub(crate) anchor: Option<RowId>,
    /// Whether autoplay is on: every line reached is handed to the player
    /// as a padded play range (the Shift+A key, plan 8.6, task P5-06).
    autoplay: bool,
    /// The play range the next bridge refresh hands to the player, set
    /// when autoplay is switched on and whenever the line changes while it
    /// is on. Taken once.
    pending_play_range: Option<(f64, f64)>,
}

impl Default for EditorController {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorController {
    /// new builds the empty controller of the start screen.
    pub fn new() -> EditorController {
        EditorController {
            loaded: None,
            current_line: 0,
            selection: BTreeSet::new(),
            anchor: None,
            autoplay: false,
            pending_play_range: None,
        }
    }

    /// open loads the song at `path`. A failure leaves the previously loaded
    /// song in place, so the UI keeps showing what the user had. The failure
    /// texts are the UI_SPEC 6 open-failure texts, verbatim.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let bytes =
            fs::read(path).map_err(|error| format!("\"{name}\" could not be read: {error}."))?;
        let decoded =
            decode(&bytes).map_err(|error| format!("\"{name}\" could not be read: {error}."))?;
        let document = classify(decoded);
        let song = Song::from_document(&document);
        // Without a usable #BPM the note sheet cannot place notes in time,
        // so the file is refused as not a song (UI_SPEC 6).
        if song.timing.is_none() {
            return Err(format!(
                "\"{name}\" is not an UltraStar song: it has no #BPM header."
            ));
        }
        // The serializer self-check: a file we cannot write back unchanged
        // is shown read-only (UI_SPEC 4.5).
        let round_trip = serialize(&document)
            .map(|written| written == bytes)
            .unwrap_or(false);
        let read_only = !version_is_editable(&song.version) || !round_trip;
        let metadata = fs::metadata(path).ok();
        let fingerprint = FileFingerprint::new(
            metadata.as_ref().map(|meta| meta.len()).unwrap_or(0),
            metadata.and_then(|meta| meta.modified().ok()),
            &bytes,
        );
        let changes = ChangeLog::snapshot(&document, &song);
        self.loaded = Some(LoadedSong {
            path: path.to_owned(),
            fingerprint,
            document,
            song,
            history: History::new(),
            changes,
            read_only,
        });
        self.current_line = 0;
        self.selection.clear();
        self.anchor = None;
        self.autoplay = false;
        self.pending_play_range = None;
        Ok(())
    }

    /// is_loaded tells whether a song is open.
    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    /// loaded returns the loaded song.
    pub fn loaded(&self) -> Option<&LoadedSong> {
        self.loaded.as_ref()
    }

    /// loaded_mut returns the loaded song for editing and saving.
    pub fn loaded_mut(&mut self) -> Option<&mut LoadedSong> {
        self.loaded.as_mut()
    }

    /// path returns the path of the loaded song.
    pub fn path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|loaded| loaded.path.as_path())
    }

    /// line_count counts the lines of all voices; 0 without a loaded song.
    pub fn line_count(&self) -> usize {
        self.loaded
            .as_ref()
            .map(|loaded| {
                loaded
                    .song
                    .voices
                    .iter()
                    .map(|voice| voice.lines.len())
                    .sum()
            })
            .unwrap_or(0)
    }

    /// current_line returns the global line index, counting from 0.
    pub fn current_line(&self) -> usize {
        self.current_line
    }

    /// voice_count counts the voice entries of the loaded song.
    pub fn voice_count(&self) -> usize {
        self.loaded
            .as_ref()
            .map(|loaded| loaded.song.voices.len())
            .unwrap_or(0)
    }

    /// current_voice_index returns the index into the song's voices that owns
    /// the current line.
    pub fn current_voice_index(&self) -> Option<usize> {
        let loaded = self.loaded.as_ref()?;
        let mut offset = 0;
        for (index, voice) in loaded.song.voices.iter().enumerate() {
            if self.current_line < offset + voice.lines.len() {
                return Some(index);
            }
            offset += voice.lines.len();
        }
        None
    }

    /// next_line moves to the following line and clamps at the last one.
    pub fn next_line(&mut self) {
        self.go_to_line(self.current_line + 1);
    }

    /// previous_line moves to the preceding line and clamps at the first one.
    pub fn previous_line(&mut self) {
        self.go_to_line(self.current_line.saturating_sub(1));
    }

    /// go_to_line jumps to a global line index and clamps into range. The
    /// selection keeps across line changes (user decision 2026-11-14);
    /// reaching a different line with autoplay on hands its padded play
    /// range to the player.
    pub fn go_to_line(&mut self, index: usize) {
        let count = self.line_count();
        if count > 0 {
            let next = index.min(count - 1);
            if next != self.current_line {
                self.current_line = next;
                self.arm_autoplay();
            }
        }
    }

    /// line_at_ms returns the global index of the line being sung at `ms`
    /// in the current line's voice: the last line whose first note starts
    /// at or before `ms`. `None` before that voice's first line and without
    /// a song.
    pub fn line_at_ms(&self, ms: f64) -> Option<usize> {
        let loaded = self.loaded.as_ref()?;
        let voice_index = self.current_voice_index()?;
        let offset: usize = loaded.song.voices[..voice_index]
            .iter()
            .map(|voice| voice.lines.len())
            .sum();
        let to_ms = |beat: f64| {
            loaded
                .song
                .timing
                .map(|value| value.beat_to_ms(beat))
                .unwrap_or(beat)
        };
        let mut found = None;
        for (index, line) in loaded.song.voices[voice_index].lines.iter().enumerate() {
            let Some(first) = line
                .notes
                .first()
                .and_then(|id| loaded.song.note(&loaded.document, *id))
            else {
                continue;
            };
            if to_ms(first.start) <= ms {
                found = Some(offset + index);
            } else {
                break;
            }
        }
        found
    }

    /// follow_playback makes the line being sung at `ms` the current line
    /// (user decision 2026-11-15, "Follow line") and tells whether the line
    /// changed. It never arms the autoplay: the player already plays.
    pub fn follow_playback(&mut self, ms: f64) -> bool {
        match self.line_at_ms(ms) {
            Some(index) if index != self.current_line => {
                self.current_line = index;
                true
            }
            _ => false,
        }
    }

    /// autoplay tells whether autoplay is on.
    pub fn autoplay(&self) -> bool {
        self.autoplay
    }

    /// set_autoplay turns autoplay on or off (the Shift+A key). Switching
    /// it on plays the current line at once (user decision 2026-11-14);
    /// switching it off drops a still pending range.
    pub fn set_autoplay(&mut self, on: bool) {
        self.autoplay = on;
        if on {
            self.arm_autoplay();
        } else {
            self.pending_play_range = None;
        }
    }

    /// take_pending_play_range takes the play range a line change (or the
    /// autoplay switch itself) handed out, or `None`; the bridge emits it
    /// as the autoPlayRange signal for the player.
    pub fn take_pending_play_range(&mut self) -> Option<(f64, f64)> {
        self.pending_play_range.take()
    }

    /// arm_autoplay stores the padded play range of the current line while
    /// autoplay is on.
    fn arm_autoplay(&mut self) {
        if self.autoplay {
            self.pending_play_range = self.line_play_range();
        }
    }

    /// line_play_range computes the padded play range of the current line:
    /// the first note's start to the phrase end (or the last note's end),
    /// each widened by the 0.3 s line padding.
    fn line_play_range(&self) -> Option<(f64, f64)> {
        let loaded = self.loaded.as_ref()?;
        let to_ms = |beat: f64| {
            loaded
                .song
                .timing
                .map(|value| value.beat_to_ms(beat))
                .unwrap_or(beat)
        };
        let line = self.current_line_data()?;
        let first = loaded.song.note(&loaded.document, *line.notes.first()?)?;
        let end_beat = line
            .end
            .and_then(|id| loaded.song.phrase_end_beat(&loaded.document, id))
            .unwrap_or(first.start + first.length as f64);
        Some((
            to_ms(first.start) - PLAY_PADDING_MS,
            to_ms(end_beat) + PLAY_PADDING_MS,
        ))
    }

    /// read_only tells whether the loaded song may be shown but not changed.
    pub fn read_only(&self) -> bool {
        self.loaded.as_ref().is_some_and(|loaded| loaded.read_only)
    }

    /// is_dirty tells whether the song changed since the load or last save.
    pub fn is_dirty(&self) -> bool {
        self.loaded
            .as_ref()
            .is_some_and(|loaded| loaded.history.is_dirty())
    }

    /// selection returns the selected row ids.
    pub fn selection(&self) -> &BTreeSet<RowId> {
        &self.selection
    }

    /// current_line_data returns the song's line at the global index.
    pub fn current_line_data(&self) -> Option<&Line> {
        self.line_data(self.current_line)
    }

    /// line_data returns the song's line at a global index across all
    /// voices, or `None` past the ends or without a song.
    pub fn line_data(&self, index: usize) -> Option<&Line> {
        let loaded = self.loaded.as_ref()?;
        let mut offset = 0;
        for voice in &loaded.song.voices {
            if let Some(line) = voice.lines.get(index - offset) {
                return Some(line);
            }
            offset += voice.lines.len();
        }
        None
    }
}

/// version_is_editable tells whether the format version may be edited: the
/// unversioned format and the published 1.x versions are, a major version
/// other than 1 and an unparsable value are shown read-only.
fn version_is_editable(version: &Version) -> bool {
    match version {
        Version::Unversioned => true,
        Version::V(major, _, _) => *major == 1,
        Version::Invalid(_) => false,
    }
}
