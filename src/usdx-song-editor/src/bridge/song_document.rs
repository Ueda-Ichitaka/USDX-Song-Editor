//! The SongDocument, LinesModel, NotesModel, PhraseEndsModel, RecentSongsModel
//! and Player bridges.
//!
//! About me: this cxx-qt bridge exposes the editor controller and the
//! settings to QML. The SongDocument QObject holds the EditorController, the
//! SaveService and the Settings and mirrors their state into Q_PROPERTIES
//! after every state change; the LinesModel QAbstractListModel feeds the
//! lines pane, the NotesModel and PhraseEndsModel feed the note sheet's
//! delegates and phrase end handles (task P5-03) and the RecentSongsModel
//! feeds the recent songs list; the Player QObject (declared here next to
//! the document it follows, its logic living in bridge/player.rs) drives
//! the audio playback of P4-04. The bridges are thin: the logic lives in
//! the plain-Rust controller (src/controller), this file only converts
//! between Qt and Rust types and routes the guarded transition results of
//! the shell (UI_SPEC 5.1, 5.2) to the right channel - the save error
//! banner property, the open error dialog signal or the changed-on-disk
//! dialog signal. The editing invokables cover the selection, the note
//! moves and actions (P5-04 to P5-06) and the line text: the Edit line
//! dialog's text, preview JSON and apply and the inline syllable edit
//! (P6-02); the health and
//! anchor roles stay neutral (0, empty) until the analysis (P7) and repair
//! (P6.3) phases fill them.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_QString_QVariant, QHashPair_i32_QByteArray, QList, QModelIndex,
    QString, QUrl, QVariant,
};
use usdx_format::headers::{Headers, Version};
use usdx_format::rows::{NoteType, RowId};

use crate::bridge::player::PlayerCore;
use crate::controller::editing::{Edge, SelectionMode, snapped_move, snapped_resize};
use crate::controller::inspector::{InspectorState, inspector_state};
use crate::controller::line_text;
use crate::controller::model_update::{RowUpdate, row_update};
use crate::controller::notes::{
    PhraseEndRow, VisibleNote, all_lines, song_end_beat, type_char, visible_notes,
    visible_phrase_ends,
};
use crate::controller::save::SaveError;
use crate::controller::settings::{Settings, settings_path};
use crate::controller::sheet::{LineNotes, line_notes};
use crate::controller::shell;
use crate::controller::view::{TimeView, follow_line};
use crate::controller::{EditorController, SaveService};

/// The QVariantList (QList<QVariant>) the sheet note properties use; the
/// bridge declares the same cxx-qt-lib type.
type QVariantList = cxx_qt_lib::QList<QVariant>;

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
        include!("cxx-qt-lib/core/qlist/qlist_QVariant.h");
        /// A QVariantList (QList<QVariant>) from cxx_qt_lib.
        type QList_QVariant = cxx_qt_lib::QList<QVariant>;
        include!("cxx-qt-lib/core/qlist/qlist_i32.h");
        /// A QList<i32>, the roles list of a dataChanged call.
        type QList_i32 = cxx_qt_lib::QList<i32>;
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

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
        type NotesModel = super::NotesModelRust;

        /// Return the data for one note row.
        #[qinvokable]
        #[cxx_override]
        fn data(self: &NotesModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Return the role names of the note rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &NotesModel) -> QHash_i32_QByteArray;

        /// Return the number of note rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &NotesModel, parent: &QModelIndex) -> i32;

        /// Refill the rows from the current line and its neighbours.
        #[qinvokable]
        fn rebuild(self: Pin<&mut NotesModel>);

        /// Set the drag preview of one note row (plan 8.3): start and length
        /// in beats, or -1 for the length to clear the preview.
        #[qinvokable]
        #[cxx_name = "setGhost"]
        fn set_ghost(self: Pin<&mut NotesModel>, row_id: i64, start: f64, length: i64);
    }

    unsafe extern "RustQt" {
        /// The model index of one row, for the ghost roles' dataChanged.
        #[inherit]
        #[cxx_name = "index"]
        fn index_of(self: &NotesModel, row: i32, column: i32) -> QModelIndex;

        /// Emit a dataChanged in place: the in-place update of a rebuild
        /// with the same rows (user decision 2026-11-15) and the ghost
        /// roles keep the delegates alive.
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn notes_data_changed(
            self: Pin<&mut NotesModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );

        /// The model index of one lines pane row.
        #[inherit]
        #[cxx_name = "index"]
        fn lines_index_of(self: &LinesModel, row: i32, column: i32) -> QModelIndex;

        /// The lines pane's in-place dataChanged.
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn lines_data_changed(
            self: Pin<&mut LinesModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );

        /// The model index of one phrase end row.
        #[inherit]
        #[cxx_name = "index"]
        fn phrase_ends_index_of(self: &PhraseEndsModel, row: i32, column: i32) -> QModelIndex;

        /// The phrase end handles' in-place dataChanged.
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn phrase_ends_data_changed(
            self: Pin<&mut PhraseEndsModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    /// The roles of the note rows (plan section 8.3).
    #[qenum(NotesModel)]
    enum NoteRoles {
        /// The note's row id.
        NoteId,
        /// The absolute start beat.
        Start,
        /// The length in beats.
        Length,
        /// The pitch as a MIDI number (C4 = 60, UI_SPEC 3.3).
        Pitch,
        /// The note type as its chip character (`:`, `*`, `F`, `R`, `G`).
        Type,
        /// The syllable text, a tab read as a space.
        Text,
        /// The line the note belongs to: -1 previous, 0 current, +1 next.
        LineOffset,
        /// Whether the note is in the selection.
        Selected,
        /// The dragged preview start beat, -1 when the note is not dragged.
        GhostStart,
        /// The dragged preview length, -1 when the note is not dragged.
        GhostLength,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
        type PhraseEndsModel = super::PhraseEndsModelRust;

        /// Return the data for one phrase end row.
        #[qinvokable]
        #[cxx_override]
        fn data(self: &PhraseEndsModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Return the role names of the phrase end rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &PhraseEndsModel) -> QHash_i32_QByteArray;

        /// Return the number of phrase end rows.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &PhraseEndsModel, parent: &QModelIndex) -> i32;

        /// Refill the rows from the current line and its neighbours.
        #[qinvokable]
        fn rebuild(self: Pin<&mut PhraseEndsModel>);
    }

    /// The roles of the phrase end rows (plan section 8.3).
    #[qenum(PhraseEndsModel)]
    enum PhraseEndRoles {
        /// The end-of-phrase row's id.
        RowId,
        /// The absolute beat of the marker.
        Beat,
        /// The lowest allowed beat: the previous note's end.
        Min,
        /// The highest allowed beat: the next note's start.
        Max,
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

    /// How one click on a note changes the selection (plan 8.4: a click
    /// selects, Ctrl adds, Shift extends in time).
    #[qenum(SongDocument)]
    enum SelectionKind {
        /// The clicked note becomes the whole selection.
        Set,
        /// The clicked note joins the selection.
        Add,
        /// The notes between the anchor and the clicked note join the
        /// selection.
        Extend,
    }

    /// The note edge an edge drag works on (plan 8.4).
    #[qenum(SongDocument)]
    enum ResizeEdge {
        /// The note's start beat.
        Start,
        /// The note's end beat.
        End,
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

        /// The same on the notes model.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn notes_begin_reset_model(self: Pin<&mut NotesModel>);

        /// Inherited endResetModel.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn notes_end_reset_model(self: Pin<&mut NotesModel>);

        /// The same on the phrase ends model.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn phrase_ends_begin_reset_model(self: Pin<&mut PhraseEndsModel>);

        /// Inherited endResetModel.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn phrase_ends_end_reset_model(self: Pin<&mut PhraseEndsModel>);
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
        // The time range the note sheet shows for the current line
        // (UI_SPEC 3.2), until the SheetViewport of P5-01 takes it over.
        #[qproperty(f64, view_start_ms, cxx_name = "viewStartMs")]
        #[qproperty(f64, view_end_ms, cxx_name = "viewEndMs")]
        // The last note end or phrase end beat of the song: the horizontal
        // scrollbar's full range (user decision 2026-11-15).
        #[qproperty(f64, song_end_beat, cxx_name = "songEndBeat")]
        #[qproperty(i32, current_line_note_count, cxx_name = "currentLineNoteCount")]
        #[qproperty(f64, current_line_rate, cxx_name = "currentLineRate")]
        #[qproperty(i32, current_line_health, cxx_name = "currentLineHealth")]
        #[qproperty(
            QString,
            current_line_health_reason,
            cxx_name = "currentLineHealthReason"
        )]
        #[qproperty(QString, current_line_type, cxx_name = "currentLineType")]
        // The lyric notes of the current line and its global neighbours for
        // the sheet's lyrics lane (UI_SPEC 3.6, task P5-02), as parallel
        // lists of start beats, texts and current flags; the pitch range of
        // the current line as low and high, -1 when no line is current.
        #[qproperty(QList_QVariant, sheet_note_starts, cxx_name = "sheetNoteStarts")]
        #[qproperty(QList_QVariant, sheet_note_texts, cxx_name = "sheetNoteTexts")]
        #[qproperty(QList_QVariant, sheet_note_currents, cxx_name = "sheetNoteCurrents")]
        #[qproperty(i32, sheet_pitch_low, cxx_name = "sheetPitchLow")]
        #[qproperty(i32, sheet_pitch_high, cxx_name = "sheetPitchHigh")]
        #[qproperty(i32, window_width, cxx_name = "windowWidth")]
        #[qproperty(i32, window_height, cxx_name = "windowHeight")]
        #[qproperty(i32, recent_count, cxx_name = "recentCount")]
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        #[qproperty(QString, app_version, cxx_name = "appVersion")]
        #[qproperty(*mut LinesModel, lines_model, cxx_name = "linesModel")]
        #[qproperty(*mut RecentSongsModel, recent_model, cxx_name = "recentModel")]
        // The note rows and the phrase end rows of the note sheet (plan 8.3,
        // task P5-03); QML binds the models here on startup.
        #[qproperty(*mut NotesModel, notes_model, cxx_name = "notesModel")]
        #[qproperty(*mut PhraseEndsModel, phrase_ends_model, cxx_name = "phraseEndsModel")]
        // The inspector contents of the current selection (UI_SPEC 4.3,
        // task P5-05): a QVariant hash with the keys label, editable,
        // single, count, noteId, typeCode, start, length, pitch,
        // pitchCommon, pitchName, text and pitchEditable.
        #[qproperty(QVariant, inspector)]
        // Whether autoplay is on (the Shift+A key, plan 8.6, task P5-06).
        #[qproperty(bool, autoplay)]
        // Whether the selection is exactly every note of the current line;
        // the header says "Whole line selected" then (UI_SPEC 4.3).
        #[qproperty(bool, whole_line_selected, cxx_name = "wholeLineSelected")]
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

        /// Emitted with the UI_SPEC 6.4 text when an editing action was
        /// refused; the shell shows it as a passive notification.
        #[qsignal]
        #[cxx_name = "editRefused"]
        fn edit_refused(self: Pin<&mut SongDocument>, message: QString);

        /// Emitted when a line was reached while autoplay is on (plan 8.6,
        /// task P5-06): the padded play range of the line; the editor page
        /// hands it to the player's playRange.
        #[qsignal]
        #[cxx_name = "autoPlayRange"]
        fn autoplay_range(self: Pin<&mut SongDocument>, start_ms: f64, end_ms: f64);
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

        /// Change the selection from one click on a note (plan 8.4).
        #[qinvokable]
        #[cxx_name = "selectNote"]
        fn select_note(self: Pin<&mut SongDocument>, id: i64, mode: SelectionKind);

        /// Empty the selection (a click on the empty sheet).
        #[qinvokable]
        #[cxx_name = "clearSelection"]
        fn clear_selection(self: Pin<&mut SongDocument>);

        /// Select every note intersecting the rubber band rectangle (plan
        /// 8.4, user decision 2026-11-14): the beats and MIDI pitches of
        /// the band, bounds may arrive swapped.
        #[qinvokable]
        #[cxx_name = "selectRange"]
        fn select_range(
            self: Pin<&mut SongDocument>,
            start_beat: f64,
            end_beat: f64,
            low_pitch: i32,
            high_pitch: i32,
        );

        /// Move the selection by the committed drag deltas (plan 8.4: one
        /// undo step per drag). Reports the refusal through `editRefused`.
        #[qinvokable]
        #[cxx_name = "moveSelection"]
        fn move_selection(self: Pin<&mut SongDocument>, d_beat: i64, d_pitch: i64) -> bool;

        /// Shift the selection and every later row of their voice as a
        /// block (the Ctrl+Shift arrow semantics).
        #[qinvokable]
        #[cxx_name = "moveSelectionAndFollowing"]
        fn move_selection_and_following(self: Pin<&mut SongDocument>, d_beat: i64) -> bool;

        /// Resize one note edge by the committed drag delta. Reports the
        /// refusal through `editRefused`.
        #[qinvokable]
        #[cxx_name = "resizeNote"]
        fn resize_note(
            self: Pin<&mut SongDocument>,
            id: i64,
            edge: ResizeEdge,
            d_beat: i64,
        ) -> bool;

        /// Compute the snapped committed deltas of a body drag (plan 8.4,
        /// user decision 2026-11-14: the ghost preview and the commit agree,
        /// the rounding happens after the snap) and return them in the hash
        /// keys dBeat and dPitch; the delegate draws its ghost from them.
        #[qinvokable]
        #[cxx_name = "previewMove"]
        fn preview_move(
            self: Pin<&mut SongDocument>,
            row_id: i64,
            d_beat: f64,
            d_pitch: i64,
            snap_on: bool,
            px_per_beat: f64,
        ) -> QHash_QString_QVariant;

        /// Compute the snapped committed delta of an edge drag and return
        /// it; the delegate draws its ghost from it. An unknown id or no song
        /// returns the plain rounded delta.
        #[qinvokable]
        #[cxx_name = "previewResize"]
        fn preview_resize(
            self: Pin<&mut SongDocument>,
            row_id: i64,
            edge: ResizeEdge,
            d_beat: f64,
            snap_on: bool,
            px_per_beat: f64,
        ) -> i64;

        /// Insert a note after the last selected one, or after the current
        /// line's last note without a selection (the Insert key, task P5-05).
        #[qinvokable]
        #[cxx_name = "insertNote"]
        fn insert_note(self: Pin<&mut SongDocument>) -> bool;

        /// Delete every selected note (the Delete key).
        #[qinvokable]
        #[cxx_name = "deleteSelection"]
        fn delete_selection(self: Pin<&mut SongDocument>) -> bool;

        /// Split the single selected note at the beat (the S key at the
        /// playhead beat); with no note or several selected a no-op.
        #[qinvokable]
        #[cxx_name = "splitSelectionAt"]
        fn split_selection_at(self: Pin<&mut SongDocument>, beat: i64) -> bool;

        /// Merge the single selected note with the next one (the M key);
        /// with no note or several selected a no-op.
        #[qinvokable]
        #[cxx_name = "mergeSelection"]
        fn merge_selection(self: Pin<&mut SongDocument>) -> bool;

        /// Set the type of every selected note (the type keys and the
        /// inspector's type buttons); the code "F" toggles freestyle.
        #[qinvokable]
        #[cxx_name = "setSelectionType"]
        fn set_selection_type(self: Pin<&mut SongDocument>, code: QString) -> bool;

        /// Set the type of every note of the current line (the line
        /// header's type combo); "F" sets freestyle.
        #[qinvokable]
        #[cxx_name = "setLineType"]
        fn set_line_type(self: Pin<&mut SongDocument>, code: QString) -> bool;

        /// Toggle every selected note between freestyle and normal (the F
        /// key; setSelectionType("F") routes here too).
        #[qinvokable]
        #[cxx_name = "toggleFreestyle"]
        fn toggle_freestyle(self: Pin<&mut SongDocument>) -> bool;

        /// Set one note's syllable (the inspector's Text field).
        #[qinvokable]
        #[cxx_name = "setNoteText"]
        fn set_note_text(self: Pin<&mut SongDocument>, row_id: i64, text: QString) -> bool;

        /// The line's text in the line text syntax for the Edit line
        /// dialog's field (global line index), empty past the last line.
        #[qinvokable]
        #[cxx_name = "lineText"]
        fn line_text(self: &SongDocument, line: i32) -> QString;

        /// The Edit line dialog's preview of a text as JSON: the syllable
        /// chips, the count changes with their note picks, the count
        /// message, the refusal reason and whether Apply is possible.
        /// `picks` lists the picked line note per count change,
        /// comma-separated (-1: automatic).
        #[qinvokable]
        #[cxx_name = "previewLineText"]
        fn preview_line_text(
            self: &SongDocument,
            line: i32,
            text: QString,
            picks: QString,
        ) -> QString;

        /// Apply a line text as one undo step (the Edit line dialog's
        /// Apply); a refusal reports through editRefused.
        #[qinvokable]
        #[cxx_name = "applyLineText"]
        fn apply_line_text(
            self: Pin<&mut SongDocument>,
            line: i32,
            text: QString,
            picks: QString,
        ) -> bool;

        /// The note's syllable without its surrounding spaces, for the
        /// inline field over the lyric label.
        #[qinvokable]
        #[cxx_name = "syllableForEditing"]
        fn syllable_for_editing(self: &SongDocument, row_id: i64) -> QString;

        /// Commit the inline field: the typed syllable replaces the note's
        /// text between its own leading and trailing spaces.
        #[qinvokable]
        #[cxx_name = "setNoteSyllable"]
        fn set_note_syllable(self: Pin<&mut SongDocument>, row_id: i64, text: QString) -> bool;

        /// The row id of the note whose lyric label covers the beat (the
        /// last note of the current voice starting at or before it), -1
        /// when none.
        #[qinvokable]
        #[cxx_name = "noteAtLyricBeat"]
        fn note_at_lyric_beat(self: &SongDocument, beat: f64) -> i64;

        /// Insert or remove the end-of-phrase marker before the selected
        /// note (the B key).
        #[qinvokable]
        #[cxx_name = "togglePhraseEndBeforeSelection"]
        fn toggle_phrase_end_before_selection(self: Pin<&mut SongDocument>) -> bool;

        /// Move one end-of-phrase marker to the beat (the phrase end
        /// handle's drag commit).
        #[qinvokable]
        #[cxx_name = "movePhraseEnd"]
        fn move_phrase_end(self: Pin<&mut SongDocument>, row_id: i64, beat: i64) -> bool;

        /// Shift the pitch of every selected note by semitone steps (the
        /// inspector's Pitch field on a multi selection).
        #[qinvokable]
        #[cxx_name = "transposeSelection"]
        fn transpose_selection(self: Pin<&mut SongDocument>, semitones: i64) -> bool;

        /// Select the note after the selection's focus in the current line
        /// (the Right key, plan 8.6); the steps stay in the line.
        #[qinvokable]
        #[cxx_name = "selectNextNote"]
        fn select_next_note(self: Pin<&mut SongDocument>);

        /// Select the note before the selection's focus in the current line
        /// (the Left key); Left at the first note selects the whole line.
        #[qinvokable]
        #[cxx_name = "selectPreviousNote"]
        fn select_previous_note(self: Pin<&mut SongDocument>);

        /// Change the length of every selected note by the beats (the
        /// Alt+Left/Right keys).
        #[qinvokable]
        #[cxx_name = "resizeSelectionEnd"]
        fn resize_selection_end(self: Pin<&mut SongDocument>, d_beat: i64) -> bool;

        /// Turn autoplay on or off (the Shift+A key); switching it on
        /// emits autoPlayRange for the current line at once.
        #[qinvokable]
        #[cxx_name = "toggleAutoplay"]
        fn toggle_autoplay(self: Pin<&mut SongDocument>);

        /// Jump to the global line index, clamped into range.
        #[qinvokable]
        #[cxx_name = "goToLine"]
        fn go_to_line(self: Pin<&mut SongDocument>, index: i32);

        /// Make the line being sung at the playback position the current
        /// line (the playback follow); never starts the autoplay.
        #[qinvokable]
        #[cxx_name = "followPlayback"]
        fn follow_playback(self: Pin<&mut SongDocument>, position_ms: f64);

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
        #[qproperty(bool, range_active, cxx_name = "rangeActive")]
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

        /// Follow a newly opened song: drop the old song's playback, pick
        /// the track the settings choose for the new song and publish the
        /// track availability; QML calls it on the document's songLoaded.
        #[qinvokable]
        #[cxx_name = "followDocument"]
        fn follow_document(self: Pin<&mut Player>);
    }

    impl cxx_qt::Threading for Player {}
}

use core::pin::Pin;

use cxx_qt::CxxQtType;

use crate::bridge::song_document::qobject::{
    ActivationKind, ChangedOnDiskKind, LinesModel, NoteRoles, NotesModel, PhraseEndRoles,
    PhraseEndsModel, RecentRoles, RecentSongsModel, ResizeEdge, Roles, SaveChangesKind,
    SelectionKind, SongDocument, TransitionKind, usdx_relative_date_text,
};

/// The Rust side of the Player: the playback core plus the Q_PROPERTY
/// storage. The logic lives in the impls in bridge/player.rs.
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
    pub(crate) range_active: bool,
    pub(crate) rate: f64,
    pub(crate) track: i32,
    pub(crate) has_vocals: bool,
    pub(crate) has_instrumental: bool,
}

impl Default for PlayerRust {
    /// The property storage starts with the core's values, so QML reads the
    /// core's 1x rate and track before the first publish.
    fn default() -> Self {
        let core = PlayerCore::new();
        Self {
            position_ms: core.position_ms(),
            duration_ms: core.duration_ms(),
            playing: core.playing(),
            range_active: core.range_active(),
            rate: core.rate(),
            track: core.track(),
            has_vocals: false,
            has_instrumental: false,
            document: std::ptr::null_mut(),
            loading: false,
            play_requested: false,
            pending_range: None,
            track_resolved: false,
            loaded_song_path: String::new(),
            core,
        }
    }
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
#[derive(Clone, PartialEq)]
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

/// One row of the notes model: the controller's visible note plus whether
/// it sits in the selection.
#[derive(PartialEq)]
pub struct NoteRow {
    note: VisibleNote,
    selected: bool,
}

/// The Rust side of the NotesModel (plan section 8.3): the notes of the
/// current line and its global neighbours, plus the drag preview of one
/// note (None when none).
#[derive(Default)]
pub struct NotesModelRust {
    rows: Vec<NoteRow>,
    /// The dragged note's preview: the row id, the preview start beat and
    /// the preview length.
    ghost: Option<(i64, f64, i64)>,
    document: *mut SongDocument,
}

/// The Rust side of the PhraseEndsModel (plan section 8.3): the visible
/// end-of-phrase markers with their drag bounds.
#[derive(Default)]
pub struct PhraseEndsModelRust {
    rows: Vec<PhraseEndRow>,
    document: *mut SongDocument,
}

/// The Rust side of the SongDocument: the controller, the save service, the
/// settings and the Q_PROPERTY storage.
pub struct SongDocumentRust {
    controller: EditorController,
    /// The sheet's time view and the song it belongs to; a different song
    /// starts a fresh view.
    view: Option<(std::path::PathBuf, TimeView)>,
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
    view_start_ms: f64,
    view_end_ms: f64,
    song_end_beat: f64,
    current_line_note_count: i32,
    current_line_rate: f64,
    current_line_health: i32,
    current_line_health_reason: QString,
    current_line_type: QString,
    sheet_note_starts: QVariantList,
    sheet_note_texts: QVariantList,
    sheet_note_currents: QVariantList,
    sheet_pitch_low: i32,
    sheet_pitch_high: i32,
    window_width: i32,
    window_height: i32,
    recent_count: i32,
    status_text: QString,
    /// The application version, shown on the About page.
    app_version: QString,
    lines_model: *mut LinesModel,
    recent_model: *mut RecentSongsModel,
    notes_model: *mut NotesModel,
    phrase_ends_model: *mut PhraseEndsModel,
    /// The inspector contents of the current selection (a QVariant hash,
    /// task P5-05).
    inspector: QVariant,
    /// Whether autoplay is on (task P5-06).
    autoplay: bool,
    /// Whether the selection is exactly the current line's notes (the
    /// header says "Whole line selected" then, task P5-06).
    whole_line_selected: bool,
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
            autoplay: snapshot.autoplay,
            whole_line_selected: snapshot.whole_line_selected,
            current_line_start_ms: snapshot.current_line_start_ms,
            current_line_end_ms: snapshot.current_line_end_ms,
            view_start_ms: 0.0,
            view_end_ms: 0.0,
            song_end_beat: 0.0,
            view: None,
            current_line_note_count: snapshot.current_line_note_count,
            current_line_rate: snapshot.current_line_rate,
            current_line_health: snapshot.current_line_health,
            current_line_health_reason: snapshot.current_line_health_reason,
            current_line_type: snapshot.current_line_type,
            sheet_note_starts: QVariantList::default(),
            sheet_note_texts: QVariantList::default(),
            sheet_note_currents: QVariantList::default(),
            sheet_pitch_low: -1,
            sheet_pitch_high: -1,
            window_width: width,
            window_height: height,
            recent_count: snapshot.recent_count,
            status_text: QString::default(),
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            lines_model: std::ptr::null_mut(),
            recent_model: std::ptr::null_mut(),
            notes_model: std::ptr::null_mut(),
            phrase_ends_model: std::ptr::null_mut(),
            inspector: QVariant::default(),
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
        // The same lines in the same order update in place, so the lines
        // pane keeps its delegates (user decision 2026-11-15).
        match row_update(&self.rows, &rows, |row| row.line_number) {
            RowUpdate::Unchanged => {}
            RowUpdate::Changed { first, last } => {
                self.as_mut().rust_mut().rows = rows;
                let top = self.lines_index_of(first as i32, 0);
                let bottom = self.lines_index_of(last as i32, 0);
                self.as_mut()
                    .lines_data_changed(&top, &bottom, &QList::<i32>::default());
            }
            RowUpdate::Reset => unsafe {
                self.as_mut().begin_reset_model();
                self.as_mut().rust_mut().rows = rows;
                self.as_mut().end_reset_model();
            },
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

impl qobject::NotesModel {
    /// data returns the value of one note row for one role.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        let note = &row.note;
        match (NoteRoles { repr: role }) {
            NoteRoles::NoteId => QVariant::from(&(note.row_id.0 as i64)),
            NoteRoles::Start => QVariant::from(&note.start),
            NoteRoles::Length => QVariant::from(&note.length),
            NoteRoles::Pitch => QVariant::from(&note.pitch),
            NoteRoles::Type => QVariant::from(&QString::from(note.kind)),
            NoteRoles::Text => QVariant::from(&QString::from(note.text.as_str())),
            NoteRoles::LineOffset => QVariant::from(&note.line_offset),
            NoteRoles::Selected => QVariant::from(&row.selected),
            NoteRoles::GhostStart => {
                let (ghost_id, ghost_start, _) = self.ghost.unwrap_or((-1, -1.0, -1));
                let beat = if ghost_id == note.row_id.0 as i64 {
                    ghost_start
                } else {
                    -1.0
                };
                QVariant::from(&beat)
            }
            NoteRoles::GhostLength => {
                let (ghost_id, _, ghost_length) = self.ghost.unwrap_or((-1, -1.0, -1));
                let length = if ghost_id == note.row_id.0 as i64 {
                    ghost_length
                } else {
                    -1
                };
                QVariant::from(&length)
            }
            _ => QVariant::default(),
        }
    }

    /// role_names returns the role numbers of the note rows.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(NoteRoles::NoteId.repr, QByteArray::from("noteId"));
        roles.insert(NoteRoles::Start.repr, QByteArray::from("start"));
        roles.insert(NoteRoles::Length.repr, QByteArray::from("length"));
        roles.insert(NoteRoles::Pitch.repr, QByteArray::from("pitch"));
        roles.insert(NoteRoles::Type.repr, QByteArray::from("type"));
        roles.insert(NoteRoles::Text.repr, QByteArray::from("text"));
        roles.insert(NoteRoles::LineOffset.repr, QByteArray::from("lineOffset"));
        roles.insert(NoteRoles::Selected.repr, QByteArray::from("selected"));
        roles.insert(NoteRoles::GhostStart.repr, QByteArray::from("ghostStart"));
        roles.insert(NoteRoles::GhostLength.repr, QByteArray::from("ghostLength"));
        roles
    }

    /// row_count returns the number of note rows.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }

    /// rebuild refills the rows from every line of the song (user decision
    /// 2026-11-15: the sheet paints the whole song), with the selection
    /// state of each note.
    pub fn rebuild(mut self: Pin<&mut Self>) {
        let rows = match unsafe { self.as_ref().document().as_ref() } {
            Some(document) => {
                let controller = &document.rust().controller;
                let (lines, current) = all_lines(controller);
                match controller.loaded() {
                    Some(loaded) => {
                        let selection = controller.selection();
                        visible_notes(&loaded.document, &loaded.song, &lines, current)
                            .into_iter()
                            .map(|note| NoteRow {
                                selected: selection.contains(&note.row_id),
                                note,
                            })
                            .collect()
                    }
                    None => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        // The same notes in the same order (a selection, a line change, a
        // move, a resize, a type change) update in place, so no note
        // delegate is recreated (user decision 2026-11-15); inserts,
        // deletes, splits and merges reset.
        match row_update(&self.rows, &rows, |row| row.note.row_id) {
            RowUpdate::Unchanged => {}
            RowUpdate::Changed { first, last } => {
                self.as_mut().rust_mut().rows = rows;
                let top = self.index_of(first as i32, 0);
                let bottom = self.index_of(last as i32, 0);
                self.as_mut()
                    .notes_data_changed(&top, &bottom, &QList::<i32>::default());
            }
            RowUpdate::Reset => unsafe {
                self.as_mut().notes_begin_reset_model();
                self.as_mut().rust_mut().rows = rows;
                self.as_mut().notes_end_reset_model();
            },
        }
    }

    /// set_ghost stores the drag preview of one note and emits the ghost
    /// roles as a dataChanged over all rows: a structural reset during a
    /// drag would destroy the delegate that is being dragged. A negative
    /// row id or length clears the preview.
    pub fn set_ghost(mut self: Pin<&mut Self>, row_id: i64, start: f64, length: i64) {
        self.as_mut().rust_mut().ghost = if row_id >= 0 && length >= 0 {
            Some((row_id, start, length))
        } else {
            None
        };
        let count = self.as_ref().rust().rows.len() as i32;
        if count == 0 {
            return;
        }
        let mut roles = QList::<i32>::default();
        roles.append(NoteRoles::GhostStart.repr);
        roles.append(NoteRoles::GhostLength.repr);
        let top_left = self.index_of(0, 0);
        let bottom_right = self.index_of(count - 1, 0);
        self.notes_data_changed(&top_left, &bottom_right, &roles);
    }
}

impl qobject::PhraseEndsModel {
    /// data returns the value of one phrase end row for one role.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        match (PhraseEndRoles { repr: role }) {
            PhraseEndRoles::RowId => QVariant::from(&(row.row_id.0 as i64)),
            PhraseEndRoles::Beat => QVariant::from(&row.beat),
            PhraseEndRoles::Min => QVariant::from(&row.min),
            PhraseEndRoles::Max => QVariant::from(&row.max),
            _ => QVariant::default(),
        }
    }

    /// role_names returns the role numbers of the phrase end rows.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(PhraseEndRoles::RowId.repr, QByteArray::from("rowId"));
        roles.insert(PhraseEndRoles::Beat.repr, QByteArray::from("beat"));
        roles.insert(PhraseEndRoles::Min.repr, QByteArray::from("min"));
        roles.insert(PhraseEndRoles::Max.repr, QByteArray::from("max"));
        roles
    }

    /// row_count returns the number of phrase end rows.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }

    /// rebuild refills the rows from every line of the song (user decision
    /// 2026-11-15: the sheet paints the whole song).
    pub fn rebuild(mut self: Pin<&mut Self>) {
        let rows = match unsafe { self.as_ref().document().as_ref() } {
            Some(document) => {
                let controller = &document.rust().controller;
                let (lines, _current) = all_lines(controller);
                match controller.loaded() {
                    Some(loaded) => visible_phrase_ends(&loaded.document, &loaded.song, &lines),
                    None => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        // The same markers in the same order update in place.
        match row_update(&self.rows, &rows, |row| row.row_id) {
            RowUpdate::Unchanged => {}
            RowUpdate::Changed { first, last } => {
                self.as_mut().rust_mut().rows = rows;
                let top = self.phrase_ends_index_of(first as i32, 0);
                let bottom = self.phrase_ends_index_of(last as i32, 0);
                self.as_mut()
                    .phrase_ends_data_changed(&top, &bottom, &QList::<i32>::default());
            }
            RowUpdate::Reset => unsafe {
                self.as_mut().phrase_ends_begin_reset_model();
                self.as_mut().rust_mut().rows = rows;
                self.as_mut().phrase_ends_end_reset_model();
            },
        }
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

    /// undo reverts the last edit step and rebuilds the song view (the
    /// controller rebuilds; the song view follows the document).
    pub fn undo(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.undo();
        self.as_mut().refresh();
    }

    /// redo re-applies the last undone edit step.
    pub fn redo(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.redo();
        self.as_mut().refresh();
    }

    /// edit_allowed checks the commit guards: an edit needs a loaded song
    /// that is not read-only. Without a song it is a silent no-op; a
    /// read-only song reports through `editRefused`.
    fn edit_allowed(mut self: Pin<&mut Self>) -> bool {
        let (loaded, read_only) = {
            let rust = self.rust();
            (
                rust.controller.loaded().is_some(),
                rust.controller.read_only(),
            )
        };
        if !loaded {
            return false;
        }
        if read_only {
            let message = QString::from("The song is read-only.");
            self.as_mut().edit_refused(message);
            return false;
        }
        true
    }

    /// finish_edit refreshes after an applied edit or reports the refusal
    /// through `editRefused` with its UI_SPEC 6.4 text.
    fn finish_edit(mut self: Pin<&mut Self>, result: Result<(), usdx_edit::EditError>) -> bool {
        match result {
            Ok(()) => {
                self.as_mut().refresh();
                true
            }
            Err(error) => {
                let message = QString::from(shell::edit_failure_reason(&error));
                self.as_mut().edit_refused(message);
                false
            }
        }
    }

    /// select_note applies one click's selection change (plan 8.4).
    pub fn select_note(mut self: Pin<&mut Self>, id: i64, mode: SelectionKind) {
        let Some(id) = row_id_of(id) else {
            return;
        };
        let mode = match (SelectionKind { repr: mode.repr }) {
            SelectionKind::Set => SelectionMode::Set,
            SelectionKind::Add => SelectionMode::Add,
            SelectionKind::Extend => SelectionMode::Extend,
            _ => return,
        };
        self.as_mut().rust_mut().controller.select_note(id, mode);
        self.as_mut().refresh();
    }

    /// clear_selection empties the selection (a click on the empty sheet).
    pub fn clear_selection(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.clear_selection();
        self.as_mut().refresh();
    }

    /// select_range replaces the selection with every note intersecting
    /// the band (plan 8.4).
    pub fn select_range(
        mut self: Pin<&mut Self>,
        start_beat: f64,
        end_beat: f64,
        low_pitch: i32,
        high_pitch: i32,
    ) {
        self.as_mut()
            .rust_mut()
            .controller
            .select_range(start_beat, end_beat, low_pitch, high_pitch);
        self.as_mut().refresh();
    }

    /// move_selection moves the selection by the committed drag deltas.
    pub fn move_selection(mut self: Pin<&mut Self>, d_beat: i64, d_pitch: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .move_selection(d_beat, d_pitch);
        self.as_mut().finish_edit(result)
    }

    /// move_selection_and_following shifts the selection and every later
    /// row of their voice as a block.
    pub fn move_selection_and_following(mut self: Pin<&mut Self>, d_beat: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .move_selection_and_following(d_beat);
        self.as_mut().finish_edit(result)
    }

    /// resize_note moves one note edge by the committed drag delta.
    pub fn resize_note(mut self: Pin<&mut Self>, id: i64, edge: ResizeEdge, d_beat: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(id) = row_id_of(id) else {
            return false;
        };
        let edge = match (ResizeEdge { repr: edge.repr }) {
            ResizeEdge::Start => Edge::Start,
            ResizeEdge::End => Edge::End,
            _ => return false,
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .resize_note(id, edge, d_beat);
        self.as_mut().finish_edit(result)
    }

    /// preview_move computes the snapped committed deltas of a body drag and
    /// returns them (plan 8.4: the ghost and the commit agree). The keys are
    /// dBeat and dPitch. It touches no model: a data change on every move
    /// would make every note delegate of the song rebind per mouse move.
    pub fn preview_move(
        self: Pin<&mut Self>,
        row_id: i64,
        d_beat: f64,
        d_pitch: i64,
        snap_on: bool,
        px_per_beat: f64,
    ) -> QHash<QHashPair_QString_QVariant> {
        let id = row_id_of(row_id);
        let committed = {
            let rust = self.rust();
            match (&id, rust.controller.loaded()) {
                (Some(id), Some(loaded)) => snapped_move(
                    &loaded.document,
                    &loaded.song,
                    *id,
                    d_beat,
                    d_pitch,
                    snap_on,
                    px_per_beat,
                ),
                _ => (d_beat.round() as i64, d_pitch),
            }
        };
        let mut hash = QHash::<QHashPair_QString_QVariant>::default();
        hash.insert(QString::from("dBeat"), QVariant::from(&committed.0));
        hash.insert(QString::from("dPitch"), QVariant::from(&committed.1));
        hash
    }

    /// preview_resize computes the snapped committed delta of an edge drag
    /// and returns it; like preview_move it touches no model.
    pub fn preview_resize(
        self: Pin<&mut Self>,
        row_id: i64,
        edge: ResizeEdge,
        d_beat: f64,
        snap_on: bool,
        px_per_beat: f64,
    ) -> i64 {
        let id = row_id_of(row_id);
        let edge = match (ResizeEdge { repr: edge.repr }) {
            ResizeEdge::Start => Some(Edge::Start),
            ResizeEdge::End => Some(Edge::End),
            _ => None,
        };
        let Some(edge) = edge else {
            return d_beat.round() as i64;
        };
        match (&id, self.rust().controller.loaded()) {
            (Some(id), Some(loaded)) => snapped_resize(
                &loaded.document,
                &loaded.song,
                *id,
                edge,
                d_beat,
                snap_on,
                px_per_beat,
            ),
            _ => d_beat.round() as i64,
        }
    }

    /// insert_note inserts a note after the last selected one or after the
    /// current line's last note (task P5-05).
    pub fn insert_note(mut self: Pin<&mut Self>) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self.as_mut().rust_mut().controller.insert_note_after();
        self.as_mut().finish_edit(result)
    }

    /// delete_selection deletes every selected note.
    pub fn delete_selection(mut self: Pin<&mut Self>) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self.as_mut().rust_mut().controller.delete_selection();
        self.as_mut().finish_edit(result)
    }

    /// split_selection_at splits the single selected note at the beat.
    pub fn split_selection_at(mut self: Pin<&mut Self>, beat: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self.as_mut().rust_mut().controller.split_selection_at(beat);
        self.as_mut().finish_edit(result)
    }

    /// merge_selection merges the single selected note with the next one.
    pub fn merge_selection(mut self: Pin<&mut Self>) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .merge_selection_with_next();
        self.as_mut().finish_edit(result)
    }

    /// set_selection_type sets the type of every selected note; the code
    /// "F" toggles freestyle instead (UI_SPEC: the F key toggles).
    /// set_line_type sets the type of every note of the current line
    /// (user decision 2026-11-15); one undo step.
    pub fn set_line_type(mut self: Pin<&mut Self>, code: QString) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(first) = code.to_string().chars().next() else {
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .set_current_line_type(first);
        self.as_mut().finish_edit(result)
    }

    pub fn set_selection_type(mut self: Pin<&mut Self>, code: QString) -> bool {
        if code.to_string() == "F" {
            return self.as_mut().toggle_freestyle();
        }
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(first) = code.to_string().chars().next() else {
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .set_selection_type(first);
        self.as_mut().finish_edit(result)
    }

    /// toggle_freestyle toggles every selected note between freestyle and
    /// normal.
    pub fn toggle_freestyle(mut self: Pin<&mut Self>) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .toggle_freestyle_selection();
        self.as_mut().finish_edit(result)
    }

    /// set_note_text sets one note's syllable.
    pub fn set_note_text(mut self: Pin<&mut Self>, row_id: i64, text: QString) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(id) = row_id_of(row_id) else {
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .set_note_text(id, &text.to_string());
        self.as_mut().finish_edit(result)
    }

    /// line_text returns the line's text in the line text syntax.
    pub fn line_text(&self, line: i32) -> QString {
        usize::try_from(line)
            .ok()
            .and_then(|line| self.rust().controller.line_text(line))
            .map(QString::from)
            .unwrap_or_default()
    }

    /// preview_line_text describes the text's plan for the Edit line
    /// dialog as JSON.
    pub fn preview_line_text(&self, line: i32, text: QString, picks: QString) -> QString {
        let Ok(line) = usize::try_from(line) else {
            return QString::default();
        };
        let preview = self.rust().controller.preview_line_text(
            line,
            &text.to_string(),
            &line_text::parse_picks(&picks.to_string()),
        );
        QString::from(line_text::preview_json(&preview))
    }

    /// apply_line_text applies the text to the line as one undo step.
    pub fn apply_line_text(
        mut self: Pin<&mut Self>,
        line: i32,
        text: QString,
        picks: QString,
    ) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Ok(line) = usize::try_from(line) else {
            return false;
        };
        let result = self.as_mut().rust_mut().controller.apply_line_text(
            line,
            &text.to_string(),
            &line_text::parse_picks(&picks.to_string()),
        );
        self.as_mut().finish_edit(result.map(|_| ()))
    }

    /// syllable_for_editing returns the note's syllable without its
    /// surrounding spaces.
    pub fn syllable_for_editing(&self, row_id: i64) -> QString {
        row_id_of(row_id)
            .and_then(|id| self.rust().controller.syllable_for_editing(id))
            .map(QString::from)
            .unwrap_or_default()
    }

    /// set_note_syllable commits the inline field's syllable.
    pub fn set_note_syllable(mut self: Pin<&mut Self>, row_id: i64, text: QString) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(id) = row_id_of(row_id) else {
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .set_note_syllable(id, &text.to_string());
        self.as_mut().finish_edit(result.map(|_| ()))
    }

    /// note_at_lyric_beat returns the row id of the note whose lyric label
    /// covers the beat, -1 when none.
    pub fn note_at_lyric_beat(&self, beat: f64) -> i64 {
        self.rust()
            .controller
            .note_at_lyric_beat(beat)
            .map_or(-1, |id| i64::from(id.0))
    }

    /// toggle_phrase_end_before_selection inserts or removes the
    /// end-of-phrase marker before the selected note.
    pub fn toggle_phrase_end_before_selection(mut self: Pin<&mut Self>) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .toggle_phrase_end_before_selection();
        self.as_mut().finish_edit(result)
    }

    /// move_phrase_end moves one end-of-phrase marker to the beat.
    pub fn move_phrase_end(mut self: Pin<&mut Self>, row_id: i64, beat: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let Some(id) = row_id_of(row_id) else {
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .move_phrase_end(id, beat);
        self.as_mut().finish_edit(result)
    }

    /// transpose_selection shifts the pitch of every selected note.
    pub fn transpose_selection(mut self: Pin<&mut Self>, semitones: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .transpose_selection(semitones);
        self.as_mut().finish_edit(result)
    }

    /// select_next_note applies the Right key (task P5-06).
    pub fn select_next_note(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.navigate_right();
        self.as_mut().refresh();
    }

    /// select_previous_note applies the Left key.
    pub fn select_previous_note(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.navigate_left();
        self.as_mut().refresh();
    }

    /// resize_selection_end changes the length of every selected note.
    pub fn resize_selection_end(mut self: Pin<&mut Self>, d_beat: i64) -> bool {
        if !self.as_mut().edit_allowed() {
            return false;
        }
        let result = self
            .as_mut()
            .rust_mut()
            .controller
            .resize_selection_end(d_beat);
        self.as_mut().finish_edit(result)
    }

    /// toggle_autoplay turns autoplay on or off and hands the current
    /// line's padded play range to the player when it switched on.
    pub fn toggle_autoplay(mut self: Pin<&mut Self>) {
        let on = !self.rust().controller.autoplay();
        self.as_mut().rust_mut().controller.set_autoplay(on);
        self.as_mut().emit_pending_play_range();
        self.as_mut().refresh();
    }

    /// emit_pending_play_range takes the controller's pending play range
    /// and emits it as the autoPlayRange signal.
    fn emit_pending_play_range(mut self: Pin<&mut Self>) {
        let range = self
            .as_mut()
            .rust_mut()
            .controller
            .take_pending_play_range();
        if let Some((start, end)) = range {
            self.as_mut().autoplay_range(start, end);
        }
    }

    /// publish_inspector copies the inspector state of the current
    /// selection into the inspector property (task P5-05). The hash keys
    /// are label, editable, single, count, typeCode, start, length,
    /// pitch, pitchCommon, pitchName, text and pitchEditable.
    fn publish_inspector(mut self: Pin<&mut Self>) {
        let ids: Vec<RowId> = self.rust().controller.selection().iter().copied().collect();
        let state = {
            let rust = self.rust();
            match rust.controller.loaded() {
                Some(loaded) => inspector_state(&loaded.document, &loaded.song, &ids),
                None => InspectorState::None,
            }
        };
        let editable = self.controller.is_loaded() && !self.controller.read_only();
        let mut hash = QHash::<QHashPair_QString_QVariant>::default();
        match &state {
            InspectorState::None => {
                hash.insert(
                    QString::from("label"),
                    QVariant::from(&QString::from("No note selected")),
                );
            }
            InspectorState::Single {
                label,
                type_code,
                start,
                length,
                pitch,
                pitch_name,
                text,
                pitch_editable,
            } => {
                hash.insert(
                    QString::from("label"),
                    QVariant::from(&QString::from(label.clone())),
                );
                hash.insert(
                    QString::from("noteId"),
                    QVariant::from(&ids.first().map(|id| i64::from(id.0)).unwrap_or(-1)),
                );
                hash.insert(
                    QString::from("typeCode"),
                    QVariant::from(&QString::from(type_code.to_string())),
                );
                hash.insert(QString::from("start"), QVariant::from(start));
                hash.insert(QString::from("length"), QVariant::from(length));
                hash.insert(QString::from("pitch"), QVariant::from(pitch));
                hash.insert(
                    QString::from("pitchName"),
                    QVariant::from(&QString::from(pitch_name.clone())),
                );
                hash.insert(
                    QString::from("text"),
                    QVariant::from(&QString::from(text.clone())),
                );
                hash.insert(
                    QString::from("pitchEditable"),
                    QVariant::from(pitch_editable),
                );
            }
            InspectorState::Multi {
                count,
                type_code,
                common_pitch,
            } => {
                hash.insert(
                    QString::from("label"),
                    QVariant::from(&QString::from(format!(
                        "{count} note{} selected",
                        if *count == 1 { "" } else { "s" }
                    ))),
                );
                hash.insert(
                    QString::from("typeCode"),
                    QVariant::from(&QString::from(
                        type_code.map(|code| code.to_string()).unwrap_or_default(),
                    )),
                );
                hash.insert(
                    QString::from("pitch"),
                    QVariant::from(&common_pitch.unwrap_or(0)),
                );
                hash.insert(
                    QString::from("pitchCommon"),
                    QVariant::from(&common_pitch.is_some()),
                );
            }
        }
        hash.insert(QString::from("editable"), QVariant::from(&editable));
        hash.insert(
            QString::from("single"),
            QVariant::from(&matches!(state, InspectorState::Single { .. })),
        );
        hash.insert(
            QString::from("count"),
            QVariant::from(&match &state {
                InspectorState::None => 0,
                InspectorState::Single { .. } => 1,
                InspectorState::Multi { count, .. } => *count as i32,
            }),
        );
        self.as_mut().set_inspector(QVariant::from(&hash));
    }

    /// go_to_line jumps to the global line index.
    pub fn go_to_line(mut self: Pin<&mut Self>, index: i32) {
        if index >= 0 {
            self.as_mut()
                .rust_mut()
                .controller
                .go_to_line(index as usize);
        }
        self.as_mut().emit_pending_play_range();
        self.as_mut().refresh();
    }

    /// follow_playback moves the current line to the line being sung at
    /// `position_ms` and refreshes only when it changed, so the per-frame
    /// calls during playback cost nothing between line changes.
    pub fn follow_playback(mut self: Pin<&mut Self>, position_ms: f64) {
        if self
            .as_mut()
            .rust_mut()
            .controller
            .follow_playback(position_ms)
        {
            self.as_mut().refresh();
        }
    }

    /// next_line moves to the following line.
    pub fn next_line(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.next_line();
        self.as_mut().emit_pending_play_range();
        self.as_mut().refresh();
    }

    /// previous_line moves to the preceding line.
    pub fn previous_line(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().controller.previous_line();
        self.as_mut().emit_pending_play_range();
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
        self.as_mut().publish_inspector();
        if let Some(model) = unsafe { self.as_ref().lines_model().as_mut() } {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
        }
        if let Some(model) = unsafe { self.as_ref().notes_model().as_mut() } {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
        }
        if let Some(model) = unsafe { self.as_ref().phrase_ends_model().as_mut() } {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
        }
        self.as_mut().rebuild_recents_if_stale();
    }

    /// follow_current_line moves the sheet's time view to the current line
    /// by the rule of UI_SPEC 3.2; another song starts a fresh view.
    fn follow_current_line(mut self: Pin<&mut Self>, loaded: bool, start_ms: f64, end_ms: f64) {
        let song = self.controller.path().map(|path| path.to_owned());
        let (Some(song), true) = (song, loaded) else {
            self.as_mut().rust_mut().view = None;
            return;
        };
        let previous = self
            .view
            .as_ref()
            .filter(|(path, _)| *path == song)
            .map(|(_, view)| *view);
        let view = follow_line(previous, start_ms, end_ms);
        self.as_mut().rust_mut().view = Some((song, view));
        self.as_mut().set_view_start_ms(view.start_ms);
        self.as_mut().set_view_end_ms(view.end_ms);
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
        self.as_mut().set_autoplay(snapshot.autoplay);
        self.as_mut()
            .set_whole_line_selected(snapshot.whole_line_selected);
        self.as_mut()
            .set_current_line_start_ms(snapshot.current_line_start_ms);
        self.as_mut()
            .set_current_line_end_ms(snapshot.current_line_end_ms);
        self.as_mut().follow_current_line(
            snapshot.is_loaded,
            snapshot.current_line_start_ms,
            snapshot.current_line_end_ms,
        );
        self.as_mut().sync_sheet_notes(snapshot.is_loaded);
        // The scrollbar's full range: the last note end or phrase end beat.
        let end_beat = match self.controller.loaded() {
            Some(loaded) => song_end_beat(&loaded.document, &loaded.song),
            None => 0.0,
        };
        self.as_mut().set_song_end_beat(end_beat);
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

    /// sync_sheet_notes copies the lyric notes of the current line and its
    /// global neighbours (user decision 2026-11-13) and the current line's
    /// pitch range into the sheet properties; without a song the lists are
    /// empty and the pitch range is -1.
    fn sync_sheet_notes(mut self: Pin<&mut Self>, loaded: bool) {
        let data = match loaded {
            true => {
                let editor = self.rust().editor();
                let (lines, current_index) = all_lines(editor);
                match editor.loaded() {
                    Some(song) => line_notes(&song.document, &song.song, &lines, current_index),
                    None => LineNotes::default(),
                }
            }
            false => LineNotes::default(),
        };
        let mut starts = QVariantList::default();
        let mut texts = QVariantList::default();
        let mut currents = QVariantList::default();
        for note in &data.notes {
            starts.append(QVariant::from(&note.start_beat));
            texts.append(QVariant::from(&QString::from(note.text.as_str())));
            currents.append(QVariant::from(&note.current));
        }
        let (low, high) = data.pitch.unwrap_or((-1, -1));
        self.as_mut().set_sheet_note_starts(starts);
        self.as_mut().set_sheet_note_texts(texts);
        self.as_mut().set_sheet_note_currents(currents);
        self.as_mut().set_sheet_pitch_low(low);
        self.as_mut().set_sheet_pitch_high(high);
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
    autoplay: bool,
    whole_line_selected: bool,
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
    snapshot.autoplay = controller.autoplay();
    snapshot.whole_line_selected = controller.whole_line_selected();
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
            let end_beat = line.end.and_then(|id| song.phrase_end_beat(doc, id));
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
            title: QString::from(shell::recent_title(&recent.path)),
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

/// qurl_to_path converts a file url to a path; other urls keep their text
/// form so that plain paths round-trip.
/// row_id_of converts a QML row id into a RowId; negative ids are unknown.
fn row_id_of(id: i64) -> Option<RowId> {
    u32::try_from(id).ok().map(RowId)
}

fn qurl_to_path(url: &QUrl) -> PathBuf {
    let text = match url.to_local_file() {
        Some(local) => String::from(&local),
        None => url.to_string(),
    };
    PathBuf::from(text)
}
