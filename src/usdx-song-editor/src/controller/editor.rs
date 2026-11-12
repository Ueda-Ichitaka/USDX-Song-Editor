//! The editor controller: load, navigate and (in later tasks) edit.
//!
//! About me: this controller holds the loaded document with its song view,
//! undo history and load-time change log, plus the navigation state (the
//! current line as a global index across all voices) and the selection. It
//! exists as the plain-Rust core of the editor: every user action lands here
//! as a method, the cxx-qt bridge forwards it, and it is testable without
//! Qt. Line navigation is global - the lines pane numbers all lines of all
//! voices from 1, so moving past the last line of one voice continues into
//! the next voice - and clamps at both ends.

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
    loaded: Option<LoadedSong>,
    /// The current line as a global index across all voices.
    current_line: usize,
    selection: BTreeSet<RowId>,
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
        }
    }

    /// open loads the song at `path`. A failure leaves the previously loaded
    /// song in place, so the UI keeps showing what the user had.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let bytes =
            fs::read(path).map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
        let decoded =
            decode(&bytes).map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
        let document = classify(decoded);
        let song = Song::from_document(&document);
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
        Ok(())
    }

    /// is_loaded tells whether a song is open.
    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
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
        let count = self.line_count();
        if count > 0 {
            self.current_line = (self.current_line + 1).min(count - 1);
        }
    }

    /// previous_line moves to the preceding line and clamps at the first one.
    pub fn previous_line(&mut self) {
        self.current_line = self.current_line.saturating_sub(1);
    }

    /// go_to_line jumps to a global line index and clamps into range.
    pub fn go_to_line(&mut self, index: usize) {
        let count = self.line_count();
        if count > 0 {
            self.current_line = index.min(count - 1);
        }
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
        let loaded = self.loaded.as_ref()?;
        let voice_index = self.current_voice_index()?;
        loaded.song.voices[voice_index]
            .lines
            .get(self.current_line - self.line_offset(voice_index)?)
    }

    /// line_offset returns the global index of a voice's first line.
    fn line_offset(&self, voice_index: usize) -> Option<usize> {
        self.loaded.as_ref().map(|loaded| {
            loaded.song.voices[..voice_index]
                .iter()
                .map(|voice| voice.lines.len())
                .sum()
        })
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
