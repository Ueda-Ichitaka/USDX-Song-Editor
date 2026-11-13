// About me: implementation of the helpers bound by src/bridge/selftest.rs.
// usdx_qml_root_object_exists reports whether the loaded QML created a window
// root object; usdx_open_song opens a song silently through the bridge and
// usdx_selftest_load_song builds on it, printing the line count and the
// window title and checking that the lines model and the title match;
// usdx_qml_root_property_bool and usdx_qml_click_child serve the G-08 drawer
// probe in tests/g08_drawer_probe.rs. The self-test opens its song through the
// window's requestOpen() guard, the way the Open button and drag and drop do,
// and usdx_selftest_ui_paths drives the rest of the QML paths a user reaches.
// The installed message handler counts every Qt warning, so the self-test can
// fail on QML binding and script errors, which QML reports only as warnings.

#include "selftest.h"

#include <QAbstractItemModel>
#include <QColor>
#include <QEventLoop>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <initializer_list>
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

// The number of warnings, criticals and fatals the message handler saw.
int g_qt_warning_count = 0;

// Runs the event loop for the given time, so bindings settle, the window
// renders and lazily created delegates exist; the self-test does not run the
// application event loop.
void pump_events(int milliseconds) {
    QEventLoop loop;
    QTimer::singleShot(milliseconds, &loop, &QEventLoop::quit);
    loop.exec();
}

// Invokes a parameterless QML function of the window root object.
bool invoke_root_function(QObject *root, const char *name) {
    QVariant ignored;
    if (!QMetaObject::invokeMethod(root, name, Q_RETURN_ARG(QVariant, ignored))) {
        std::cerr << "self-test failed: the window function " << name << "() was not found\n";
        return false;
    }
    return true;
}

// Triggers the Kirigami.Action with the object name, the way a click on its
// dialog button does.
bool trigger_action(QObject *root, const char *objectName) {
    QObject *const action = root->findChild<QObject *>(QLatin1String(objectName));
    if (action == nullptr) {
        std::cerr << "self-test failed: the action " << objectName << " was not found\n";
        return false;
    }
    if (!QMetaObject::invokeMethod(action, "trigger")) {
        std::cerr << "self-test failed: the action " << objectName << " could not be triggered\n";
        return false;
    }
    return true;
}

// The toolbar controls the start screen hides and the editor shows (UI_SPEC
// 4.1, 4.3), by object name.
constexpr const char *kSongOnlyControls[] = {"playLineButton", "loopButton", "trackLabel",
                                             "trackCombo", "speedLabel", "speedCombo"};

// Checks that every named control is shown (or hidden) as wanted.
bool check_visible(QObject *root, std::initializer_list<const char *> names, bool wanted,
                   const char *screen) {
    for (const char *name : names) {
        QObject *const item = root->findChild<QObject *>(QLatin1String(name));
        if (item == nullptr) {
            std::cerr << "self-test failed: the control " << name << " was not found\n";
            return false;
        }
        if (item->property("visible").toBool() != wanted) {
            std::cerr << "self-test failed: the " << screen << " " << (wanted ? "hides" : "shows")
                      << " " << name << "\n";
            return false;
        }
    }
    return true;
}

bool check_song_only_controls(QObject *root, bool wanted, const char *screen) {
    for (const char *name : kSongOnlyControls) {
        if (!check_visible(root, {name}, wanted, screen)) {
            return false;
        }
    }
    return true;
}

void usdx_message_handler(QtMsgType type, const QMessageLogContext &,
                          const QString &message) {
    if (type == QtWarningMsg || type == QtCriticalMsg || type == QtFatalMsg) {
        ++g_qt_warning_count;
    }
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

int usdx_qt_warning_count() {
    return g_qt_warning_count;
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
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    QVariant ignored;
    if (!QMetaObject::invokeMethod(root, "requestOpen", Q_RETURN_ARG(QVariant, ignored),
                                   Q_ARG(QVariant, QVariant(QUrl::fromLocalFile(path))))) {
        std::cerr << "self-test failed: the window function requestOpen() was not found\n";
        return false;
    }
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const model = root->property("linesModel").value<QObject *>();
    if (document == nullptr || model == nullptr) {
        std::cerr << "self-test failed: the songDocument or linesModel window property "
                     "is missing\n";
        return false;
    }

    if (!document->property("isLoaded").toBool()) {
        std::cerr << "self-test failed: requestOpen() did not open the song\n";
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

bool usdx_selftest_ui_paths(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    // Render the loaded song: the lines pane delegates and the recent songs
    // menu entries are created only now.
    pump_events(300);
    const auto recentEntries = root->findChildren<QObject *>(QStringLiteral("recentSongAction"));
    if (recentEntries.isEmpty()) {
        std::cerr << "self-test failed: the Open Recent menu has no entry for the opened song\n";
        return false;
    }
    // The editor shows every playback control and the song facts instead of
    // the start screen's status text.
    if (!check_song_only_controls(root, true, "editor") ||
        !check_visible(root, {"noSongStatus"}, false, "editor")) {
        return false;
    }
    // The toolbar's Speed combo shows the player's rate, which is 1x until
    // the user picks another speed.
    QObject *const player = root->findChild<QObject *>(QStringLiteral("player"));
    if (player == nullptr) {
        std::cerr << "self-test failed: the player object was not found\n";
        return false;
    }
    const double rate = player->property("rate").toDouble();
    if (rate != 1.0) {
        std::cerr << "self-test failed: the player rate starts at " << rate << ", not 1\n";
        return false;
    }
    // The time range the sheet shows for the current line (UI_SPEC 3.2).
    QObject *const document = root->property("songDocument").value<QObject *>();
    std::cout << "sheet view: " << document->property("viewStartMs").toDouble() << " "
              << document->property("viewEndMs").toDouble() << std::endl;
    // The waveform lane analyses the song audio on worker threads; wait
    // (bounded) while it loads, then report what it shows (UI_SPEC 3.7).
    QObject *const lanes = root->findChild<QObject *>(QStringLiteral("sheetBackground"));
    if (lanes == nullptr) {
        std::cerr << "self-test failed: the sheet background was not found\n";
        return false;
    }
    constexpr int kLaneLoading = 1;
    for (int waited = 0; waited < 15000 && lanes->property("laneStatus").toInt() == kLaneLoading;
         waited += 50) {
        pump_events(50);
    }
    std::cout << "lane colour: "
              << lanes->property("waveformColor").value<QColor>().name(QColor::HexArgb).toStdString()
              << std::endl;
    const int laneStatus = lanes->property("laneStatus").toInt();
    std::cout << "lane status: "
              << (laneStatus == 0 ? "no audio" : laneStatus == kLaneLoading ? "loading" : "ready")
              << std::endl;
    // The Track combo shows the track chosen when the song opened.
    std::cout << "player track: " << player->property("track").toInt() << std::endl;
    // The activation check runs when the window becomes active again.
    if (!invoke_root_function(root, "checkActivationNow")) {
        return false;
    }
    // The dialog choices that never write the song: each evaluates its
    // choice and the transition result the way a button click does.
    for (const char *name : {"saveChangesDiscard", "saveChangesCancel", "changedOnDiskReload"}) {
        if (!trigger_action(root, name)) {
            return false;
        }
    }
    pump_events(100);
    // Quit goes through the same unsaved-changes guard as closing the window.
    bool quitRequested = false;
    QObject::connect(&engine, &QQmlEngine::quit, [&quitRequested] { quitRequested = true; });
    if (!invoke_root_function(root, "requestQuit")) {
        return false;
    }
    pump_events(100);
    if (!quitRequested) {
        std::cerr << "self-test failed: requestQuit() did not quit\n";
        return false;
    }
    std::cout << "ui paths: ok" << std::endl;
    return true;
}

bool usdx_selftest_start_screen(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    pump_events(200);
    if (!check_song_only_controls(root, false, "start screen") ||
        !check_visible(root, {"noSongStatus"}, true, "start screen")) {
        return false;
    }
    std::cout << "start screen: ok" << std::endl;
    return true;
}
