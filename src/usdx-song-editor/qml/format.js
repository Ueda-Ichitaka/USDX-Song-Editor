// About me: shared formatting helpers of the QML shell (UI_SPEC 2.5). Times
// in the line list are m:ss.s, times everywhere else m:ss.ss, durations are
// seconds with one decimal and rates are syllables per second with one
// decimal; every number uses a dot as the decimal separator regardless of
// the locale, because these are file values. Files import this with
// `import "format.js" as Time`.

function timeText(ms, decimals) {
    const scale = Math.pow(10, decimals);
    const total = Math.round(ms / 1000 * scale) / scale;
    let whole = Math.floor(total);
    let frac = Math.round((total - whole) * scale);
    if (frac >= scale) {
        whole += 1;
        frac -= scale;
    }
    const fraction = frac > 0 ? "." + String(frac).padStart(decimals, "0") : "";
    return Math.floor(whole / 60) + ":" + String(whole % 60).padStart(2, "0") + fraction;
}

// shortTime renders a start time for the lines pane: m:ss.s.
function shortTime(ms) {
    return timeText(ms, 1);
}

// longTime renders a time for headers and dialogs: m:ss.ss.
function longTime(ms) {
    return timeText(ms, 2);
}

// durationText renders the line duration: "4.8 s".
function durationText(ms) {
    return (ms / 1000).toFixed(1) + " s";
}

// rateText renders the syllable rate: "2.5 syl/s".
function rateText(rate) {
    return rate.toFixed(1) + " syl/s";
}

// fileName returns the last path segment of a plain path or file url.
function fileName(path) {
    const cut = path.lastIndexOf("/");
    return cut >= 0 ? path.slice(cut + 1) : path;
}

// fileUrlOf turns a plain absolute path into an encoded file url string;
// url strings pass through. QML resolves relative url strings against the
// QML base folder, so every url handed to a file dialog or the bridge must
// be absolute.
function fileUrlOf(path) {
    if (path.length === 0 || path.startsWith("file://")) {
        return path;
    }
    return "file://" + encodeURI(path);
}

// folderUrlOf returns the folder part of a plain path or file url as a file
// url string, empty for a bare file name.
function folderUrlOf(path) {
    const cut = path.lastIndexOf("/");
    if (cut < 0) {
        return "";
    }
    return fileUrlOf(path.slice(0, cut));
}
