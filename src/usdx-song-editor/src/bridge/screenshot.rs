//! About me: cxx bridge for the screenshot harness helpers in
//! cpp/screenshot.{h,cpp} (UI_SPEC 8.1, task P3-06). The cxx-qt-lib bindings
//! do not include QQuickWindow, QQuickItemGrabResult or QImage, so the
//! screenshot mode of the application and the screenshot tests reach them
//! through these small extern functions.

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("screenshot.h");
        fn usdx_resize_window(engine: &QQmlApplicationEngine, width: i32, height: i32) -> bool;
        fn usdx_screenshot_window(engine: &QQmlApplicationEngine, path: &QString) -> bool;
        fn usdx_png_size(path: &QString) -> QString;
        fn usdx_png_pixel(path: &QString, x: i32, y: i32) -> QString;
    }
}

pub use ffi::{usdx_png_pixel, usdx_png_size, usdx_resize_window, usdx_screenshot_window};
