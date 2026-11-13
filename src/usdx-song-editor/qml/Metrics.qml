// About me: the application metrics of the UI_SPEC in one place. Every pixel
// number of the shell that is not a clean multiple of a Kirigami unit lives
// here as a named constant; the QML files instantiate `Metrics {}` (the
// contents are constants, so one instance per file needs no state). The
// values follow UI_SPEC 2.1 measured on this machine with Kirigami 6.31.

import QtQuick

QtObject {
    // Start screen (UI_SPEC 4.1): the placeholder's top offset, the recent
    // list geometry and the gap between them.
    property real startPlaceholderTop: 70
    property real recentListWidth: 460
    property real recentListGap: 18

    // Lines pane (UI_SPEC 4.3): the pane geometry, the row height and the
    // columns of a row.
    property real linesPaneWidth: 300
    property real linesPaneMinimumWidth: 220
    property real linesPaneMaximumWidth: 480
    property real lineRowHeight: 29
    property real lineNumberWidth: 22
    property real lineTimeWidth: 44
    property real lineColumnGap: 6
    property real lineHealthWidth: 16
    property real healthDotSize: 9
    property real racingBarWidth: 3

    // Search field margin above the list.
    property real searchMargin: 8

    // Line header and editor column paddings (8 horizontal, 10 vertical) and
    // the 1 px borders.
    property real columnPaddingHorizontal: 8
    property real columnPaddingVertical: 10
    property real borderThickness: 1

    // Chips: text padding (0 vertical, 4 horizontal) and corner radius.
    property real chipPaddingVertical: 0
    property real chipPaddingHorizontal: 4
    property real chipRadius: 3

    // Pills (health in the line header, unsaved/read-only/audio in the
    // status bar): fully rounded with 1 / 7 px padding.
    property real pillPaddingHorizontal: 7
    property real pillPaddingVertical: 1

    // Note sheet (UI_SPEC 3.1): the left gutter, the gap above the waveform
    // lane, the waveform and activity lane heights, and the progress bar
    // width of the "Reading the waveform" state (UI_SPEC 3.7; the Breeze bar
    // brings its own 6 px track).
    property real sheetGutterWidth: 46
    property real sheetGutterLabelMargin: 4
    property real laneGap: 8
    property real waveformLaneHeight: 54
    property real activityLaneHeight: 6
    property real waveformProgressWidth: 260

    // Status bar: height, side padding and label spacing.
    property real statusBarHeight: 24
    property real statusBarPadding: 10
    property real statusBarSpacing: 16
}
