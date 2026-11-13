// About me: declaration of the relative-date helper for the recent songs
// list (UI_SPEC 4.1 shows "today", "yesterday" or "10 Nov 2026"). The date
// arithmetic lives in a tiny C++ helper because Qt owns the calendar and the
// app's dependency list has no Rust date crate.

#pragma once
#include <QString>
#include <cstdint>

// Renders when a file was last opened (Unix seconds) as "today",
// "yesterday" or the short civil date "10 Nov 2026". The parameter is
// std::int64_t so the cxx bridge's generated declaration matches exactly;
// qint64 is a different type on this platform.
QString usdx_relative_date_text(std::int64_t opened_secs);
