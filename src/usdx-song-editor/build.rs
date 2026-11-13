// About me: build script of the usdx-song-editor application. It builds the QML
// module io.github.ueda_ichitaka.UsdxSongEditor (the shell QML files, with the
// shared format.js helper that is not a component and stays out of the qmldir),
// runs the cxx-qt code generation for the bridge files (including the painted
// SheetBackground item, a QQuickPaintedItem from the Quick module), and compiles the small
// C++ helpers: the --self-test helpers, the relative-date formatter for the
// recent songs list, and the screenshot harness helpers (UI_SPEC 8.1), which
// the cxx-qt-lib bindings do not provide.

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_module = QmlModule::new("io.github.ueda_ichitaka.UsdxSongEditor").qml_files([
        "qml/Main.qml",
        "qml/Metrics.qml",
        "qml/Palette.qml",
        "qml/MainMenu.qml",
        "qml/EditorPage.qml",
        "qml/LinesPane.qml",
        "qml/LineHeader.qml",
        "qml/StatusBar.qml",
        "qml/format.js",
    ]);
    unsafe {
        CxxQtBuilder::new_qml_module(qml_module)
            // The screenshot helpers use QQuickWindow and QQuickItemGrabResult,
            // which live in the Quick module.
            .qt_module("Quick")
            .files([
                "src/bridge/selftest.rs",
                "src/bridge/song_document.rs",
                "src/bridge/screenshot.rs",
                "src/bridge/sheet_background.rs",
            ])
            .cpp_files(["cpp/selftest.cpp", "cpp/format.cpp", "cpp/screenshot.cpp"])
            .cc_builder(|cc| {
                cc.include("cpp");
            })
            .build();
    }
}
