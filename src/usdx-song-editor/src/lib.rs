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
//! verifies offscreen that the QML loads and the window root object exists
//! (plan section 8.1); with a song path it opens the song through the SongDocument
//! bridge and reports its line count on stdout (plan section 8.3, task P3-04).

pub mod bridge;
pub mod controller;

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

/// Runs the application for the given command line arguments (without the program
/// name) and returns the process exit code.
pub fn app_main(args: impl Iterator<Item = String>) -> i32 {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("{}", version_line());
        return 0;
    }
    let self_test = args.iter().any(|arg| arg == "--self-test");
    // The positional argument is the song path: `usdx-song-editor [SONG.txt]`,
    // and `--self-test SONG.txt` loads it through the bridge (task P3-04).
    let positional: Vec<&String> = args.iter().filter(|arg| !arg.starts_with('-')).collect();

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
        if let Some(path) = positional.first() {
            let ok = engine.as_ref().is_some_and(|engine| {
                bridge::selftest::usdx_selftest_load_song(engine, &QString::from(path.as_str()))
            });
            if !ok {
                eprintln!("self-test failed: the song did not load through the bridge");
                return 1;
            }
            return 0;
        }
        let ok = engine
            .as_ref()
            .is_some_and(bridge::selftest::usdx_qml_root_object_exists);
        if !ok {
            eprintln!("self-test failed: the window root object was not created");
            return 1;
        }
        return 0;
    }

    match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    }
}
