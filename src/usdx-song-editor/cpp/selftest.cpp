// About me: implementation of the helpers bound by src/bridge/selftest.rs.
// usdx_qml_root_object_exists reports whether the loaded QML created a window
// root object; usdx_open_song opens a song silently through the bridge and
// usdx_selftest_load_song builds on it, printing the line count and the
// window title and checking that the lines model and the title match;
// usdx_qml_root_property_bool and usdx_qml_click_child serve the G-08 drawer
// probe in tests/g08_drawer_probe.rs. The self-test opens its song through the
// window's requestOpen() guard, the way the Open button and drag and drop do,
// and usdx_selftest_ui_paths drives the rest of the QML paths a user reaches.
// usdx_drawer_check locks in the main-menu fix and usdx_sheet_interaction_check
// locks in the note drag, rubber band and sheet navigation fixes (skipped on
// a song whose current line has fewer than two notes), usdx_playhead_check
// covers the waveform-lane seek, the playhead line and the playback follow,
// usdx_text_edit_check the Edit line dialog and the inline syllable field,
// and usdx_song_end_check the scroll bounds, Home / End, End song here and
// the drag guides, all driven through synthesized mouse, wheel and key events
// (send_mouse/send_wheel/send_key/send_text). The
// installed message handler counts every Qt warning, so the self-test can
// fail on QML binding and script errors, which QML reports only as warnings.

#include "selftest.h"

#include "screenshot.h"

#include <QAbstractItemModel>
#include <QColor>
#include <QCoreApplication>
#include <QEventLoop>
#include <QImage>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMouseEvent>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQmlError>
#include <QSet>
#include <QStringList>
// The self-test's input helpers send mouse events through the qpa
// QWindowSystemInterface with synchronous delivery, the way a platform
// plugin does; the queued QTest helpers never drain in the offscreen
// session.
#include <QtGui/qpa/qwindowsysteminterface.h>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <functional>
#include <initializer_list>
#include <algorithm>
#include <cmath>
#include <cstdint>
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

// The live delegates of the note and phrase end Repeaters, cached until the
// next event pump (see live_delegate).
QSet<QQuickItem *> g_live_items;
bool g_live_items_valid = false;

// Runs the event loop for the given time, so bindings settle, the window
// renders and lazily created delegates exist; the self-test does not run the
// application event loop.
void pump_events(int milliseconds) {
    g_live_items_valid = false;
    QEventLoop loop;
    QTimer::singleShot(milliseconds, &loop, &QEventLoop::quit);
    loop.exec();
}

// Sends one mouse event through the window system interface with
// synchronous delivery - the way a platform plugin does - and pumps the
// event loop afterwards; queued QTest helpers never drain offscreen.
void send_mouse(QWindow *window, const QPointF &local, QEvent::Type type,
                Qt::MouseButtons buttons, Qt::MouseButton button, ulong timestamp,
                Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
    QWindowSystemInterface::handleMouseEvent<QWindowSystemInterface::SynchronousDelivery>(
        window, timestamp, local, window->mapToGlobal(local), buttons, button, type,
        modifiers, Qt::MouseEventNotSynthesized);
    pump_events(10);
}

// Sends one key press and release through the window system interface with
// synchronous delivery and pumps the event loop afterwards, so window
// shortcuts see the key the way a keyboard produces it.
void send_key(QWindow *window, int key, Qt::KeyboardModifiers modifiers) {
    QWindowSystemInterface::handleKeyEvent<QWindowSystemInterface::SynchronousDelivery>(
        window, QEvent::KeyPress, key, modifiers);
    QWindowSystemInterface::handleKeyEvent<QWindowSystemInterface::SynchronousDelivery>(
        window, QEvent::KeyRelease, key, modifiers);
    pump_events(20);
}

// Types the text into the focused item, one key press and release per
// character carrying the character's text, the way a keyboard does.
void send_text(QWindow *window, const QString &text) {
    for (const QChar ch : text) {
        const int key = ch.toUpper().unicode();
        QWindowSystemInterface::handleKeyEvent<QWindowSystemInterface::SynchronousDelivery>(
            window, QEvent::KeyPress, key, Qt::NoModifier, QString(ch));
        QWindowSystemInterface::handleKeyEvent<QWindowSystemInterface::SynchronousDelivery>(
            window, QEvent::KeyRelease, key, Qt::NoModifier, QString(ch));
    }
    pump_events(30);
}

// Double-clicks at the scene position: two presses and releases 60 ms
// apart, inside the double-click interval.
void send_double_click(QWindow *window, const QPointF &at, ulong timestamp) {
    send_mouse(window, at, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, timestamp);
    send_mouse(window, at, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
               timestamp + 20);
    send_mouse(window, at, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
               timestamp + 60);
    send_mouse(window, at, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
               timestamp + 80);
    pump_events(100);
}

// Calls a QString-returning invokable of the document that takes a line
// index (an i32 on the Rust side).
QString document_text(QObject *document, const char *name, int line) {
    QString result;
    QMetaObject::invokeMethod(document, name, Q_RETURN_ARG(QString, result), Q_ARG(int, line));
    return result;
}

// Calls a QString-returning invokable of the document that takes a row id
// (an i64 on the Rust side, std::int64_t in the generated C++).
QString document_text(QObject *document, const char *name, std::int64_t rowId) {
    QString result;
    QMetaObject::invokeMethod(document, name, Q_RETURN_ARG(QString, result),
                              Q_ARG(std::int64_t, rowId));
    return result;
}

// Sends one wheel event straight to the window and pumps the event loop
// afterwards; the window system interface has no synchronous wheel
// delivery, and the queued events never drain in the offscreen session.
void send_wheel(QWindow *window, const QPointF &local, QPoint angle_delta,
                Qt::KeyboardModifiers modifiers) {
    QWheelEvent event(local, window->mapToGlobal(local), QPoint(0, 0), angle_delta,
                      Qt::NoButton, modifiers, Qt::NoScrollPhase, false,
                      Qt::MouseEventNotSynthesized);
    QCoreApplication::sendEvent(window, &event);
    pump_events(10);
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
    // A click on a recent song of the start screen opens it through the same
    // guard as the menu entry. The start screen builds its rows only while
    // it is shown, so it is shown for the click and hidden again afterwards
    // (the song stays loaded, which is what its visibility binding says).
    QObject *const startScreen = root->findChild<QObject *>(QStringLiteral("startScreen"));
    if (startScreen == nullptr) {
        std::cerr << "self-test failed: the start screen was not found\n";
        return false;
    }
    startScreen->setProperty("visible", true);
    pump_events(300);
    // ListView delegates have no QObject parent, so the row is reached
    // through the list's itemAtIndex().
    QObject *const startList = root->findChild<QObject *>(QStringLiteral("startRecentList"));
    QQuickItem *startRow = nullptr;
    if (startList != nullptr) {
        QMetaObject::invokeMethod(startList, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, startRow),
                                  Q_ARG(int, 0));
    }
    if (startRow == nullptr || startRow->objectName() != QStringLiteral("startRecentRow")) {
        std::cerr << "self-test failed: the start screen lists no recent song\n";
        return false;
    }
    const int warningsBeforeClick = usdx_qt_warning_count();
    QMetaObject::invokeMethod(startRow, "click");
    pump_events(300);
    startScreen->setProperty("visible", false);
    pump_events(50);
    if (usdx_qt_warning_count() != warningsBeforeClick) {
        std::cerr << "self-test failed: clicking a recent song on the start screen raised a "
                     "QML error\n";
        return false;
    }
    QObject *const openedDocument = root->property("songDocument").value<QObject *>();
    if (openedDocument == nullptr || !openedDocument->property("isLoaded").toBool()) {
        std::cerr << "self-test failed: the recent song of the start screen did not open\n";
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

// Checks the main menu drawer for the Kirigami regression found in the
// phase-5 testing: GlobalDrawer.qml forces `enabled: !isMenu` on the whole
// drawer, which greyed out every action even with a song open. MainMenu.qml
// overrides it with `enabled: true`; this check opens the drawer and asserts
// that the drawer is enabled and at least one action item is enabled.
bool usdx_drawer_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    QObject *const drawer = root->property("globalDrawer").value<QObject *>();
    if (drawer == nullptr) {
        std::cerr << "drawer check: no globalDrawer on the window\n";
        return false;
    }
    if (!drawer->property("enabled").toBool()) {
        std::cerr << "drawer check: the drawer is disabled (Kirigami's"
                     " enabled: !isMenu regression)\n";
        return false;
    }
    if (!QMetaObject::invokeMethod(drawer, "open")) {
        std::cerr << "drawer check: the drawer has no open() method\n";
        return false;
    }
    pump_events(300);
    auto *const window = qobject_cast<QQuickWindow *>(root);
    if (window == nullptr) {
        std::cerr << "drawer check: the root is not a window\n";
        return false;
    }
    // The action items only exist once the drawer opened.
    int action_items = 0;
    int enabled_items = 0;
    std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
        for (QQuickItem *const child : item->childItems()) {
            if (QString::fromUtf8(child->metaObject()->className())
                    .contains(QStringLiteral("GlobalDrawerActionItem"))) {
                action_items++;
                if (child->isEnabled()) {
                    enabled_items++;
                }
            }
            walk(child);
        }
    };
    if (QQuickItem *const content = drawer->property("contentItem").value<QQuickItem *>()) {
        walk(content);
    }
    QMetaObject::invokeMethod(drawer, "close");
    pump_events(100);
    if (action_items == 0) {
        std::cerr << "drawer check: no action items rendered in the drawer\n";
        return false;
    }
    if (enabled_items == 0) {
        std::cerr << "drawer check: every action item is disabled\n";
        return false;
    }
    std::cout << "drawer check: ok" << std::endl;
    return true;
}

namespace {

// Finds visible note delegates by walking the window's VISUAL tree; the
// delegates are visual children of the clip area, but their QObject parent
// chain does not reach them through findChildren (Repeater keeps the
// QObject parenting). Returns the delegates of the current line (opaque,
// lineOffset 0), first first.
// The items the note and phrase end Repeaters currently hold. A model reset
// leaves the old delegates in the item tree until their deferred deletion,
// which never runs here: the self-test works before the application event
// loop starts. Only the Repeaters' own items are live. The set is cached
// until the next event pump, which is where models change.
bool live_delegate(QQuickItem *item) {
    if (!g_live_items_valid && item->window() != nullptr) {
        g_live_items.clear();
        // The QML objects are QObject children of the window, not of its
        // content item.
        QQuickWindow *const window = item->window();
        for (const char *name : {"noteRepeater", "phraseEndRepeater"}) {
            QObject *const repeater = window->findChild<QObject *>(QLatin1String(name));
            const int count = repeater != nullptr ? repeater->property("count").toInt() : 0;
            for (int index = 0; index < count; index++) {
                QQuickItem *member = nullptr;
                QMetaObject::invokeMethod(repeater, "itemAt", Q_RETURN_ARG(QQuickItem *, member),
                                          Q_ARG(int, index));
                if (member != nullptr) {
                    g_live_items.insert(member);
                }
            }
        }
        g_live_items_valid = true;
    }
    return g_live_items.contains(item);
}

void find_current_line_delegates(QQuickWindow *window, QList<QObject *> &out) {
    std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
        for (QQuickItem *const child : item->childItems()) {
            const QVariant note_id = child->property("noteId");
            if (live_delegate(child) && note_id.isValid() && note_id.toLongLong() >= 0
                && child->isVisible() && child->opacity() >= 1.0 && child->property("lineOffset").toInt() == 0) {
                out.append(child);
            }
            walk(child);
        }
    };
    walk(window->contentItem());
}

} // namespace

// Locks in the phase-5 interaction fixes as regression checks: the
// DragHandlers use `centroid` (DragHandler has no `point` property), the
// rubber band's bandDrag cannot steal an exclusive grab from a note's
// bodyDrag (grabPermissions TakeOverForbidden), a commit's model reset
// cannot throw inside the delegate's handler (the QML resolves references
// before the call), a drag on an unselected note selects it at release, and
// the wheel gestures move the viewport. Everything runs through the same
// input path a user's device produces.
bool usdx_sheet_interaction_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    auto *const window = qobject_cast<QQuickWindow *>(root);
    if (window == nullptr) {
        std::cerr << "sheet interaction: the root is not a window\n";
        return false;
    }
    QObject *const bg = root->findChild<QObject *>(QStringLiteral("sheetBackground"));
    if (bg == nullptr) {
        std::cerr << "sheet interaction: no sheetBackground object\n";
        return false;
    }
    // The check needs two notes in the current line; a smaller song skips
    // it (user decision 2026-11-15) instead of failing.
    QObject *const document = root->property("songDocument").value<QObject *>();
    if (document != nullptr && document->property("currentLineNoteCount").toInt() < 2) {
        std::cout << "sheet interaction: skipped (the current line has fewer than two notes)"
                  << std::endl;
        return true;
    }
    QList<QObject *> delegates;
    find_current_line_delegates(window, delegates);
    auto centre_of = [](QObject *note) -> QPointF {
        auto *const item = qobject_cast<QQuickItem *>(note);
        return item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
    };
    // Whole-song painting creates delegates for every line; only notes
    // whose centre lies inside the window are pressable.
    auto on_screen = [&](QObject *note) -> bool {
        const QPointF centre = centre_of(note);
        return centre.x() > 0.0 && centre.y() > 0.0 && centre.x() < window->width()
            && centre.y() < window->height();
    };
    QList<QObject *> pressable;
    for (QObject *note : delegates) {
        if (on_screen(note)) {
            pressable.append(note);
        }
    }
    if (pressable.size() < 2) {
        // A line wider than the pitch rows scrolls vertically (UI_SPEC
        // 3.3), so its notes can lie outside the visible rows.
        std::cout << "sheet interaction: skipped (fewer than two current-line notes on screen)"
                  << std::endl;
        return true;
    }

    // Songs can hold notes stacked on the same beats and pitch (Factory
    // output); a note the check presses must be the only live note under
    // its centre, or the press lands on the other one.
    auto uncovered = [&](QObject *note) -> bool {
        QList<QObject *> every_note;
        std::function<void(QQuickItem *)> collect = [&](QQuickItem *item) {
            for (QQuickItem *const child : item->childItems()) {
                if (live_delegate(child) && child->isVisible()
                    && child->property("noteId").isValid()) {
                    every_note.append(child);
                }
                collect(child);
            }
        };
        collect(window->contentItem());
        const QPointF centre = centre_of(note);
        for (QObject *const other : every_note) {
            auto *const item = qobject_cast<QQuickItem *>(other);
            if (other != note
                && item->mapRectToScene(QRectF(0, 0, item->width(), item->height()))
                       .contains(centre)) {
                return false;
            }
        }
        return true;
    };

    // Tap the first note: the TapHandler must select it (UI_SPEC 3.5). The
    // tap is verified on the same noteId, because the selection change
    // resets the notes model and recreates the delegates.
    auto note_id_of = [](QObject *note) -> qint64 {
        return note->property("noteId").toLongLong();
    };
    auto find_by_id = [&](qint64 id) -> QObject * {
        QList<QObject *> all;
        find_current_line_delegates(window, all);
        for (QObject *note : all) {
            if (note_id_of(note) == id) {
                return note;
            }
        }
        return nullptr;
    };
    QObject *tapped = nullptr;
    for (QObject *const note : pressable) {
        if (uncovered(note)) {
            tapped = note;
            break;
        }
    }
    if (tapped == nullptr) {
        std::cout << "sheet interaction: skipped (every current-line note is covered by another)"
                  << std::endl;
        return true;
    }
    const qint64 tapped_id = note_id_of(tapped);
    const QPointF tap_point = centre_of(tapped);
    send_mouse(window, tap_point, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, 10);
    send_mouse(window, tap_point, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton, 20);
    pump_events(50);
    if (QObject *const note = find_by_id(tapped_id)) {
        if (!note->property("selected").toBool()) {
            std::cerr << "sheet interaction: the tap did not select the note\n";
            return false;
        }
    } else {
        std::cerr << "sheet interaction: the tapped note did not survive the tap\n";
        return false;
    }
    // Ctrl+click adds the second note to the selection (plan 8.4); both
    // notes are selected afterwards.
    {
        QList<QObject *> current;
        find_current_line_delegates(window, current);
        QObject *second = nullptr;
        for (QObject *note : current) {
            if (note_id_of(note) != tapped_id && on_screen(note) && uncovered(note)) {
                second = note;
                break;
            }
        }
        // Without an uncovered second note the Ctrl+click step is skipped.
        const qint64 second_id = second != nullptr ? note_id_of(second) : -1;
        if (second != nullptr) {
        const QPointF point = centre_of(second);
        send_mouse(window, point, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, 3000,
                   Qt::ControlModifier);
        send_mouse(window, point, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton, 3010,
                   Qt::ControlModifier);
        pump_events(50);
        QObject *const first_after = find_by_id(tapped_id);
        QObject *const second_after = find_by_id(second_id);
        if (first_after == nullptr || second_after == nullptr ||
            !first_after->property("selected").toBool() ||
            !second_after->property("selected").toBool()) {
            std::cerr << "sheet interaction: Ctrl+click did not add the note to the selection\n";
            return false;
        }
        // A plain click on the first note makes it the only selected note
        // again, so the drag below finds an unselected note.
        const QPointF first_point = centre_of(first_after);
        send_mouse(window, first_point, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
                   4000);
        send_mouse(window, first_point, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
                   4010);
        pump_events(50);
        QObject *const second_cleared = find_by_id(second_id);
        if (second_cleared == nullptr || second_cleared->property("selected").toBool()) {
            std::cerr << "sheet interaction: a plain click did not replace the selection\n";
            return false;
        }
        }
    }

    // Drag an on-screen unselected note 60 px right: the bodyDrag must
    // activate and the release must select the note implicitly and move it.
    // The tap's selection change reset the notes model and destroyed every
    // old delegate pointer, so the candidates are collected fresh.
    delegates.clear();
    find_current_line_delegates(window, delegates);
    pressable.clear();
    for (QObject *note : delegates) {
        if (on_screen(note)) {
            pressable.append(note);
        }
    }
    if (pressable.size() < 2) {
        std::cerr << "sheet interaction: fewer than two pressable notes after the tap\n";
        return false;
    }
    QObject *dragged = nullptr;
    for (QObject *note : pressable) {
        if (note_id_of(note) != tapped_id && !note->property("selected").toBool()
            && uncovered(note)) {
            dragged = note;
            break;
        }
    }
    if (dragged == nullptr) {
        std::cout << "sheet interaction: skipped (no uncovered unselected note to drag)"
                  << std::endl;
        return true;
    }
    const qint64 dragged_id = note_id_of(dragged);
    const QPointF drag_start = centre_of(dragged);
    const QPointF drag_end = drag_start + QPointF(60, 0);
    const qreal start_before = dragged->property("start").toReal();
    send_mouse(window, drag_start, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, 30);
    for (int step = 1; step <= 8; step++) {
        send_mouse(window, drag_start + (drag_end - drag_start) * (step / 8.0),
                   QEvent::MouseMove, Qt::LeftButton, Qt::NoButton, 30 + step * 20);
    }
    const bool dragging = dragged->property("dragging").toBool();
    // The dragged note follows the pointer: its offset is the pointer's
    // travel since the press. A delta measured in the moving note's own
    // coordinates would swing back and forth between events (flicker).
    const double followed = dragged->property("dragOffsetX").toDouble();
    if (std::abs(followed - (drag_end.x() - drag_start.x())) > 1.0) {
        std::cerr << "sheet interaction: the dragged note does not follow the pointer (offset "
                  << followed << " px, pointer moved " << (drag_end.x() - drag_start.x())
                  << " px)\n";
        return false;
    }
    // Reports whether the rubber band Rectangle (z 10) is visible right
    // now. During a note drag it must stay hidden: the ancestor bandDrag
    // must not steal the bodyDrag's grab. On the empty pitch area it must
    // show (a fresh grab is not a takeover).
    auto band_showing = [&]() -> bool {
        bool visible = false;
        std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
            for (QQuickItem *const child : item->childItems()) {
                if (child->z() == 10
                    && QString::fromUtf8(child->metaObject()->className())
                           .contains(QStringLiteral("Rectangle"))) {
                    visible = visible || child->isVisible();
                }
                walk(child);
            }
        };
        walk(window->contentItem());
        return visible;
    };
    // Both state captures happen mid-drag, before the release commits.
    if (!dragging || band_showing()) {
        std::cerr << "sheet interaction: mid-drag state wrong: dragging="
                  << (dragging ? "true" : "false")
                  << " band showing=" << (band_showing() ? "true" : "false") << "\n";
        return false;
    }
    send_mouse(window, drag_end, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton, 220);
    pump_events(50);
    // The commit resets the notes model and recreates the delegates; the
    // moved note is verified on its noteId.
    if (QObject *const note = find_by_id(dragged_id)) {
        const qreal start_after = note->property("start").toReal();
        if (!(start_after > start_before)) {
            std::cerr << "sheet interaction: the drag on an unselected note did not move it"
                         " (implicit select)\n";
            return false;
        }
    } else {
        std::cerr << "sheet interaction: the dragged note did not survive the drag\n";
        return false;
    }
    // The timestamps of the later synthesized events keep growing, far
    // apart, so no press reads as a double click.
    ulong next_time = 10000;
    // Ctrl+Z undoes the drag and Ctrl+Shift+Z redoes it (plan 8.6).
    window->requestActivate();
    pump_events(20);
    auto start_of = [&](qint64 id) -> qreal {
        QObject *const note = find_by_id(id);
        return note != nullptr ? note->property("start").toReal() : -1.0;
    };
    const qreal moved_start = start_of(dragged_id);
    send_key(window, Qt::Key_Z, Qt::ControlModifier);
    if (start_of(dragged_id) != start_before) {
        std::cerr << "sheet interaction: Ctrl+Z did not undo the drag (start "
                  << start_of(dragged_id) << ", wanted " << start_before << ")\n";
        return false;
    }
    send_key(window, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
    if (start_of(dragged_id) != moved_start) {
        std::cerr << "sheet interaction: Ctrl+Shift+Z did not redo the drag (start "
                  << start_of(dragged_id) << ", wanted " << moved_start << ")\n";
        return false;
    }
    // Ctrl+S of the main menu saves the song (the self-test song is a
    // temporary copy).
    QObject *const songDocument = root->property("songDocument").value<QObject *>();
    if (!songDocument->property("dirty").toBool()) {
        std::cerr << "sheet interaction: the redone drag left the song clean\n";
        return false;
    }
    send_key(window, Qt::Key_S, Qt::ControlModifier);
    pump_events(100);
    if (songDocument->property("dirty").toBool()) {
        std::cerr << "sheet interaction: Ctrl+S did not save the song\n";
        return false;
    }
    // Esc clears the selection while nothing plays (plan 8.6).
    if (QObject *const note = find_by_id(dragged_id); note == nullptr ||
        !note->property("selected").toBool()) {
        std::cerr << "sheet interaction: the dragged note is not selected before Esc\n";
        return false;
    }
    send_key(window, Qt::Key_Escape, Qt::NoModifier);
    if (QObject *const note = find_by_id(dragged_id); note == nullptr ||
        note->property("selected").toBool()) {
        std::cerr << "sheet interaction: Esc did not clear the selection\n";
        return false;
    }
    // The edge drags of a selected note (UI_SPEC 3.5): a press within 5 px
    // of an edge resizes from that edge, the edge follows the pointer (it
    // moves with the note, so a delta in the note's own coordinates would
    // swing back and forth), and the release commits one resize.
    auto select_by_tap = [&](qint64 id) -> QPointer<QObject> {
        if (QObject *const note = find_by_id(id)) {
            const QPointF centre = centre_of(note);
            send_mouse(window, centre, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
                       next_time += 1000);
            send_mouse(window, centre, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
                       next_time += 10);
            pump_events(50);
        }
        return QPointer<QObject>(find_by_id(id));
    };
    // The widest on-screen note of the current line carries the edge
    // drags; the handles exist only from 12 px width (UI_SPEC 3.5), so a
    // song whose notes are all narrower at the fit zoom skips them.
    qint64 edge_id = -1;
    double edge_width = 0.0;
    {
        QList<QObject *> current;
        find_current_line_delegates(window, current);
        for (QObject *const candidate : current) {
            auto *const item = qobject_cast<QQuickItem *>(candidate);
            if (on_screen(candidate) && uncovered(candidate) && item->width() > edge_width) {
                edge_width = item->width();
                edge_id = note_id_of(candidate);
            }
        }
    }
    auto edge_drag = [&](bool startEdge, qint64 target_id) -> bool {
        const char *const edgeName = startEdge ? "start" : "end";
        // Real songs can push the note partly out of the window or under
        // another note (an earlier edge drag lengthened it); such a note
        // skips the drag instead of pressing beside it.
        auto in_window = [&](const QPointF &point) {
            return point.x() > 0.0 && point.y() > 0.0 && point.x() < window->width()
                && point.y() < window->height();
        };
        if (QObject *const before = find_by_id(target_id);
            before == nullptr || !uncovered(before) || !in_window(centre_of(before))) {
            return true;
        }
        const QPointer<QObject> note = select_by_tap(target_id);
        auto *const item = note ? qobject_cast<QQuickItem *>(note.data()) : nullptr;
        if (item != nullptr && note->property("selected").toBool() && item->width() >= 12) {
            const QPointF edge = item->mapToScene(
                QPointF(startEdge ? 2.0 : item->width() - 2.0, item->height() / 2));
            const QPointF target = edge + QPointF(startEdge ? -40.0 : 40.0, 0.0);
            if (!in_window(edge) || !in_window(target)) {
                return true;
            }
        }
        if (item == nullptr || !note->property("selected").toBool() || item->width() < 12) {
            std::cerr << "sheet interaction: no selected note wide enough for the " << edgeName
                      << " edge drag\n";
            return false;
        }
        const qint64 lengthBefore = note->property("length").toLongLong();
        const double startBefore = note->property("start").toDouble();
        const QPointF from = item->mapToScene(
            QPointF(startEdge ? 2.0 : item->width() - 2.0, item->height() / 2));
        const QPointF to = from + QPointF(startEdge ? -40.0 : 40.0, 0.0);
        send_mouse(window, from, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
                   next_time += 1000);
        for (int step = 1; step <= 8; step++) {
            send_mouse(window, from + (to - from) * (step / 8.0), QEvent::MouseMove,
                       Qt::LeftButton, Qt::NoButton, next_time += 20);
        }
        if (note.isNull()) {
            std::cerr << "sheet interaction: the note was destroyed during the " << edgeName
                      << " edge drag\n";
            return false;
        }
        const bool resizing = note->property("resizing").toBool();
        const bool moving = note->property("dragging").toBool();
        // The committed delta the preview reported; snapping can make it 0
        // where notes touch.
        const qint64 pending = note->property("pendingDBeat").toLongLong();
        const double followed =
            note->property(startEdge ? "startEdgeOffsetX" : "endEdgeOffsetX").toDouble();
        send_mouse(window, to, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
                   next_time += 20);
        pump_events(50);
        if (!resizing || moving || std::abs(followed - (to.x() - from.x())) > 1.0) {
            std::cerr << "sheet interaction: the " << edgeName
                      << " edge does not follow the pointer (resizing " << resizing
                      << ", moving " << moving << ", offset " << followed
                      << " px, pointer moved " << (to.x() - from.x()) << " px)\n";
            return false;
        }
        // The resize operations refuse a start before beat 0 and a length
        // below 1 (usdx-edit ops); a refused commit changes nothing.
        const qint64 newLength = lengthBefore + (startEdge ? -pending : pending);
        const bool refused = newLength < 1 || (startEdge && startBefore + pending < 0.0);
        QObject *const after = find_by_id(target_id);
        const qint64 wantedLength = refused ? lengthBefore : newLength;
        if (after == nullptr || after->property("length").toLongLong() != wantedLength) {
            std::cerr << "sheet interaction: the " << edgeName << " edge drag committed length "
                      << (after != nullptr ? after->property("length").toLongLong() : -1)
                      << ", the preview showed " << wantedLength << "\n";
            return false;
        }
        std::cout << "sheet interaction: " << edgeName << " edge resized" << std::endl;
        return true;
    };
    if (edge_width >= 12.0 && (!edge_drag(true, edge_id) || !edge_drag(false, edge_id))) {
        return false;
    }
    // The end edge of the line's last note, which usually ends at the
    // phrase end marker: the selected note's edge wins over the marker's
    // grab area there.
    {
        qint64 last_id = -1;
        double last_start = -1.0e9;
        double last_width = 0.0;
        QList<QObject *> current;
        find_current_line_delegates(window, current);
        for (QObject *const candidate : current) {
            const double start = candidate->property("start").toDouble();
            if (on_screen(candidate) && start > last_start) {
                last_start = start;
                last_id = note_id_of(candidate);
                last_width = qobject_cast<QQuickItem *>(candidate)->width();
            }
        }
        if (last_id >= 0 && last_width >= 12.0 && !edge_drag(false, last_id)) {
            return false;
        }
    }
    // The inspector's Start field moves the selected note and keeps its
    // length (user decision 2026-11-15).
    if (QObject *const startField = root->findChild<QObject *>(QStringLiteral("inspectorStart"))) {
        const QPointer<QObject> picked = select_by_tap(edge_id >= 0 ? edge_id : dragged_id);
        const qint64 picked_id = edge_id >= 0 ? edge_id : dragged_id;
        if (picked.isNull() || !picked->property("selected").toBool()) {
            std::cerr << "sheet interaction: no single selected note for the Start field\n";
            return false;
        }
        const double startBefore = picked->property("start").toDouble();
        const qint64 lengthBefore = picked->property("length").toLongLong();
        startField->setProperty("value", static_cast<int>(startBefore) + 1);
        QMetaObject::invokeMethod(startField, "valueModified");
        pump_events(50);
        QObject *const moved = find_by_id(picked_id);
        if (moved == nullptr || moved->property("start").toDouble() != startBefore + 1.0
            || moved->property("length").toLongLong() != lengthBefore) {
            std::cerr << "sheet interaction: the Start field did not move the note by one beat "
                         "(start "
                      << (moved ? moved->property("start").toDouble() : -1.0) << ", length "
                      << (moved ? moved->property("length").toLongLong() : -1) << ")\n";
            return false;
        }
    } else {
        std::cerr << "sheet interaction: the inspector Start field was not found\n";
        return false;
    }
    // The line type combo shows the current line's type and sets it for
    // every note of the line (user decision 2026-11-15).
    if (QObject *const combo = root->findChild<QObject *>(QStringLiteral("lineTypeCombo"))) {
        QObject *const doc = root->property("songDocument").value<QObject *>();
        const QString lineType = doc->property("currentLineType").toString();
        const QString shown = combo->property("currentValue").toString();
        if (lineType != QStringLiteral("mixed") && shown != lineType) {
            std::cerr << "sheet interaction: the line type combo shows " << shown.toStdString()
                      << " for a " << lineType.toStdString() << " line\n";
            return false;
        }
        const int golden = 1;
        combo->setProperty("currentIndex", golden);
        QMetaObject::invokeMethod(combo, "activated", Q_ARG(int, golden));
        pump_events(50);
        QList<QObject *> line_notes;
        find_current_line_delegates(window, line_notes);
        bool all_golden = !line_notes.isEmpty();
        for (QObject *const note : line_notes) {
            all_golden = all_golden && note->property("type").toString() == QStringLiteral("*");
        }
        if (!all_golden || doc->property("currentLineType").toString() != QStringLiteral("*")
            || combo->property("currentValue").toString() != QStringLiteral("*")) {
            std::cerr << "sheet interaction: choosing Golden in the line type combo did not "
                         "make the line golden (notes golden "
                      << all_golden << ", line type "
                      << doc->property("currentLineType").toString().toStdString()
                      << ", combo " << combo->property("currentValue").toString().toStdString()
                      << ", notes " << line_notes.size() << ")\n";
            return false;
        }
    } else {
        std::cerr << "sheet interaction: the line type combo was not found\n";
        return false;
    }

    // Rubber band on the empty pitch area: a fresh grab is not a takeover,
    // so the band must still grow (TakeOverForbidden only blocks steals).
    // The press point is scanned on a grid until it sits on no visible
    // note delegate of any line, because whole-song painting covers more
    // of the pitch area than the current line alone.
    QList<QObject *> all_notes;
    std::function<void(QQuickItem *)> collect_notes = [&](QQuickItem *item) {
        for (QQuickItem *const child : item->childItems()) {
            const QVariant note_id = child->property("noteId");
            if (live_delegate(child) && note_id.isValid() && note_id.toLongLong() >= 0
                && child->isVisible()) {
                all_notes.append(child);
            }
            collect_notes(child);
        }
    };
    collect_notes(window->contentItem());
    auto covers = [&](const QPointF &point) -> bool {
        for (QObject *note : all_notes) {
            auto *const item = qobject_cast<QQuickItem *>(note);
            const QRectF rect(item->mapToScene(QPointF(0, 0)),
                              QSizeF(item->width(), item->height()));
            if (rect.adjusted(-4, -4, 4, 4).contains(point)) {
                return true;
            }
        }
        return false;
    };
    QPointF band_press(0, 0);
    bool found_empty = false;
    // The scan runs inside the pitch area of the sheet, computed from the
    // SheetBackground's own geometry properties (the ruler, lyrics, gap,
    // waveform and activity lanes sit outside it).
    auto *const sheet_item = qobject_cast<QQuickItem *>(bg);
    if (sheet_item != nullptr) {
        const qreal gutter = bg->property("gutterWidth").toReal();
        const qreal ruler = bg->property("rulerHeight").toReal();
        qreal below = ruler;
        if (bg->property("showLyricsLane").toBool()) {
            below += bg->property("lyricsHeight").toReal()
                + bg->property("gapHeight").toReal();
        }
        below += bg->property("waveformHeight").toReal()
            + bg->property("activityHeight").toReal();
        const qreal pitch_bottom = sheet_item->height() - below;
        const qreal pitch_top = ruler;
        for (qreal y = pitch_top + 10; y <= pitch_bottom - 10 && !found_empty; y += 15) {
            for (qreal x = gutter + 10; x < sheet_item->width() - 10; x += 16) {
                const QPointF scene_point = sheet_item->mapToScene(QPointF(x, y));
                if (!covers(scene_point)) {
                    band_press = scene_point;
                    found_empty = true;
                    break;
                }
            }
        }
    }
    if (!found_empty) {
        std::cerr << "sheet interaction: no empty pitch point for the rubber band\n";
        return false;
    }
    send_mouse(window, band_press, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
               240);
    for (int step = 1; step <= 8; step++) {
        send_mouse(window, band_press + QPointF(step * 8.0, step * 2.0), QEvent::MouseMove,
                   Qt::LeftButton, Qt::NoButton, 240 + step * 20);
    }
    const bool band_grew = band_showing();
    send_mouse(window, band_press + QPointF(64, 16), QEvent::MouseButtonRelease, Qt::NoButton,
               Qt::LeftButton, 420);
    pump_events(50);
    if (!band_grew) {
        std::cerr << "sheet interaction: the rubber band did not grow on empty area\n";
        return false;
    }

    // Wheel gestures (UI_SPEC 3.2): Ctrl+wheel zooms x1.25 per step around
    // the cursor, a plain wheel step scrolls time, Shift+wheel scrolls the
    // pitch range one row.
    const qreal pxb = bg->property("pxPerBeat").toReal();
    const qreal startb = bg->property("startBeat").toReal();
    const qreal ptop = bg->property("pitchTop").toReal();
    for (int step = 0; step < 3; step++) {
        send_wheel(window, QPointF(700, 300), QPoint(0, 120), Qt::ControlModifier);
    }
    send_wheel(window, QPointF(700, 300), QPoint(0, -120), Qt::NoModifier);
    send_wheel(window, QPointF(700, 300), QPoint(0, -120), Qt::NoModifier);
    send_wheel(window, QPointF(700, 300), QPoint(0, 120), Qt::ShiftModifier);
    const qreal pxb_after = bg->property("pxPerBeat").toReal();
    const qreal startb_after = bg->property("startBeat").toReal();
    const qreal ptop_after = bg->property("pitchTop").toReal();
    // Three steps zoom x1.953125, capped at the viewport's maximum of 40 px
    // per beat (MAX_PX_PER_BEAT in controller/viewport.rs).
    const qreal pxb_wanted = std::min(pxb * 1.953125, 40.0);
    if (qAbs(pxb_after - pxb_wanted) > pxb * 0.01) {
        std::cerr << "sheet interaction: Ctrl+wheel did not zoom x1.25 per step ("
                  << pxb << " -> " << pxb_after << ")\n";
        return false;
    }
    if (startb_after == startb) {
        std::cerr << "sheet interaction: the plain wheel did not scroll time\n";
        return false;
    }
    if (ptop_after != ptop + 1) {
        std::cerr << "sheet interaction: Shift+wheel did not scroll one pitch row ("
                  << ptop << " -> " << ptop_after << ")\n";
        return false;
    }

    // The horizontal scrollbar exists and tracks the view (recorded spec
    // deviation, user decision 2026-11-15).
    QObject *const bar = root->findChild<QObject *>(QStringLiteral("sheetScrollBar"));
    if (bar == nullptr) {
        std::cerr << "sheet interaction: no sheetScrollBar object\n";
        return false;
    }
    if (bar->property("size").toReal() <= 0.0 || bar->property("size").toReal() > 1.0) {
        std::cerr << "sheet interaction: the scrollbar handle size is out of range\n";
        return false;
    }
    // A phrase end marker with room to move follows the pointer: the
    // preview beat is the beat under the pointer's travel from the press,
    // rounded and clamped, and the release commits it. Songs without a
    // movable marker skip this step.
    // The marker travel in whole beats: at least two beats and at least
    // 30 px, so the drag passes the drag threshold at any zoom.
    auto marker_beats = [&](QQuickItem *) -> double {
        const double pxPerBeat = std::max(0.001, bg->property("pxPerBeat").toDouble());
        return std::max(2.0, std::ceil(30.0 / pxPerBeat));
    };
    // The marker must lie in the visible sheet: right of the gutter (the
    // sheet left of it sits clipped under the lines pane) and left of the
    // sheet's right edge.
    auto *const bgItem = qobject_cast<QQuickItem *>(bg);
    const double sheetLeft =
        bgItem->mapToScene(QPointF(bg->property("gutterWidth").toDouble() + 2.0, 0)).x();
    const double sheetRight = bgItem->mapToScene(QPointF(bgItem->width() - 2.0, 0)).x();
    auto find_marker = [&]() -> QQuickItem * {
        QQuickItem *found = nullptr;
        std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
            for (QQuickItem *const child : item->childItems()) {
                // Finite bounds at least two beats apart, and the marker
                // plus two beats of travel inside the window.
                const double min = child->property("min").toDouble();
                const double max = child->property("max").toDouble();
                const double sceneX = child->mapToScene(QPointF(child->width() / 2, 0)).x();
                const double travel =
                    marker_beats(child) * bg->property("pxPerBeat").toDouble();
                const double beat = child->property("beat").toDouble();
                if (found == nullptr && child->property("previewBeat").isValid()
                    && child->isVisible() && std::isfinite(min) && std::isfinite(max)
                    && beat >= min && beat + marker_beats(child) <= max
                    && sceneX > sheetLeft && sceneX + travel < sheetRight) {
                    found = child;
                }
                walk(child);
            }
        };
        walk(window->contentItem());
        return found;
    };
    if (QQuickItem *const marker = find_marker()) {
        const qint64 markerId = marker->property("rowId").toLongLong();
        const double beatBefore = marker->property("beat").toDouble();
        const double maxBeat = marker->property("max").toDouble();
        const double pxPerBeat = bg->property("pxPerBeat").toDouble();
        const double wantedBeat = std::min(maxBeat, beatBefore + marker_beats(marker));
        const QPointF from = marker->mapToScene(QPointF(marker->width() / 2, 40.0));
        const QPointF to = from + QPointF((wantedBeat - beatBefore) * pxPerBeat, 0.0);
        send_mouse(window, from, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
                   next_time += 1000);
        for (int step = 1; step <= 8; step++) {
            send_mouse(window, from + (to - from) * (step / 8.0), QEvent::MouseMove,
                       Qt::LeftButton, Qt::NoButton, next_time += 20);
        }
        const double preview = marker->property("previewBeat").toDouble();
        send_mouse(window, to, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
                   next_time += 20);
        pump_events(50);
        if (preview != wantedBeat) {
            std::cerr << "sheet interaction: the phrase end preview is beat " << preview
                      << ", the pointer is at beat " << wantedBeat << "\n";
            return false;
        }
        double committed = -1.0;
        std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
            for (QQuickItem *const child : item->childItems()) {
                if (child->property("previewBeat").isValid()
                    && child->property("rowId").toLongLong() == markerId) {
                    committed = child->property("beat").toDouble();
                }
                walk(child);
            }
        };
        walk(window->contentItem());
        if (committed != wantedBeat) {
            std::cerr << "sheet interaction: the phrase end committed beat " << committed
                      << ", wanted " << wantedBeat << "\n";
            return false;
        }
    }
    std::cout << "sheet interaction: ok" << std::endl;
    return true;
}

bool usdx_playhead_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    auto *const window = qobject_cast<QQuickWindow *>(root);
    QObject *const bg = root->findChild<QObject *>(QStringLiteral("sheetBackground"));
    QObject *const player = root->findChild<QObject *>(QStringLiteral("player"));
    QObject *const document = root->property("songDocument").value<QObject *>();
    auto *const playhead = root->findChild<QQuickItem *>(QStringLiteral("playhead"));
    auto *const strip = root->findChild<QQuickItem *>(QStringLiteral("waveformSeekStrip"));
    if (window == nullptr || bg == nullptr || player == nullptr || document == nullptr ||
        playhead == nullptr || strip == nullptr) {
        std::cerr << "playhead: a sheet object is missing (window, sheetBackground, player, "
                     "songDocument, playhead or waveformSeekStrip)\n";
        return false;
    }
    const double bpm = document->property("bpm").toDouble();
    const double gapMs = document->property("gapMs").toDouble();
    if (bpm <= 0.0) {
        std::cout << "playhead: skipped (the song has no BPM)" << std::endl;
        return true;
    }
    // The ms a sheet x maps to, through the SheetBackground's own axis.
    auto ms_at = [&](double sheetX) -> double {
        double beat = 0.0;
        QMetaObject::invokeMethod(bg, "xToBeat", Q_RETURN_ARG(double, beat),
                                  Q_ARG(double, sheetX));
        return std::max(0.0, gapMs + beat * 15000.0 / bpm);
    };
    auto *const bgItem = qobject_cast<QQuickItem *>(bg);
    const double gutter = bg->property("gutterWidth").toDouble();
    // A press in the waveform lane seeks there, a drag scrubs.
    // Passive notifications of the earlier checks (a save, a refused edit)
    // sit at the bottom of the window over the waveform lane for 2 to 4 s;
    // they are closed first, as a user would wait for them.
    for (int shown = 0; shown < 5; shown++) {
        QVariant ignored;
        QMetaObject::invokeMethod(root, "hidePassiveNotification",
                                  Q_RETURN_ARG(QVariant, ignored), Q_ARG(QVariant, QVariant(0)));
    }
    pump_events(500);
    const QPointF pressLocal(gutter + (strip->width() - gutter) * 0.4, strip->height() / 2);
    const QPointF press = strip->mapToScene(pressLocal);
    send_mouse(window, press, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, 1000);
    const double wanted = ms_at(bgItem->mapFromScene(press).x());
    if (std::abs(player->property("positionMs").toDouble() - wanted) > 1.0) {
        std::cerr << "playhead: a press in the waveform lane did not seek to " << wanted
                  << " ms, the position is " << player->property("positionMs").toDouble()
                  << " ms\n";
        return false;
    }
    const QPointF dragged = press + QPointF(60.0, 0.0);
    send_mouse(window, press + QPointF(20.0, 0.0), QEvent::MouseMove, Qt::LeftButton,
               Qt::NoButton, 1010);
    send_mouse(window, dragged, QEvent::MouseMove, Qt::LeftButton, Qt::NoButton, 1020);
    const double wantedDrag = ms_at(bgItem->mapFromScene(dragged).x());
    if (std::abs(player->property("positionMs").toDouble() - wantedDrag) > 1.0) {
        std::cerr << "playhead: dragging in the waveform lane did not scrub to " << wantedDrag
                  << " ms, the position is " << player->property("positionMs").toDouble()
                  << " ms\n";
        return false;
    }
    send_mouse(window, dragged, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton, 1030);
    pump_events(50);
    // The playhead line stands where the pointer let go.
    const double playheadX = playhead->mapToScene(QPointF(playhead->width() / 2, 0)).x();
    if (!playhead->isVisible() || std::abs(playheadX - dragged.x()) > 2.0) {
        std::cerr << "playhead: the line is not under the pointer (visible "
                  << playhead->isVisible() << ", x " << playheadX << ", pointer "
                  << dragged.x() << ")\n";
        return false;
    }
    // Following the playback makes the line being sung the current line.
    if (document->property("lineCount").toInt() >= 2) {
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 1));
        const double secondStart = document->property("currentLineStartMs").toDouble();
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
        QMetaObject::invokeMethod(document, "followPlayback", Q_ARG(double, secondStart + 1.0));
        pump_events(50);
        if (document->property("currentLine").toInt() != 1) {
            std::cerr << "playhead: following the playback did not reach the second line\n";
            return false;
        }
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
        pump_events(50);
    }
    // The lines pane keeps its scroll position when the selection changes
    // (a long song scrolled to a late line must not jump back to line 1).
    if (QObject *const list = root->findChild<QObject *>(QStringLiteral("linesList"));
        list != nullptr && document->property("lineCount").toInt() >= 40) {
        QMetaObject::invokeMethod(document, "goToLine",
                                  Q_ARG(int, document->property("lineCount").toInt() - 1));
        pump_events(100);
        const double scrolled = list->property("contentY").toDouble();
        QMetaObject::invokeMethod(document, "clearSelection");
        pump_events(100);
        const double after = list->property("contentY").toDouble();
        if (scrolled <= 0.0 || std::abs(after - scrolled) > 1.0) {
            std::cerr << "playhead: the lines pane scrolled from " << scrolled << " to " << after
                      << " on a selection change\n";
            return false;
        }
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
        pump_events(50);
    }
    std::cout << "playhead: ok" << std::endl;
    return true;
}

// Checks the text editing of task P6-02 through real key and mouse events:
// Ctrl+E opens the Edit line dialog with the current line's text, typing
// previews, Enter applies one undo step that Ctrl+Z takes back, an added
// word splits a note (when the line's last note is long enough), a double
// click on a note opens the inline syllable field over its lyric label,
// Enter commits it with the word space kept, Tab moves it to the next
// note, and a double click on the lyric label opens the field too, where
// Esc cancels. Prints "text
// editing: ok", or "text editing: skipped" for a read-only song or an empty
// first line.
bool usdx_text_edit_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    auto *const window = qobject_cast<QQuickWindow *>(root);
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const dialog = root->findChild<QObject *>(QStringLiteral("lineTextDialog"));
    auto *const lineField = root->findChild<QQuickItem *>(QStringLiteral("lineTextField"));
    auto *const inlineField =
        root->findChild<QQuickItem *>(QStringLiteral("inlineSyllableField"));
    auto *const lyrics = root->findChild<QQuickItem *>(QStringLiteral("lyricsLaneArea"));
    if (window == nullptr || document == nullptr || dialog == nullptr || lineField == nullptr
        || inlineField == nullptr || lyrics == nullptr) {
        std::cerr << "text editing: an object is missing (window, songDocument, "
                     "lineTextDialog, lineTextField, inlineSyllableField or lyricsLaneArea)\n";
        return false;
    }
    if (document->property("readOnly").toBool()) {
        std::cout << "text editing: skipped (the song is read-only)" << std::endl;
        return true;
    }
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
    QMetaObject::invokeMethod(document, "clearSelection");
    pump_events(100);
    // The earlier checks leave the view wherever their gestures ended; the
    // first line's start is scrolled into view.
    if (QObject *const sheetObject = root->findChild<QObject *>(QStringLiteral("sheetBackground"));
        sheetObject != nullptr && document->property("bpm").toDouble() > 0.0) {
        const double lineBeat = (document->property("currentLineStartMs").toDouble()
                                 - document->property("gapMs").toDouble())
            * document->property("bpm").toDouble() / 15000.0;
        QMetaObject::invokeMethod(sheetObject, "scrollToBeat", Q_ARG(double, lineBeat - 1.0));
        pump_events(100);
    }
    const int notes = document->property("currentLineNoteCount").toInt();
    const QString original = document_text(document, "lineText", 0);
    if (notes < 1 || original.isEmpty()) {
        std::cout << "text editing: skipped (the first line has no text)" << std::endl;
        return true;
    }
    // No text input may hold the focus, or the shortcuts stand back.
    window->contentItem()->forceActiveFocus();
    pump_events(50);

    // Ctrl+E opens the dialog with the line's text.
    auto open_dialog = [&]() -> bool {
        send_key(window, Qt::Key_E, Qt::ControlModifier);
        pump_events(400);
        if (!dialog->property("visible").toBool()) {
            std::cerr << "text editing: Ctrl+E did not open the Edit line dialog\n";
            return false;
        }
        if (lineField->property("text").toString() != original) {
            std::cerr << "text editing: the dialog field shows \""
                      << lineField->property("text").toString().toStdString()
                      << "\" instead of \"" << original.toStdString() << "\"\n";
            return false;
        }
        return true;
    };
    if (!open_dialog()) {
        return false;
    }
    // Typing previews; Enter applies one undo step.
    send_key(window, Qt::Key_End, Qt::NoModifier);
    send_text(window, QStringLiteral("zz"));
    const QVariantMap preview = dialog->property("preview").toMap();
    if (!preview.value(QStringLiteral("canApply")).toBool()
        || !preview.value(QStringLiteral("changed")).toBool()) {
        std::cerr << "text editing: the preview of a changed syllable cannot apply\n";
        return false;
    }
    send_key(window, Qt::Key_Return, Qt::NoModifier);
    pump_events(400);
    if (dialog->property("visible").toBool()) {
        std::cerr << "text editing: Enter did not apply and close the dialog\n";
        return false;
    }
    if (document_text(document, "lineText", 0) != original + QStringLiteral("zz")
        || document->property("undoText").toString() != QStringLiteral("Edit line text")) {
        std::cerr << "text editing: the applied line reads \""
                  << document_text(document, "lineText", 0).toStdString() << "\" (undo \""
                  << document->property("undoText").toString().toStdString() << "\")\n";
        return false;
    }
    send_key(window, Qt::Key_Z, Qt::ControlModifier);
    if (document_text(document, "lineText", 0) != original) {
        std::cerr << "text editing: Ctrl+Z did not take the line text back\n";
        return false;
    }
    // An added word splits a note when the plan allows it.
    {
        QString json;
        QMetaObject::invokeMethod(document, "previewLineText", Q_RETURN_ARG(QString, json),
                                  Q_ARG(int, 0), Q_ARG(QString, original + QStringLiteral(" zz")),
                                  Q_ARG(QString, QString()));
        const QJsonObject wordPreview = QJsonDocument::fromJson(json.toUtf8()).object();
        if (wordPreview.value(QStringLiteral("canApply")).toBool()) {
            window->contentItem()->forceActiveFocus();
            if (!open_dialog()) {
                return false;
            }
            send_key(window, Qt::Key_End, Qt::NoModifier);
            send_text(window, QStringLiteral(" zz"));
            send_key(window, Qt::Key_Return, Qt::NoModifier);
            pump_events(400);
            if (document->property("currentLineNoteCount").toInt() != notes + 1) {
                std::cerr << "text editing: an added word did not split a note ("
                          << document->property("currentLineNoteCount").toInt() << " notes, "
                          << notes + 1 << " wanted)\n";
                return false;
            }
            send_key(window, Qt::Key_Z, Qt::ControlModifier);
            if (document->property("currentLineNoteCount").toInt() != notes) {
                std::cerr << "text editing: Ctrl+Z did not merge the split back\n";
                return false;
            }
        } else {
            std::cout << "text editing: the added-word split skipped (the last note is too "
                         "short)"
                      << std::endl;
        }
    }

    // A double click on a note opens the inline field with its syllable.
    window->contentItem()->forceActiveFocus();
    pump_events(50);
    QList<QObject *> delegates;
    find_current_line_delegates(window, delegates);
    // The note's centre must lie in the visible sheet: right of the gutter
    // (a note left of the view sits clipped under the lines pane) and inside
    // the pitch rows.
    auto *const sheet = root->findChild<QQuickItem *>(QStringLiteral("sheetBackground"));
    if (sheet == nullptr) {
        std::cerr << "text editing: no sheetBackground item\n";
        return false;
    }
    const double gutter = sheet->property("gutterWidth").toDouble();
    const double pitchTop = sheet->property("rulerHeight").toDouble();
    const double pitchBottom =
        sheet->height() - sheet->property("lyricsHeight").toDouble()
        - sheet->property("gapHeight").toDouble() - sheet->property("waveformHeight").toDouble()
        - sheet->property("activityHeight").toDouble();
    QQuickItem *target = nullptr;
    for (QObject *const candidate : delegates) {
        auto *const item = qobject_cast<QQuickItem *>(candidate);
        const QPointF centre = sheet->mapFromItem(
            item, QPointF(item->width() / 2, item->height() / 2));
        if (centre.x() > gutter + 2.0 && centre.x() < sheet->width() - 2.0
            && centre.y() > pitchTop && centre.y() < pitchBottom && item->width() >= 4.0) {
            target = item;
            break;
        }
    }
    if (target == nullptr) {
        std::cout << "text editing: skipped the inline field (no note of the first line in view)"
                  << std::endl;
        return true;
    }
    const std::int64_t noteId = target->property("noteId").toLongLong();
    const QString syllable = document_text(document, "syllableForEditing", noteId);
    // The commits and undos below reset the notes model, which destroys
    // and recreates the delegate, and the view may scroll between the
    // clicks: the note's item is re-collected by id and its geometry
    // re-read for every click after a commit (a captured QQuickItem of an
    // earlier model generation is a zombie with stale geometry).
    auto note_item = [&]() -> QQuickItem * {
        QList<QObject *> current;
        find_current_line_delegates(window, current);
        for (QObject *const candidate : current) {
            if (candidate->property("noteId").toLongLong() == noteId) {
                return qobject_cast<QQuickItem *>(candidate);
            }
        }
        return nullptr;
    };
    const QPointF noteCentre =
        target->mapToScene(QPointF(target->width() / 2, target->height() / 2));
    send_double_click(window, noteCentre, 50000);
    if (!inlineField->isVisible() || inlineField->property("noteId").toLongLong() != noteId
        || inlineField->property("text").toString() != syllable) {
        std::cerr << "text editing: a double click on a note did not open the inline field "
                     "with its syllable (note "
                  << noteId << " at " << noteCentre.x() << "," << noteCentre.y()
                  << ", selected "
                  << document->property("inspector").toMap().value(QStringLiteral("noteId"))
                         .toLongLong()
                  << ", field visible " << inlineField->isVisible() << " for note "
                  << inlineField->property("noteId").toLongLong() << ", text \""
                  << inlineField->property("text").toString().toStdString() << "\")\n";
        return false;
    }
    send_key(window, Qt::Key_End, Qt::NoModifier);
    send_text(window, QStringLiteral("q"));
    send_key(window, Qt::Key_Return, Qt::NoModifier);
    pump_events(100);
    if (inlineField->isVisible()
        || document_text(document, "syllableForEditing", noteId) != syllable + QStringLiteral("q")
        || document->property("undoText").toString() != QStringLiteral("Edit syllable")) {
        std::cerr << "text editing: Enter did not commit the inline syllable (now \""
                  << document_text(document, "syllableForEditing", noteId).toStdString()
                  << "\")\n";
        return false;
    }
    send_key(window, Qt::Key_Z, Qt::ControlModifier);
    if (document_text(document, "syllableForEditing", noteId) != syllable) {
        std::cerr << "text editing: Ctrl+Z did not take the inline syllable back\n";
        return false;
    }
    // Tab commits and moves the field to the next note of the line (the
    // last note keeps the field closed). The commit and the undo above
    // reset the notes model, so the note's item and the click point are
    // re-read.
    QQuickItem *const tabItem = note_item();
    if (tabItem == nullptr) {
        std::cout << "text editing: skipped the Tab step (the note left the view)" << std::endl;
        return true;
    }
    const QPointF tabCentre = tabItem->mapToScene(
        QPointF(tabItem->width() / 2, tabItem->height() / 2));
    send_double_click(window, tabCentre, 55000);
    send_key(window, Qt::Key_Tab, Qt::NoModifier);
    pump_events(100);
    if (inlineField->isVisible() && inlineField->property("noteId").toLongLong() == noteId) {
        std::cerr << "text editing: Tab did not move the inline field to the next note\n";
        return false;
    }
    if (inlineField->isVisible()) {
        send_key(window, Qt::Key_Escape, Qt::NoModifier);
    }
    if (document_text(document, "syllableForEditing", noteId) != syllable) {
        std::cerr << "text editing: Tab changed an untouched syllable\n";
        return false;
    }
    // A double click on the lyric label opens the field; Esc cancels.
    // The label's x, kept inside the visible sheet: a long note can start
    // left of it, under the gutter. The Tab commit reset the notes model,
    // so the note's item and the click point are re-read.
    QQuickItem *const labelItem = note_item();
    if (labelItem == nullptr) {
        std::cout << "text editing: skipped the label step (the note left the view)" << std::endl;
        return true;
    }
    const double laneLeft = lyrics->mapToScene(QPointF(2.0, 0)).x();
    const QPointF label(std::max(labelItem->mapToScene(QPointF(1.0, 0.0)).x(), laneLeft),
                        lyrics->mapToScene(QPointF(0, lyrics->height() / 2)).y());
    send_double_click(window, label, 60000);
    if (!inlineField->isVisible()) {
        auto *const sheetItem = root->findChild<QQuickItem *>(QStringLiteral("sheetBackground"));
        double beat = 0.0;
        QMetaObject::invokeMethod(sheetItem, "xToBeat", Q_RETURN_ARG(double, beat),
                                  Q_ARG(double, sheetItem->mapFromScene(label).x()));
        std::int64_t hit = -1;
        QMetaObject::invokeMethod(document, "noteAtLyricBeat", Q_RETURN_ARG(std::int64_t, hit),
                                  Q_ARG(double, beat));
        std::cerr << "text editing: a double click on a lyric label did not open the inline "
                     "field (at "
                  << label.x() << "," << label.y() << ", beat " << beat << ", note " << hit
                  << ", selected "
                  << document->property("inspector").toMap().value(QStringLiteral("count")).toInt()
                  << ")\n";
        return false;
    }
    const std::int64_t labelNote = inlineField->property("noteId").toLongLong();
    const QString labelSyllable = document_text(document, "syllableForEditing", labelNote);
    send_text(window, QStringLiteral("w"));
    send_key(window, Qt::Key_Escape, Qt::NoModifier);
    pump_events(100);
    if (inlineField->isVisible()
        || document_text(document, "syllableForEditing", labelNote) != labelSyllable) {
        std::cerr << "text editing: Esc did not cancel the inline field\n";
        return false;
    }
    std::cout << "text editing: ok" << std::endl;
    return true;
}

// Checks the song end features (user decisions 2026-11-15) through real
// input: the time scroll stops at 0:00 and 2 s after the song end, End
// moves the playhead to the song end and Home back to 0:00, the toolbar's
// End song here button writes #END at the playhead, removes the notes after
// it and shades the area behind it (Ctrl+Z restores both), and a note drag
// shows the dashed guides at the landing position's lower corners down to
// the sheet's bottom. Prints "song end: ok", or "song end: skipped" for a
// read-only song or one without a tempo.
bool usdx_song_end_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    auto *const window = qobject_cast<QQuickWindow *>(root);
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const player = root->findChild<QObject *>(QStringLiteral("player"));
    auto *const bg = root->findChild<QQuickItem *>(QStringLiteral("sheetBackground"));
    auto *const endButton = root->findChild<QQuickItem *>(QStringLiteral("endSongButton"));
    auto *const endArea = root->findChild<QQuickItem *>(QStringLiteral("songEndArea"));
    if (window == nullptr || document == nullptr || player == nullptr || bg == nullptr
        || endButton == nullptr || endArea == nullptr) {
        std::cerr << "song end: an object is missing (window, songDocument, player, "
                     "sheetBackground, endSongButton or songEndArea)\n";
        return false;
    }
    const double bpm = document->property("bpm").toDouble();
    if (bpm <= 0.0 || document->property("readOnly").toBool()) {
        std::cout << "song end: skipped (read-only or no tempo)" << std::endl;
        return true;
    }
    const double gapMs = document->property("gapMs").toDouble();
    const double msPerBeat = 15000.0 / bpm;
    auto view_end = [&]() {
        return bg->property("startBeat").toDouble()
            + (bg->width() - bg->property("gutterWidth").toDouble())
                  / bg->property("pxPerBeat").toDouble();
    };
    window->contentItem()->forceActiveFocus();
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
    pump_events(100);
    // The scroll stops at both bounds: 0:00 and 2 s after the song end.
    const double minBeat = bg->property("minBeat").toDouble();
    const double maxBeat = bg->property("maxBeat").toDouble();
    if (std::abs(minBeat - (0.0 - gapMs) / msPerBeat) > 1e-6) {
        std::cerr << "song end: the scroll starts at beat " << minBeat << ", not at 0:00\n";
        return false;
    }
    const QPointF centre = bg->mapToScene(QPointF(bg->width() * 0.6, bg->height() * 0.4));
    // Wheels in one direction until the view stops moving (a long song
    // needs many steps), at most 2000 steps.
    auto wheel_until_still = [&](int notches) {
        for (int step = 0; step < 2000; step++) {
            const double before = bg->property("startBeat").toDouble();
            send_wheel(window, centre, QPoint(0, 120 * notches), Qt::NoModifier);
            if (bg->property("startBeat").toDouble() == before) {
                break;
            }
        }
    };
    wheel_until_still(-50);
    // A view wider than the bounds starts at 0:00 and may end past them.
    const bool wider = view_end() - bg->property("startBeat").toDouble() >= maxBeat - minBeat;
    if (wider ? std::abs(bg->property("startBeat").toDouble() - minBeat) > 1e-6
              : view_end() > maxBeat + 1e-6) {
        std::cerr << "song end: the view scrolled to beat " << view_end() << " past the bound "
                  << maxBeat << "\n";
        return false;
    }
    wheel_until_still(50);
    if (std::abs(bg->property("startBeat").toDouble() - minBeat) > 1e-6) {
        std::cerr << "song end: the view stops at beat " << bg->property("startBeat").toDouble()
                  << ", not at 0:00 (beat " << minBeat << ")\n";
        return false;
    }
    // End moves the playhead to the song end, Home back to 0:00.
    double endTarget = 0.0;
    QMetaObject::invokeMethod(document, "songEndTargetMs", Q_RETURN_ARG(double, endTarget),
                              Q_ARG(double, player->property("durationMs").toDouble()));
    send_key(window, Qt::Key_End, Qt::NoModifier);
    if (std::abs(player->property("positionMs").toDouble() - endTarget) > 1.0
        || view_end() < (endTarget - gapMs) / msPerBeat) {
        std::cerr << "song end: End moved the playhead to "
                  << player->property("positionMs").toDouble() << " ms (wanted " << endTarget
                  << ") and the view to beat " << view_end() << "\n";
        return false;
    }
    send_key(window, Qt::Key_Home, Qt::NoModifier);
    if (player->property("positionMs").toDouble() != 0.0
        || std::abs(bg->property("startBeat").toDouble() - minBeat) > 1e-6) {
        std::cerr << "song end: Home did not return to 0:00\n";
        return false;
    }
    // End song here 1 ms after the last line's first note starts: the
    // line keeps that note and loses the ones after it.
    const int lines = document->property("lineCount").toInt();
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, lines - 1));
    pump_events(50);
    const double lineStart = document->property("currentLineStartMs").toDouble();
    const int notesBefore = document->property("currentLineNoteCount").toInt();
    const double endBefore = document->property("endMs").toDouble();
    const double cutMs = std::ceil(lineStart) + 1.0;
    QMetaObject::invokeMethod(player, "seekMs", Q_ARG(double, cutMs));
    pump_events(50);
    // A direct seek does not scroll the view: with a large #GAP the cut
    // lies far right of 0:00, where the riding End song here button is
    // clipped out of the window. The cut's beat is scrolled to the gutter.
    QMetaObject::invokeMethod(bg, "scrollToBeat", Q_ARG(double, (cutMs - gapMs) / msPerBeat - 1.0));
    pump_events(50);
    if (notesBefore >= 2) {
        const QPointF button = endButton->mapToScene(
            QPointF(endButton->width() / 2, endButton->height() / 2));
        send_mouse(window, button, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton,
                   70000);
        send_mouse(window, button, QEvent::MouseButtonRelease, Qt::NoButton, Qt::LeftButton,
                   70020);
        pump_events(100);
        if (document->property("endMs").toDouble() != cutMs
            || document->property("undoText").toString() != QStringLiteral("End song here")) {
            std::cerr << "song end: End song here wrote #END "
                      << document->property("endMs").toDouble() << " (wanted " << cutMs
                      << ")\n";
            return false;
        }
        if (document->property("currentLineNoteCount").toInt() >= notesBefore) {
            std::cerr << "song end: End song here removed no note after the playhead\n";
            return false;
        }
        if (!endArea->isVisible()) {
            std::cerr << "song end: the area after #END is not shaded\n";
            return false;
        }
        send_key(window, Qt::Key_Z, Qt::ControlModifier);
        if (document->property("endMs").toDouble() != endBefore) {
            std::cerr << "song end: Ctrl+Z did not restore #END (now "
                      << document->property("endMs").toDouble() << ", before " << endBefore
                      << ")\n";
            return false;
        }
    } else {
        std::cout << "song end: the cut skipped (the last line has one note)" << std::endl;
    }
    // A note drag shows the guides at the landing position's lower corners.
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, 0));
    QMetaObject::invokeMethod(document, "clearSelection");
    // The cut step above scrolled the view to the cut; the first line's
    // start (re-read after goToLine) is scrolled back into view for the
    // note drag.
    if (bpm > 0.0) {
        const double firstStart = document->property("currentLineStartMs").toDouble();
        QMetaObject::invokeMethod(bg, "scrollToBeat",
                                  Q_ARG(double, (firstStart - gapMs) / msPerBeat - 1.0));
    }
    pump_events(100);
    QList<QObject *> delegates;
    find_current_line_delegates(window, delegates);
    const double gutter = bg->property("gutterWidth").toDouble();
    // The press point is the candidate's centre; a later sibling delegate
    // covering the centre takes the press and drags its own note (Factory
    // quirks overlap wide neighbours), so the check picks the first
    // in-view candidate no later delegate covers.
    QQuickItem *note = nullptr;
    for (int i = 0; i < delegates.size() && note == nullptr; i++) {
        auto *const item = qobject_cast<QQuickItem *>(delegates[i]);
        const QPointF local = bg->mapFromItem(item, QPointF(item->width() / 2, item->height() / 2));
        if (!(local.x() > gutter + 50.0 && local.x() < bg->width() - 50.0 && local.y() > 30.0
              && item->width() >= 6.0)) {
            continue;
        }
        const QPointF centre = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
        bool covered = false;
        for (int j = i + 1; j < delegates.size() && !covered; j++) {
            auto *const later = qobject_cast<QQuickItem *>(delegates[j]);
            covered = later->contains(later->mapFromScene(centre));
        }
        if (!covered) {
            note = item;
        }
    }
    if (note == nullptr) {
        std::cout << "song end: skipped the drag guides (no pressable note in view)"
                  << std::endl;
        return true;
    }
    auto guide = [&](const char *name) -> QQuickItem * {
        for (QQuickItem *const child : note->childItems()) {
            if (child->objectName() == QLatin1String(name)) {
                return child;
            }
        }
        return nullptr;
    };
    QQuickItem *const startGuide = guide("dragGuideStart");
    QQuickItem *const endGuide = guide("dragGuideEnd");
    if (startGuide == nullptr || endGuide == nullptr) {
        std::cerr << "song end: the note has no drag guides\n";
        return false;
    }
    if (startGuide->isVisible() || endGuide->isVisible()) {
        std::cerr << "song end: the drag guides show without a drag\n";
        return false;
    }
    const QPointF press = note->mapToScene(QPointF(note->width() / 2, note->height() / 2));
    send_mouse(window, press, QEvent::MouseButtonPress, Qt::LeftButton, Qt::LeftButton, 80000);
    for (int step = 1; step <= 6; step++) {
        send_mouse(window, press + QPointF(8.0 * step, 0.0), QEvent::MouseMove, Qt::LeftButton,
                   Qt::NoButton, 80000 + 20 * step);
    }
    const double ghostX = note->property("ghostX").toDouble();
    const double ghostWidth = std::max(2.0, note->property("ghostWidth").toDouble());
    const double wantedStart = bg->mapToScene(QPointF(ghostX, 0)).x();
    const double wantedEnd = bg->mapToScene(QPointF(ghostX + ghostWidth - 1.0, 0)).x();
    const double startX = startGuide->mapToScene(QPointF(0, 0)).x();
    const double endX = endGuide->mapToScene(QPointF(0, 0)).x();
    const double guideBottom =
        startGuide->mapToScene(QPointF(0, startGuide->height())).y();
    const double sheetBottom = bg->mapToScene(QPointF(0, bg->height())).y();
    const bool shown = startGuide->isVisible() && endGuide->isVisible();
    send_mouse(window, press + QPointF(48.0, 0.0), QEvent::MouseButtonRelease, Qt::NoButton,
               Qt::LeftButton, 80200);
    pump_events(100);
    if (!shown || std::abs(startX - wantedStart) > 1.0 || std::abs(endX - wantedEnd) > 1.0
        || std::abs(guideBottom - sheetBottom) > 1.0) {
        std::cerr << "song end: the drag guides are wrong (shown " << shown << ", start "
                  << startX << " / " << wantedStart << ", end " << endX << " / " << wantedEnd
                  << ", bottom " << guideBottom << " / " << sheetBottom << ")\n";
        return false;
    }
    if (document->property("canUndo").toBool()) {
        send_key(window, Qt::Key_Z, Qt::ControlModifier);
    }
    std::cout << "song end: ok" << std::endl;
    return true;
}

// usdx_block_drag_check drives the block mode's selection band (UI_SPEC
// 4.7, task P7-08): it pushes the repair page in the block mode over the
// first line, pins the block's new times to whole beats, and drags the
// body (move by one beat, the length keeps), the start edge (scale from
// the opposite edge, snapped to whole beats), the start edge with Shift
// (the snapping off, the fractional beat keeps) and the end edge. The
// band's items live in RepairSheet.qml; the times are read back through
// the repair bridge's blockNewStartMs/blockNewEndMs.
bool usdx_block_drag_check(const QQmlApplicationEngine &engine) {
    QObject *const root = selftest_root(engine);
    if (root == nullptr) {
        return false;
    }
    auto *const window = qobject_cast<QQuickWindow *>(root);
    if (window == nullptr) {
        std::cerr << "block band: the root is not a window\n";
        return false;
    }
    // The page opens with the song's data before any repair action: the
    // overview strip holds one block per line of the current voice.
    {
        QObject *const openedRepair = root->property("repair").value<QObject *>();
        QObject *const openedDocument = root->property("songDocument").value<QObject *>();
        if (openedRepair == nullptr || openedDocument == nullptr
            || !QMetaObject::invokeMethod(root, "showRepairPage")) {
            std::cerr << "block band: the repair page could not be opened\n";
            return false;
        }
        pump_events(100);
        const int overviewLines = openedRepair->property("overviewLines").toList().size();
        if (openedDocument->property("voiceCount").toInt() == 1
            && overviewLines != openedDocument->property("lineCount").toInt()) {
            std::cerr << "block band: the opened repair page shows " << overviewLines
                      << " overview lines, the song has "
                      << openedDocument->property("lineCount").toInt() << "\n";
            return false;
        }
    }
    if (!usdx_open_demo_repair_block(engine, 0, 0)) {
        std::cerr << "block band: the repair page in the block mode could not be opened\n";
        return false;
    }
    QObject *const repair = root->property("repair").value<QObject *>();
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const bg = root->findChild<QObject *>(QStringLiteral("repairSheetBackground"));
    auto *const band = qobject_cast<QQuickItem *>(
        root->findChild<QObject *>(QStringLiteral("blockBand")));
    auto *const bodyDrag = qobject_cast<QQuickItem *>(
        root->findChild<QObject *>(QStringLiteral("blockBandBody")));
    auto *const startDrag = qobject_cast<QQuickItem *>(
        root->findChild<QObject *>(QStringLiteral("blockBandStartDrag")));
    auto *const endDrag = qobject_cast<QQuickItem *>(
        root->findChild<QObject *>(QStringLiteral("blockBandEndDrag")));
    if (repair == nullptr || document == nullptr || bg == nullptr || band == nullptr
        || bodyDrag == nullptr || startDrag == nullptr || endDrag == nullptr) {
        std::cerr << "block band: the band items are missing\n";
        return false;
    }
    if (!band->isVisible()) {
        std::cerr << "block band: the band is not visible\n";
        return false;
    }
    // The drawer check ran before this one; its close animation leaves
    // the modal overlay swallowing synthesized presses for a moment
    // (lib.rs's check order note), so the presses settle first.
    pump_events(300);
    const double bpm = document->property("bpm").toDouble();
    const double gap = document->property("gapMs").toDouble();
    double pxPerBeat = bg->property("pxPerBeat").toDouble();
    if (bpm <= 0.0 || pxPerBeat <= 0.0) {
        std::cerr << "block band: the song timing or the sheet zoom is unknown\n";
        return false;
    }
    const double msPerBeat = 15000.0 / bpm;
    const auto beat_ms = [&](double beat) { return beat * msPerBeat + gap; };
    const auto ms_beat = [&](double ms) { return (ms - gap) / msPerBeat; };

    // The whole check runs with the block's edges on screen; the drag
    // helpers press the drag area's centre and move in eight steps, the
    // way the sheet interaction check drags.
    ulong next_time = 90000;
    auto drag = [&](QQuickItem *area, double dx, Qt::KeyboardModifiers modifiers) {
        const QPointF from = area->mapToScene(QPointF(area->width() / 2.0,
                                                      area->height() / 2.0));
        send_mouse(window, from, QEvent::MouseButtonPress, Qt::LeftButton,
                   Qt::LeftButton, next_time += 1000);
        for (int step = 1; step <= 8; step++) {
            send_mouse(window, from + QPointF(dx * step / 8.0, 0.0),
                       QEvent::MouseMove, Qt::LeftButton, Qt::NoButton,
                       next_time += 20, modifiers);
        }
        send_mouse(window, from + QPointF(dx, 0.0), QEvent::MouseButtonRelease,
                   Qt::NoButton, Qt::LeftButton, next_time += 20);
        pump_events(50);
    };
    auto on_screen = [&](QQuickItem *area) {
        const double x = area->mapToScene(QPointF(0.0, 0.0)).x();
        return x >= 0.0 && x + area->width() <= window->width();
    };

    // A deterministic base: the block's new times at whole beats 2 and
    // 10, so the snapping expectations are independent of the fixture.
    if (!QMetaObject::invokeMethod(repair, "setBlockTimes", Q_ARG(double, beat_ms(2.0)),
                                   Q_ARG(double, beat_ms(10.0)))) {
        std::cerr << "block band: setBlockTimes is not callable\n";
        return false;
    }
    pump_events(50);
    if (!on_screen(startDrag) || !on_screen(endDrag) || !on_screen(bodyDrag)) {
        std::cerr << "block band: skipped (a band edge lies off screen)\n";
        return true;
    }
    // New times outside the view refit it, so the zoom is read again.
    pxPerBeat = bg->property("pxPerBeat").toDouble();

    // The body drag moves both times by one snapped beat; the length
    // keeps. The view stays where it is: the band moves, not the sheet.
    const double bodyStart = repair->property("blockNewStartMs").toDouble();
    const double bodyEnd = repair->property("blockNewEndMs").toDouble();
    const double viewStart = bg->property("startBeat").toDouble();
    drag(bodyDrag, pxPerBeat * 1.4, Qt::NoModifier);
    if (std::abs(bg->property("startBeat").toDouble() - viewStart) > 1e-6
        || std::abs(bg->property("pxPerBeat").toDouble() - pxPerBeat) > 1e-6) {
        std::cerr << "block band: the view moved after a drag inside it (start beat "
                  << viewStart << " -> " << bg->property("startBeat").toDouble() << ")\n";
        return false;
    }
    const double bodyStartAfter = repair->property("blockNewStartMs").toDouble();
    const double bodyEndAfter = repair->property("blockNewEndMs").toDouble();
    if (std::abs((bodyStartAfter - bodyStart) / msPerBeat - 1.0) > 0.02
        || std::abs((bodyEndAfter - bodyEnd) / msPerBeat - 1.0) > 0.02
        || std::abs((bodyEndAfter - bodyStartAfter) - (bodyEnd - bodyStart)) > 0.05 * msPerBeat) {
        std::cerr << "block band: the body drag did not move the block by one beat (start "
                  << bodyStart << " -> " << bodyStartAfter << ", end " << bodyEnd << " -> "
                  << bodyEndAfter << ")\n";
        return false;
    }
    std::cout << "block band: body moved" << std::endl;

    // The start edge scales from the opposite edge, snapped to whole
    // beats: beat 3 + 2.4 rounds to beat 5.
    const double edgeStart = repair->property("blockNewStartMs").toDouble();
    drag(startDrag, pxPerBeat * 2.4, Qt::NoModifier);
    const double edgeStartAfter = repair->property("blockNewStartMs").toDouble();
    if (std::abs((edgeStartAfter - edgeStart) / msPerBeat - 2.0) > 0.02) {
        std::cerr << "block band: the start edge drag is not snapped to two beats ("
                  << edgeStart << " -> " << edgeStartAfter << ")\n";
        return false;
    }
    std::cout << "block band: start edge snapped" << std::endl;

    // Shift turns the snapping off: the fractional beat keeps.
    const double shiftStart = repair->property("blockNewStartMs").toDouble();
    drag(startDrag, pxPerBeat * 2.4, Qt::ShiftModifier);
    const double shiftStartAfter = repair->property("blockNewStartMs").toDouble();
    if (std::abs((shiftStartAfter - shiftStart) / msPerBeat - 2.4) > 0.05) {
        std::cerr << "block band: the Shift drag did not keep the fractional beat ("
                  << shiftStart << " -> " << shiftStartAfter << ", "
                  << ms_beat(shiftStartAfter) << " beats)\n";
        return false;
    }
    std::cout << "block band: shift unsnapped" << std::endl;

    // The end edge scales from the opposite edge, snapped to whole
    // beats: beat 11 - 2.4 rounds down to beat 9.
    const double edgeEnd = repair->property("blockNewEndMs").toDouble();
    drag(endDrag, -pxPerBeat * 2.4, Qt::NoModifier);
    const double edgeEndAfter = repair->property("blockNewEndMs").toDouble();
    if (std::abs((edgeEndAfter - edgeEnd) / msPerBeat + 2.0) > 0.02) {
        std::cerr << "block band: the end edge drag is not snapped to two beats ("
                  << edgeEnd << " -> " << edgeEndAfter << ")\n";
        return false;
    }
    std::cout << "block band: end edge snapped" << std::endl;
    // The band belongs to the block mode: the anchor mode hides it, the
    // block it keeps shows again when the block mode returns.
    QMetaObject::invokeMethod(repair, "setMode", Q_ARG(int, 0));
    pump_events(50);
    if (band->isVisible()) {
        std::cerr << "block band: the band shows in the anchor mode\n";
        return false;
    }
    QMetaObject::invokeMethod(repair, "setMode", Q_ARG(int, 1));
    pump_events(50);
    if (!band->isVisible()) {
        std::cerr << "block band: the band does not come back in the block mode\n";
        return false;
    }
    // Apply commits the dragged block; the page then shows the song as it
    // is: no pending preview (the block's new times are its times now).
    if (repair->property("hasPreview").toBool()) {
        bool applied = false;
        QMetaObject::invokeMethod(repair, "apply", Q_RETURN_ARG(bool, applied));
        pump_events(50);
        if (!applied || repair->property("hasPreview").toBool()) {
            std::cerr << "block band: after Apply the preview is still pending (applied "
                      << applied << ")\n";
            return false;
        }
        send_key(window, Qt::Key_Z, Qt::ControlModifier);
    }
    std::cout << "block band: ok" << std::endl;
    return true;
}
