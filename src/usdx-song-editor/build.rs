// About me: build script of the usdx-song-editor application. It builds the QML
// module io.github.ueda_ichitaka.UsdxSongEditor (Main.qml is registered under
// qrc:/qt/qml/io/github/ueda_ichitaka/UsdxSongEditor/qml/Main.qml), runs the cxx-qt
// code generation for the bridge files, and compiles the small C++ helper for the
// --self-test root object check, which the cxx-qt-lib bindings do not provide.

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_module =
        QmlModule::new("io.github.ueda_ichitaka.UsdxSongEditor").qml_files(["qml/Main.qml"]);
    unsafe {
        CxxQtBuilder::new_qml_module(qml_module)
            .files(["src/bridge/selftest.rs"])
            .cpp_file("cpp/selftest.cpp")
            .cc_builder(|cc| {
                cc.include("cpp");
            })
            .build();
    }
}
