// About me: declaration of the C++ helpers of the note sheet background
// (task P5-02) and the repair overview strip (task P7-08). cxx-qt-lib 0.10
// provides QFont but no QFontMetrics, so the width, the vertically centred
// baseline offset and the elision of the sheet's labels run through these
// small C++ functions; the rounded rectangle fill joins here because
// cxx-qt-lib has no brush setter either.

#pragma once
#include <QColor>
#include <QFont>
#include <QPainter>
#include <QRectF>
#include <QString>

// The horizontal advance of the text in the font, in pixels.
double usdx_text_width(const QFont& font, const QString& text);

// Half the difference of the font's ascent and descent: the baseline offset
// that centres a text line in a given height when added to half the height.
double usdx_text_baseline_offset(const QFont& font);

// The text elided on the right with an ellipsis so it fits max_width
// pixels; unchanged when it already fits.
QString usdx_elide_text(const QFont& font, const QString& text, double max_width);

// Fills the rounded rectangle with the colour, no outline. cxx-qt-lib 0.10
// has no brush setter, so the sheet background cannot reach
// QPainter::fillPath itself.
void usdx_fill_rounded_rect(QPainter& painter, const QRectF& rect, double radius,
                            const QColor& color);
