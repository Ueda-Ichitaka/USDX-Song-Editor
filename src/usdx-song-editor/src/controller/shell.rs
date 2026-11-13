//! The shell state: window titles, enablement, audio state and the guarded
//! transition flows.
//!
//! About me: this module lifts the parts of the QML shell that the plan
//! requires as controller tests (task P3-05) into plain Rust: the window and
//! sub-page title rules (UI_SPEC 2.6), the toolbar and menu enablement matrix
//! (UI_SPEC 7), the audio state behind the read-only and audio-missing
//! banners (UI_SPEC 4.4, 4.5) and the guarded transitions of the
//! Save-changes and Changed-on-disk dialogs (UI_SPEC 5.1, 5.2). The QML layer
//! renders these values and drives the dialogs; every decision the dialogs
//! feed back into lands here, so the flows behave the same no matter which
//! control triggered them.

use std::fs;
use std::io;
use std::path::Path;

use usdx_format::bytes::decode;
use usdx_format::headers::Headers;
use usdx_format::rows::classify;

use crate::controller::save::SaveError;
use crate::controller::{EditorController, SaveService};

/// The app name used in every window title.
const APP_TITLE: &str = "USDX Song Editor";

/// The audio situation of the loaded song (UI_SPEC 4.4). The Mix track file
/// is the `#AUDIO` header value, falling back to `#MP3` (plan section 7.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioState {
    /// The file named by the header is in the song folder.
    Present,
    /// The header names a file that is not in the song folder; the value is
    /// the header text for the banner message.
    Missing(String),
    /// The song names no audio file (or the value is empty).
    Unnamed,
}

/// One playback track of the song (plan section 7.1: Mix = `#AUDIO` else
/// `#MP3`, Vocals = `#VOCALS`, Instrumental = `#INSTRUMENTAL`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Mix,
    Vocals,
    Instrumental,
}

/// Which toolbar and menu controls are enabled for the current state
/// (UI_SPEC 4.1, 4.2, 7). Open and Open Recent are always enabled and have
/// no entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellEnablement {
    /// Save: song open, unsaved changes, not read-only.
    pub save: bool,
    /// Save As: song open, not read-only.
    pub save_as: bool,
    pub undo: bool,
    pub redo: bool,
    /// Play, Play line, Loop, Track, Speed: song open with the audio file in
    /// the folder (the "song, no audio" column of the matrix).
    pub play: bool,
    pub play_line: bool,
    pub loop_playback: bool,
    pub track: bool,
    pub speed: bool,
    /// Repair timing: song open, not read-only.
    pub repair: bool,
    /// Song properties: song open (the read-only form opens for read-only
    /// songs).
    pub properties: bool,
    /// Export anchors: song open, also for read-only songs.
    pub export_anchors: bool,
}

/// What the shell must do to finish a guarded transition (open another song,
/// close the window, reload): continue, ask in one of the two dialogs, or
/// stop. The `Stop` message, when present, is the reason for a save error
/// banner or an open error dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transition {
    /// The pending action may continue.
    Continue,
    /// The song has unsaved changes: show the Save changes? dialog first
    /// (UI_SPEC 5.1).
    AskSaveChanges,
    /// The save hit an external change: show the Song changed on disk dialog
    /// (UI_SPEC 5.2).
    AskChangedOnDisk,
    /// The pending action does not continue.
    Stop(Option<String>),
}

/// The user's choice in the Save changes? dialog (UI_SPEC 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveChangesChoice {
    Save,
    Discard,
    Cancel,
}

/// The user's choice in the Song changed on disk dialog (UI_SPEC 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangedOnDiskChoice {
    Overwrite,
    Reload,
    Cancel,
}

/// The result of the activation check (UI_SPEC 5.2: the file is checked when
/// the window becomes active again).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivationChange {
    /// The file is as it was when loaded or saved.
    Unchanged,
    /// The file changed on disk and the song is clean: reload it silently
    /// and show the passive notification.
    ReloadSilently,
    /// The file changed on disk and the song has unsaved changes: show the
    /// changed-on-disk dialog.
    AskChangedOnDisk,
    /// The file could not be read; the value is the reason.
    Failed(String),
}

/// window_title builds the main window title (UI_SPEC 2.6): the song title
/// and artist joined with " - ", " (read-only)" after the artist for a
/// read-only song, "* " while there are unsaved changes, and the plain app
/// name without a song.
pub fn window_title(controller: &EditorController) -> String {
    let Some(loaded) = controller.loaded() else {
        return String::from(APP_TITLE);
    };
    let headers = Headers::from_document(&loaded.document);
    let song_part = song_title_and_artist(&headers);
    if song_part.is_empty() {
        return String::from(APP_TITLE);
    }
    let mut title = if controller.is_dirty() {
        String::from("* ")
    } else {
        String::new()
    };
    title.push_str(&song_part);
    if loaded.read_only {
        title.push_str(" (read-only)");
    }
    title.push_str(" \u{b7} ");
    title.push_str(APP_TITLE);
    title
}

/// page_title builds a sub-page title (UI_SPEC 2.6): pages that show the song
/// carry " - <Title>" (`with_song`), pages like Settings do not.
pub fn page_title(page: &str, with_song: bool, controller: &EditorController) -> String {
    let headers = controller
        .loaded()
        .map(|loaded| Headers::from_document(&loaded.document));
    let title = headers
        .as_ref()
        .and_then(|headers| headers.get("TITLE"))
        .filter(|title| !title.is_empty())
        .filter(|_| with_song)
        .map(|title| format!("{page} - {title} \u{b7} {APP_TITLE}"));
    title.unwrap_or_else(|| format!("{page} \u{b7} {APP_TITLE}"))
}

/// recent_title reads a song file's headers for the "Artist - Title" line
/// of the recent songs (UI_SPEC 4.1); a missing part is left out, and a file
/// that cannot be read or has neither header falls back to its name.
pub fn recent_title(path: &Path) -> String {
    let title = fs::read(path)
        .ok()
        .and_then(|bytes| decode(&bytes).ok())
        .map(classify)
        .map(|document| Headers::from_document(&document))
        .map(|headers| {
            [headers.get("ARTIST"), headers.get("TITLE")]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty())
                .collect::<Vec<&str>>()
                .join(" - ")
        })
        .filter(|title| !title.is_empty());
    title.unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned())
    })
}

/// song_title_and_artist joins the #TITLE and #ARTIST header values that are
/// present with " - " (UI_SPEC 2.5: the separator is a spaced hyphen-minus).
fn song_title_and_artist(headers: &Headers) -> String {
    let parts: Vec<&str> = [headers.get("TITLE"), headers.get("ARTIST")]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect();
    parts.join(" - ")
}

/// audio_state reads the audio situation of the loaded song for the editor
/// banner and the playback enablement; `None` without a song.
pub fn audio_state(controller: &EditorController) -> Option<AudioState> {
    let loaded = controller.loaded()?;
    let headers = Headers::from_document(&loaded.document);
    let name = audio_header_value(&headers);
    match name {
        None => Some(AudioState::Unnamed),
        Some(name) => {
            if track_file(&loaded.path, name).is_some() {
                Some(AudioState::Present)
            } else {
                Some(AudioState::Missing(String::from(name)))
            }
        }
    }
}

/// audio_header_value returns the `#AUDIO` value, falling back to `#MP3`
/// (plan section 7.1); empty values count as missing.
fn audio_header_value(headers: &Headers) -> Option<&str> {
    headers
        .get("AUDIO")
        .filter(|value| !value.is_empty())
        .or_else(|| headers.get("MP3").filter(|value| !value.is_empty()))
}

/// has_track tells whether the track's file exists in the song folder; a
/// missing track is listed but disabled in the UI (UI_SPEC 4.3, plan 7.1).
pub fn has_track(controller: &EditorController, track: TrackKind) -> bool {
    track_path(controller, track).is_some()
}

/// track_path returns the audio file of the track when it exists in the
/// song folder (plan 7.1: Mix = `#AUDIO` else `#MP3`, Vocals = `#VOCALS`,
/// Instrumental = `#INSTRUMENTAL`); None without a song or a missing file.
/// The player bridge switches tracks through this path.
pub fn track_path(controller: &EditorController, track: TrackKind) -> Option<std::path::PathBuf> {
    let loaded = controller.loaded()?;
    let headers = Headers::from_document(&loaded.document);
    match track {
        TrackKind::Mix => {
            let name = audio_header_value(&headers)?;
            track_file(&loaded.path, name)
        }
        TrackKind::Vocals => header_track_file(&headers, "VOCALS", &loaded.path),
        TrackKind::Instrumental => header_track_file(&headers, "INSTRUMENTAL", &loaded.path),
    }
}

/// header_track_file returns the file a track header names, when it exists.
fn header_track_file(headers: &Headers, key: &str, song_path: &Path) -> Option<std::path::PathBuf> {
    let name = headers.get(key).filter(|value| !value.is_empty())?;
    track_file(song_path, name)
}

/// track_file resolves a track file name against the song folder (absolute
/// names are used as they are) and returns it when it exists.
fn track_file(song_path: &Path, name: &str) -> Option<std::path::PathBuf> {
    let path = Path::new(name);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        song_path
            .parent()
            .map(|folder| folder.join(name))
            .unwrap_or_else(|| path.to_owned())
    };
    path.is_file().then_some(path)
}

/// enablement computes the enabled state of every toolbar and menu control
/// for the current controller state (UI_SPEC 7).
pub fn enablement(controller: &EditorController) -> ShellEnablement {
    let open = controller.is_loaded();
    let read_only = controller.read_only();
    let audio = audio_state(controller) == Some(AudioState::Present);
    let (can_undo, can_redo) = controller
        .loaded()
        .map(|loaded| {
            (
                loaded.history.undo_label().is_some(),
                loaded.history.redo_label().is_some(),
            )
        })
        .unwrap_or((false, false));
    ShellEnablement {
        save: open && controller.is_dirty() && !read_only,
        save_as: open && !read_only,
        undo: open && can_undo && !read_only,
        redo: open && can_redo && !read_only,
        play: open && audio,
        play_line: open && audio,
        loop_playback: open && audio,
        track: open && audio,
        speed: open && audio,
        repair: open && !read_only,
        properties: open,
        export_anchors: open,
    }
}

/// transition_start decides how a guarded transition (open another song,
/// close the window, reload) begins: clean songs continue at once, dirty
/// songs ask through the Save changes? dialog (UI_SPEC 5.1).
pub fn transition_start(controller: &EditorController) -> Transition {
    if controller.is_dirty() {
        Transition::AskSaveChanges
    } else {
        Transition::Continue
    }
}

/// transition_save_changes applies the user's choice in the Save changes?
/// dialog: Save saves and then continues (a failed save stops, UI_SPEC 5.1),
/// Discard continues without saving, Cancel stops.
pub fn transition_save_changes(
    controller: &mut EditorController,
    saves: &mut SaveService,
    choice: SaveChangesChoice,
) -> Transition {
    match choice {
        SaveChangesChoice::Save => save_to_own_path(controller, saves, false),
        SaveChangesChoice::Discard => Transition::Continue,
        SaveChangesChoice::Cancel => Transition::Stop(None),
    }
}

/// transition_changed_on_disk applies the user's choice in the Song changed
/// on disk dialog (UI_SPEC 5.2): Overwrite saves over the changed file,
/// Reload opens the file as it is now (dropping the unsaved changes), Cancel
/// stops without touching anything.
pub fn transition_changed_on_disk(
    controller: &mut EditorController,
    saves: &mut SaveService,
    choice: ChangedOnDiskChoice,
) -> Transition {
    match choice {
        ChangedOnDiskChoice::Overwrite => save_to_own_path(controller, saves, true),
        ChangedOnDiskChoice::Reload => transition_reload(controller),
        ChangedOnDiskChoice::Cancel => Transition::Stop(None),
    }
}

/// transition_reload reopens the file from disk, dropping the unsaved
/// changes; a failure leaves the loaded song in place and stops.
pub fn transition_reload(controller: &mut EditorController) -> Transition {
    let Some(path) = controller.path().map(|path| path.to_owned()) else {
        return Transition::Stop(None);
    };
    match controller.open(&path) {
        Ok(()) => Transition::Continue,
        Err(message) => Transition::Stop(Some(message)),
    }
}

/// save_to_own_path saves the loaded song to its path and maps the outcome
/// onto the transition flow; `confirmed` skips the external-change check
/// (the Overwrite choice).
fn save_to_own_path(
    controller: &mut EditorController,
    saves: &mut SaveService,
    confirmed: bool,
) -> Transition {
    let Some(path) = controller.path().map(|path| path.to_owned()) else {
        return Transition::Stop(None);
    };
    let Some(loaded) = controller.loaded_mut() else {
        return Transition::Stop(None);
    };
    let result = if confirmed {
        saves.save_confirmed(loaded, &path)
    } else {
        saves.save(loaded, &path)
    };
    match result {
        Ok(()) => Transition::Continue,
        Err(SaveError::ExternalChange) => Transition::AskChangedOnDisk,
        Err(error) => Transition::Stop(Some(save_failure_reason(&error, &path))),
    }
}

/// save_failure_reason renders a save failure as the user-facing reason of
/// the UI_SPEC 6 banner text `Could not save: <reason>. Your changes are
/// still here.` The `path` is the save target; its file name appears in the
/// wording.
pub fn save_failure_reason(error: &SaveError, path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    match error {
        SaveError::ReadOnly => String::from("the song is read-only"),
        // The save flow turns this into the changed-on-disk dialog before a
        // reason is rendered; the text keeps the refusal honest anyway.
        SaveError::ExternalChange => String::from("the file changed on disk since it was loaded"),
        SaveError::Serialize(detail) => {
            format!("the song text cannot be written: {detail}")
        }
        SaveError::Io(error) => match error.kind() {
            io::ErrorKind::PermissionDenied => format!("no permission to write \"{name}\""),
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => {
                String::from("the disk is full")
            }
            io::ErrorKind::NotFound => String::from("the folder no longer exists"),
            _ => format!("\"{name}\" could not be written: {error}"),
        },
    }
}

/// activation_change checks the loaded file when the window becomes active
/// again (UI_SPEC 5.2): a change without unsaved changes reloads silently, a
/// change with unsaved changes asks.
pub fn activation_change(controller: &EditorController, saves: &SaveService) -> ActivationChange {
    let Some(loaded) = controller.loaded() else {
        return ActivationChange::Unchanged;
    };
    let Some(path) = controller.path() else {
        return ActivationChange::Unchanged;
    };
    match saves.changed_on_disk(loaded, path) {
        Ok(false) => ActivationChange::Unchanged,
        Ok(true) if controller.is_dirty() => ActivationChange::AskChangedOnDisk,
        Ok(true) => ActivationChange::ReloadSilently,
        Err(error) => ActivationChange::Failed(error.to_string()),
    }
}
