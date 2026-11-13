// About me: implementation of the helpers bound by src/bridge/selftest.rs.
// usdx_qml_root_object_exists reports whether the loaded QML created a window
// root object; usdx_open_song opens a song silently through the bridge and
// usdx_selftest_load_song builds on it, printing the line count and the
// window title and checking that the lines model and the title match;
// usdx_qml_root_property_bool and usdx_qml_click_child serve the G-08 drawer
// probe in tests/g08_drawer_probe.rs.

#include "selftest.h"

#include <QAbstractItemModel>
#include <QUrl>
#include <QVariant>
#include <cstdio>
#include <iostream>

namespace {

// The app title the window falls back to when no song title reached the
// property chain; a loaded song must produce something different.
constexpr auto kAppTitle = "USDX Song Editor";

QObject *selftest_root(const QQmlApplicationEngine &engine) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        std::cerr << "self-test failed: no window root object\n";
        return nullptr;
    }
    return roots.first();
}

void usdx_message_handler(QtMsgType type, const QMessageLogContext &,
                          const QString &message) {
    const char *level = type == QtDebugMsg        ? "debug"
                        : type == QtInfoMsg       ? "info"
                        : type == QtWarningMsg    ? "warning"
                        : type == QtCriticalMsg   ? "critical"
                        : type == QtFatalMsg      ? "fatal"
                                                  : "unknown";
    std::fprintf(stderr, "qt %s: %s\n", level, message.toUtf8().constData());
}

} // namespace

void usdx_install_message_handler() {
    qInstallMessageHandler(usdx_message_handler);
}

bool usdx_qml_root_object_exists(const QQmlApplicationEngine &engine) {
    return !engine.rootObjects().isEmpty();
}

bool usdx_open_song(const QQmlApplicationEngine &engine, const QString &path) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    QObject *const document = root->property("songDocument").value<QObject *>();
    if (document == nullptr) {
        std::cerr << "self-test failed: the songDocument window property is missing\n";
        return false;
    }
    bool opened = false;
    if (!QMetaObject::invokeMethod(document, "open", Q_RETURN_ARG(bool, opened),
                                   Q_ARG(QUrl, QUrl::fromLocalFile(path)))) {
        std::cerr << "self-test failed: the open invokable was not found\n";
        return false;
    }
    if (!opened) {
        std::cerr << "self-test failed: the song did not open\n";
        return false;
    }
    return true;
}

bool usdx_selftest_load_song(const QQmlApplicationEngine &engine, const QString &path) {
    if (!usdx_open_song(engine, path)) {
        return false;
    }
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const model = root->property("linesModel").value<QObject *>();
    if (document == nullptr || model == nullptr) {
        std::cerr << "self-test failed: the songDocument or linesModel window property "
                     "is missing\n";
        return false;
    }

    const int lineCount = document->property("lineCount").toInt();
    const auto *const abstractModel = qobject_cast<const QAbstractItemModel *>(model);
    const int modelCount = abstractModel != nullptr ? abstractModel->rowCount() : -1;
    std::cout << "lines: " << lineCount << std::endl;
    if (lineCount <= 0 || lineCount != modelCount) {
        std::cerr << "self-test failed: the lines model has " << modelCount
                  << " rows for " << lineCount << " lines\n";
        return false;
    }

    const QString title = root->property("title").toString();
    std::cout << "title: " << title.toStdString() << std::endl;
    if (title == QLatin1String(kAppTitle)) {
        std::cerr << "self-test failed: the window title stayed the plain app name\n";
        return false;
    }
    return true;
}

bool usdx_qml_root_property_bool(const QQmlApplicationEngine &engine, const QString &name) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return false;
    }
    return roots.first()->property(name.toUtf8().constData()).toBool();
}

bool usdx_qml_click_child(const QQmlApplicationEngine &engine, const QString &objectName) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return false;
    }
    QObject *const child = roots.first()->findChild<QObject *>(objectName);
    if (child == nullptr) {
        return false;
    }
    return QMetaObject::invokeMethod(child, "click");
}
