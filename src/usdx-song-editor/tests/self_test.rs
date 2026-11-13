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
