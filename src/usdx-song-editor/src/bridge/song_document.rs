//! The SongDocument, LinesModel, RecentSongsModel and Player bridges.
//!
//! About me: this cxx-qt bridge exposes the editor controller and the
//! settings to QML. The SongDocument QObject holds the EditorController, the
//! SaveService and the Settings and mirrors their state into Q_PROPERTIES
//! after every state change; the LinesModel QAbstractListModel feeds the
//! lines pane and the RecentSongsModel feeds the recent songs list; the
//! Player QObject (declared here next to the document it follows, its logic
//! living in bridge/player.rs) drives the audio playback of P4-04. The
//! bridges are thin: the logic lives in the plain-Rust controller
//! (src/controller), this file only converts between Qt and Rust types and
//! routes the guarded transition results of the shell (UI_SPEC 5.1, 5.2) to
//! the right channel - the save error banner property, the open error dialog
//! signal or the changed-on-disk dialog signal. The editing invokables
//! (selection, note moves, line text) and the NotesModel arrive with the
//! later tasks (P5-03 to P6-02); the health and anchor roles stay neutral
//! (0, empty) until the analysis (P7) and repair (P6.3) phases fill them.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_QString_QVariant, QHashPair_i32_QByteArray, QModelIndex, QString,
    QUrl, QVariant,
};
use usdx_format::bytes::decode;
use usdx_format::headers::{Headers, Version};
use usdx_format::rows::{NoteType, RowKind};

use crate::bridge::player::PlayerCore;
use crate::controller::save::SaveError;
use crate::controller::settings::{Settings, settings_path};
use crate::controller::shell;
use crate::controller::{EditorController, SaveService};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        /// Base class of the LinesModel and RecentSongsModel.
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        /// QString from cxx_qt_lib
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        /// QUrl from cxx_qt_lib
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qvariant.h");
        /// QVariant from cxx_qt_lib
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        /// QHash<i32, QByteArray> from cxx_qt_lib
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        /// QHash<QString, QVariant> (the QVariantHash) from cxx_qt_lib
        type QHash_QString_QVariant = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_QString_QVariant>;
        include!("cxx-qt-lib/qmodelindex.h");
        /// QModelIndex from cxx_qt_lib
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qbytearray.h");
        /// QByteArray from cxx_qt_lib
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("format.h");
        /// Relative-date text ("today", "yesterday", "10 Nov 2026") for the
        /// recent songs list (UI_SPEC 4.1).
        #[cxx_name = "usdx_relative_date_text"]
        fn usdx_relative_date_text(opened_secs: i64) -> QString;
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
        #[qproperty(QString, search_text, cxx_name = "searchText")]
        #[qproperty(i32, current_row_index, cxx_name = "currentRowIndex")]
        type LinesModel = super::LinesModelRust;

        /// Return the data for one line row.
        #[qinvokable]
        #[cxx_override]
        fn data(self: &LinesModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Return the role names of the lines pane rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &LinesModel) -> QHash_i32_QByteArray;

        /// Return the number of line rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &LinesModel, parent: &QModelIndex) -> i32;

        /// Refill the rows from the document and re-apply the search text.
        #[qinvokable]
        fn rebuild(self: Pin<&mut LinesModel>);
    }

    /// The roles of the lines pane rows (plan section 8.3).
    #[qenum(LinesModel)]
    enum Roles {
        /// The 1-based global line number.
        LineNumber,
        /// The line's start time in ms.
        StartMs,
        /// The line's end time in ms.
        EndMs,
        /// The joined syllable text.
        Text,
        /// The type summary (`:`, `*`, `F`, `R`, `G` or `mixed`).
        TypeSummary,
        /// The number of notes in the line.
        NoteCount,
        /// 0 ok, 1 suspicious, 2 racing (analysis arrives with P7).
        Health,
        /// The reason of a health mark, empty for ok lines.
        HealthReason,
        /// The anchor chip text (`A1`, `A2`, ...), empty outside repair.
        Anchor,
        /// Whether the row is the current line.
        IsCurrent,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
        type RecentSongsModel = super::RecentSongsModelRust;

        /// Return the data for one recent song row.
        #[qinvokable]
        #[cxx_override]
        fn data(self: &RecentSongsModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Return the role names of the recent songs rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &RecentSongsModel) -> QHash_i32_QByteArray;

        /// Return the number of recent song rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &RecentSongsModel, parent: &QModelIndex) -> i32;

        /// Refill the rows from the settings.
        #[qinvokable]
        fn rebuild(self: Pin<&mut RecentSongsModel>);

        /// Return one recent song as a QVariant hash with the keys title,
        /// pathUrl, displayPath, exists and opened, for the dynamic
        /// Open Recent submenu.
        #[qinvokable]
        fn entry(self: &RecentSongsModel, index: i32) -> QVariant;
    }

    /// The roles of the recent songs rows (UI_SPEC 4.1).
    #[qenum(RecentSongsModel)]
    enum RecentRoles {
        /// "Artist - Title" of the song, or its file name as fallback.
        Title,
        /// The file as an encoded `file://` url string.
        PathUrl,
        /// The folder path with `~` for the home folder.
        DisplayPath,
        /// Whether the file exists; a missing song is shown but not clickable.
        Exists,
        /// The relative date text ("today", "yesterday", "10 Nov 2026").
        OpenedText,
    }

    /// What the QML must do to finish a guarded transition (UI_SPEC 5.1,
    /// 5.2): continue, show one of the two dialogs, or stop. Save failures
    /// land in the saveError property, open and reload failures in the
    /// errorOccurred signal.
    #[qenum(SongDocument)]
    enum TransitionKind {
        /// The pending action may continue.
        Continue,
        /// Show the Save changes? dialog first.
        AskSaveChanges,
        /// Show the Song changed on disk dialog.
        AskChangedOnDisk,
        /// The pending action does not continue.
        Stop,
    }

    /// The user's choice in the Save changes? dialog (UI_SPEC 5.1).
    #[qenum(SongDocument)]
    enum SaveChangesKind {
        Save,
        Discard,
        Cancel,
    }

    /// The user's choice in the Song changed on disk dialog (UI_SPEC 5.2).
    #[qenum(SongDocument)]
    enum ChangedOnDiskKind {
        Overwrite,
        Reload,
        Cancel,
    }

    /// The result of the activation check (UI_SPEC 5.2). ReloadSilently
    /// means the document has already reloaded; QML shows the notification.
    #[qenum(SongDocument)]
    enum ActivationKind {
        Unchanged,
        ReloadSilently,
        AskChangedOnDisk,
        Failed,
    }

    unsafe extern "RustQt" {
        /// Inherited beginResetModel; the caller must balance it with
        /// end_reset_model.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut LinesModel>);

        /// Inherited endResetModel.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut LinesModel>);

        /// The same on the recent songs model.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn recent_begin_reset_model(self: Pin<&mut RecentSongsModel>);

        /// Inherited endResetModel.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn recent_end_reset_model(self: Pin<&mut RecentSongsModel>);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, file_path, cxx_name = "filePath")]
        #[qproperty(QString, title)]
        #[qproperty(QString, artist)]
        #[qproperty(f64, bpm)]
        #[qproperty(f64, gap_ms, cxx_name = "gapMs")]
        #[qproperty(QString, format_version, cxx_name = "formatVersion")]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, read_only, cxx_name = "readOnly")]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, can_redo, cxx_name = "canRedo")]
        #[qproperty(QString, undo_text, cxx_name = "undoText")]
        #[qproperty(QString, redo_text, cxx_name = "redoText")]
        #[qproperty(i32, line_count, cxx_name = "lineCount")]
        #[qproperty(i32, current_line, cxx_name = "currentLine")]
        #[qproperty(i32, current_voice, cxx_name = "currentVoice")]
        #[qproperty(i32, voice_count, cxx_name = "voiceCount")]
        #[qproperty(QString, window_title, cxx_name = "windowTitle")]
        #[qproperty(bool, is_loaded, cxx_name = "isLoaded")]
        #[qproperty(i32, unsaved_steps, cxx_name = "unsavedSteps")]
        #[qproperty(QString, save_error, cxx_name = "saveError")]
        #[qproperty(bool, audio_present, cxx_name = "audioPresent")]
        #[qproperty(QString, audio_file_name, cxx_name = "audioFileName")]
        #[qproperty(bool, has_vocals, cxx_name = "hasVocals")]
        #[qproperty(bool, has_instrumental, cxx_name = "hasInstrumental")]
        #[qproperty(bool, can_save, cxx_name = "canSave")]
        #[qproperty(bool, can_save_as, cxx_name = "canSaveAs")]
        #[qproperty(bool, can_play, cxx_name = "canPlay")]
        #[qproperty(bool, can_play_line, cxx_name = "canPlayLine")]
        #[qproperty(bool, can_loop, cxx_name = "canLoop")]
        #[qproperty(bool, can_track, cxx_name = "canTrack")]
        #[qproperty(bool, can_speed, cxx_name = "canSpeed")]
        #[qproperty(bool, can_repair, cxx_name = "canRepair")]
        #[qproperty(bool, can_properties, cxx_name = "canProperties")]
        #[qproperty(bool, can_export_anchors, cxx_name = "canExportAnchors")]
        #[qproperty(f64, current_line_start_ms, cxx_name = "currentLineStartMs")]
        #[qproperty(f64, current_line_end_ms, cxx_name = "currentLineEndMs")]
        #[qproperty(i32, current_line_note_count, cxx_name = "currentLineNoteCount")]
        #[qproperty(f64, current_line_rate, cxx_name = "currentLineRate")]
        #[qproperty(i32, current_line_health, cxx_name = "currentLineHealth")]
        #[qproperty(
            QString,
            current_line_health_reason,
            cxx_name = "currentLineHealthReason"
        )]
        #[qproperty(QString, current_line_type, cxx_name = "currentLineType")]
        #[qproperty(i32, window_width, cxx_name = "windowWidth")]
        #[qproperty(i32, window_height, cxx_name = "windowHeight")]
        #[qproperty(i32, recent_count, cxx_name = "recentCount")]
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        #[qproperty(QString, app_version, cxx_name = "appVersion")]
        #[qproperty(*mut LinesModel, lines_model, cxx_name = "linesModel")]
        #[qproperty(*mut RecentSongsModel, recent_model, cxx_name = "recentModel")]
        type SongDocument = super::SongDocumentRust;
    }

    unsafe extern "RustQt" {
        /// Emitted after a song loaded successfully.
        #[qsignal]
        #[cxx_name = "songLoaded"]
        fn song_loaded(self: Pin<&mut SongDocument>);

        /// Emitted with a user-facing message when an open or reload failed;
        /// the open error dialog shows it and the previous song stays open.
        #[qsignal]
        #[cxx_name = "errorOccurred"]
        fn error_occurred(self: Pin<&mut SongDocument>, message: QString);

        /// Emitted when the file on disk changed since the load or last save;
        /// the save waits for the user's decision.
        #[qsignal]
        #[cxx_name = "externalChangeDetected"]
        fn external_change_detected(self: Pin<&mut SongDocument>);
    }

    extern "RustQt" {
        /// Open the song at the url; on failure the previous song stays
        /// loaded and `errorOccurred` carries the message.
        #[qinvokable]
        fn open(self: Pin<&mut SongDocument>, url: QUrl) -> bool;

        /// Save the loaded song to its path.
        #[qinvokable]
        fn save(self: Pin<&mut SongDocument>) -> bool;

        /// Save the loaded song after the user confirmed an external change.
        #[qinvokable]
        #[cxx_name = "saveConfirmed"]
        fn save_confirmed(self: Pin<&mut SongDocument>) -> bool;

        /// Save the loaded song to the url; the document switches to it.
        #[qinvokable]
        #[cxx_name = "saveAs"]
        fn save_as(self: Pin<&mut SongDocument>, url: QUrl) -> bool;

        /// Reload the song from its path.
        #[qinvokable]
        fn reload(self: Pin<&mut SongDocument>);

        /// Undo the last edit step.
        #[qinvokable]
        fn undo(self: Pin<&mut SongDocument>);

        /// Redo the last undone edit step.
        #[qinvokable]
        fn redo(self: Pin<&mut SongDocument>);

        /// Jump to the global line index, clamped into range.
        #[qinvokable]
        #[cxx_name = "goToLine"]
        fn go_to_line(self: Pin<&mut SongDocument>, index: i32);

        /// Move to the following line, clamped at the last one.
        #[qinvokable]
        #[cxx_name = "nextLine"]
        fn next_line(self: Pin<&mut SongDocument>);

        /// Move to the preceding line, clamped at the first one.
        #[qinvokable]
        #[cxx_name = "previousLine"]
        fn previous_line(self: Pin<&mut SongDocument>);

        /// Begin a guarded transition (open, close, reload): Continue or
        /// AskSaveChanges.
        #[qinvokable]
        #[cxx_name = "transitionStart"]
        fn transition_start(self: Pin<&mut SongDocument>) -> TransitionKind;

        /// Apply the user's choice in the Save changes? dialog.
        #[qinvokable]
        #[cxx_name = "transitionSaveChanges"]
        fn transition_save_changes(
            self: Pin<&mut SongDocument>,
            kind: SaveChangesKind,
        ) -> TransitionKind;

        /// Apply the user's choice in the Song changed on disk dialog.
        #[qinvokable]
        #[cxx_name = "transitionChangedOnDisk"]
        fn transition_changed_on_disk(
            self: Pin<&mut SongDocument>,
            kind: ChangedOnDiskKind,
        ) -> TransitionKind;

        /// Reload the song from its path as part of a guarded transition; a
        /// failure reports through `errorOccurred` and stops.
        #[qinvokable]
        #[cxx_name = "transitionReload"]
        fn transition_reload(self: Pin<&mut SongDocument>) -> TransitionKind;

        /// Check the loaded file when the window becomes active again; a
        /// silent reload already happened when ReloadSilently is returned.
        #[qinvokable]
        #[cxx_name = "checkActivation"]
        fn check_activation(self: Pin<&mut SongDocument>) -> ActivationKind;

        /// Remove every entry from the recent songs list.
        #[qinvokable]
        #[cxx_name = "clearRecentList"]
        fn clear_recent_list(self: Pin<&mut SongDocument>);

        /// Rebuild the recent songs model from the settings; QML calls it
        /// once on startup because no state change precedes it.
        #[qinvokable]
        #[cxx_name = "refreshRecents"]
        fn refresh_recents(self: Pin<&mut SongDocument>);

        /// Persist the current window size to the settings; QML sets the
        /// windowWidth and windowHeight properties before closing.
        #[qinvokable]
        #[cxx_name = "rememberWindowSize"]
        fn remember_window_size(self: Pin<&mut SongDocument>);

        /// Return and clear the backup path of the last save, empty when
        /// that save wrote none; QML shows the "backup written"
        /// notification from it.
        #[qinvokable]
        #[cxx_name = "takeLastBackup"]
        fn take_last_backup(self: Pin<&mut SongDocument>) -> QString;

        /// Return the audio file path of the track (0 mix, 1 vocals,
        /// 2 instrumental), empty when the song has no such file; the
        /// waveform lane and the playback use it.
        #[qinvokable]
        #[cxx_name = "audioPath"]
        fn audio_path(self: &SongDocument, track: i32) -> QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
        #[qproperty(f64, position_ms, cxx_name = "positionMs")]
        #[qproperty(f64, duration_ms, cxx_name = "durationMs")]
        #[qproperty(bool, playing)]
        // The WRITE setters are the setRate and setTrack invokables below:
        // a generated setter would collide with them on the Rust side.
        #[qproperty(f64, rate, READ, WRITE = set_rate, NOTIFY)]
        #[qproperty(i32, track, READ, WRITE = set_track, NOTIFY)]
        #[qproperty(bool, has_vocals, cxx_name = "hasVocals")]
        #[qproperty(bool, has_instrumental, cxx_name = "hasInstrumental")]
        type Player = super::PlayerRust;
    }

    /// The audio track a playback request addresses (plan section 8.3); the
    /// values double as the i32 of the track property.
    #[qenum(Player)]
    enum Track {
        /// The mixed song audio.
        Mix,
        /// The vocals stem.
        Vocals,
        /// The instrumental stem.
        Instrumental,
    }

    unsafe extern "RustQt" {
        /// Emitted when the played range is over; QML replays it while
        /// Loop is on.
        #[qsignal]
        #[cxx_name = "rangeDone"]
        fn range_done(self: Pin<&mut Player>);

        /// Emitted with a user-facing message when a load, a switch or the
        /// playback failed; shown as a passive notification.
        #[qsignal]
        #[cxx_name = "errorOccurred"]
        fn error_occurred(self: Pin<&mut Player>, message: QString);
    }

    extern "RustQt" {
        /// Play the whole song from the current position; when no track is
        /// loaded yet, it loads first and then plays.
        #[qinvokable]
        fn play(self: Pin<&mut Player>);

        /// Pause playback.
        #[qinvokable]
        fn pause(self: Pin<&mut Player>);

        /// Play or pause, whatever does the opposite of the current state.
        #[qinvokable]
        fn toggle(self: Pin<&mut Player>);

        /// Seek to `ms` milliseconds into the song.
        #[qinvokable]
        #[cxx_name = "seekMs"]
        fn seek_ms(self: Pin<&mut Player>, ms: f64);

        /// Play only the range from start to end in milliseconds; the
        /// `rangeDone` signal follows when the range is over.
        #[qinvokable]
        #[cxx_name = "playRange"]
        fn play_range(self: Pin<&mut Player>, start_ms: f64, end_ms: f64);

        /// Change the playback rate (0.5, 0.75, 1.0); doubles as the WRITE
        /// setter of the rate property.
        #[qinvokable]
        #[cxx_name = "setRate"]
        fn set_rate(self: Pin<&mut Player>, rate: f64);

        /// Switch to the track with the enum value (0 mix, 1 vocals,
        /// 2 instrumental); doubles as the WRITE setter of the track
        /// property. The switch keeps the position, the playing state and
        /// the rate and runs on a worker thread.
        #[qinvokable]
        #[cxx_name = "setTrack"]
        fn set_track(self: Pin<&mut Player>, track: i32);

        /// Drain the bus events into the properties and follow the
        /// playhead; the FrameAnimation calls it every frame while playing.
        #[qinvokable]
        fn tick(self: Pin<&mut Player>);
    }

    impl cxx_qt::Threading for Player {}
}

use core::pin::Pin;

use cxx_qt::CxxQtType;

use crate::bridge::song_document::qobject::{
    ActivationKind, ChangedOnDiskKind, LinesModel, RecentRoles, RecentSongsModel, Roles,
    SaveChangesKind, SongDocument, TransitionKind, usdx_relative_date_text,
};

/// The Rust side of the Player: the playback core plus the Q_PROPERTY
/// storage. The logic lives in the impls in bridge/player.rs.
#[derive(Default)]
pub struct PlayerRust {
    pub(crate) core: PlayerCore,
    /// The document the playback follows; QML sets it once on startup.
    pub(crate) document: *mut SongDocument,
    /// True while a load or a track switch runs on the worker thread.
    pub(crate) loading: bool,
    /// True when Play was pressed while the track still loads; the load
    /// finished handler starts the playback.
    pub(crate) play_requested: bool,
    /// The range a parked play request plays, None for the whole song.
    pub(crate) pending_range: Option<(f64, f64)>,
    /// True once the track was resolved for the current song (from the
    /// settings default); set_track marks it resolved too.
    pub(crate) track_resolved: bool,
    /// The song path the current playback state belongs to; a change
    /// unloads the player so the next play loads the new song's audio.
    pub(crate) loaded_song_path: String,
    // The Q_PROPERTY storage, mirrored from the core; the setters run
    // through the generated set_* methods.
    pub(crate) position_ms: f64,
    pub(crate) duration_ms: f64,
    pub(crate) playing: bool,
    pub(crate) rate: f64,
    pub(crate) track: i32,
    pub(crate) has_vocals: bool,
    pub(crate) has_instrumental: bool,
}

impl SongDocumentRust {
    /// The controller behind the document; the player resolves the track
    /// files through it.
    pub(crate) fn editor(&self) -> &EditorController {
        &self.controller
    }

    /// The settings behind the document; the player reads the default
    /// track from it.
    pub(crate) fn editor_settings(&self) -> &Settings {
        &self.settings
    }
}

/// One row of the lines pane model (plan section 8.3).
#[derive(Clone)]
pub struct LineRow {
    line_number: i32,
    start_ms: f64,
    end_ms: f64,
    text: QString,
    /// The lower-cased text, the key the search filter matches against.
    search_key: String,
    type_summary: QString,
    note_count: i32,
    health: i32,
    health_reason: QString,
    anchor: QString,
    is_current: bool,
}

/// The Rust side of the LinesModel.
#[derive(Default)]
pub struct LinesModelRust {
    /// The filtered rows the model shows.
    rows: Vec<LineRow>,
    search_text: QString,
    current_row_index: i32,
    document: *mut SongDocument,
}

/// One row of the recent songs model (UI_SPEC 4.1).
pub struct RecentRow {
    title: QString,
    path_url: QString,
    display_path: QString,
    exists: bool,
    opened_text: QString,
}

/// The Rust side of the RecentSongsModel.
#[derive(Default)]
pub struct RecentSongsModelRust {
    rows: Vec<RecentRow>,
    document: *mut SongDocument,
}

/// The Rust side of the SongDocument: the controller, the save service, the
/// settings and the Q_PROPERTY storage.
pub struct SongDocumentRust {
    controller: EditorController,
    saves: SaveService,
    settings: Settings,
    /// Set when the recent list changed and the model must rebuild on the
    /// next refresh; QML also sets it once on startup.
    recents_stale: bool,
    file_path: QString,
    title: QString,
    artist: QString,
    bpm: f64,
    gap_ms: f64,
    format_version: QString,
    dirty: bool,
    read_only: bool,
    can_undo: bool,
    can_redo: bool,
    undo_text: QString,
    redo_text: QString,
    line_count: i32,
    current_line: i32,
    current_voice: i32,
    voice_count: i32,
    window_title: QString,
    is_loaded: bool,
    unsaved_steps: i32,
    save_error: QString,
    audio_present: bool,
    audio_file_name: QString,
    has_vocals: bool,
    has_instrumental: bool,
    can_save: bool,
    can_save_as: bool,
    can_play: bool,
    can_play_line: bool,
    can_loop: bool,
    can_track: bool,
    can_speed: bool,
    can_repair: bool,
    can_properties: bool,
    can_export_anchors: bool,
    current_line_start_ms: f64,
    current_line_end_ms: f64,
    current_line_note_count: i32,
    current_line_rate: f64,
    current_line_health: i32,
    current_line_health_reason: QString,
    current_line_type: QString,
    window_width: i32,
    window_height: i32,
    recent_count: i32,
    status_text: QString,
    /// The application version, shown on the About page.
    app_version: QString,
    lines_model: *mut LinesModel,
    recent_model: *mut RecentSongsModel,
}

impl Default for SongDocumentRust {
    /// default loads the settings (pruning missing recent songs) and fills
    /// the properties from the empty controller, so the window title and
    /// every enablement binding are correct before the first state change.
    fn default() -> Self {
        let settings = load_settings();
        let controller = EditorController::default();
        let saves = SaveService::new();
        let snapshot = build_snapshot(&controller, &settings);
        let [width, height] = settings.window_size.unwrap_or([1280, 800]);
        SongDocumentRust {
            controller,
            saves,
            settings,
            recents_stale: true,
            file_path: snapshot.file_path,
            title: snapshot.title,
            artist: snapshot.artist,
            bpm: snapshot.bpm,
            gap_ms: snapshot.gap_ms,
            format_version: snapshot.format_version,
            dirty: snapshot.dirty,
            read_only: snapshot.read_only,
            can_undo: snapshot.can_undo,
            can_redo: snapshot.can_redo,
            undo_text: snapshot.undo_text,
            redo_text: snapshot.redo_text,
            line_count: snapshot.line_count,
            current_line: snapshot.current_line,
            current_voice: snapshot.current_voice,
            voice_count: snapshot.voice_count,
            window_title: snapshot.window_title,
            is_loaded: snapshot.is_loaded,
            unsaved_steps: snapshot.unsaved_steps,
            save_error: QString::default(),
            audio_present: snapshot.audio_present,
            audio_file_name: snapshot.audio_file_name,
            has_vocals: snapshot.has_vocals,
            has_instrumental: snapshot.has_instrumental,
            can_save: snapshot.can_save,
            can_save_as: snapshot.can_save_as,
            can_play: snapshot.can_play,
            can_play_line: snapshot.can_play_line,
            can_loop: snapshot.can_loop,
            can_track: snapshot.can_track,
            can_speed: snapshot.can_speed,
            can_repair: snapshot.can_repair,
            can_properties: snapshot.can_properties,
            can_export_anchors: snapshot.can_export_anchors,
            current_line_start_ms: snapshot.current_line_start_ms,
            current_line_end_ms: snapshot.current_line_end_ms,
            current_line_note_count: snapshot.current_line_note_count,
            current_line_rate: snapshot.current_line_rate,
            current_line_health: snapshot.current_line_health,
            current_line_health_reason: snapshot.current_line_health_reason,
            current_line_type: snapshot.current_line_type,
            window_width: width,
            window_height: height,
            recent_count: snapshot.recent_count,
            status_text: QString::default(),
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            lines_model: std::ptr::null_mut(),
            recent_model: std::ptr::null_mut(),
        }
    }
}

impl qobject::LinesModel {
    /// data returns the value of one line row for one role.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        match (Roles { repr: role }) {
            Roles::LineNumber => QVariant::from(&row.line_number),
            Roles::StartMs => QVariant::from(&row.start_ms),
            Roles::EndMs => QVariant::from(&row.end_ms),
            Roles::Text => QVariant::from(&row.text),
            Roles::TypeSummary => QVariant::from(&row.type_summary),
            Roles::NoteCount => QVariant::from(&row.note_count),
            Roles::Health => QVariant::from(&row.health),
            Roles::HealthReason => QVariant::from(&row.health_reason),
            Roles::Anchor => QVariant::from(&row.anchor),
            Roles::IsCurrent => QVariant::from(&row.is_current),
            _ => QVariant::default(),
        }
    }

    /// role_names returns the role numbers of the lines pane rows.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(Roles::LineNumber.repr, QByteArray::from("lineNumber"));
        roles.insert(Roles::StartMs.repr, QByteArray::from("startMs"));
        roles.insert(Roles::EndMs.repr, QByteArray::from("endMs"));
        roles.insert(Roles::Text.repr, QByteArray::from("text"));
        roles.insert(Roles::TypeSummary.repr, QByteArray::from("typeSummary"));
        roles.insert(Roles::NoteCount.repr, QByteArray::from("noteCount"));
        roles.insert(Roles::Health.repr, QByteArray::from("health"));
        roles.insert(Roles::HealthReason.repr, QByteArray::from("healthReason"));
        roles.insert(Roles::Anchor.repr, QByteArray::from("anchor"));
        roles.insert(Roles::IsCurrent.repr, QByteArray::from("isCurrent"));
        roles
    }

    /// row_count returns the number of line rows.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }

    /// rebuild refills the rows from the document, applies the search filter
    /// and tracks where the current line sits in the filtered rows.
    pub fn rebuild(mut self: Pin<&mut Self>) {
        let search = String::from(&self.search_text).to_lowercase();
        let (rows, current) = match unsafe { self.as_ref().document().as_ref() } {
            Some(document) => {
                let all = line_rows(&document.rust().controller);
                let rows = if search.is_empty() {
                    all
                } else {
                    all.into_iter()
                        .filter(|row| row.search_key.contains(&search))
                        .collect()
                };
                let current = rows
                    .iter()
                    .position(|row| row.is_current)
                    .map(|position| position as i32)
                    .unwrap_or(-1);
                (rows, current)
            }
            None => (Vec::new(), -1),
        };
        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().rows = rows;
            self.as_mut().end_reset_model();
        }
        self.as_mut().set_current_row_index(current);
    }
}

impl qobject::RecentSongsModel {
    /// data returns the value of one recent song row for one role.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        match (RecentRoles { repr: role }) {
            RecentRoles::Title => QVariant::from(&row.title),
            RecentRoles::PathUrl => QVariant::from(&row.path_url),
            RecentRoles::DisplayPath => QVariant::from(&row.display_path),
            RecentRoles::Exists => QVariant::from(&row.exists),
            RecentRoles::OpenedText => QVariant::from(&row.opened_text),
            _ => QVariant::default(),
        }
    }

    /// role_names returns the role numbers of the recent songs rows.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(RecentRoles::Title.repr, QByteArray::from("title"));
        roles.insert(RecentRoles::PathUrl.repr, QByteArray::from("pathUrl"));
        roles.insert(
            RecentRoles::DisplayPath.repr,
            QByteArray::from("displayPath"),
        );
        roles.insert(RecentRoles::Exists.repr, QByteArray::from("exists"));
        roles.insert(RecentRoles::OpenedText.repr, QByteArray::from("openedText"));
        roles
    }

    /// row_count returns the number of recent song rows.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }

    /// rebuild refills the rows from the settings of the document.
    pub fn rebuild(mut self: Pin<&mut Self>) {
        let rows = match unsafe { self.as_ref().document().as_ref() } {
            Some(document) => recent_rows(&document.rust().settings),
            None => Vec::new(),
        };
        unsafe {
            self.as_mut().recent_begin_reset_model();
            self.as_mut().rust_mut().rows = rows;
            self.as_mut().recent_end_reset_model();
        }
    }

    /// entry returns one recent song as a QVariant hash for the dynamic
    /// Open Recent submenu.
    pub fn entry(&self, index: i32) -> QVariant {
        let Some(row) = self.rows.get(index as usize) else {
            return QVariant::default();
        };
        let mut hash = QHash::<QHashPair_QString_QVariant>::default();
        hash.insert(QString::from("title"), QVariant::from(&row.title));
        hash.insert(QString::from("pathUrl"), QVariant::from(&row.path_url));
        hash.insert(
            QString::from("displayPath"),
            QVariant::from(&row.display_path),
        );
        hash.insert(QString::from("exists"), QVariant::from(&row.exists));
        hash.insert(QString::from("opened"), QVariant::from(&row.opened_text));
        QVariant::from(&hash)
    }
}

impl qobject::SongDocument {
    /// open loads the song at the url.
    pub fn open(mut self: Pin<&mut Self>, url: QUrl) -> bool {
        let path = qurl_to_path(&url);
        self.as_mut().open_path(&path)
    }

    /// save writes the loaded song to its path.
    pub fn save(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().save_current(false)
    }

    /// save_confirmed writes the loaded song after the user confirmed an
    /// external change.
    pub fn save_confirmed(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().save_current(true)
    }

    /// save_as writes the loaded song to the url and switches to it.
    pub fn save_as(mut self: Pin<&mut Self>, url: QUrl) -> bool {
        let path = qurl_to_path(&url);
        self.as_mut().save_to(&path, false, true)
    }

    /// reload re-opens the song at its path.
    pub fn reload(mut self: Pin<&mut Self>) {
        let path = self.controller.path().map(|path| path.to_owned());
        if let Some(path) = path {
            self.as_mut().open_path(&path);
        }
    }

    /// undo reverts the last edit step.
    pub fn undo(mut self: Pin<&mut Self>) {
        if let Some(loaded) = self.as_mut().rust_mut().controller.loaded_mut() {
            loaded.history.undo(&mut loaded.document);
        }
        self.as_mut().refresh();
    }

    /// redo re-applies the last undone edit step.
    pub fn redo(mut self: Pin<&mut Self>) {
        if let Some(loaded) = self.as_mut().rust_mut().controller.loaded_mut() {
            loaded.history.redo(&mut loaded.document);
        }
        self.as_mut().refresh();
    }

    /// go_to_line jumps to the global line index.
    pub fn go_to_line(mut self: Pin<&mut Self>, index: i32) {
        if index >= 0 {
            self.as_mut()
                .rust_mut()
                .controller
                .go_to_line(index as usize);
        }
        self.as_mut().refresh();
    }

    /// next_line moves to the following line.
    pub fn next_line(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.next_line();
        self.as_mut().refresh();
    }

    /// previous_line moves to the preceding line.
    pub fn previous_line(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.previous_line();
        self.as_mut().refresh();
    }

    /// transition_start begins a guarded transition.
    pub fn transition_start(mut self: Pin<&mut Self>) -> TransitionKind {
        let transition = shell::transition_start(&self.controller);
        self.as_mut().route_save_transition(transition)
    }

    /// transition_save_changes applies the Save changes? dialog choice.
    pub fn transition_save_changes(
        mut self: Pin<&mut Self>,
        kind: SaveChangesKind,
    ) -> TransitionKind {
        let choice = match (SaveChangesKind { repr: kind.repr }) {
            SaveChangesKind::Save => shell::SaveChangesChoice::Save,
            SaveChangesKind::Discard => shell::SaveChangesChoice::Discard,
            _ => shell::SaveChangesChoice::Cancel,
        };
        let transition = {
            let rust = self.as_mut().rust_mut().get_mut();
            shell::transition_save_changes(&mut rust.controller, &mut rust.saves, choice)
        };
        if choice == shell::SaveChangesChoice::Save && transition == shell::Transition::Continue {
            self.as_mut().set_save_error(QString::default());
        }
        self.as_mut().route_save_transition(transition)
    }

    /// transition_changed_on_disk applies the Song changed on disk dialog
    /// choice.
    pub fn transition_changed_on_disk(
        mut self: Pin<&mut Self>,
        kind: ChangedOnDiskKind,
    ) -> TransitionKind {
        let choice = match (ChangedOnDiskKind { repr: kind.repr }) {
            ChangedOnDiskKind::Overwrite => shell::ChangedOnDiskChoice::Overwrite,
            ChangedOnDiskKind::Reload => shell::ChangedOnDiskChoice::Reload,
            _ => shell::ChangedOnDiskChoice::Cancel,
        };
        let transition = {
            let rust = self.as_mut().rust_mut().get_mut();
            shell::transition_changed_on_disk(&mut rust.controller, &mut rust.saves, choice)
        };
        if choice == shell::ChangedOnDiskChoice::Overwrite
            && transition == shell::Transition::Continue
        {
            self.as_mut().set_save_error(QString::default());
        }
        match choice {
            // Overwrite goes through the save flow; its failures are save
            // failures and reach the banner.
            shell::ChangedOnDiskChoice::Overwrite => {
                self.as_mut().route_save_transition(transition)
            }
            // Reload and Cancel route read failures to the open error
            // dialog.
            _ => self.as_mut().route_open_transition(transition),
        }
    }

    /// transition_reload reloads as part of a guarded transition.
    pub fn transition_reload(mut self: Pin<&mut Self>) -> TransitionKind {
        let transition = {
            let rust = self.as_mut().rust_mut().get_mut();
            shell::transition_reload(&mut rust.controller)
        };
        self.as_mut().route_open_transition(transition)
    }

    /// check_activation checks the loaded file when the window becomes
    /// active again; a silent reload happens here, QML only notifies.
    pub fn check_activation(mut self: Pin<&mut Self>) -> ActivationKind {
        let change = shell::activation_change(&self.controller, &self.saves);
        match change {
            shell::ActivationChange::Unchanged => ActivationKind::Unchanged,
            shell::ActivationChange::Failed(reason) => {
                // The check already failed once; keep the session as it is
                // instead of throwing an error at the user.
                log::warn!("The file could not be checked on activation: {reason}");
                ActivationKind::Unchanged
            }
            shell::ActivationChange::ReloadSilently => {
                // Reload here, not through open_path: a reload must not
                // touch the recent list. A failure reports through the open
                // error dialog.
                let path = self.controller.path().map(|path| path.to_owned());
                if let Some(path) = path {
                    let result = self.as_mut().rust_mut().get_mut().controller.open(&path);
                    match result {
                        Ok(()) => {
                            self.as_mut().refresh();
                            self.as_mut().song_loaded();
                        }
                        Err(message) => self.as_mut().fail(&QString::from(message)),
                    }
                }
                ActivationKind::ReloadSilently
            }
            shell::ActivationChange::AskChangedOnDisk => ActivationKind::AskChangedOnDisk,
        }
    }

    /// clear_recent_list removes every recent song entry and persists.
    pub fn clear_recent_list(mut self: Pin<&mut Self>) {
        {
            let rust = self.as_mut().rust_mut().get_mut();
            rust.settings.recent_files.clear();
            persist_settings(&rust.settings);
            rust.recents_stale = true;
        }
        self.as_mut().refresh();
    }

    /// refresh_recents rebuilds the recent songs model on demand.
    pub fn refresh_recents(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().recents_stale = true;
        self.as_mut().rebuild_recents_if_stale();
    }

    /// remember_window_size persists the window size the QML reported.
    pub fn remember_window_size(mut self: Pin<&mut Self>) {
        let rust = self.as_mut().rust_mut().get_mut();
        rust.settings.window_size = Some([rust.window_width, rust.window_height]);
        persist_settings(&rust.settings);
    }

    /// take_last_backup returns and clears the backup path of the last save.
    pub fn take_last_backup(mut self: Pin<&mut Self>) -> QString {
        let backup = self.as_mut().rust_mut().get_mut().saves.take_last_backup();
        backup
            .map(|path| QString::from(path.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    /// audio_path returns the audio file of the track, empty when the song
    /// has none; the waveform lane and the playback use it (plan 8.3).
    pub fn audio_path(&self, track: i32) -> QString {
        let rust = self.rust();
        let Some(kind) = crate::bridge::player::track_kind(track) else {
            return QString::default();
        };
        shell::track_path(&rust.controller, kind)
            .map(|path| QString::from(path.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    /// open_path loads the song at the path and reports the outcome.
    fn open_path(mut self: Pin<&mut Self>, path: &Path) -> bool {
        let result = self.as_mut().rust_mut().get_mut().controller.open(path);
        match result {
            Ok(()) => {
                {
                    let rust = self.as_mut().rust_mut().get_mut();
                    let opened = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|duration| duration.as_secs())
                        .unwrap_or(0);
                    rust.settings.remember_recent_file(path, opened);
                    persist_settings(&rust.settings);
                    rust.recents_stale = true;
                }
                self.as_mut().refresh();
                self.as_mut().song_loaded();
                true
            }
            Err(message) => {
                self.as_mut().fail(&QString::from(message));
                false
            }
        }
    }

    /// save_current writes the loaded song to its own path.
    fn save_current(mut self: Pin<&mut Self>, confirmed: bool) -> bool {
        let path = self.controller.path().map(|path| path.to_owned());
        let Some(path) = path else {
            self.as_mut().fail(&QString::from("No song is open."));
            return false;
        };
        self.as_mut().save_to(&path, confirmed, false)
    }

    /// save_to runs the save service against `path`.
    fn save_to(mut self: Pin<&mut Self>, path: &Path, confirmed: bool, switch: bool) -> bool {
        let result = {
            // get_mut is fine: the Rust struct holds no pinned data.
            let rust = self.as_mut().rust_mut().get_mut();
            match rust.controller.loaded_mut() {
                Some(loaded) => {
                    if confirmed {
                        rust.saves.save_confirmed(loaded, path)
                    } else {
                        rust.saves.save(loaded, path)
                    }
                }
                // The path only exists when a song is open, so this arm is
                // unreachable in normal flow; it keeps the match total.
                None => Err(SaveError::ReadOnly),
            }
        };
        match result {
            Ok(()) => {
                if switch && let Some(loaded) = self.as_mut().rust_mut().controller.loaded_mut() {
                    loaded.path = path.to_owned();
                }
                self.as_mut().set_save_error(QString::default());
                self.as_mut().refresh();
                true
            }
            Err(SaveError::ExternalChange) => {
                self.as_mut().set_status_text(QString::from(
                    "The file changed on disk since it was loaded.",
                ));
                self.as_mut().external_change_detected();
                false
            }
            Err(error) => {
                self.as_mut()
                    .report_save_failure(QString::from(shell::save_failure_reason(&error, path)));
                false
            }
        }
    }

    /// refresh mirrors the controller state into the Q_PROPERTYs and the
    /// models.
    fn refresh(mut self: Pin<&mut Self>) {
        self.as_mut().sync();
        if let Some(model) = unsafe { self.as_ref().lines_model().as_mut() } {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
        }
        self.as_mut().rebuild_recents_if_stale();
    }

    /// sync copies the controller state into the Q_PROPERTY storage.
    fn sync(mut self: Pin<&mut Self>) {
        let snapshot = build_snapshot(&self.controller, &self.settings);
        self.as_mut().set_file_path(snapshot.file_path);
        self.as_mut().set_title(snapshot.title);
        self.as_mut().set_artist(snapshot.artist);
        self.as_mut().set_bpm(snapshot.bpm);
        self.as_mut().set_gap_ms(snapshot.gap_ms);
        self.as_mut().set_format_version(snapshot.format_version);
        self.as_mut().set_dirty(snapshot.dirty);
        self.as_mut().set_read_only(snapshot.read_only);
        self.as_mut().set_can_undo(snapshot.can_undo);
        self.as_mut().set_can_redo(snapshot.can_redo);
        self.as_mut().set_undo_text(snapshot.undo_text);
        self.as_mut().set_redo_text(snapshot.redo_text);
        self.as_mut().set_line_count(snapshot.line_count);
        self.as_mut().set_current_line(snapshot.current_line);
        self.as_mut().set_current_voice(snapshot.current_voice);
        self.as_mut().set_voice_count(snapshot.voice_count);
        self.as_mut().set_window_title(snapshot.window_title);
        self.as_mut().set_is_loaded(snapshot.is_loaded);
        self.as_mut().set_unsaved_steps(snapshot.unsaved_steps);
        self.as_mut().set_audio_present(snapshot.audio_present);
        self.as_mut().set_audio_file_name(snapshot.audio_file_name);
        self.as_mut().set_has_vocals(snapshot.has_vocals);
        self.as_mut()
            .set_has_instrumental(snapshot.has_instrumental);
        self.as_mut().set_can_save(snapshot.can_save);
        self.as_mut().set_can_save_as(snapshot.can_save_as);
        self.as_mut().set_can_play(snapshot.can_play);
        self.as_mut().set_can_play_line(snapshot.can_play_line);
        self.as_mut().set_can_loop(snapshot.can_loop);
        self.as_mut().set_can_track(snapshot.can_track);
        self.as_mut().set_can_speed(snapshot.can_speed);
        self.as_mut().set_can_repair(snapshot.can_repair);
        self.as_mut().set_can_properties(snapshot.can_properties);
        self.as_mut()
            .set_can_export_anchors(snapshot.can_export_anchors);
        self.as_mut()
            .set_current_line_start_ms(snapshot.current_line_start_ms);
        self.as_mut()
            .set_current_line_end_ms(snapshot.current_line_end_ms);
        self.as_mut()
            .set_current_line_note_count(snapshot.current_line_note_count);
        self.as_mut()
            .set_current_line_rate(snapshot.current_line_rate);
        self.as_mut()
            .set_current_line_health(snapshot.current_line_health);
        self.as_mut()
            .set_current_line_health_reason(snapshot.current_line_health_reason);
        self.as_mut()
            .set_current_line_type(snapshot.current_line_type);
        self.as_mut().set_recent_count(snapshot.recent_count);
    }

    /// rebuild_recents_if_stale rebuilds the recent songs model when the
    /// list changed since the last rebuild.
    fn rebuild_recents_if_stale(mut self: Pin<&mut Self>) {
        if self.recents_stale
            && let Some(model) = unsafe { self.as_ref().recent_model().as_mut() }
        {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
            self.as_mut().rust_mut().recents_stale = false;
        }
    }

    /// route_save_transition maps a shell transition onto the TransitionKind
    /// return value; save failures land in the saveError banner property.
    fn route_save_transition(
        mut self: Pin<&mut Self>,
        transition: shell::Transition,
    ) -> TransitionKind {
        match transition {
            shell::Transition::Continue => TransitionKind::Continue,
            shell::Transition::AskSaveChanges => TransitionKind::AskSaveChanges,
            shell::Transition::AskChangedOnDisk => TransitionKind::AskChangedOnDisk,
            shell::Transition::Stop(Some(message)) => {
                self.as_mut().report_save_failure(QString::from(message));
                TransitionKind::Stop
            }
            shell::Transition::Stop(None) => TransitionKind::Stop,
        }
    }

    /// route_open_transition maps a shell transition onto the TransitionKind
    /// return value; open and reload failures reach the open error dialog
    /// through the errorOccurred signal.
    fn route_open_transition(
        mut self: Pin<&mut Self>,
        transition: shell::Transition,
    ) -> TransitionKind {
        match transition {
            shell::Transition::Continue => TransitionKind::Continue,
            shell::Transition::AskSaveChanges => TransitionKind::AskSaveChanges,
            shell::Transition::AskChangedOnDisk => TransitionKind::AskChangedOnDisk,
            shell::Transition::Stop(Some(message)) => {
                self.as_mut().fail(&QString::from(message));
                TransitionKind::Stop
            }
            shell::Transition::Stop(None) => TransitionKind::Stop,
        }
    }

    /// report_save_failure shows a failed save through the banner property
    /// and the status text; the error dialog stays for open failures.
    fn report_save_failure(mut self: Pin<&mut Self>, message: QString) {
        self.as_mut().set_save_error(message.clone());
        self.as_mut().set_status_text(message.clone());
    }

    /// fail reports a failed operation through the status text and the
    /// error signal.
    fn fail(mut self: Pin<&mut Self>, message: &QString) {
        self.as_mut().set_status_text(message.clone());
        self.as_mut().error_occurred(message.clone());
    }
}

/// The Q_PROPERTY values of one controller state.
#[derive(Default)]
struct Snapshot {
    file_path: QString,
    title: QString,
    artist: QString,
    bpm: f64,
    gap_ms: f64,
    format_version: QString,
    dirty: bool,
    read_only: bool,
    can_undo: bool,
    can_redo: bool,
    undo_text: QString,
    redo_text: QString,
    line_count: i32,
    current_line: i32,
    current_voice: i32,
    voice_count: i32,
    window_title: QString,
    is_loaded: bool,
    unsaved_steps: i32,
    audio_present: bool,
    audio_file_name: QString,
    has_vocals: bool,
    has_instrumental: bool,
    can_save: bool,
    can_save_as: bool,
    can_play: bool,
    can_play_line: bool,
    can_loop: bool,
    can_track: bool,
    can_speed: bool,
    can_repair: bool,
    can_properties: bool,
    can_export_anchors: bool,
    current_line_start_ms: f64,
    current_line_end_ms: f64,
    current_line_note_count: i32,
    current_line_rate: f64,
    current_line_health: i32,
    current_line_health_reason: QString,
    current_line_type: QString,
    recent_count: i32,
}

/// build_snapshot reads every Q_PROPERTY value from the controller, the save
/// service and the settings.
fn build_snapshot(controller: &EditorController, settings: &Settings) -> Snapshot {
    let enablement = shell::enablement(controller);
    let audio = shell::audio_state(controller);
    let (audio_present, audio_file_name) = match &audio {
        Some(shell::AudioState::Present) => (true, QString::default()),
        Some(shell::AudioState::Missing(name)) => (false, QString::from(name.clone())),
        _ => (false, QString::default()),
    };
    let mut snapshot = Snapshot {
        window_title: QString::from(shell::window_title(controller)),
        is_loaded: controller.is_loaded(),
        unsaved_steps: controller
            .loaded()
            .map(|loaded| loaded.history.unsaved_steps() as i32)
            .unwrap_or(0),
        audio_present,
        audio_file_name,
        has_vocals: shell::has_track(controller, shell::TrackKind::Vocals),
        has_instrumental: shell::has_track(controller, shell::TrackKind::Instrumental),
        can_save: enablement.save,
        can_save_as: enablement.save_as,
        can_play: enablement.play,
        can_play_line: enablement.play_line,
        can_loop: enablement.loop_playback,
        can_track: enablement.track,
        can_speed: enablement.speed,
        can_repair: enablement.repair,
        can_properties: enablement.properties,
        can_export_anchors: enablement.export_anchors,
        recent_count: settings.recent_files.len() as i32,
        ..Snapshot::default()
    };
    let Some(loaded) = controller.loaded() else {
        return snapshot;
    };
    let headers = Headers::from_document(&loaded.document);
    let history = &loaded.history;
    snapshot.file_path = QString::from(loaded.path.to_string_lossy().into_owned());
    snapshot.title = QString::from(headers.get("TITLE").unwrap_or_default());
    snapshot.artist = QString::from(headers.get("ARTIST").unwrap_or_default());
    snapshot.bpm = loaded.song.timing.map(|timing| timing.bpm).unwrap_or(0.0);
    snapshot.gap_ms = loaded
        .song
        .timing
        .map(|timing| timing.gap_ms)
        .unwrap_or(0.0);
    snapshot.format_version = QString::from(version_text(&loaded.song.version));
    snapshot.dirty = history.is_dirty();
    snapshot.read_only = loaded.read_only;
    snapshot.can_undo = enablement.undo;
    snapshot.can_redo = enablement.redo;
    snapshot.undo_text = QString::from(history.undo_label().unwrap_or_default());
    snapshot.redo_text = QString::from(history.redo_label().unwrap_or_default());
    snapshot.line_count = controller.line_count() as i32;
    snapshot.current_line = controller.current_line() as i32;
    snapshot.current_voice = controller
        .current_voice_index()
        .map(|index| index as i32)
        .unwrap_or(-1);
    snapshot.voice_count = controller.voice_count() as i32;
    if let Some(current) = line_rows(controller).into_iter().find(|row| row.is_current) {
        let duration_s = (current.end_ms - current.start_ms) / 1000.0;
        snapshot.current_line_start_ms = current.start_ms;
        snapshot.current_line_end_ms = current.end_ms;
        snapshot.current_line_note_count = current.note_count;
        snapshot.current_line_rate = if duration_s > 0.0 {
            current.note_count as f64 / duration_s
        } else {
            0.0
        };
        snapshot.current_line_health = current.health;
        snapshot.current_line_health_reason = current.health_reason;
        snapshot.current_line_type = current.type_summary;
    }
    snapshot
}

/// line_rows builds the lines pane rows of the loaded song.
fn line_rows(controller: &EditorController) -> Vec<LineRow> {
    let Some(loaded) = controller.loaded() else {
        return Vec::new();
    };
    let doc = &loaded.document;
    let song = &loaded.song;
    let timing = song.timing;
    let to_ms = |beat: f64| timing.map(|value| value.beat_to_ms(beat)).unwrap_or(beat);
    let current = controller.current_line();
    let mut rows = Vec::new();
    let mut global = 0usize;
    for voice in &song.voices {
        for line in &voice.lines {
            let first = line.notes.first().and_then(|id| song.note(doc, *id));
            let last_end = line
                .notes
                .iter()
                .rev()
                .find_map(|id| song.note(doc, *id))
                .map(|note| note.start + note.length as f64);
            let end_beat = line
                .end
                .and_then(|id| doc.rows.get(id.0 as usize))
                .and_then(|row| match &row.kind {
                    RowKind::PhraseEnd {
                        beat: Some(beat), ..
                    } => Some(*beat as f64),
                    _ => None,
                });
            let start_ms = first.map(|note| to_ms(note.start)).unwrap_or(0.0);
            let end_ms = end_beat.or(last_end).map(to_ms).unwrap_or(start_ms);
            let kinds: Vec<NoteType> = line
                .notes
                .iter()
                .filter_map(|id| song.note(doc, *id).map(|note| note.kind.clone()))
                .collect();
            let type_summary = match kinds.first() {
                None => ":",
                Some(first) if kinds.iter().all(|kind| kind == first) => type_char(first),
                Some(_) => "mixed",
            };
            let text = line.text(doc);
            rows.push(LineRow {
                line_number: global as i32 + 1,
                start_ms,
                end_ms,
                text: QString::from(text.clone()),
                search_key: text.to_lowercase(),
                type_summary: QString::from(type_summary),
                note_count: line.notes.len() as i32,
                health: 0,
                health_reason: QString::default(),
                anchor: QString::default(),
                is_current: global == current,
            });
            global += 1;
        }
    }
    rows
}

/// recent_rows builds the recent songs rows from the settings (UI_SPEC 4.1).
fn recent_rows(settings: &Settings) -> Vec<RecentRow> {
    settings
        .recent_files
        .iter()
        .map(|recent| RecentRow {
            title: QString::from(recent_title(&recent.path)),
            path_url: QString::from(
                QUrl::from_local_file(&QString::from(recent.path.to_string_lossy().into_owned()))
                    .to_string(),
            ),
            display_path: QString::from(display_path(&recent.path)),
            exists: recent.path.exists(),
            opened_text: usdx_relative_date_text(recent.opened as i64),
        })
        .collect()
}

/// recent_title reads the song headers for the "Artist - Title" line; a file
/// that cannot be read or parsed falls back to its name.
fn recent_title(path: &Path) -> String {
    let title = fs::read(path)
        .ok()
        .and_then(|bytes| decode(&bytes).ok())
        .map(usdx_format::rows::classify)
        .map(|document| Headers::from_document(&document))
        .map(|headers| {
            [headers.get("TITLE"), headers.get("ARTIST")]
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

/// display_path renders a path with `~` for the home folder (UI_SPEC 4.1).
fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return format!("~{}", rest.display());
    }
    path.to_string_lossy().into_owned()
}

/// load_settings reads the settings file and prunes missing recent songs at
/// startup (UI_SPEC 4.1), writing back when the prune changed the list.
fn load_settings() -> Settings {
    let Some(path) = settings_path() else {
        return Settings::default();
    };
    let mut settings = Settings::load(&path);
    if settings.drop_missing_recent_files()
        && let Err(error) = settings.save(&path)
    {
        log::warn!(
            "The settings could not be written to {}: {error}",
            path.display()
        );
    }
    settings
}

/// persist_settings writes the settings file; a failure is logged, the
/// session continues with the in-memory values.
fn persist_settings(settings: &Settings) {
    if let Some(path) = settings_path()
        && let Err(error) = settings.save(&path)
    {
        log::warn!(
            "The settings could not be written to {}: {error}",
            path.display()
        );
    }
}

/// version_text renders the format version for the UI; the unversioned
/// format shows as empty.
fn version_text(version: &Version) -> String {
    match version {
        Version::Unversioned => String::new(),
        Version::V(major, minor, patch) => format!("{major}.{minor}.{patch}"),
        Version::Invalid(value) => value.clone(),
    }
}

/// type_char maps a note type to its summary character (plan section 8.3);
/// unknown types display as freestyle (the Phase 1 decision).
fn type_char(kind: &NoteType) -> &'static str {
    match kind {
        NoteType::Normal => ":",
        NoteType::Golden => "*",
        NoteType::Freestyle | NoteType::Other(_) => "F",
        NoteType::Rap => "R",
        NoteType::RapGolden => "G",
    }
}

/// qurl_to_path converts a file url to a path; other urls keep their text
/// form so that plain paths round-trip.
fn qurl_to_path(url: &QUrl) -> PathBuf {
    let text = match url.to_local_file() {
        Some(local) => String::from(&local),
        None => url.to_string(),
    };
    PathBuf::from(text)
}
