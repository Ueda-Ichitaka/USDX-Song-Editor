// About me: declaration of the --self-test helpers bound by src/bridge/selftest.rs.
// They check QQmlApplicationEngine::rootObjects() so the app can exit non-zero when
// the QML window root object was not created, and load a song through the
// SongDocument bridge and report its line count on stdout (task P3-04).

#pragma once
#include <QQmlApplicationEngine>
#include <QString>

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine);
bool usdx_selftest_load_song(const QQmlApplicationEngine &engine, const QString &path);

// Reads the named property of the loaded window root object as a bool; false
// when no root object exists. Used by the G-08 drawer probe.
bool usdx_qml_root_property_bool(const QQmlApplicationEngine &engine, const QString &name);

// Finds the first descendant with the object name and invokes its click()
// method, the way a real interaction reaches the onClicked handler; false when
// no such descendant exists or the click could not be invoked.
bool usdx_qml_click_child(const QQmlApplicationEngine &engine, const QString &objectName);
