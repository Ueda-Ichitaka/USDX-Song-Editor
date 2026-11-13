//! About me: library of the usdx-song-editor application. It holds the application
//! logic as a lib target so that integration tests link the full dependency graph
//! (rustc bundles the cxx-qt generated C++ archives into the lib rlib; a test that
//! references the lib therefore resolves the initializer chain) and so that the
//! CMake import in task P0-04 can reuse the crate. The binary in main.rs is a thin
//! entry point around `app_main`.
//!
//! About the startup: it applies the Breeze desktop style, associates the window
//! with the installed desktop file, loads Main.qml from the compiled-in QML module
//! and runs the event loop. --version prints the version line and --self-test
//! verifies offscreen that the QML loads, the window root object exists and
//! the start screen matches UI_SPEC 4.1 (plan section 8.1); with a song path
//! it opens the song through the window's requestOpen() guard, reports the
//! line count and window title on stdout (plan section 8.3, task P3-04) and
//! drives the QML paths a user reaches (render, activation check, dialog
//! choices, quit). Any Qt warning fails the self-test. It redirects
//! XDG_CONFIG_HOME to a throwaway folder so it never touches the user's
//! settings. In debug builds, --screenshot renders a demo state of the
//! UI_SPEC 8.1 harness to a PNG: --demo-state names the state, --song opens a
//! copy of a song and --size resizes the window; the fixed offscreen
//! environment is applied for values the caller did not set.

pub mod bridge;
pub mod controller;

use std::time::{SystemTime, UNIX_EPOCH};

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use cxx_qt_lib_extras::QApplication;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const QML_MAIN_URL: &str = "qrc:/qt/qml/io/github/ueda_ichitaka/UsdxSongEditor/qml/Main.qml";
const DESKTOP_FILE_NAME: &str = "io.github.ueda_ichitaka.UsdxSongEditor";

/// The version line printed by `--version`, as a function so that consumers create
/// a real link-time reference to this library.
pub fn version_line() -> String {
    format!("usdx-song-editor {VERSION}")
}

/// Isolates the settings for the offscreen self-test: points XDG_CONFIG_HOME
/// at a throwaway folder created before Qt starts (both Qt and the settings
/// path read it at startup) and returns the folder. The caller removes it
/// when the self-test ends.
fn self_test_config_home() -> std::io::Result<std::path::PathBuf> {
    let unique = format!(
        "usdx-self-test-config-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    );
    let config_home = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&config_home)?;
    // Edition 2024: the process is single-threaded here and Qt has not read
    // the environment yet, so setting it is safe.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &config_home);
    }
    Ok(config_home)
}

/// The arguments of the screenshot harness (UI_SPEC 8.1):
/// `--screenshot <out.png> --demo-state <name> [--song <file>] [--size WxH]`.
#[derive(Debug)]
struct ScreenshotArgs {
    out: std::path::PathBuf,
    state: String,
    song: Option<std::path::PathBuf>,
    size: Option<(i32, i32)>,
}

/// Parses the screenshot harness arguments; None when the command line does
/// not ask for a screenshot, Some(Err) when the flags are malformed.
#[cfg(debug_assertions)]
fn parse_screenshot_args(args: &[String]) -> Option<Result<ScreenshotArgs, String>> {
    if !args.iter().any(|arg| arg == "--screenshot") {
        return None;
    }
    let mut out = None;
    let mut state = None;
    let mut song = None;
    let mut size = None;
    let mut index = 0;
    while index < args.len() {
        let name = args[index].as_str();
        let value = |index: &mut usize| {
            *index += 1;
            args.get(*index).cloned()
        };
        match name {
            "--screenshot" => match value(&mut index) {
                Some(value) => out = Some(std::path::PathBuf::from(value)),
                None => return Some(Err("--screenshot needs a path".to_owned())),
            },
            "--demo-state" => match value(&mut index) {
                Some(value) => state = Some(value),
                None => return Some(Err("--demo-state needs a name".to_owned())),
            },
            "--song" => match value(&mut index) {
                Some(value) => song = Some(std::path::PathBuf::from(value)),
                None => return Some(Err("--song needs a path".to_owned())),
            },
            "--size" => {
                let Some(value) = value(&mut index) else {
                    return Some(Err("--size needs WIDTHxHEIGHT".to_owned()));
                };
                match parse_size(&value) {
                    Some(parsed) => size = Some(parsed),
                    None => {
                        return Some(Err(format!(
                            "--size must be WIDTHxHEIGHT with positive numbers, got {value:?}"
                        )));
                    }
                }
            }
            other => {
                return Some(Err(format!(
                    "unknown argument {other:?} for the screenshot harness"
                )));
            }
        }
        index += 1;
    }
    let Some(out) = out else {
        return Some(Err(
            "--demo-state was given but no --screenshot path".to_owned()
        ));
    };
    let Some(state) = state else {
        return Some(Err(
            "--screenshot was given but no --demo-state name".to_owned()
        ));
    };
    Some(Ok(ScreenshotArgs {
        out,
        state,
        song,
        size,
    }))
}

/// Parses a `WIDTHxHEIGHT` size argument; both parts must be positive.
#[cfg(debug_assertions)]
fn parse_size(value: &str) -> Option<(i32, i32)> {
    let (width, height) = value.split_once('x')?;
    let width = width.parse::<i32>().ok()?;
    let height = height.parse::<i32>().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

/// Copies the song into a temporary folder and symlinks the audio files it
/// names beside the copy, so the original file and its media stay untouched
/// and later demo states still find the audio (UI_SPEC 8.1: a copy in a temp
/// folder, never the original). Returns the copy's path and the folder to
/// remove when the run ends.
#[cfg(debug_assertions)]
fn copy_song_with_audio(
    path: &std::path::Path,
) -> std::io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let folder = std::env::temp_dir().join(format!(
        "usdx-screenshot-song-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&folder)?;
    let copy = folder.join(path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the song path has no file name",
        )
    })?);
    std::fs::copy(path, &copy)?;
    // The audio headers the format defines for the track selector; each one
    // names a file relative to the song folder.
    if let Ok(bytes) = std::fs::read(path)
        && let Ok(decoded) = usdx_format::bytes::decode(&bytes)
    {
        let document = usdx_format::rows::classify(decoded);
        for value in document
            .rows
            .iter()
            .filter_map(|row| match &row.kind {
                usdx_format::rows::RowKind::Header { key, value }
                    if matches!(key.as_str(), "AUDIO" | "MP3" | "VOCALS" | "INSTRUMENTAL") =>
                {
                    Some(value)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
        {
            let audio = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(value);
            if let Some(name) = audio.file_name() {
                let link = folder.join(name);
                if !link.exists() && audio.is_file() {
                    std::os::unix::fs::symlink(&audio, &link)?;
                }
            }
        }
    }
    Ok((copy, folder))
}

/// Applies the named demo state to the loaded window (UI_SPEC 8.1). The
/// states of screens that later tasks build are added by those tasks; every
/// unknown name is a clear failure instead of a wrong screenshot.
#[cfg(debug_assertions)]
fn apply_demo_state(state: &str) -> Result<(), String> {
    match state {
        "start-screen" => Ok(()),
        other => Err(format!(
            "unknown demo state \"{other}\"; available: start-screen"
        )),
    }
}

/// Runs the screenshot harness: applies the fixed offscreen environment,
/// loads the window, opens the song copy, applies the demo state, resizes,
/// waits for the window to settle and saves the PNG (UI_SPEC 8.1).
#[cfg(debug_assertions)]
fn run_screenshot(screenshot: ScreenshotArgs) -> i32 {
    let ScreenshotArgs {
        out,
        state,
        song,
        size,
    } = screenshot;

    // The fixed environment of UI_SPEC 8.1, but only for values the caller
    // did not already choose: scripts/ui-shots.sh sets its own theme through
    // XDG_CONFIG_HOME and runs offscreen itself.
    for (name, value) in [
        ("QT_QPA_PLATFORM", "offscreen"),
        ("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop"),
        ("QT_QPA_PLATFORMTHEME", "kde"),
    ] {
        if std::env::var_os(name).is_none() {
            // Single-threaded start-up, Qt has not read the environment yet.
            unsafe { std::env::set_var(name, value) };
        }
    }

    // The song opens from a copy, so the screenshot can never modify the
    // original file (UI_SPEC 8.1).
    let song_copy = match &song {
        Some(path) => match copy_song_with_audio(path) {
            Ok(copy) => Some(copy),
            Err(error) => {
                eprintln!("screenshot failed: the song copy could not be made: {error}");
                return 1;
            }
        },
        None => None,
    };

    let mut app = QApplication::new();
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&"org.kde.desktop".into());
    }
    QGuiApplication::set_desktop_file_name(&DESKTOP_FILE_NAME.into());

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(QML_MAIN_URL));
    }
    let engine_ref = match engine.as_ref() {
        Some(engine) => engine,
        None => {
            eprintln!("screenshot failed: the window root object was not created");
            return 1;
        }
    };

    if let Some((copy, _folder)) = &song_copy
        && !bridge::selftest::usdx_open_song(
            engine_ref,
            &QString::from(copy.to_string_lossy().as_ref()),
        )
    {
        eprintln!("screenshot failed: the song copy did not open");
        return 1;
    }

    if let Err(message) = apply_demo_state(&state) {
        eprintln!("screenshot failed: {message}");
        return 1;
    }

    if let Some((width, height)) = size
        && !bridge::screenshot::usdx_resize_window(engine_ref, width, height)
    {
        eprintln!("screenshot failed: the window could not be resized to {width}x{height}");
        return 1;
    }

    if !bridge::screenshot::usdx_screenshot_window(
        engine_ref,
        &QString::from(out.to_string_lossy().as_ref()),
    ) {
        eprintln!(
            "screenshot failed: the window did not save a PNG at {}",
            out.display()
        );
        return 1;
    }

    if let Some((_, folder)) = &song_copy {
        let _ = std::fs::remove_dir_all(folder);
    }
    let _ = &mut app;
    0
}

/// Runs the application for the given command line arguments (without the program
/// name) and returns the process exit code.
pub fn app_main(args: impl Iterator<Item = String>) -> i32 {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("{}", version_line());
        return 0;
    }

    // The screenshot harness is a development tool (UI_SPEC 8.1) and only
    // exists in debug builds; a release build refuses it with a clear
    // message instead of silently ignoring the flags.
    #[cfg(debug_assertions)]
    if let Some(parsed) = parse_screenshot_args(&args) {
        let screenshot = match parsed {
            Ok(screenshot) => screenshot,
            Err(message) => {
                eprintln!("screenshot failed: {message}");
                return 1;
            }
        };
        return run_screenshot(screenshot);
    }
    #[cfg(not(debug_assertions))]
    if args.iter().any(|arg| arg == "--screenshot") {
        eprintln!("screenshot failed: the screenshot harness is only in debug builds");
        return 1;
    }

    let self_test = args.iter().any(|arg| arg == "--self-test");
    // The positional argument is the song path: `usdx-song-editor [SONG.txt]`,
    // and `--self-test SONG.txt` loads it through the window's open guard.
    let positional: Vec<&String> = args.iter().filter(|arg| !arg.starts_with('-')).collect();

    // The self-test must not touch the user's settings: the config folder is
    // redirected before QApplication reads the environment.
    let config_home = if self_test {
        match self_test_config_home() {
            Ok(config_home) => Some(config_home),
            Err(error) => {
                eprintln!("self-test failed: the config folder could not be created: {error}");
                return 1;
            }
        }
    } else {
        None
    };

    if self_test {
        bridge::selftest::usdx_install_message_handler();
    }

    let mut app = QApplication::new();
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&"org.kde.desktop".into());
    }
    QGuiApplication::set_desktop_file_name(&DESKTOP_FILE_NAME.into());

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(QML_MAIN_URL));
    }

    if self_test {
        let code = if let Some(path) = positional.first() {
            let ok = engine.as_ref().is_some_and(|engine| {
                bridge::selftest::usdx_selftest_load_song(engine, &QString::from(path.as_str()))
                    && bridge::selftest::usdx_selftest_ui_paths(engine)
            });
            if !ok {
                eprintln!("self-test failed: the song did not load through the bridge");
                1
            } else {
                0
            }
        } else {
            let ok = engine.as_ref().is_some_and(|engine| {
                bridge::selftest::usdx_qml_root_object_exists(engine)
                    && bridge::selftest::usdx_selftest_start_screen(engine)
            });
            if !ok {
                eprintln!("self-test failed: the window or its start screen is not as expected");
                1
            } else {
                0
            }
        };
        // QML reports binding and script errors only as warnings, so any
        // warning fails the self-test.
        let warnings = bridge::selftest::usdx_qt_warning_count();
        let code = if code == 0 && warnings > 0 {
            eprintln!("self-test failed: Qt printed {warnings} warning(s)");
            1
        } else {
            code
        };
        if let Some(config_home) = config_home {
            let _ = std::fs::remove_dir_all(config_home);
        }
        return code;
    }

    match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    }
}
