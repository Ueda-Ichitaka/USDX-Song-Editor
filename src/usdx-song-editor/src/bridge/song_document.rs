//! The SongDocument and LinesModel bridges.
//!
//! About me: this cxx-qt bridge exposes the editor controller to QML. The
//! SongDocument QObject holds the EditorController and the SaveService and
//! mirrors their state into Q_PROPERTIES after every state change; the
//! LinesModel QAbstractListModel feeds the lines pane and rebuilds itself
//! from the document on every sync. The bridges are thin: the logic lives
//! in the plain-Rust controller (src/controller), this file only converts
//! between Qt and Rust types. The editing invokables (selection, note
//! moves, line text) and the NotesModel arrive with the later tasks
//! (P5-03 to P6-02), the health and anchor roles stay neutral (0, empty)
//! until the analysis (P7) and repair (P6.3) phases fill them.

use std::path::{Path, PathBuf};

use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QUrl, QVariant,
};
use usdx_format::headers::{Headers, Version};
use usdx_format::rows::{NoteType, RowKind};

use crate::controller::save::SaveError;
use crate::controller::{EditorController, SaveService};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        /// Base class of the LinesModel.
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
        include!("cxx-qt-lib/qmodelindex.h");
        /// QModelIndex from cxx_qt_lib
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qbytearray.h");
        /// QByteArray from cxx_qt_lib
        type QByteArray = cxx_qt_lib::QByteArray;
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(*mut SongDocument, document)]
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
        /// 0 ok, 1 suspicious, 2 racing (analysis arrives with P7).
        Health,
        /// The reason of a health mark, empty for ok lines.
        HealthReason,
        /// The anchor chip text (`A1`, `A2`, ...), empty outside repair.
        Anchor,
        /// Whether the row is the current line.
        IsCurrent,
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
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        #[qproperty(*mut LinesModel, lines_model, cxx_name = "linesModel")]
        type SongDocument = super::SongDocumentRust;
    }

    unsafe extern "RustQt" {
        /// Emitted after a song loaded successfully.
        #[qsignal]
        #[cxx_name = "songLoaded"]
        fn song_loaded(self: Pin<&mut SongDocument>);

        /// Emitted with a user-facing message when an operation failed.
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
    }
}

use core::pin::Pin;

use cxx_qt::CxxQtType;

use crate::bridge::song_document::qobject::{LinesModel, SongDocument};

/// One row of the lines pane model (plan section 8.3).
pub struct LineRow {
    line_number: i32,
    start_ms: f64,
    end_ms: f64,
    text: QString,
    type_summary: QString,
    health: i32,
    health_reason: QString,
    anchor: QString,
    is_current: bool,
}

/// The Rust side of the LinesModel.
#[derive(Default)]
pub struct LinesModelRust {
    rows: Vec<LineRow>,
    document: *mut SongDocument,
}

/// The Rust side of the SongDocument: the controller, the save service and
/// the Q_PROPERTY storage.
#[derive(Default)]
pub struct SongDocumentRust {
    controller: EditorController,
    saves: SaveService,
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
    status_text: QString,
    lines_model: *mut LinesModel,
}

impl qobject::LinesModel {
    /// data returns the value of one line row for one role.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.rows.get(index.row() as usize) else {
            return QVariant::default();
        };
        match (qobject::Roles { repr: role }) {
            qobject::Roles::LineNumber => QVariant::from(&row.line_number),
            qobject::Roles::StartMs => QVariant::from(&row.start_ms),
            qobject::Roles::EndMs => QVariant::from(&row.end_ms),
            qobject::Roles::Text => QVariant::from(&row.text),
            qobject::Roles::TypeSummary => QVariant::from(&row.type_summary),
            qobject::Roles::Health => QVariant::from(&row.health),
            qobject::Roles::HealthReason => QVariant::from(&row.health_reason),
            qobject::Roles::Anchor => QVariant::from(&row.anchor),
            qobject::Roles::IsCurrent => QVariant::from(&row.is_current),
            _ => QVariant::default(),
        }
    }

    /// role_names returns the role numbers of the lines pane rows.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::Roles::LineNumber.repr,
            QByteArray::from("lineNumber"),
        );
        roles.insert(qobject::Roles::StartMs.repr, QByteArray::from("startMs"));
        roles.insert(qobject::Roles::EndMs.repr, QByteArray::from("endMs"));
        roles.insert(qobject::Roles::Text.repr, QByteArray::from("text"));
        roles.insert(
            qobject::Roles::TypeSummary.repr,
            QByteArray::from("typeSummary"),
        );
        roles.insert(qobject::Roles::Health.repr, QByteArray::from("health"));
        roles.insert(
            qobject::Roles::HealthReason.repr,
            QByteArray::from("healthReason"),
        );
        roles.insert(qobject::Roles::Anchor.repr, QByteArray::from("anchor"));
        roles.insert(
            qobject::Roles::IsCurrent.repr,
            QByteArray::from("isCurrent"),
        );
        roles
    }

    /// row_count returns the number of line rows.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows.len() as i32
    }

    /// rebuild refills the rows from the document and resets the model.
    pub fn rebuild(mut self: Pin<&mut Self>) {
        let rows = match unsafe { self.as_ref().document().as_ref() } {
            Some(document) => document.line_rows(),
            None => Vec::new(),
        };
        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().rows = rows;
            self.as_mut().end_reset_model();
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

    /// open_path loads the song at the path and reports the outcome.
    fn open_path(mut self: Pin<&mut Self>, path: &Path) -> bool {
        let result = self.as_mut().rust_mut().controller.open(path);
        match result {
            Ok(()) => {
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
            Err(SaveError::ReadOnly) => {
                self.as_mut()
                    .fail(&QString::from("The song is read-only and cannot be saved."));
                false
            }
            Err(error) => {
                self.as_mut().fail(&QString::from(error.to_string()));
                false
            }
        }
    }

    /// refresh mirrors the controller state into the Q_PROPERTYs and the
    /// lines model.
    fn refresh(mut self: Pin<&mut Self>) {
        self.as_mut().sync();
        if let Some(model) = unsafe { self.as_ref().lines_model().as_mut() } {
            let pinned = unsafe { Pin::new_unchecked(model) };
            pinned.rebuild();
        }
    }

    /// sync copies the controller state into the Q_PROPERTY storage.
    fn sync(mut self: Pin<&mut Self>) {
        let snapshot = document_snapshot(&self.controller);
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
    }

    /// fail reports a failed operation through the status text and the
    /// error signal.
    fn fail(mut self: Pin<&mut Self>, message: &QString) {
        self.as_mut().set_status_text(message.clone());
        self.as_mut().error_occurred(message.clone());
    }

    /// line_rows builds the lines pane rows of the loaded song.
    fn line_rows(&self) -> Vec<LineRow> {
        let Some(loaded) = self.controller.loaded() else {
            return Vec::new();
        };
        let doc = &loaded.document;
        let song = &loaded.song;
        let timing = song.timing;
        let to_ms = |beat: f64| timing.map(|value| value.beat_to_ms(beat)).unwrap_or(beat);
        let current = self.controller.current_line();
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
                rows.push(LineRow {
                    line_number: global as i32 + 1,
                    start_ms,
                    end_ms,
                    text: QString::from(line.text(doc)),
                    type_summary: QString::from(type_summary),
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
}

/// document_snapshot reads every Q_PROPERTY value from the controller.
fn document_snapshot(controller: &EditorController) -> Snapshot {
    let Some(loaded) = controller.loaded() else {
        return Snapshot::default();
    };
    let headers = Headers::from_document(&loaded.document);
    let history = &loaded.history;
    Snapshot {
        file_path: QString::from(loaded.path.to_string_lossy().into_owned()),
        title: QString::from(headers.get("TITLE").unwrap_or_default()),
        artist: QString::from(headers.get("ARTIST").unwrap_or_default()),
        bpm: loaded.song.timing.map(|timing| timing.bpm).unwrap_or(0.0),
        gap_ms: loaded
            .song
            .timing
            .map(|timing| timing.gap_ms)
            .unwrap_or(0.0),
        format_version: QString::from(version_text(&loaded.song.version)),
        dirty: history.is_dirty(),
        read_only: loaded.read_only,
        can_undo: history.undo_label().is_some(),
        can_redo: history.redo_label().is_some(),
        undo_text: QString::from(history.undo_label().unwrap_or_default()),
        redo_text: QString::from(history.redo_label().unwrap_or_default()),
        line_count: controller.line_count() as i32,
        current_line: controller.current_line() as i32,
        current_voice: controller
            .current_voice_index()
            .map(|index| index as i32)
            .unwrap_or(-1),
        voice_count: controller.voice_count() as i32,
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
