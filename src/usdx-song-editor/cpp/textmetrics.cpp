// About me: implementation of the C++ helpers of the note sheet background
// (task P5-02) and the repair overview strip (task P7-08); see textmetrics.h
// for what they do.

#include "textmetrics.h"

#include <QFontMetrics>
#include <QPainterPath>

double usdx_text_width(const QFont& font, const QString& text) {
    return QFontMetrics(font).horizontalAdvance(text);
}

double usdx_text_baseline_offset(const QFont& font) {
    const QFontMetrics metrics(font);
    return (metrics.ascent() - metrics.descent()) / 2.0;
}

QString usdx_elide_text(const QFont& font, const QString& text, double max_width) {
    return QFontMetrics(font).elidedText(text, Qt::ElideRight, max_width);
}

void usdx_fill_rounded_rect(QPainter& painter, const QRectF& rect, double radius,
                            const QColor& color) {
    QPainterPath path;
    path.addRoundedRect(rect, radius, radius, Qt::AbsoluteSize);
    painter.fillPath(path, color);
}
