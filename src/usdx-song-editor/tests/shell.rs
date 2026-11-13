//! About me: tests for the shell state that task P3-05 lifts out of QML so it
//! is testable (plan: "Controller tests for title text, enablement (UI_SPEC 7)
//! and the dialog flows"). They assert the window and sub-page title rules of
//! UI_SPEC 2.6, the toolbar and menu enablement of UI_SPEC 7 (including the
//! play and track controls depending on the audio files), the audio state
//! behind the 4.4 banner (header missing / file missing / present, plus the
//! per-track file checks), and the guarded transition flows of UI_SPEC 5.1
//! and 5.2: save then continue, failed save stops, external change asks,
//! overwrite / reload / cancel choices, discard, and the activation check
//! that reloads silently when the file changed on disk without unsaved
//! changes.

use std::fs;
use std::io;
use std::path::Path;

use tempfile::TempDir;
use usdx_edit::ops::move_notes;
use usdx_song_editor::controller::save::SaveError;
use usdx_song_editor::controller::shell::{
    ActivationChange, AudioState, ChangedOnDiskChoice, SaveChangesChoice, TrackKind, Transition,
    activation_change, audio_state, enablement, has_track, page_title, save_failure_reason,
    transition_changed_on_disk, transition_reload, transition_save_changes, transition_start,
    window_title,
};
use usdx_song_editor::controller::{EditorController, SaveService};

/// fixture writes a song text into a fresh temporary directory and returns
/// the directory (kept alive for the test) and the file path.
fn fixture(text: &str) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    fs::write(&path, text).expect("the fixture is written");
    (dir, path)
}

/// a song with title and artist headers and two lines.
const SONG: &str =
    "#TITLE:Song Title\n#ARTIST:The Artist\n#BPM:240\n: 0 4 0 la\n: 4 4 1 la\n- 8\n: 8 4 2 la\nE\n";

/// SONG plus an #AUDIO header naming a file.
const SONG_WITH_AUDIO: &str =
    "#TITLE:Song Title\n#ARTIST:The Artist\n#AUDIO:song.ogg\n#BPM:240\n: 0 4 0 la\n- 4\n";

/// SONG plus an #AUDIO header naming a file that does not exist.
const SONG_WITH_MISSING_AUDIO: &str =
    "#TITLE:Song Title\n#ARTIST:The Artist\n#AUDIO:gone.ogg\n#BPM:240\n: 0 4 0 la\n- 4\n";

/// a major version 2 song, which loads read-only.
const READ_ONLY: &str = "#VERSION:2.0.0\n#TITLE:Song Title\n#ARTIST:The Artist\n#AUDIO:song.ogg\n#BPM:240\n: 0 4 0 la\n- 4\n";

/// opened opens the fixture song in a fresh controller.
fn opened(text: &str) -> (TempDir, std::path::PathBuf, EditorController) {
    let (dir, path) = fixture(text);
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, path, controller)
}

/// edit moves the first note through the history, so the song is dirty.
fn edit(controller: &mut EditorController) {
    let document = controller.loaded().expect("loaded").document.clone();
    let song = usdx_format::song::Song::from_document(&document);
    let first_note = song
        .voices
        .iter()
        .find_map(|voice| {
            voice
                .lines
                .iter()
                .find_map(|line| line.notes.first().copied())
        })
        .expect("the song has a note");
    let patch = move_notes(&document, &song, &[first_note], 2, 0).expect("the move builds");
    let loaded = controller.loaded_mut().expect("loaded");
    loaded
        .history
        .commit(&mut loaded.document, patch)
        .expect("the commit applies");
}

// --- Title text (UI_SPEC 2.6) ---

#[test]
fn the_window_title_without_a_song_is_the_app_name() {
    let controller = EditorController::new();
    assert_eq!(window_title(&controller), "USDX Song Editor");
}

#[test]
fn the_window_title_names_title_and_artist() {
    let (_dir, _path, controller) = opened(SONG);
    assert_eq!(
        window_title(&controller),
        "Song Title - The Artist \u{b7} USDX Song Editor"
    );
}

#[test]
fn unsaved_changes_prefix_the_title_with_a_star() {
    let (_dir, _path, mut controller) = opened(SONG);
    edit(&mut controller);
    assert_eq!(
        window_title(&controller),
        "* Song Title - The Artist \u{b7} USDX Song Editor"
    );
}

#[test]
fn a_read_only_song_gets_the_read_only_suffix() {
    let (_dir, _path, controller) = opened(READ_ONLY);
    assert_eq!(
        window_title(&controller),
        "Song Title - The Artist (read-only) \u{b7} USDX Song Editor"
    );
}

#[test]
fn dirty_and_read_only_combine_in_the_title() {
    let (_dir, _path, mut controller) = opened(READ_ONLY);
    edit(&mut controller);
    assert_eq!(
        window_title(&controller),
        "* Song Title - The Artist (read-only) \u{b7} USDX Song Editor"
    );
}

#[test]
fn sub_pages_prefix_their_name_with_the_song_context() {
    let (_dir, _path, controller) = opened(SONG);
    assert_eq!(
        page_title("Repair timing", true, &controller),
        "Repair timing - Song Title \u{b7} USDX Song Editor"
    );
    assert_eq!(
        page_title("Song properties", true, &controller),
        "Song properties - Song Title \u{b7} USDX Song Editor"
    );
    assert_eq!(
        page_title("Settings", false, &controller),
        "Settings \u{b7} USDX Song Editor"
    );
}

#[test]
fn sub_pages_fall_back_to_the_plain_app_title_without_a_song() {
    let controller = EditorController::new();
    assert_eq!(
        page_title("Repair timing", true, &controller),
        "Repair timing \u{b7} USDX Song Editor"
    );
}

// --- Audio state (UI_SPEC 4.4) and per-track checks ---

#[test]
fn a_song_whose_audio_header_is_missing_names_no_audio() {
    let (_dir, _path, controller) = opened(SONG);
    assert_eq!(audio_state(&controller), Some(AudioState::Unnamed));
}

#[test]
fn a_song_whose_audio_file_is_missing_reports_the_name() {
    let (_dir, _path, controller) = opened(SONG_WITH_MISSING_AUDIO);
    assert_eq!(
        audio_state(&controller),
        Some(AudioState::Missing(String::from("gone.ogg")))
    );
}

#[test]
fn a_song_with_its_audio_file_in_the_folder_is_present() {
    let (dir, _path, controller) = opened(SONG_WITH_AUDIO);
    fs::write(dir.path().join("song.ogg"), b"audio").expect("the audio file is written");
    assert_eq!(audio_state(&controller), Some(AudioState::Present));
}

#[test]
fn the_audio_state_is_none_without_a_song() {
    let controller = EditorController::new();
    assert_eq!(audio_state(&controller), None);
}

#[test]
fn the_mix_track_falls_back_to_the_mp3_header() {
    let (dir, _path, controller) = opened("#TITLE:T\n#MP3:song.mp3\n#BPM:240\n: 0 4 0 la\n- 4\n");
    assert_eq!(
        audio_state(&controller),
        Some(AudioState::Missing(String::from("song.mp3")))
    );
    fs::write(dir.path().join("song.mp3"), b"audio").expect("the mp3 file is written");
    assert_eq!(audio_state(&controller), Some(AudioState::Present));
}

#[test]
fn tracks_exist_only_when_their_file_is_in_the_folder() {
    let (dir, _path, controller) = opened(
        "#TITLE:T\n#BPM:240\n#VOCALS:vocals.ogg\n#INSTRUMENTAL:instr.ogg\n: 0 4 0 la\n- 4\n",
    );
    assert!(!has_track(&controller, TrackKind::Vocals));
    assert!(!has_track(&controller, TrackKind::Instrumental));
    assert!(!has_track(&controller, TrackKind::Mix));
    fs::write(dir.path().join("vocals.ogg"), b"v").expect("the vocals file is written");
    fs::write(dir.path().join("instr.ogg"), b"i").expect("the instrumental file is written");
    fs::write(dir.path().join("mix.ogg"), b"m").expect("the mix file is written");
    // Mix has no audio header here, so it stays absent even with a file.
    assert!(has_track(&controller, TrackKind::Vocals));
    assert!(has_track(&controller, TrackKind::Instrumental));
    assert!(!has_track(&controller, TrackKind::Mix));
    // Without a song no track exists.
    let empty = EditorController::new();
    assert!(!has_track(&empty, TrackKind::Vocals));
}

// --- Enablement (UI_SPEC 7) ---

#[test]
fn without_a_song_everything_is_disabled() {
    let enablement = enablement(&EditorController::new());
    assert!(!enablement.save);
    assert!(!enablement.save_as);
    assert!(!enablement.undo);
    assert!(!enablement.redo);
    assert!(!enablement.play);
    assert!(!enablement.play_line);
    assert!(!enablement.loop_playback);
    assert!(!enablement.track);
    assert!(!enablement.speed);
    assert!(!enablement.repair);
    assert!(!enablement.properties);
    assert!(!enablement.export_anchors);
}

#[test]
fn a_song_with_audio_enables_everything_but_save_until_it_is_dirty() {
    let (dir, _path, controller) = opened(SONG_WITH_AUDIO);
    fs::write(dir.path().join("song.ogg"), b"audio").expect("the audio file is written");
    let enablement = enablement(&controller);
    assert!(!enablement.save, "Save needs unsaved changes");
    assert!(enablement.save_as);
    assert!(!enablement.undo && !enablement.redo, "no history yet");
    assert!(enablement.play);
    assert!(enablement.play_line);
    assert!(enablement.loop_playback);
    assert!(enablement.track);
    assert!(enablement.speed);
    assert!(enablement.repair);
    assert!(enablement.properties);
    assert!(enablement.export_anchors);
}

#[test]
fn unsaved_changes_enable_save() {
    let (_dir, _path, mut controller) = opened(SONG);
    edit(&mut controller);
    let enablement = enablement(&controller);
    assert!(enablement.save);
    assert!(enablement.undo);
    assert!(!enablement.redo);
    assert!(!enablement.play, "the song names no audio file");
}

#[test]
fn a_song_without_audio_disables_the_playback_controls() {
    let (_dir, _path, controller) = opened(SONG);
    let enablement = enablement(&controller);
    assert!(!enablement.play);
    assert!(!enablement.play_line);
    assert!(!enablement.loop_playback);
    assert!(!enablement.track);
    assert!(!enablement.speed);
    assert!(
        enablement.repair,
        "repair works without audio (tap-along needs it)"
    );
    assert!(enablement.properties);
    assert!(enablement.export_anchors);
}

#[test]
fn a_read_only_song_disables_everything_that_edits() {
    let (dir, _path, mut controller) = opened(READ_ONLY);
    fs::write(dir.path().join("song.ogg"), b"audio").expect("the audio file is written");
    edit(&mut controller);
    let enablement = enablement(&controller);
    assert!(!enablement.save, "read-only songs cannot save");
    assert!(!enablement.save_as);
    assert!(!enablement.undo && !enablement.redo);
    assert!(enablement.play);
    assert!(!enablement.repair);
    assert!(enablement.properties, "the read-only form opens");
    assert!(enablement.export_anchors);
}

// --- Guarded transitions (UI_SPEC 5.1, 5.2) ---

#[test]
fn a_clean_song_continues_without_asking() {
    let (_dir, _path, controller) = opened(SONG);
    assert_eq!(transition_start(&controller), Transition::Continue);
}

#[test]
fn unsaved_changes_ask_before_a_transition() {
    let (_dir, _path, mut controller) = opened(SONG);
    edit(&mut controller);
    assert_eq!(transition_start(&controller), Transition::AskSaveChanges);
}

#[test]
fn choosing_save_saves_and_then_continues() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    assert_eq!(outcome, Transition::Continue);
    assert!(!controller.is_dirty(), "the save cleared the dirty flag");
    let written = fs::read(&path).expect("the file is read");
    let document = &controller.loaded().expect("loaded").document;
    assert_eq!(
        written,
        usdx_format::serialize::serialize(document).expect("the document serializes"),
        "the file on disk is the saved version"
    );
}

#[test]
fn a_failed_save_stops_the_transition() {
    let (_dir, _path, mut controller) = opened(READ_ONLY);
    edit(&mut controller);
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    assert_eq!(
        outcome,
        Transition::Stop(Some(String::from("the song is read-only")))
    );
    assert!(
        controller.is_dirty(),
        "a stopped transition keeps the changes"
    );
}

#[test]
fn choosing_cancel_stops_without_saving() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let before = fs::read(&path).expect("the file is read");
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Cancel);
    assert_eq!(outcome, Transition::Stop(None));
    assert_eq!(fs::read(&path).expect("the file is read"), before);
    assert!(controller.is_dirty());
}

#[test]
fn choosing_discard_continues_without_saving() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let before = fs::read(&path).expect("the file is read");
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Discard);
    assert_eq!(outcome, Transition::Continue);
    assert_eq!(fs::read(&path).expect("the file is read"), before);
}

#[test]
fn a_save_that_hits_an_external_change_asks_what_to_do() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    fs::write(
        &path,
        "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n",
    )
    .expect("the external change is written");
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    assert_eq!(outcome, Transition::AskChangedOnDisk);
}

#[test]
fn overwriting_the_changed_file_saves_and_continues() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    fs::write(
        &path,
        "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n",
    )
    .expect("the external change is written");
    let mut saves = SaveService::new();
    transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    let outcome =
        transition_changed_on_disk(&mut controller, &mut saves, ChangedOnDiskChoice::Overwrite);
    assert_eq!(outcome, Transition::Continue);
    assert!(!controller.is_dirty());
    let written = fs::read(&path).expect("the file is read");
    let document = &controller.loaded().expect("loaded").document;
    assert_eq!(
        written,
        usdx_format::serialize::serialize(document).expect("the document serializes"),
        "the overwrite replaced the external version"
    );
}

#[test]
fn reloading_after_an_external_change_opens_the_file_as_it_is_now() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let external = "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n";
    fs::write(&path, external).expect("the external change is written");
    let mut saves = SaveService::new();
    transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    let outcome =
        transition_changed_on_disk(&mut controller, &mut saves, ChangedOnDiskChoice::Reload);
    assert_eq!(outcome, Transition::Continue);
    assert!(
        !controller.is_dirty(),
        "the reload dropped the unsaved changes"
    );
    let written = fs::read(&path).expect("the file is read");
    let document = &controller.loaded().expect("loaded").document;
    assert_eq!(
        written,
        usdx_format::serialize::serialize(document).expect("the document serializes"),
        "the reloaded document matches the file as it is now"
    );
}

#[test]
fn cancelling_after_an_external_change_stops_without_touching_the_file() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let external = "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n";
    fs::write(&path, external).expect("the external change is written");
    let mut saves = SaveService::new();
    transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    let outcome =
        transition_changed_on_disk(&mut controller, &mut saves, ChangedOnDiskChoice::Cancel);
    assert_eq!(outcome, Transition::Stop(None));
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        external.as_bytes()
    );
    assert!(controller.is_dirty());
}

#[test]
fn the_activation_check_reloads_silently_when_the_song_is_clean() {
    let (_dir, path, controller) = opened(SONG);
    let saves = SaveService::new();
    assert_eq!(
        activation_change(&controller, &saves),
        ActivationChange::Unchanged
    );
    fs::write(
        &path,
        "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n",
    )
    .expect("the external change is written");
    assert_eq!(
        activation_change(&controller, &saves),
        ActivationChange::ReloadSilently
    );
}

#[test]
fn the_activation_check_asks_when_there_are_unsaved_changes() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    fs::write(
        &path,
        "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n",
    )
    .expect("the external change is written");
    let saves = SaveService::new();
    assert_eq!(
        activation_change(&controller, &saves),
        ActivationChange::AskChangedOnDisk
    );
}

#[test]
fn reload_opens_the_file_as_it_is_now() {
    let (_dir, path, mut controller) = opened(SONG);
    edit(&mut controller);
    let external = "#TITLE:Changed elsewhere\n#BPM:240\n: 0 4 0 la\n- 4\n";
    fs::write(&path, external).expect("the external change is written");
    let outcome = transition_reload(&mut controller);
    assert_eq!(outcome, Transition::Continue);
    assert!(!controller.is_dirty());
    let document = &controller.loaded().expect("loaded").document;
    assert_eq!(
        fs::read(&path).expect("the file is read"),
        usdx_format::serialize::serialize(document).expect("the document serializes"),
    );
}

#[test]
fn a_failing_reload_stops_with_the_reason() {
    let (dir, path, mut controller) = opened(SONG);
    // Remove the file: the reload cannot read it and stops with a reason,
    // the previously loaded song stays in place.
    fs::remove_file(&path).expect("the file is removed");
    assert_eq!(
        transition_reload(&mut controller),
        Transition::Stop(Some(format!(
            "\"song.txt\" could not be read: {}.",
            "No such file or directory (os error 2)"
        ))),
    );
    drop(dir);
}

// --- Save failure reason texts (UI_SPEC 6) ---

#[test]
fn a_refused_save_names_the_missing_permission() {
    let error = SaveError::Io(io::Error::from(io::ErrorKind::PermissionDenied));
    let reason = save_failure_reason(&error, Path::new("/somewhere/song.txt"));
    assert_eq!(reason, "no permission to write \"song.txt\"");
}

#[test]
fn a_full_disk_says_so() {
    let error = SaveError::Io(io::Error::from(io::ErrorKind::StorageFull));
    let reason = save_failure_reason(&error, Path::new("/somewhere/song.txt"));
    assert_eq!(reason, "the disk is full");
}

#[test]
fn a_quota_error_says_the_disk_is_full() {
    // EDQUOT is a disk-space problem for the user as well.
    let error = SaveError::Io(io::Error::from(io::ErrorKind::QuotaExceeded));
    let reason = save_failure_reason(&error, Path::new("/somewhere/song.txt"));
    assert_eq!(reason, "the disk is full");
}

#[test]
fn a_missing_folder_says_it_is_gone() {
    // The write creates a temp file in the target's folder; the folder being
    // gone surfaces as NotFound.
    let error = SaveError::Io(io::Error::from(io::ErrorKind::NotFound));
    let reason = save_failure_reason(&error, Path::new("/somewhere/song.txt"));
    assert_eq!(reason, "the folder no longer exists");
}

#[test]
fn another_io_failure_names_the_file_and_the_reason() {
    let error = SaveError::Io(io::Error::other("read-only file system"));
    let reason = save_failure_reason(&error, Path::new("/somewhere/song.txt"));
    assert_eq!(
        reason,
        "\"song.txt\" could not be written: read-only file system"
    );
}

#[test]
fn a_read_only_song_says_it_cannot_be_written() {
    let reason = save_failure_reason(&SaveError::ReadOnly, Path::new("/somewhere/song.txt"));
    assert_eq!(reason, "the song is read-only");
}

#[test]
fn a_serialize_failure_carries_the_detail() {
    // The serializer's detail is developer-facing, but the frame stays
    // honest about what happened. The declared UTF-8 encoding with invalid
    // bytes is a certain decode failure.
    let detail = usdx_format::bytes::decode(b"#ENCODING:UTF-8\n\xFF\xFE")
        .expect_err("declared UTF-8 with invalid bytes does not decode");
    let reason = save_failure_reason(
        &SaveError::Serialize(detail),
        Path::new("/somewhere/song.txt"),
    );
    assert!(
        reason.starts_with("the song text cannot be written: "),
        "the serialize reason frames the detail, got: {reason}"
    );
}

#[test]
fn a_failed_save_stops_with_the_exact_reason() {
    // Integration: the shell transition carries the UI_SPEC 6 reason, not
    // the raw error Display.
    let (_dir, _path, mut controller) = opened(READ_ONLY);
    edit(&mut controller);
    let mut saves = SaveService::new();
    let outcome = transition_save_changes(&mut controller, &mut saves, SaveChangesChoice::Save);
    assert_eq!(
        outcome,
        Transition::Stop(Some(String::from("the song is read-only")))
    );
}
