// About me: helpers of the screenshot harness (UI_SPEC 8.1, task P3-06). They
// exist because cxx-qt-lib does not bind QQuickWindow, QQuickItemGrabResult or
// QImage: one helper resizes the window of a loaded scene, one grabs the
// window content offscreen with grabToImage, waits for the result (at most
// 10 s) and saves it as a PNG, two read the size and a pixel colour back
// from a PNG so the tests can assert what was rendered without adding an
// image-decoding crate, and two drive the waveform lane of the audio demo
// states: one pins it at a loading fraction, one waits for its analysis and
// describes what it shows; the rest set up the editor and line-text demo
// states.

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

// Selects the note at the given position among the current line's notes of
// the notes model (0-based, counting only the current line's rows) through
// selectRange with a band inside the note; false without a document, model
// or row. The editor demo state uses it so the screenshot shows the
// selection and the populated inspector like the mockup.
bool usdx_select_demo_note(const QQmlApplicationEngine &engine, int position);

// Seeks the player to the given fraction of the current line's time span,
// so the editor screenshot shows the playhead inside the view like the
// mockup; false without a document or player.
bool usdx_seek_demo_playhead(const QQmlApplicationEngine &engine, double fraction);

// Makes the global line current and opens the Edit line dialog for it with
// the given text (the line-text demo state, UI_SPEC 5.4); false without the
// document or the dialog.
bool usdx_open_demo_line_text(const QQmlApplicationEngine &engine, int line, const QString &text);

// Pushes the repair page, switches the repair bridge to the block mode
// and seeds the block over the global lines first..=last (the block demo
// state, task P7-08); false when the seed does not land in the bridge
// (the blockFirst/blockLast/mode properties are read back).
bool usdx_open_demo_repair_block(const QQmlApplicationEngine &engine, int first, int last);

// Sets up the repair demo states on the synthetic fault of plan 6.7 (UI_SPEC
// 8.1): opens the repair page, compresses a run of lines to a quarter of its
// duration with a block transform (lines 15-22 on a song of at least 24
// lines, else the first line), and then either pins the first compressed
// line and the line after the run at their original starts in the anchor
// mode (the fault moves the following lines along, so the line after the
// run leaves its place), or selects the compressed lines in the block mode
// with their old end as the new end (the fault keeps the following lines
// in place). Returns the summary line
// the harness prints ("repair demo: lines 15-22 compressed to a quarter,
// anchors 2, preview yes"), empty when a step failed.
QString usdx_open_demo_repair_fault(const QQmlApplicationEngine &engine, bool blockMode);

// Sets up the tap-along demo state (UI_SPEC 8.1): the synthetic fault of the
// anchor demo, then the tap-along page over the compressed lines in the
// syllable mode, the session recording with up to 12 taps at the note
// starts (fed as positions, no playback). Returns the summary line the
// harness prints ("tap demo: lines 15-22, taps 12 / 57, state recording"),
// empty when a step failed.
QString usdx_open_demo_tap_along(const QQmlApplicationEngine &engine);

// Sets up the export-anchors demo state (UI_SPEC 8.1): the anchor demo on
// the synthetic fault, the stretch applied, then the Export anchors dialog
// open. Returns the summary line the harness prints ("export demo: 9 lines,
// 7 changed, 2 pinned"), empty when a step failed.
QString usdx_open_demo_export_anchors(const QQmlApplicationEngine &engine);

// Sets up the latency-calibration demo state (UI_SPEC 8.1, 5.7): the
// tap-along demo's page with the Measure tap latency dialog open, the clicks
// stopped and five taps fed at 34, 38, 36, 31 and 37 ms after clicks 1 to 5.
// Returns the summary line the harness prints ("calibration demo: 5 taps,
// median 36 ms"), empty when a step failed.
QString usdx_open_demo_latency_calibration(const QQmlApplicationEngine &engine);
