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

    // Note sheet (UI_SPEC 3.1): the left gutter with its label margin, the
    // ruler above the pitch rows, the lyrics lane and the gap between it and
    // the waveform, the waveform and activity lane heights, and the progress
    // bar width of the "Reading the waveform" state (UI_SPEC 3.7; the Breeze
    // bar brings its own 6 px track).
    property real sheetGutterWidth: 46
    property real sheetGutterLabelMargin: 4
    property real sheetRulerHeight: 22
    property real sheetLyricsHeight: 34
    property real sheetGapHeight: 8
    property real waveformLaneHeight: 54
    property real activityLaneHeight: 6
    property real waveformProgressWidth: 260

    // Dialogs (UI_SPEC 5.4): the 16 px content padding and 12 px spacing.
    property real dialogPadding: 16
    property real dialogSpacing: 12

    // The line text editor in the lyrics lane (UI_SPEC 5.4): the panel
    // padding and spacing, the preview strip height and its syllable chips
    // (1 px border, radius 4, 3 / 6 px padding), and the lyrics lane's
    // Edit line text button with its icon (UI_SPEC 3.6).
    property real lineTextPanelPadding: 6
    property real lineTextStripHeight: 24
    property real lineTextChipMinimumWidth: 12
    property real lyricsEditButtonSize: 22
    property real lyricsEditIconSize: 16
    property real syllableChipRadius: 4
    property real syllableChipPaddingHorizontal: 6

    // The Keyboard Shortcuts page (UI_SPEC 4.11): the 300 px key column,
    // the key caps (1 px border, radius 3, 1 / 6 px padding) and the gaps.
    property real shortcutsKeyColumnWidth: 300
    property real shortcutsCapPaddingHorizontal: 6
    property real shortcutsCapPaddingVertical: 1
    property real shortcutsCapRadius: 3
    property real shortcutsCapSpacing: 4
    property real shortcutsRowSpacing: 6
    property real shortcutsGroupSpacing: 18

    // The Measure tap latency dialog (UI_SPEC 5.7): its preferred width and
    // the eight 18 px click circles 10 px apart.
    property real calibrationDialogWidth: 470
    property real calibrationCircleSize: 18
    property real calibrationCircleSpacing: 10

    // The Export anchors dialog (UI_SPEC 5.5): its preferred width.
    property real exportDialogWidth: 640

    // The inline syllable field over a lyric label (user decision
    // 2026-11-15): at least as wide as the inspector's Text field.
    property real inlineSyllableMinimumWidth: 90

    // The tap-along page (UI_SPEC 4.8): the centred 760 px column with
    // 26 / 18 px padding and 14 px spacing, the 140 x 64 px Space key cap
    // (radius 10, 2 px border, 5 px at the bottom), the 70 px lane and the
    // 6 x 14 px note blocks in it.
    property real tapContentWidth: 760
    property real tapPaddingHorizontal: 26
    property real tapPaddingVertical: 18
    property real tapSpacing: 14
    property real keyCapWidth: 140
    property real keyCapHeight: 64
    property real keyCapRadius: 10
    property real keyCapBorder: 2
    property real keyCapBottomBorder: 5
    property real tapLaneHeight: 70
    property real tapLanePadding: 10
    property real tapLaneNoteWidth: 6
    property real tapLaneNoteHeight: 14

    // Status bar: height, side padding and label spacing.
    property real statusBarHeight: 24
    property real statusBarPadding: 10
    property real statusBarSpacing: 16
}
