// About me: implementation of the --self-test helper bound by src/bridge/selftest.rs;
// it checks QQmlApplicationEngine::rootObjects() so the app can exit non-zero when the
// QML window root object was not created.

#include "selftest.h"

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine) {
    return !engine.rootObjects().isEmpty();
}
