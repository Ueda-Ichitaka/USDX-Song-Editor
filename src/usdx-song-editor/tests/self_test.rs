//! About me: integration test for the app skeleton (task P0-03). It runs the built
//! binary with `--self-test` under `QT_QPA_PLATFORM=offscreen` and expects exit 0
//! when the QML module loads and the window root object exists, and checks that
//! `--version` prints the crate version, so the cxx-qt and Kirigami skeleton is
//! proven to start without a real display. With a song, the self-test also
//! drives the QML paths a user reaches (open through the unsaved-changes
//! guard, the rendered lines pane and recent-songs menu, the activation
//! check, the dialog choices, quit) and fails on any Qt warning, because QML
//! reports binding and script errors only as warnings.

use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_usdx-song-editor"))
}

#[test]
fn self_test_starts_offscreen() {
    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .output()
        .expect("the binary was built and --self-test could run");
    assert!(
        out.status.success(),
        "--self-test failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn self_test_checks_the_start_screen() {
    // Without a song the toolbar keeps only Play of the playback controls
    // and the status bar says "No song open" (UI_SPEC 4.1).
    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "--self-test failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("start screen: ok"),
        "the start screen must be checked and reported on stdout, got: {stdout}"
    );
}

#[test]
fn self_test_reports_the_line_count_of_a_song() {
    // A song with two lines in one voice and one line in a second voice.
    let text = "#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\n- 4\n: 4 4 1 b\n- 8\nP2\n: 0 4 0 c\n- 4\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("lines: 3"),
        "the line count must be reported on stdout, got: {stdout}"
    );
}

#[test]
fn self_test_verifies_the_window_title() {
    // The self-test loads a song and checks that the window title reflects
    // it (UI_SPEC 2.6): "Artist - Title · USDX Song Editor" through the
    // windowTitle property of the bridge.
    let text = "#TITLE:T\n#ARTIST:A\n#BPM:240\nP1\n: 0 4 0 a\n- 4\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("title: T - A \u{b7} USDX Song Editor"),
        "the window title must be verified and reported on stdout, got: {stdout}"
    );
}

#[test]
fn self_test_drives_the_ui_paths_without_qt_warnings() {
    let text = "#TITLE:T\n#ARTIST:A\n#BPM:240\n: 0 4 0 a\n: 4 4 1 b\n- 8\n: 8 4 2 c\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("ui paths: ok"),
        "the UI paths must be driven and reported on stdout, got: {stdout}"
    );
    // The app palette resolves the theme: the waveform is the highlight
    // colour at 55 % (UI_SPEC 2.3), never black from an unresolved theme.
    assert!(
        stdout.contains("lane colour: #8c") && !stdout.contains("lane colour: #8c000000"),
        "the waveform colour must come from the theme, got: {stdout}"
    );
    // Line 1 spans beats 0 to 8 at 240 BPM, 0 to 500 ms; the sheet shows it
    // with 1.5 s on both sides (UI_SPEC 3.2).
    assert!(
        stdout.contains("sheet view: -1500 2000"),
        "the sheet view of the current line must be reported, got: {stdout}"
    );
    assert!(
        !stderr.contains("qt warning") && !stderr.contains("qt critical"),
        "the self-test printed Qt warnings:\n{stderr}"
    );
}

#[test]
fn self_test_runs_the_interaction_checks_on_a_multi_note_song() {
    // Two lines of three notes (240 BPM, GAP 100 ms): enough notes for the
    // sheet interaction check, a second line for the playback follow, and a
    // four-beat gap before it so the phrase end marker can move.
    let text = concat!(
        "#TITLE:T\n#ARTIST:A\n#BPM:240\n#GAP:100\n",
        ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
        ": 16 4 0 d\n: 20 4 2 e\n: 24 4 4 f\n- 28\nE\n",
    );
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{stdout}\n{stderr}"
    );
    for report in ["sheet interaction: ok", "drawer check: ok", "playhead: ok"] {
        assert!(
            stdout.contains(report),
            "the self-test must report {report:?}, got: {stdout}\n{stderr}"
        );
    }
}

#[test]
fn self_test_resizes_a_note_whose_end_touches_the_phrase_end() {
    // Line 1's widest note (beats 4 to 12) ends exactly at the phrase end
    // marker (12): the marker's grab area overlaps the note's end edge
    // zone, and the selected note's end edge must still resize.
    let text = concat!(
        "#TITLE:T\n#ARTIST:A\n#BPM:240\n",
        ": 0 2 0 a\n: 2 2 2 b\n: 4 8 4 c\n- 12\n",
        ": 16 2 0 d\n: 18 2 2 e\n: 20 2 4 f\n- 22\nE\n",
    );
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && stdout.contains("sheet interaction: end edge resized")
            && stdout.contains("sheet interaction: ok"),
        "--self-test failed or skipped the end edge:\n{stdout}\n{stderr}"
    );
}

#[test]
fn self_test_never_writes_the_song_it_is_given() {
    // The interaction check edits and saves (Ctrl+S); it must do that on a
    // copy, so the user's song and its folder stay untouched.
    let text = concat!(
        "#TITLE:T\n#ARTIST:A\n#BPM:240\n#GAP:100\n",
        ": 0 4 0 a\n: 4 4 2 b\n: 8 4 4 c\n- 12\n",
        ": 16 4 0 d\n: 20 4 2 e\n: 24 4 4 f\n- 28\nE\n",
    );
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    assert!(
        out.status.success(),
        "--self-test failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let after = std::fs::read_to_string(&path).expect("the song is still there");
    assert_eq!(after, text, "the self-test changed the song file");
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("the folder is readable")
        .map(|entry| entry.expect("the entry is readable").file_name())
        .collect();
    assert_eq!(
        entries,
        vec![std::ffi::OsString::from("song.txt")],
        "the folder changed"
    );
}

#[test]
fn self_test_keeps_the_lines_pane_scroll_on_a_long_song() {
    // Sixty two-note lines: the lines pane scrolls, and a selection change
    // must not move it.
    let mut text = String::from("#TITLE:T\n#ARTIST:A\n#BPM:240\n");
    for line in 0..60 {
        let beat = line * 8;
        text.push_str(&format!(
            ": {} 2 0 a\n: {} 2 2 b\n- {}\n",
            beat,
            beat + 2,
            beat + 6
        ));
    }
    text.push_str("E\n");
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stdout.contains("playhead: ok"),
        "--self-test with a long song failed:\n{stdout}\n{stderr}"
    );
}

#[test]
fn self_test_skips_the_sheet_interaction_on_a_one_note_line() {
    // One note in the line: the interaction check needs two notes and
    // reports that it skipped (user decision 2026-11-15) instead of failing.
    let text = "#TITLE:T\n#ARTIST:A\n#BPM:240\n: 0 4 0 a\n- 4\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "--self-test with a one-note song failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("sheet interaction: skipped"),
        "the skipped interaction check must be reported, got: {stdout}"
    );
}

#[test]
fn self_test_reports_the_track_chosen_when_the_song_opens() {
    // The default setting picks Vocals when the song has a vocals file
    // (UI_SPEC 4.10), right when the song opens, before any playback.
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let with_vocals =
        "#TITLE:T\n#ARTIST:A\n#MP3:song.opus\n#VOCALS:vocals.opus\n#BPM:240\n: 0 4 0 a\nE\n";
    let without_vocals = "#TITLE:T\n#ARTIST:A\n#MP3:song.opus\n#BPM:240\n: 0 4 0 a\nE\n";
    std::fs::write(dir.path().join("song.opus"), b"").expect("the mix file is written");
    std::fs::write(dir.path().join("vocals.opus"), b"").expect("the vocals file is written");
    for (name, text, expected) in [
        ("vocals.txt", with_vocals, "player track: 1"),
        ("mix.txt", without_vocals, "player track: 0"),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, text).expect("the fixture is written");
        let out = binary()
            .env("QT_QPA_PLATFORM", "offscreen")
            .arg("--self-test")
            .arg(&path)
            .output()
            .expect("the binary was built and --self-test could run");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "--self-test with {name} failed:\n{stdout}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stdout.contains(expected),
            "{name}: expected \"{expected}\" on stdout, got: {stdout}"
        );
    }
}

#[test]
fn self_test_reads_the_waveform_of_the_song_audio() {
    // A generated tone as the song audio: the lane analyses it on a worker
    // thread and ends ready (UI_SPEC 3.7); the analysis cache goes to a
    // temporary folder.
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let cache = tempfile::tempdir().expect("the temporary cache is created");
    usdx_audio::testmedia::tone_opus(&dir.path().join("tone.opus"))
        .expect("the test tone is generated");
    let path = dir.path().join("song.txt");
    std::fs::write(
        &path,
        "#TITLE:T\n#ARTIST:A\n#MP3:tone.opus\n#BPM:240\n: 0 4 0 a\n- 8\n: 8 4 2 b\nE\n",
    )
    .expect("the fixture is written");
    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("XDG_CACHE_HOME", cache.path())
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "--self-test failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("lane status: ready"),
        "the waveform lane must end ready, got: {stdout}"
    );
}

#[test]
fn self_test_shows_no_audio_without_an_audio_file() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, "#TITLE:T\n#BPM:240\n: 0 4 0 a\nE\n").expect("the fixture is written");
    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("lane status: no audio"),
        "the waveform lane must say no audio, got: {stdout}"
    );
}

#[test]
fn self_test_does_not_touch_the_user_settings() {
    // The self-test isolates its settings: with the user home redirected to
    // an empty temporary folder, no settings file may appear there, because
    // the app points XDG_CONFIG_HOME at a throwaway folder before Qt starts.
    let home = tempfile::tempdir().expect("the temporary home is created");
    let text = "#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\n- 4\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("HOME", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let settings = home
        .path()
        .join(".config")
        .join("io.github.ueda_ichitaka.UsdxSongEditor");
    assert!(
        !settings.exists(),
        "the self-test wrote settings into the redirected home: {}",
        settings.display()
    );
}

#[test]
fn version_prints_crate_version() {
    // Reference the library so that this test links the full dependency graph;
    // the cxx-qt initializer archives of the package's build script are linked into
    // every target and resolve through the dependency rlibs bundled in the library.
    let expected = usdx_song_editor::version_line();
    let out = binary()
        .arg("--version")
        .output()
        .expect("the binary was built and --version could run");
    assert!(out.status.success(), "--version exited with an error");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), expected);
}
