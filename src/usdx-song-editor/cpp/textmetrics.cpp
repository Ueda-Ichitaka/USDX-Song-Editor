// About me: implementation of the text measuring helpers of the note sheet
// background (task P5-02); see textmetrics.h for what they do.

#include "textmetrics.h"

#include <QFontMetrics>

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
