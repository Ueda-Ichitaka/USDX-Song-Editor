//! About me: cxx bridge for the screenshot harness helpers in
//! cpp/screenshot.{h,cpp} (UI_SPEC 8.1, task P3-06). The cxx-qt-lib bindings
//! do not include QQuickWindow, QQuickItemGrabResult or QImage, so the
//! screenshot mode of the application and the screenshot tests reach them
//! through these small extern functions; two more drive the waveform lane of
//! the audio demo states (pin it loading, wait for it and describe it).

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
        fn usdx_hold_lane_loading(engine: &QQmlApplicationEngine, fraction: f64) -> bool;
        fn usdx_settle_lanes(engine: &QQmlApplicationEngine, timeout_ms: i32) -> QString;
    }
}

pub use ffi::{
    usdx_hold_lane_loading, usdx_png_pixel, usdx_png_size, usdx_resize_window,
    usdx_screenshot_window, usdx_settle_lanes,
};
