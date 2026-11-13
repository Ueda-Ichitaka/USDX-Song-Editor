// About me: declaration of the --self-test helpers bound by src/bridge/selftest.rs.
// They check QQmlApplicationEngine::rootObjects() so the app can exit non-zero when
// the QML window root object was not created, open a song silently through the
// SongDocument bridge (also the seam for the later --song flag), and verify the
// loaded song end to end: line count, lines model rows and window title.

#pragma once
#include <QQmlApplicationEngine>
#include <QString>

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine);

// Installs a Qt message handler that writes every warning to stderr; the
// self-test uses it so QML load errors are visible without a display.
void usdx_install_message_handler();

// Opens the song at path through the SongDocument bridge's open() invokable
// without any output; false when the open failed or the bridge was not found.
bool usdx_open_song(const QQmlApplicationEngine &engine, const QString &path);

// Self-test verification: opens the song, prints "lines: N" and
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
