// About me: declaration of the --self-test helper that reports whether the loaded QML
// created a window root object. The cxx bridge in src/bridge/selftest.rs binds it,
// because the cxx-qt-lib bindings do not expose QQmlApplicationEngine::rootObjects().

#pragma once
#include <QQmlApplicationEngine>

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine);
