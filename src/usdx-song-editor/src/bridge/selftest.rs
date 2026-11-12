//! About me: cxx bridge for the --self-test root object check. The cxx-qt-lib
//! bindings of QQmlApplicationEngine do not include rootObjects(), so this bridge
//! binds the small C++ helper in cpp/selftest.cpp that reports whether the loaded QML
//! created a window root object.

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("selftest.h");
        fn usdx_qml_root_object_exists(engine: &QQmlApplicationEngine) -> bool;
    }
}

pub use ffi::usdx_qml_root_object_exists;
