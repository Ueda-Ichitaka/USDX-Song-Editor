// About me: Qt Quick Test cases for qml/format.js, the shared formatting
// helpers of the QML shell (UI_SPEC 2.5). They pin the time formats: m:ss.s
// in the lines pane and m:ss.ss everywhere else, always with every decimal,
// including the carry into the next second and minute, and the song file a
// drop picks. tests/qml_tests.rs runs this file with qmltestrunner.

import QtQuick
import QtTest
import "../../qml/format.js" as Time

TestCase {
    name: "Format"

    function test_shortTime_data() {
        return [
            { tag: "whole second", ms: 47000, expected: "0:47.0" },
            { tag: "tenths", ms: 14937, expected: "0:14.9" },
            { tag: "zero", ms: 0, expected: "0:00.0" },
            { tag: "carry into the minute", ms: 59960, expected: "1:00.0" },
            { tag: "minutes", ms: 125050, expected: "2:05.1" }
        ];
    }

    function test_shortTime(data) {
        compare(Time.shortTime(data.ms), data.expected);
    }

    function test_longTime_data() {
        return [
            { tag: "whole second", ms: 47000, expected: "0:47.00" },
            { tag: "hundredths", ms: 49290, expected: "0:49.29" },
            { tag: "leading zero in the fraction", ms: 125050, expected: "2:05.05" },
            { tag: "carry into the minute", ms: 59996, expected: "1:00.00" }
        ];
    }

    function test_longTime(data) {
        compare(Time.longTime(data.ms), data.expected);
    }

    function test_songUrlOf_data() {
        return [
            { tag: "no urls (dragged text)", urls: [], expected: null },
            { tag: "one song", urls: ["file:///songs/a/song.txt"],
              expected: "file:///songs/a/song.txt" },
            { tag: "upper-case extension", urls: ["file:///songs/a/SONG.TXT"],
              expected: "file:///songs/a/SONG.TXT" },
            { tag: "the first song among other files",
              urls: ["file:///songs/a/cover.jpg", "file:///songs/a/song.txt",
                     "file:///songs/b/other.txt"],
              expected: "file:///songs/a/song.txt" },
            { tag: "no song among the files", urls: ["file:///songs/a/cover.jpg"],
              expected: null }
        ];
    }

    function test_songUrlOf(data) {
        compare(Time.songUrlOf(data.urls), data.expected);
    }
}
