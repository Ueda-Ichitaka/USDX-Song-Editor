//! About me: tests for the save service (task P3-02, plan section 8.7).
//! They assert, against temporary directories, that a save serializes the
//! document and clears the dirty flag, that the `.bak` backup is written on
//! the first save of a session only and holds the file as it was before,
//! that a serialization failure (injected) leaves the original file intact
//! and removes the temp file, that an external change of the same file size
//! is detected through the content hash and needs a confirmed save, that a
//! read-only song refuses to save, that a save-as to a fresh path needs
//! no backup, and that a backup that could not be written is tried again on
//! the next save.

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;
use usdx_edit::ops::move_notes;
use usdx_format::rows::RowId;
use usdx_song_editor::controller::save::SaveError;
use usdx_song_editor::controller::{EditorController, SaveService};

/// a song with two lines in one voice.
const SONG: &str = "#TITLE:T\n#BPM:240\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

/// opened writes the song text into a temporary directory, opens it in a
/// controller and returns the directory, the file path and the controller.
fn opened(text: &str) -> (TempDir, PathBuf, EditorController) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    fs::write(&path, text).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, path, controller)
}

/// edit moves the first note through the history, so the song is dirty.
fn edit(controller: &mut EditorController) {
    let document = controller.loaded().expect("loaded").document.clone();
    let song = usdx_format::song::Song::from_document(&document);
    let patch = move_notes(&document, &song, &[RowId(3)], 2, 0).expect("the move builds");
    let loaded = controller.loaded_mut().expect("loaded");
    loaded
        .history
        .commit(&mut loaded.document, patch)
        .expect("the commit applies");
}

/// serialized returns the bytes the loaded document serializes to.
fn serialized(controller: &EditorController) -> Vec<u8> {
    usdx_format::serialize::serialize(&controller.loaded().expect("loaded").document)
        .expect("the document serializes")
}

#[test]
fn save_writes_the_document_and_clears_the_dirty_flag() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    assert!(controller.is_dirty());
    let mut saves = SaveService::new();
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the save works");
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        serialized(&controller)
    );
    assert!(!controller.is_dirty());
    // The backup holds the file as it was before the first save.
    let backup = path.with_file_name("song.txt.bak");
    assert_eq!(
        fs::read(&backup).expect("the backup is read"),
        SONG.as_bytes()
    );
    // The fresh fingerprint makes an immediate second save unremarkable.
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the second save works");
}

#[test]
fn a_failed_backup_is_tried_again_on_the_next_save() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let backup = path.with_file_name("song.txt.bak");
    // A folder in the backup's place makes the copy fail.
    fs::create_dir(&backup).expect("the blocking folder is created");
    let mut saves = SaveService::new();
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect_err("the backup cannot be written");
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        SONG.as_bytes(),
        "a failed backup leaves the song untouched"
    );
    fs::remove_dir(&backup).expect("the blocking folder is removed");
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the second save works");
    assert_eq!(
        fs::read(&backup).expect("the backup is written now"),
        SONG.as_bytes()
    );
}

#[test]
fn the_backup_is_written_on_the_first_save_only() {
    let (_dir, path, mut controller) = opened(SONG);
    let mut saves = SaveService::new();
    edit(&mut controller);
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the first save works");
    edit(&mut controller);
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the second save works");
    let backup = path.with_file_name("song.txt.bak");
    assert_eq!(
        fs::read(&backup).expect("the backup is read"),
        SONG.as_bytes(),
        "the backup still holds the pre-session content"
    );
}

#[test]
fn a_serialization_failure_leaves_the_original_untouched() {
    let (dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let mut saves = SaveService::new();
    let result = saves.save_with(
        controller.loaded_mut().expect("loaded"),
        &path,
        |_| Err(SaveError::Io(std::io::Error::other("the injected failure"))),
        false,
    );
    assert!(matches!(result, Err(SaveError::Io(_))));
    assert_eq!(fs::read(&path).expect("the file is read"), SONG.as_bytes());
    assert!(controller.is_dirty());
    // The temp file is gone; only the song and the plan's pre-write backup
    // remain in the folder.
    let mut names: Vec<String> = fs::read_dir(dir.path())
        .expect("the folder is listed")
        .map(|entry| {
            entry
                .expect("the entry is read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["song.txt", "song.txt.bak"]);
}

#[test]
fn an_external_change_is_detected_and_needs_confirmation() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let mut saves = SaveService::new();
    // Same file size, one character changed: only the hash notices.
    let external = SONG.replacen("la", "lb", 1);
    assert_eq!(external.len(), SONG.len());
    fs::write(&path, external.as_bytes()).expect("the external change is written");
    let result = saves.save(controller.loaded_mut().expect("loaded"), &path);
    assert!(matches!(result, Err(SaveError::ExternalChange)));
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        external.as_bytes(),
        "a detected external change writes nothing"
    );
    // After the user confirms, the save goes through.
    saves
        .save_confirmed(controller.loaded_mut().expect("loaded"), &path)
        .expect("the confirmed save works");
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        serialized(&controller)
    );
    assert!(!controller.is_dirty());
}

#[test]
fn a_read_only_song_refuses_to_save() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    let text = "#TITLE:T\n#VERSION:2.0.0\n#BPM:240\n: 0 4 0 la\nE\n";
    fs::write(&path, text).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    let mut saves = SaveService::new();
    let result = saves.save(controller.loaded_mut().expect("loaded"), &path);
    assert!(matches!(result, Err(SaveError::ReadOnly)));
    assert_eq!(fs::read(&path).expect("the file is read"), text.as_bytes());
}

#[test]
fn a_save_as_to_a_fresh_path_writes_without_a_backup() {
    let (dir, _path, mut controller) = opened(SONG);
    edit(&mut controller);
    let fresh = dir.path().join("other.txt");
    let mut saves = SaveService::new();
    saves
        .save(controller.loaded_mut().expect("loaded"), &fresh)
        .expect("the save works");
    assert_eq!(
        fs::read(&fresh).expect("the file is read"),
        serialized(&controller)
    );
    assert!(
        !fresh.with_file_name("other.txt.bak").exists(),
        "a fresh path has nothing to back up"
    );
    assert!(!controller.is_dirty());
}

#[test]
fn the_first_save_reports_the_written_backup() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let mut saves = SaveService::new();
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the save works");
    let backup = saves.take_last_backup();
    assert_eq!(
        backup.as_deref(),
        Some(path.with_file_name("song.txt.bak").as_path()),
        "the first save reports its backup"
    );
    assert!(
        backup.is_some_and(|backup| backup.exists()),
        "the backup file exists"
    );

    // A following save of the same file writes no backup.
    let patch_backup = saves.take_last_backup();
    assert_eq!(patch_backup, None, "take clears the report");
    saves
        .save(controller.loaded_mut().expect("loaded"), &path)
        .expect("the second save works");
    assert_eq!(saves.take_last_backup(), None);
}
