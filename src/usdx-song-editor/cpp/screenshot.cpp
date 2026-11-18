// About me: implementation of the screenshot harness helpers (UI_SPEC 8.1,
// task P3-06); see screenshot.h for what each function does. The grab uses
// grabToImage on the window's content item (which carries the page header
// and every child) and spins a local event loop until the result is ready,
// because grabToImage is asynchronous and the screenshot mode does not run
// the application event loop. The lane helpers reach the sheet background
// by its object name and spin the same kind of bounded local loop; the demo
// helpers call the document's and the dialogs' invokables by name.

#include "screenshot.h"

#include <algorithm>

#include <QtCore/QEventLoop>
#include <QtCore/QModelIndex>
#include <QtCore/QTimer>
#include <QtCore/QAbstractItemModel>
#include <QtGui/QColor>
#include <QtGui/QImage>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickItemGrabResult>
#include <QtQuick/QQuickWindow>

namespace {

// Returns the first root object of the engine as a window, or nullptr when
// the scene did not load or its root is not a window.
QQuickWindow *screenshot_window(const QQmlApplicationEngine &engine) {
    if (engine.rootObjects().isEmpty()) {
        return nullptr;
    }
    return qobject_cast<QQuickWindow *>(engine.rootObjects().first());
}

} // namespace

bool usdx_resize_window(const QQmlApplicationEngine &engine, int width, int height) {
    QQuickWindow *window = screenshot_window(engine);
    if (window == nullptr) {
        return false;
    }
    window->resize(width, height);
    return true;
}

bool usdx_screenshot_window(const QQmlApplicationEngine &engine, const QString &path) {
    QQuickWindow *window = screenshot_window(engine);
    if (window == nullptr) {
        return false;
    }
    // Let bindings, layout and any pending animations of the demo state
    // settle before the grab: 1 s covers the show animations of Kirigami's
    // inline messages, which a shorter pump catches half faded in. The
    // screenshot mode does not run the application event loop, so this is a
    // bounded pump.
    QEventLoop settle;
    QTimer::singleShot(1000, &settle, &QEventLoop::quit);
    settle.exec();
    QQuickItem *content = window->contentItem();
    if (content == nullptr) {
        return false;
    }
    auto result = content->grabToImage();
    if (result.isNull()) {
        return false;
    }
    // grabToImage delivers the result through the ready signal, so the local
    // event loop runs until it arrives or the 10 s idle limit passes
    // (UI_SPEC 8.1).
    QEventLoop loop;
    QObject::connect(result.data(), &QQuickItemGrabResult::ready, &loop, &QEventLoop::quit);
    QTimer::singleShot(10 * 1000, &loop, &QEventLoop::quit);
    loop.exec();
    return result->saveToFile(path);
}

QString usdx_png_size(const QString &path) {
    QImage image(path);
    if (image.isNull()) {
        return QString();
    }
    return QStringLiteral("%1x%2").arg(image.width()).arg(image.height());
}

QString usdx_png_pixel(const QString &path, int x, int y) {
    QImage image(path);
    if (image.isNull()) {
        return QString();
    }
    if (x < 0 || y < 0 || x >= image.width() || y >= image.height()) {
        return QString();
    }
    return image.pixelColor(x, y).name(QColor::HexRgb);
}

namespace {

// Returns the sheet background item of the loaded scene, or nullptr.
QObject *lane_item(const QQmlApplicationEngine &engine) {
    if (engine.rootObjects().isEmpty()) {
        return nullptr;
    }
    return engine.rootObjects().first()->findChild<QObject *>(QStringLiteral("sheetBackground"));
}

// The laneStatus values of the SheetBackground bridge.
constexpr int kLaneNoAudio = 0;
constexpr int kLaneLoading = 1;

} // namespace

bool usdx_hold_lane_loading(const QQmlApplicationEngine &engine, double fraction) {
    QObject *const lanes = lane_item(engine);
    if (lanes == nullptr) {
        return false;
    }
    return QMetaObject::invokeMethod(lanes, "holdLoading", Q_ARG(double, fraction));
}

QString usdx_settle_lanes(const QQmlApplicationEngine &engine, int timeoutMs) {
    QObject *const lanes = lane_item(engine);
    if (lanes == nullptr) {
        return QString();
    }
    for (int waited = 0; waited < timeoutMs && lanes->property("laneStatus").toInt() == kLaneLoading;
         waited += 50) {
        QEventLoop loop;
        QTimer::singleShot(50, &loop, &QEventLoop::quit);
        loop.exec();
    }
    const int status = lanes->property("laneStatus").toInt();
    if (status == kLaneNoAudio) {
        return QStringLiteral("no audio");
    }
    if (status == kLaneLoading) {
        const int percent = qRound(lanes->property("analysisProgress").toDouble() * 100.0);
        return QStringLiteral("loading %1 %").arg(percent);
    }
    return lanes->property("showActivity").toBool() ? QStringLiteral("ready")
                                                    : QStringLiteral("ready, no vocals");
}

bool usdx_select_demo_note(const QQmlApplicationEngine &engine, int position) {
    if (engine.rootObjects().isEmpty()) {
        return false;
    }
    QObject *const document =
        engine.rootObjects().first()->findChild<QObject *>(QStringLiteral("songDocument"));
    if (document == nullptr) {
        return false;
    }
    const QVariant modelVariant = document->property("notesModel");
    auto *const model = modelVariant.value<QAbstractItemModel *>();
    if (model == nullptr) {
        return false;
    }
    // The role numbers of the model, resolved from its role names.
    QHash<int, QByteArray> roles = model->roleNames();
    int noteIdRole = -1;
    int startRole = -1;
    int lengthRole = -1;
    int pitchRole = -1;
    int lineOffsetRole = -1;
    for (auto it = roles.constBegin(); it != roles.constEnd(); ++it) {
        if (it.value() == QByteArrayLiteral("noteId")) {
            noteIdRole = it.key();
        } else if (it.value() == QByteArrayLiteral("start")) {
            startRole = it.key();
        } else if (it.value() == QByteArrayLiteral("length")) {
            lengthRole = it.key();
        } else if (it.value() == QByteArrayLiteral("pitch")) {
            pitchRole = it.key();
        } else if (it.value() == QByteArrayLiteral("lineOffset")) {
            lineOffsetRole = it.key();
        }
    }
    if (noteIdRole < 0 || startRole < 0 || lengthRole < 0 || pitchRole < 0
        || lineOffsetRole < 0) {
        return false;
    }
    // The position-th note of the current line (lineOffset 0).
    int seen = 0;
    for (int row = 0; row < model->rowCount(); ++row) {
        const QModelIndex index = model->index(row, 0);
        if (index.data(lineOffsetRole).toInt() != 0) {
            continue;
        }
        if (seen++ != position) {
            continue;
        }
        const double start = index.data(startRole).toDouble();
        const double length = index.data(lengthRole).toDouble();
        const int pitch = index.data(pitchRole).toInt();
        if (length < 1.0) {
            return false;
        }
        // A band inside the note (a whole beat wide around its middle)
        // selects exactly this note.
        const double middle = start + length / 2.0;
        return QMetaObject::invokeMethod(
            document, "selectRange", Q_ARG(double, middle - 0.5),
            Q_ARG(double, middle + 0.5), Q_ARG(int, pitch), Q_ARG(int, pitch));
    }
    return false;
}

bool usdx_seek_demo_playhead(const QQmlApplicationEngine &engine, double fraction) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return false;
    }
    QObject *const root = roots.first();
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const player = root->findChild<QObject *>(QStringLiteral("player"));
    if (document == nullptr || player == nullptr) {
        return false;
    }
    const double start = document->property("currentLineStartMs").toDouble();
    const double end = document->property("currentLineEndMs").toDouble();
    return QMetaObject::invokeMethod(player, "seekMs",
                                     Q_ARG(double, start + (end - start) * fraction));
}

bool usdx_open_demo_line_text(const QQmlApplicationEngine &engine, int line, const QString &text) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return false;
    }
    QObject *const root = roots.first();
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const sheet = root->findChild<QObject *>(QStringLiteral("sheetArea"));
    if (document == nullptr || sheet == nullptr) {
        return false;
    }
    if (!QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, line))) {
        return false;
    }
    return QMetaObject::invokeMethod(sheet, "editLineText", Q_ARG(QVariant, QVariant(text)));
}

bool usdx_open_demo_repair_block(const QQmlApplicationEngine &engine, int first, int last) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return false;
    }
    QObject *const root = roots.first();
    if (!QMetaObject::invokeMethod(root, "showRepairPage")) {
        return false;
    }
    QObject *const repair = root->property("repair").value<QObject *>();
    if (repair == nullptr) {
        return false;
    }
    if (!QMetaObject::invokeMethod(repair, "setMode", Q_ARG(int, 1))) {
        return false;
    }
    if (!QMetaObject::invokeMethod(repair, "setBlock", Q_ARG(int, first), Q_ARG(int, last))) {
        return false;
    }
    // The bridge holds the block after the seed (the set_mode seeding
    // and the block bar mirror it).
    return repair->property("blockFirst").toInt() == first
        && repair->property("blockLast").toInt() == last
        && repair->property("mode").toInt() == 1;
}

QString usdx_open_demo_repair_fault(const QQmlApplicationEngine &engine, bool blockMode) {
    const auto roots = engine.rootObjects();
    if (roots.isEmpty()) {
        return {};
    }
    QObject *const root = roots.first();
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const repair = root->property("repair").value<QObject *>();
    if (document == nullptr || repair == nullptr) {
        return {};
    }
    const int lines = document->property("lineCount").toInt();
    if (lines < 2) {
        return {};
    }
    const int first = lines >= 24 ? 14 : 0;
    const int last = lines >= 24 ? 21 : std::max(first, lines - 2);
    const int after = last + 1;
    // The line after the run keeps its place: its start is the second
    // anchor's target.
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, after));
    const double afterStart = document->property("currentLineStartMs").toDouble();
    QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, first));
    if (!QMetaObject::invokeMethod(root, "showRepairPage")) {
        return {};
    }
    // The fault: the run squeezed into a quarter of its duration. For the
    // anchor mode the following lines move along with it (the line after
    // the run leaves its place, so the anchor at its original start
    // restores the run, plan 6.7's acceptance test); for the block mode
    // they stay (the block's old end is the repair).
    QMetaObject::invokeMethod(repair, "setMode", Q_ARG(int, 1));
    QMetaObject::invokeMethod(repair, "setBlock", Q_ARG(int, first), Q_ARG(int, last));
    QMetaObject::invokeMethod(repair, "setFollow", Q_ARG(int, blockMode ? 0 : 1));
    const double oldStart = repair->property("blockStartMs").toDouble();
    const double oldEnd = repair->property("blockEndMs").toDouble();
    if (oldEnd <= oldStart) {
        return {};
    }
    QMetaObject::invokeMethod(repair, "setBlockTimes", Q_ARG(double, oldStart),
                              Q_ARG(double, oldStart + (oldEnd - oldStart) / 4.0));
    bool applied = false;
    QMetaObject::invokeMethod(repair, "apply", Q_RETURN_ARG(bool, applied));
    if (!applied) {
        return {};
    }
    QString summary = QStringLiteral("repair demo: lines %1-%2 compressed to a quarter, ")
                          .arg(first + 1)
                          .arg(last + 1);
    if (blockMode) {
        QMetaObject::invokeMethod(repair, "setBlock", Q_ARG(int, first), Q_ARG(int, last));
        QMetaObject::invokeMethod(repair, "setFollow", Q_ARG(int, 0));
        QMetaObject::invokeMethod(repair, "setBlockTimes", Q_ARG(double, oldStart),
                                  Q_ARG(double, oldEnd));
        summary += QStringLiteral("block %1-%2, ")
                       .arg(repair->property("blockFirst").toInt() + 1)
                       .arg(repair->property("blockLast").toInt() + 1);
    } else {
        QMetaObject::invokeMethod(repair, "setMode", Q_ARG(int, 0));
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, first));
        QMetaObject::invokeMethod(repair, "pinCurrentLineAt", Q_ARG(double, oldStart));
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, after));
        QMetaObject::invokeMethod(repair, "pinCurrentLineAt", Q_ARG(double, afterStart));
        QMetaObject::invokeMethod(document, "goToLine", Q_ARG(int, first));
        summary += QStringLiteral("anchors %1, ")
                       .arg(repair->property("anchors").toList().size());
    }
    summary += repair->property("hasPreview").toBool() ? QStringLiteral("preview yes")
                                                       : QStringLiteral("preview no");
    return summary;
}

QString usdx_open_demo_tap_along(const QQmlApplicationEngine &engine) {
    if (usdx_open_demo_repair_fault(engine, false).isEmpty()) {
        return {};
    }
    QObject *const root = engine.rootObjects().first();
    QObject *const document = root->property("songDocument").value<QObject *>();
    QObject *const session = root->property("tapSession").value<QObject *>();
    QObject *const player = root->findChild<QObject *>(QStringLiteral("player"));
    if (document == nullptr || session == nullptr || player == nullptr) {
        return {};
    }
    // The same run of lines the fault compressed.
    const int lines = document->property("lineCount").toInt();
    const int first = lines >= 24 ? 14 : 0;
    const int last = lines >= 24 ? 21 : std::max(first, lines - 2);
    if (!QMetaObject::invokeMethod(root, "showTapAlongPage", Q_ARG(int, first),
                                   Q_ARG(int, last))) {
        return {};
    }
    // The note starts of the session's lines, in tap order.
    QList<double> starts;
    for (const QVariant &line : session->property("lines").toList()) {
        for (const QVariant &start : line.toMap().value(QStringLiteral("starts")).toList()) {
            starts.append(start.toDouble());
        }
    }
    if (starts.isEmpty()) {
        return {};
    }
    // The first press starts the pre-roll; the playback stays paused, the
    // positions are fed directly: the first target starts the recording,
    // then one tap per note start (the latency added, so the tap lands on
    // the start).
    const double latency = session->property("latencyMs").toDouble();
    QMetaObject::invokeMethod(session, "keyDown", Q_ARG(double, starts.first() - 2000.0));
    QMetaObject::invokeMethod(session, "keyUp", Q_ARG(double, starts.first() - 1900.0));
    QMetaObject::invokeMethod(player, "pause");
    QMetaObject::invokeMethod(session, "tick", Q_ARG(double, starts.first()));
    const int taps = std::min<int>(12, starts.size());
    for (int index = 0; index < taps; index++) {
        const double at = starts.at(index) + latency;
        QMetaObject::invokeMethod(session, "tick", Q_ARG(double, at));
        QMetaObject::invokeMethod(session, "keyDown", Q_ARG(double, at));
        QMetaObject::invokeMethod(session, "keyUp", Q_ARG(double, at + 60.0));
    }
    QMetaObject::invokeMethod(player, "pause");
    const int state = session->property("state").toInt();
    const char *const names[] = {"ready", "pre-roll", "recording", "finished"};
    return QStringLiteral("tap demo: lines %1-%2, taps %3 / %4, state %5")
        .arg(first + 1)
        .arg(last + 1)
        .arg(session->property("tapCount").toInt())
        .arg(session->property("expectedCount").toInt())
        .arg(QString::fromLatin1(state >= 0 && state < 4 ? names[state] : "unknown"));
}

QString usdx_open_demo_export_anchors(const QQmlApplicationEngine &engine) {
    if (usdx_open_demo_repair_fault(engine, false).isEmpty()) {
        return {};
    }
    QObject *const root = engine.rootObjects().first();
    QObject *const repair = root->property("repair").value<QObject *>();
    QObject *const dialog = root->findChild<QObject *>(QStringLiteral("exportAnchorsDialog"));
    if (repair == nullptr || dialog == nullptr) {
        return {};
    }
    bool applied = false;
    QMetaObject::invokeMethod(repair, "apply", Q_RETURN_ARG(bool, applied));
    if (!applied || !QMetaObject::invokeMethod(dialog, "openDialog")) {
        return {};
    }
    const QVariantMap preview = dialog->property("preview").toMap();
    return QStringLiteral("export demo: %1 lines, %2 changed, %3 pinned, preview after apply %4")
        .arg(preview.value(QStringLiteral("lineCount")).toInt())
        .arg(preview.value(QStringLiteral("changedCount")).toInt())
        .arg(preview.value(QStringLiteral("pinnedCount")).toInt())
        .arg(repair->property("hasPreview").toBool() ? QStringLiteral("yes")
                                                     : QStringLiteral("no"));
}

QString usdx_open_demo_latency_calibration(const QQmlApplicationEngine &engine) {
    if (usdx_open_demo_tap_along(engine).isEmpty()) {
        return {};
    }
    QObject *const root = engine.rootObjects().first();
    QObject *const dialog = root->findChild<QObject *>(QStringLiteral("calibrationDialog"));
    QObject *const calibration = root->findChild<QObject *>(QStringLiteral("calibration"));
    if (dialog == nullptr || calibration == nullptr) {
        return {};
    }
    QMetaObject::invokeMethod(dialog, "open");
    QEventLoop loop;
    QTimer::singleShot(400, &loop, &QEventLoop::quit);
    loop.exec();
    // The demo shows a round in progress: the clicks stop, the taps of the
    // first five clicks are fed, the sixth click is the one playing.
    QMetaObject::invokeMethod(calibration, "stop");
    QMetaObject::invokeMethod(calibration, "start");
    QMetaObject::invokeMethod(calibration, "stop");
    const double offsets[] = {34.0, 38.0, 36.0, 31.0, 37.0};
    for (int click = 0; click < 5; click++) {
        QMetaObject::invokeMethod(calibration, "tapAt",
                                  Q_ARG(double, click * 1000.0 + offsets[click]));
    }
    calibration->setProperty("click", 5);
    calibration->setProperty("running", true);
    int taps = 0;
    for (const QVariant &offset : calibration->property("offsets").toList()) {
        if (!offset.isNull()) {
            taps++;
        }
    }
    return QStringLiteral("calibration demo: %1 taps, median %2 ms")
        .arg(taps)
        .arg(calibration->property("medianMs").toInt());
}
