// About me: implementation of the screenshot harness helpers (UI_SPEC 8.1,
// task P3-06); see screenshot.h for what each function does. The grab uses
// grabToImage on the window's content item (which carries the page header
// and every child) and spins a local event loop until the result is ready,
// because grabToImage is asynchronous and the screenshot mode does not run
// the application event loop.

#include "screenshot.h"

#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
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
    // settle before the grab; the screenshot mode does not run the
    // application event loop, so this is a bounded pump.
    QEventLoop settle;
    QTimer::singleShot(200, &settle, &QEventLoop::quit);
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
