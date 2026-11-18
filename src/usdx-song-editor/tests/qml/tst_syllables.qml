// About me: Qt Quick Test cases for qml/syllables.js, the placement of the
// line text editor's preview strip (UI_SPEC 5.4, user decision 2026-11-18):
// one chip per syllable at the sheet x of its beat, never left of the gutter
// edge, each elided at the next chip's x minus 2 px, the last one unbounded.
// tests/qml_tests.rs runs this file with qmltestrunner.

import QtQuick
import QtTest
import "../../qml/syllables.js" as Syllables

TestCase {
    name: "Syllables"

    function test_chips_sit_at_their_beats() {
        // Gutter edge 60 px, the view starts at beat 10, 8 px per beat.
        const layout = Syllables.stripLayout([10, 14, 20], 10, 8, 60);
        compare(layout.length, 3);
        compare(layout[0].x, 60);
        compare(layout[1].x, 92);
        compare(layout[2].x, 140);
    }

    function test_each_chip_ends_before_the_next() {
        const layout = Syllables.stripLayout([10, 14, 20], 10, 8, 60);
        compare(layout[0].maxWidth, 30);
        compare(layout[1].maxWidth, 46);
        compare(layout[2].maxWidth, Infinity);
    }

    function test_a_chip_left_of_the_view_sits_at_the_gutter_edge() {
        const layout = Syllables.stripLayout([2, 12], 10, 8, 60);
        compare(layout[0].x, 60);
        compare(layout[1].x, 76);
        compare(layout[0].maxWidth, 14);
    }

    function test_chips_on_one_x_get_no_negative_width() {
        const layout = Syllables.stripLayout([0, 1, 12], 10, 8, 60);
        compare(layout[0].maxWidth, 0);
        compare(layout[1].maxWidth, 14);
    }

    function test_no_beats_give_no_chips() {
        compare(Syllables.stripLayout([], 0, 8, 60).length, 0);
    }
}
