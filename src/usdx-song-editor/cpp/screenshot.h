// About me: helpers of the screenshot harness (UI_SPEC 8.1, task P3-06). They
// exist because cxx-qt-lib does not bind QQuickWindow, QQuickItemGrabResult or
// QImage: one helper resizes the window of a loaded scene, one grabs the
// window content offscreen with grabToImage, waits for the result (at most
// 10 s) and saves it as a PNG, two read the size and a pixel colour back
// from a PNG so the tests can assert what was rendered without adding an
// image-decoding crate, and two drive the waveform lane of the audio demo
// states: one pins it at a loading fraction, one waits for its analysis and
// describes what it shows.

#pragma once

#include <cxx-qt-lib/qqmlapplicationengine.h>
#include <cxx-qt-lib/qstring.h>

// Resizes the window of the loaded scene to the given size; false when no
// window root object was created.
bool usdx_resize_window(const QQmlApplicationEngine &engine, int width, int height);

// Grabs the content of the loaded window with grabToImage, waits for the
// grab result (at most 10 s) and saves it as a PNG at the path; false when
// no window exists, the grab failed or it timed out.
bool usdx_screenshot_window(const QQmlApplicationEngine &engine, const QString &path);

// Reads a PNG and reports its size as "widthxheight", or an empty string
// when the file cannot be read as an image.
QString usdx_png_size(const QString &path);

// Reads a PNG and reports the colour of the pixel at (x, y) as "#rrggbb",
// or an empty string when the file cannot be read or the point is outside.
QString usdx_png_pixel(const QString &path, int x, int y);

// Pins the waveform lane of the sheet background in the loading state at the
// fraction (the audio-states-loading demo state); false without the item.
bool usdx_hold_lane_loading(const QQmlApplicationEngine &engine, double fraction);

// Waits up to timeoutMs while the waveform lane loads, then describes it:
// "no audio", "loading N %", "ready" or "ready, no vocals"; an empty string
// without the item.
QString usdx_settle_lanes(const QQmlApplicationEngine &engine, int timeoutMs);
