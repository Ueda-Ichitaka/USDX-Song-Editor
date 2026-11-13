// About me: implementation of the relative-date helper bound by the
// SongDocument bridge (src/bridge/song_document.rs). It compares the opened
// date with the current date and formats older dates with the C locale so
// the month names stay English regardless of the system locale.

#include "format.h"

#include <QDate>
#include <QLocale>
#include <QDateTime>

QString usdx_relative_date_text(std::int64_t opened_secs) {
    const QDate opened = QDateTime::fromSecsSinceEpoch(opened_secs).date();
    const QDate today = QDate::currentDate();
    if (opened == today) {
        return QStringLiteral("today");
    }
    if (opened == today.addDays(-1)) {
        return QStringLiteral("yesterday");
    }
    return QLocale::c().toString(opened, QStringLiteral("d MMM yyyy"));
}
