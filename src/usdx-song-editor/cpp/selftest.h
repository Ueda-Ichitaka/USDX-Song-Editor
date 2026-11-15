// About me: declaration of the --self-test helpers bound by src/bridge/selftest.rs.
// They check QQmlApplicationEngine::rootObjects() so the app can exit non-zero when
// the QML window root object was not created, open a song silently through the
// SongDocument bridge (the seam of the --song flag), open and verify a song
// through the window's requestOpen() guard (line count, lines model rows and
// window title), drive the QML paths a user reaches, count Qt warnings, and
// run the regression checks (drawer, sheet interaction, playhead, text
// editing) through synthesized input events.

#pragma once
#include <QQmlApplicationEngine>
#include <QString>

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine);

// Installs a Qt message handler that writes every message to stderr and counts
// the warnings; the self-test uses it so QML errors are visible without a
// display and fail the run.
void usdx_install_message_handler();

// The number of warnings, criticals and fatals since the handler was installed.
int usdx_qt_warning_count();

// Opens the song at path through the SongDocument bridge's open() invokable
// without any output; false when the open failed or the bridge was not found.
bool usdx_open_song(const QQmlApplicationEngine &engine, const QString &path);

// Self-test verification: opens the song through the window's requestOpen()
// guard, prints "lines: N" and
// "title: <window title>" on stdout and checks that the lines model row count
// matches the document and that the window title shows the song instead of
// the plain app name.
bool usdx_selftest_load_song(const QQmlApplicationEngine &engine, const QString &path);

// Reads the named property of the loaded window root object as a bool; false
// when no root object exists. Used by the G-08 drawer probe.
bool usdx_qml_root_property_bool(const QQmlApplicationEngine &engine, const QString &name);

// Finds the first descendant with the object name and invokes its click()
// method, the way a real interaction reaches the onClicked handler; false when
// no such descendant exists or the click could not be invoked.
bool usdx_qml_click_child(const QQmlApplicationEngine &engine, const QString &objectName);

// Drives the QML paths a user reaches after opening a song: renders the window
// (lines pane delegates, Open Recent entries), checks that the editor shows
// every playback control and no start-screen status, that the player rate
// behind the Speed combo starts at 1x, prints "sheet view: START END" (the
// time range the sheet shows), "lane colour: #aarrggbb" (the waveform colour
// from the app palette), "lane status: no audio|loading|ready" (the
// waveform lane after its analysis, waiting up to 15 s) and "player track: N" (the track
// chosen when the song opened), runs the activation check,
// triggers the Discard and Cancel choices of the Save changes? dialog and the
// Reload choice of the Song changed on disk dialog, and requests quit through
// the unsaved-changes guard. Prints "ui paths: ok" on success.
bool usdx_selftest_ui_paths(const QQmlApplicationEngine &engine);

// Checks the start screen without a song (UI_SPEC 4.1): the toolbar hides the
// playback controls other than Play and the status bar shows "No song open".
// Prints "start screen: ok" on success.
bool usdx_selftest_start_screen(const QQmlApplicationEngine &engine);

// Checks the main menu drawer for the Kirigami regression found in the
// phase-5 testing: GlobalDrawer.qml forces `enabled: !isMenu` on the whole
// drawer, which greyed out every action even with a song open. MainMenu.qml
// overrides it with `enabled: true`; this check opens the drawer and asserts
// that the drawer is enabled and at least one action item is enabled.
// Prints "drawer check: ok" on success.
bool usdx_drawer_check(const QQmlApplicationEngine &engine);

// Locks in the phase-5 interaction fixes as regression checks, driven
// through synthesized mouse and wheel events: a tap selects a note, a drag
// on an unselected note selects it implicitly and moves it on release, the
// rubber band stays hidden during a note drag but grows on the empty pitch
// area, Ctrl+wheel zooms x1.25 per step, a plain wheel step scrolls time,
// Shift+wheel scrolls one pitch row, and the horizontal scrollbar tracks
// the view. Prints "sheet interaction: ok" on success.
bool usdx_sheet_interaction_check(const QQmlApplicationEngine &engine);

// Checks the playhead (user decisions 2026-11-15): a press in the waveform
// lane seeks to the time under the pointer, a drag scrubs, the dotted
// playhead line stands under the pointer afterwards, and following the
// playback makes the line being sung the current line. Prints
// "playhead: ok" on success.
bool usdx_playhead_check(const QQmlApplicationEngine &engine);

// Checks the text editing (task P6-02) through real key and mouse events:
// the Edit line dialog (Ctrl+E, preview, Enter applies one undo step, an
// added word splits a note) and the inline syllable field (a double click
// on a note or its lyric label, Enter commits, Esc cancels). Prints "text
// editing: ok" on success.
bool usdx_text_edit_check(const QQmlApplicationEngine &engine);

// Checks the song end features (user decisions 2026-11-15) through real
// input: the scroll bounds, Home / End, the End song here button with its
// shaded #END area and undo, and the dashed drag guides. Prints "song end:
// ok" on success.
bool usdx_song_end_check(const QQmlApplicationEngine &engine);
