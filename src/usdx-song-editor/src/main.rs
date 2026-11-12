//! The usdx-song-editor binary.
//!
//! About me: thin entry point of the editor application. It forwards the command
//! line to the library (`app_main` in src/lib.rs), which starts the Qt event loop,
//! loads Main.qml from the compiled-in QML module and handles --version and
//! --self-test. All application logic lives in the lib target so that tests and the
//! CMake import of task P0-04 can reuse it.

fn main() {
    let code = usdx_song_editor::app_main(std::env::args().skip(1));
    std::process::exit(code);
}
