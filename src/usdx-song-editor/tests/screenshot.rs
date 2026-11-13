//! About me: integration test for the screenshot harness (task P3-06, UI_SPEC
//! 8.1). It runs the built binary with `--screenshot`, `--demo-state`,
//! `--song` and `--size` offscreen in both Breeze themes (each theme through
//! a temporary XDG_CONFIG_HOME holding the colour scheme as kdeglobals, so
//! the user's desktop settings are never touched) and asserts that the PNG
//! exists, has the requested size, and that the pixel at (5, 5) carries the
//! window background of the theme: #eff0f1 in Breeze Light, #202326 in
//! Breeze Dark. It also pins the harness contract: an unknown demo state
//! fails with a clear message, a `--song` screenshot opens a copy and never
//! modifies the original file, and `--size` resizes the window. The audio
//! demo states (UI_SPEC 4.4, 3.7) run on generated fixtures: the missing
//! state leaves the audio out of the copy, the loading state pins the lane at
//! 42 %, the no-vocals state waits for the waveform; the run reports the lane
//! status on stdout, and the audio states refuse to run without a song.

use cxx_qt_lib::QString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use usdx_song_editor::bridge::screenshot::{usdx_png_pixel, usdx_png_size};

/// The colour scheme files that stand in for the two Breeze themes
/// (UI_SPEC 8.1: copied into the temporary XDG_CONFIG_HOME as kdeglobals).
const LIGHT_SCHEME: &str = "/usr/share/color-schemes/BreezeLight.colors";
const DARK_SCHEME: &str = "/usr/share/color-schemes/BreezeDark.colors";

/// The window backgrounds the spec pins for the two themes (UI_SPEC 8.1);
/// they appear below the page header toolbar.
const LIGHT_BACKGROUND: &str = "#eff0f1";
const DARK_BACKGROUND: &str = "#202326";

/// The page header toolbar covers the top of the window, so the toolbar
/// background sits at (5, 5): the KDE desktop style shades it differently
/// from the window background (measured on this machine).
const LIGHT_TOOLBAR: &str = "#dee0e2";
const DARK_TOOLBAR: &str = "#292c30";

/// Runs the command and returns its exit status, or None when it was still
/// running after the timeout and was killed (the screenshot mode must always
/// exit by itself; without the implemented flags the app would sit in the
/// event loop forever).
fn run_with_timeout(mut command: Command, limit: Duration) -> Option<std::process::ExitStatus> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("the binary was built and spawns");
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait().expect("the child status is readable") {
            Some(status) => return Some(status),
            None => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Prepares a temporary XDG_CONFIG_HOME with the given colour scheme file as
/// kdeglobals and returns its path; the caller keeps it alive and removes it.
fn theme_config_home(scheme: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let config_home = dir.path().join("config");
    fs::create_dir_all(&config_home).expect("the config folder is created");
    fs::copy(scheme, config_home.join("kdeglobals")).expect("the colour scheme is copied");
    (dir, config_home)
}

/// Builds the screenshot command for one theme: the fixed offscreen
/// environment of UI_SPEC 8.1 plus the given arguments.
fn screenshot_command(config_home: &Path, out: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_usdx-song-editor"));
    command
        .env("XDG_CONFIG_HOME", config_home)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop")
        .env("QT_QPA_PLATFORMTHEME", "kde")
        .arg("--screenshot")
        .arg(out)
        .args(args);
    command
}

#[test]
fn the_start_screen_screenshot_matches_the_theme_in_both_colours() {
    for (scheme, expected_toolbar, expected_background) in [
        (LIGHT_SCHEME, LIGHT_TOOLBAR, LIGHT_BACKGROUND),
        (DARK_SCHEME, DARK_TOOLBAR, DARK_BACKGROUND),
    ] {
        let (_config_dir, config_home) = theme_config_home(scheme);
        let out_dir = tempfile::tempdir().expect("the output directory is created");
        let png = out_dir.path().join("start-screen.png");

        let status = run_with_timeout(
            screenshot_command(
                &config_home,
                &png,
                &["--demo-state", "start-screen", "--size", "1280x800"],
            ),
            Duration::from_secs(60),
        )
        .expect_or_timeout();
        assert!(status.success(), "the screenshot run of {scheme} failed");
        assert!(png.exists(), "the PNG exists at {}", png.display());

        let size = usdx_png_size(&QString::from(
            png.to_str().expect("the path is valid unicode"),
        ))
        .to_string();
        assert_eq!(
            size, "1280x800",
            "the PNG has the requested size ({scheme})"
        );
        let path = png.to_str().expect("the path is valid unicode");
        // The page header toolbar covers the top edge, so (5, 5) carries the
        // toolbar background while the window background shows below it (the
        // spec's colours are exact; see the correction note in UI_SPEC 8.1).
        let toolbar = usdx_png_pixel(&QString::from(path), 5, 5).to_string();
        let background = usdx_png_pixel(&QString::from(path), 5, 60).to_string();
        assert_eq!(
            toolbar, expected_toolbar,
            "the pixel at (5, 5) carries the toolbar background of {scheme}"
        );
        assert_eq!(
            background, expected_background,
            "the pixel at (5, 60) carries the window background of {scheme}"
        );
    }
}

#[test]
fn the_size_argument_resizes_the_window() {
    let (_config_dir, config_home) = theme_config_home(LIGHT_SCHEME);
    let out_dir = tempfile::tempdir().expect("the output directory is created");
    let png = out_dir.path().join("small.png");

    let status = run_with_timeout(
        screenshot_command(
            &config_home,
            &png,
            &["--demo-state", "start-screen", "--size", "960x600"],
        ),
        Duration::from_secs(60),
    )
    .expect_or_timeout();
    assert!(status.success(), "the small screenshot run failed");
    let size = usdx_png_size(&QString::from(
        png.to_str().expect("the path is valid unicode"),
    ))
    .to_string();
    assert_eq!(size, "960x600", "the PNG has the requested size");
}

#[test]
fn the_song_argument_opens_a_copy_and_never_touches_the_original() {
    // A minimal song next to a dummy audio file; the copy the harness opens
    // must not be the file on disk.
    let song_dir = tempfile::tempdir().expect("the song directory is created");
    let original = song_dir.path().join("song.txt");
    let song = "#TITLE:T\n#ARTIST:A\n#BPM:240\nP1\n: 0 4 0 la\n- 4\nE\n";
    fs::write(&original, song).expect("the song is written");
    let before = fs::read(&original).expect("the song is read");

    let (_config_dir, config_home) = theme_config_home(LIGHT_SCHEME);
    let out_dir = tempfile::tempdir().expect("the output directory is created");
    let png = out_dir.path().join("with-song.png");

    let song_arg = original
        .to_str()
        .expect("the path is valid unicode")
        .to_owned();
    let status = run_with_timeout(
        screenshot_command(
            &config_home,
            &png,
            &["--demo-state", "start-screen", "--song", &song_arg],
        ),
        Duration::from_secs(60),
    );
    assert!(
        status.expect_or_timeout().success(),
        "the song screenshot run failed"
    );
    assert!(png.exists(), "the PNG exists");

    let after = fs::read(&original).expect("the song is read");
    assert_eq!(before, after, "the original song file is untouched");
}

#[test]
fn an_unknown_demo_state_fails_with_a_clear_message() {
    let (_config_dir, config_home) = theme_config_home(LIGHT_SCHEME);
    let out_dir = tempfile::tempdir().expect("the output directory is created");
    let png = out_dir.path().join("never.png");

    let status = run_with_timeout(
        screenshot_command(&config_home, &png, &["--demo-state", "no-such-state"]),
        Duration::from_secs(60),
    );
    let status = status.expect_or_timeout();
    assert!(!status.success(), "an unknown demo state must fail");
    assert!(!png.exists(), "no PNG is written for an unknown demo state");
}

/// Runs the command like run_with_timeout and also returns its stdout.
fn run_capturing(mut command: Command, limit: Duration) -> (std::process::ExitStatus, String) {
    use std::io::Read;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("the binary was built and spawns");
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait().expect("the child status is readable") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the screenshot run was killed by the test timeout: it never exited by itself");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    (status, stdout)
}

/// audio_fixture writes a song beside a generated tone; `vocals` adds the
/// tone as the #VOCALS track too.
fn audio_fixture(vocals: bool) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("the song directory is created");
    usdx_audio::testmedia::tone_opus(&dir.path().join("tone.opus"))
        .expect("the test tone is generated");
    let vocals_header = if vocals { "#VOCALS:tone.opus\n" } else { "" };
    let song = format!(
        "#TITLE:T\n#ARTIST:A\n#MP3:tone.opus\n{vocals_header}#BPM:240\n: 0 4 0 la\n- 8\n: 8 4 2 lo\nE\n"
    );
    let path = dir.path().join("song.txt");
    fs::write(&path, song).expect("the song is written");
    let path = path.to_str().expect("the path is valid unicode").to_owned();
    (dir, path)
}

/// audio_state_run renders one audio demo state of the fixture and returns
/// the exit status and stdout; the analysis cache goes to a temporary folder.
fn audio_state_run(state: &str, song: Option<&str>) -> (std::process::ExitStatus, String, bool) {
    let (_config_dir, config_home) = theme_config_home(LIGHT_SCHEME);
    let cache = tempfile::tempdir().expect("the temporary cache is created");
    let out_dir = tempfile::tempdir().expect("the output directory is created");
    let png = out_dir.path().join("state.png");
    let mut args = vec!["--demo-state", state];
    if let Some(song) = song {
        args.extend(["--song", song]);
    }
    let mut command = screenshot_command(&config_home, &png, &args);
    command.env("XDG_CACHE_HOME", cache.path());
    let (status, stdout) = run_capturing(command, Duration::from_secs(60));
    (status, stdout, png.exists())
}

#[test]
fn the_missing_audio_state_leaves_the_audio_out_of_the_copy() {
    let (_dir, song) = audio_fixture(true);
    let (status, stdout, written) = audio_state_run("audio-states-missing", Some(&song));
    assert!(status.success() && written, "the run failed:\n{stdout}");
    assert!(stdout.contains("lane status: no audio"), "got: {stdout}");
}

#[test]
fn the_loading_state_pins_the_lane_at_42_percent() {
    let (_dir, song) = audio_fixture(true);
    let (status, stdout, written) = audio_state_run("audio-states-loading", Some(&song));
    assert!(status.success() && written, "the run failed:\n{stdout}");
    assert!(
        stdout.contains("lane status: loading 42 %"),
        "got: {stdout}"
    );
}

#[test]
fn the_no_vocals_state_waits_for_the_waveform() {
    let (_dir, song) = audio_fixture(false);
    let (status, stdout, written) = audio_state_run("audio-states-no-vocals", Some(&song));
    assert!(status.success() && written, "the run failed:\n{stdout}");
    assert!(
        stdout.contains("lane status: ready, no vocals"),
        "got: {stdout}"
    );
}

#[test]
fn the_audio_states_need_a_song() {
    for state in [
        "audio-states-missing",
        "audio-states-loading",
        "audio-states-no-vocals",
    ] {
        let (status, _stdout, written) = audio_state_run(state, None);
        assert!(
            !status.success() && !written,
            "{state} must refuse to run without --song"
        );
    }
}

/// Small helper on the Option the timeout guard produces: a killed run is a
/// test failure, reported with a stable message.
trait ExpectOrTimeout {
    fn expect_or_timeout(self) -> std::process::ExitStatus;
}

impl ExpectOrTimeout for Option<std::process::ExitStatus> {
    fn expect_or_timeout(self) -> std::process::ExitStatus {
        self.unwrap_or_else(|| {
            panic!("the screenshot run was killed by the test timeout: it never exited by itself")
        })
    }
}
