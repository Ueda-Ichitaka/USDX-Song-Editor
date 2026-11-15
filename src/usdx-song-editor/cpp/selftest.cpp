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
// all driven through synthesized mouse and wheel events (send_mouse/
// send_wheel). The
// installed message handler counts every Qt warning, so the self-test can
// fail on QML binding and script errors, which QML reports only as warnings.

#include "selftest.h"

#include <QAbstractItemModel>
#include <QColor>
#include <QCoreApplication>
#include <QEventLoop>
#include <QImage>
#include <QMouseEvent>
#include <QQuickItem>
#include <QQuickWindow>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlError>
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

// Sends one mouse event through the window system interface with
// synchronous delivery - the way a platform plugin does - and pumps the
// event loop afterwards; queued QTest helpers never drain offscreen.
void send_mouse(QWindow *window, const QPointF &local, QEvent::Type type,
                Qt::MouseButtons buttons, Qt::MouseButton button, ulong timestamp) {
    QWindowSystemInterface::handleMouseEvent<QWindowSystemInterface::SynchronousDelivery>(
        window, timestamp, local, window->mapToGlobal(local), buttons, button, type,
        Qt::NoModifier, Qt::MouseEventNotSynthesized);
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
void find_current_line_delegates(QQuickWindow *window, QList<QObject *> &out) {
    std::function<void(QQuickItem *)> walk = [&](QQuickItem *item) {
        for (QQuickItem *const child : item->childItems()) {
            const QVariant note_id = child->property("noteId");
            if (note_id.isValid() && note_id.toLongLong() >= 0 && child->isVisible()
                && child->opacity() >= 1.0 && child->property("lineOffset").toInt() == 0) {
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
        std::cerr << "sheet interaction: fewer than two pressable current-line notes\n";
        return false;
    }

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
    QObject *const tapped = pressable[0];
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
        if (note_id_of(note) != tapped_id && !note->property("selected").toBool()) {
            dragged = note;
            break;
        }
    }
    if (dragged == nullptr) {
        std::cerr << "sheet interaction: no unselected pressable note for the drag\n";
        return false;
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

    // Rubber band on the empty pitch area: a fresh grab is not a takeover,
    // so the band must still grow (TakeOverForbidden only blocks steals).
    // The press point is scanned on a grid until it sits on no visible
    // note delegate of any line, because whole-song painting covers more
    // of the pitch area than the current line alone.
    QList<QObject *> all_notes;
    std::function<void(QQuickItem *)> collect_notes = [&](QQuickItem *item) {
        for (QQuickItem *const child : item->childItems()) {
            const QVariant note_id = child->property("noteId");
            if (note_id.isValid() && note_id.toLongLong() >= 0 && child->isVisible()) {
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
    if (qAbs(pxb_after - pxb * 1.953125) > pxb * 0.01) {
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
    std::cout << "playhead: ok" << std::endl;
    return true;
}
