//! About me: cxx bridge for the --self-test helpers and the QML probes. The
//! cxx-qt-lib bindings of QQmlApplicationEngine do not include rootObjects(),
//! so this bridge binds the small C++ helpers in cpp/selftest.cpp: one reports
//! whether the loaded QML created a window root object, one opens a song
//! silently through the SongDocument bridge (the seam the --song flag of the
//! screenshot harness uses), one verifies the loaded song end to end and
//! prints the line count and window title, and two serve the G-08 drawer
//! probe (reading a window property as bool and clicking a child object
//! through its invokable click() method).

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("selftest.h");
        fn usdx_qml_root_object_exists(engine: &QQmlApplicationEngine) -> bool;
        fn usdx_install_message_handler();
        fn usdx_open_song(engine: &QQmlApplicationEngine, path: &QString) -> bool;
        fn usdx_selftest_load_song(engine: &QQmlApplicationEngine, path: &QString) -> bool;
        fn usdx_qml_root_property_bool(engine: &QQmlApplicationEngine, name: &QString) -> bool;
        fn usdx_qml_click_child(engine: &QQmlApplicationEngine, object_name: &QString) -> bool;
    }
}

pub use ffi::{
    usdx_install_message_handler, usdx_open_song, usdx_qml_click_child,
    usdx_qml_root_object_exists, usdx_qml_root_property_bool, usdx_selftest_load_song,
};
