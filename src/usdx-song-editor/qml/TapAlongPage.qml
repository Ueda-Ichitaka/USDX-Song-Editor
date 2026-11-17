// About me: the tap-along page (UI_SPEC 4.8, task P7-08), pushed from the
// repair page's Tap-along mode button for a line range (the racing section
// or the current line). It drives the TapSession bridge: the toolbar picks
// what a tap pins (Line starts or Syllables) and the playback speed, both
// restarting the session while nothing is recorded yet, and shows the
// latency with the Calibrate button (the Measure tap latency dialog). Space
// starts the pre-roll and then
// records a tap at every press (held for the note length if wanted, the
// release is the tap's up time); Backspace takes the last tap back;
// auto-repeat presses are ignored. The main area shows the status line,
// the current line with the tapped syllables greyed and the next one
// highlighted, the next line, the Space key cap that flashes on every tap
// and the help line; the lane under it shows the current notes as blocks
// and every tap as a tick, with time labels every 2 s. The status bar
// counts the taps, names the playback speed and carries Stop and Use taps
// (one undo step, then back to the repair page).

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Kirigami.Page {
    id: root

    // The document, the player, the tap session bridge and the application
    // window (the way back and the calibration dialog).
    property Usdx.SongDocument document
    property Usdx.Player player
    property Usdx.TapSession tapSession
    property QtObject shell

    // The session's global line range (0-based), set by openFor.
    property int firstLine: -1
    property int lastLine: -1

    // What a tap pins (0 line starts, 1 syllables) and the playback rate.
    property int tapMode: 1
    property real rate: 0.5

    // openFor starts a session over the global lines first..last.
    function openFor(first: int, last: int) {
        root.firstLine = first;
        root.lastLine = last;
        root.restart();
        root.forceActiveFocus();
    }

    // restart opens the session again with the page's mode and rate; the
    // toolbar uses it while no tap is recorded.
    function restart() {
        if (root.tapSession && root.firstLine >= 0) {
            root.tapSession.startSession(root.firstLine, root.lastLine, root.tapMode, root.rate);
        }
    }

    // The session's lines (UI_SPEC 4.8): the line holding the next target
    // and how many of its targets are tapped.
    readonly property var lines: root.tapSession ? root.tapSession.lines : []
    readonly property int current: root.tapSession ? root.tapSession.currentSyllable : 0
    readonly property var position: {
        // In line mode a target is a line; in syllable mode a note.
        if (root.lines.length === 0) {
            return { line: -1, tapped: 0 };
        }
        if (root.tapMode === 0) {
            const index = Math.min(root.current, root.lines.length - 1);
            return { line: index, tapped: root.current > index ? 1 : 0 };
        }
        let before = 0;
        for (let index = 0; index < root.lines.length; index++) {
            const count = root.lines[index].syllables.length;
            if (root.current < before + count || index === root.lines.length - 1) {
                return { line: index, tapped: Math.min(count, root.current - before) };
            }
            before += count;
        }
        return { line: -1, tapped: 0 };
    }
    readonly property var currentLine: position.line >= 0 ? root.lines[position.line] : null
    readonly property var nextLine:
        position.line >= 0 && position.line + 1 < root.lines.length
        ? root.lines[position.line + 1] : null

    // The status line's state text (UI_SPEC 4.8).
    readonly property string stateText: {
        if (!root.tapSession) {
            return "";
        }
        switch (root.tapSession.state) {
        case Usdx.TapSession.TapStateKind.PreRoll:
            return qsTr("pre-roll");
        case Usdx.TapSession.TapStateKind.Recording:
            return qsTr("recording");
        case Usdx.TapSession.TapStateKind.Finished:
            return qsTr("finished");
        default:
            return qsTr("ready - press Space to start");
        }
    }

    // The pre-roll in whole seconds before the first line.
    readonly property real preRollSeconds:
        root.tapSession && root.tapSession.preRollStartMs >= 0 && root.lines.length > 0
        && root.lines[0].starts.length > 0
        ? Math.max(0, (root.lines[0].starts[0] - root.tapSession.preRollStartMs) / 1000) : 0

    // lineText joins a session line's syllables with spaces at word ends.
    function lineText(line) {
        if (!line) {
            return "";
        }
        let text = "";
        for (let index = 0; index < line.syllables.length; index++) {
            text += line.syllables[index] + (line.wordEnds[index] ? " " : "");
        }
        return text.trim();
    }

    title: ""
    padding: 0
    focus: true

    // Space presses and releases feed the session with the playback
    // position; auto-repeat presses are ignored; Backspace takes the last
    // tap back.
    Keys.onPressed: (event) => {
        if (!root.tapSession || !root.player) {
            return;
        }
        if (event.key === Qt.Key_Space) {
            if (!event.isAutoRepeat) {
                root.tapSession.keyDown(root.player.positionMs);
                keyCap.flash();
            }
            event.accepted = true;
        } else if (event.key === Qt.Key_Backspace) {
            root.tapSession.undoTap();
            event.accepted = true;
        }
    }
    Keys.onReleased: (event) => {
        if (root.tapSession && root.player && event.key === Qt.Key_Space && !event.isAutoRepeat) {
            root.tapSession.keyUp(root.player.positionMs);
            event.accepted = true;
        }
    }

    // The session follows the playback every frame while the page shows
    // (the editor page's frame loop drains the player's events).
    FrameAnimation {
        running: root.visible && root.player !== null && root.player.playing
        onTriggered: root.tapSession.tick(root.player.positionMs)
    }

    Usdx.Metrics {
        id: metrics
    }

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            QQC2.ToolButton {
                objectName: "tapBackButton"
                icon.name: "go-previous"
                text: qsTr("Repair")
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: {
                    root.tapSession.stop();
                    root.shell.closeTapAlongPage();
                }
            }

            Kirigami.Separator {}

            QQC2.Label {
                text: qsTr("Tap")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            RowLayout {
                spacing: 0

                Repeater {
                    model: [qsTr("Line starts"), qsTr("Syllables")]

                    QQC2.Button {
                        required property string modelData
                        required property int index

                        objectName: index === 0 ? "tapModeLines" : "tapModeSyllables"
                        text: modelData
                        checkable: true
                        checked: root.tapMode === index
                        enabled: root.tapSession && root.tapSession.tapCount === 0
                        onClicked: {
                            root.tapMode = index;
                            checked = Qt.binding(() => root.tapMode === index);
                            root.restart();
                        }
                    }
                }
            }

            QQC2.Label {
                text: qsTr("Speed")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            RowLayout {
                spacing: 0

                Repeater {
                    model: [0.5, 0.75, 1.0]

                    QQC2.Button {
                        required property real modelData

                        text: modelData + "x"
                        checkable: true
                        checked: root.rate === modelData
                        enabled: root.tapSession && root.tapSession.tapCount === 0
                        onClicked: {
                            root.rate = modelData;
                            checked = Qt.binding(() => root.rate === modelData);
                            root.restart();
                        }
                    }
                }
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Label {
                text: qsTr("Latency")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.TextField {
                objectName: "tapLatencyField"
                implicitWidth: Kirigami.Units.gridUnit * 4
                readOnly: true
                text: qsTr("%1 ms").arg(root.tapSession ? Math.round(root.tapSession.latencyMs) : 0)
            }

            QQC2.ToolButton {
                objectName: "tapCalibrateButton"
                icon.name: "input-keyboard"
                text: qsTr("Calibrate…")
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: calibrationDialog.open()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The main area: the centred column on the view background.
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Kirigami.Theme.colorSet: Kirigami.Theme.View
            Kirigami.Theme.inherit: false
            color: Kirigami.Theme.backgroundColor

            ColumnLayout {
                anchors.horizontalCenter: parent.horizontalCenter
                y: metrics.tapPaddingVertical
                width: Math.min(metrics.tapContentWidth,
                                parent.width - 2 * metrics.tapPaddingHorizontal)
                spacing: metrics.tapSpacing

                QQC2.Label {
                    objectName: "tapStatusLine"
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    text: root.currentLine
                          ? qsTr("Line %1 of %2-%3 · pre-roll %4 s before line %2 · %5")
                                .arg(root.currentLine.line + 1)
                                .arg(root.firstLine + 1)
                                .arg(root.lastLine + 1)
                                .arg(Math.round(root.preRollSeconds))
                                .arg(root.stateText)
                          : root.stateText
                }

                // The current line in 22 pt DemiBold: tapped syllables in
                // the disabled colour, the next one highlighted, the rest
                // in the text colour.
                Flow {
                    objectName: "tapCurrentLine"
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: parent.width

                    Repeater {
                        model: root.currentLine ? root.currentLine.syllables.length : 0

                        Rectangle {
                            id: syllableBox

                            required property int index

                            readonly property bool isTapped:
                                root.tapMode === 1 ? index < root.position.tapped
                                                   : root.position.tapped > 0
                            readonly property bool isNext:
                                root.tapMode === 1 ? index === root.position.tapped
                                                   : root.position.tapped === 0 && index === 0
                            readonly property bool wordEnd: root.currentLine.wordEnds[index]

                            implicitWidth: syllableLabel.implicitWidth
                                           + (isNext ? 12 : 0) + (wordEnd ? spaceMetrics.advanceWidth : 0)
                            implicitHeight: syllableLabel.implicitHeight
                            radius: 4
                            color: "transparent"

                            Rectangle {
                                visible: syllableBox.isNext
                                width: syllableLabel.implicitWidth + 12
                                height: parent.height
                                radius: 4
                                color: Kirigami.Theme.highlightColor
                            }

                            QQC2.Label {
                                id: syllableLabel

                                x: syllableBox.isNext ? 6 : 0
                                font.pointSize: 22
                                font.weight: Font.DemiBold
                                text: root.currentLine.syllables[syllableBox.index]
                                color: syllableBox.isNext ? Kirigami.Theme.highlightedTextColor
                                       : syllableBox.isTapped ? Kirigami.Theme.disabledTextColor
                                       : Kirigami.Theme.textColor
                            }
                        }
                    }
                }

                TextMetrics {
                    id: spaceMetrics

                    font.pointSize: 22
                    font.weight: Font.DemiBold
                    text: " "
                }

                QQC2.Label {
                    objectName: "tapNextLine"
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    visible: root.nextLine !== null
                    elide: Text.ElideRight
                    font.pointSize: 12
                    color: Kirigami.Theme.disabledTextColor
                    text: qsTr("next: %1").arg(root.lineText(root.nextLine))
                }

                // The Space key cap; it turns to the highlight colour for
                // 80 ms on every tap.
                Rectangle {
                    id: keyCap

                    property bool lit: false

                    function flash() {
                        keyCap.lit = true;
                        flashTimer.restart();
                    }

                    objectName: "tapKeyCap"
                    Layout.alignment: Qt.AlignHCenter
                    implicitWidth: metrics.keyCapWidth
                    implicitHeight: metrics.keyCapHeight
                    radius: metrics.keyCapRadius
                    color: Kirigami.ColorUtils.linearInterpolation(
                               Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.20)

                    // The face: the window background inset by the border,
                    // 5 px at the bottom.
                    Rectangle {
                        x: metrics.keyCapBorder
                        y: metrics.keyCapBorder
                        width: parent.width - 2 * metrics.keyCapBorder
                        height: parent.height - metrics.keyCapBorder - metrics.keyCapBottomBorder
                        radius: metrics.keyCapRadius - metrics.keyCapBorder
                        color: keyCap.lit ? windowColors.Kirigami.Theme.highlightColor
                                          : windowColors.Kirigami.Theme.backgroundColor

                        QQC2.Label {
                            anchors.centerIn: parent
                            font.pointSize: 14
                            font.bold: true
                            text: qsTr("Space")
                            color: keyCap.lit ? Kirigami.Theme.highlightedTextColor
                                              : Kirigami.Theme.textColor
                        }
                    }

                    Timer {
                        id: flashTimer

                        interval: 80
                        onTriggered: keyCap.lit = false
                    }
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    text: qsTr("Press Space at the start of each highlighted syllable. Hold it for the note length if you like. Backspace takes the last tap back.")
                }
            }
        }

        Item {
            id: windowColors

            Kirigami.Theme.colorSet: Kirigami.Theme.Window
            Kirigami.Theme.inherit: false
        }

        // The lane (UI_SPEC 4.8): 1 px border above, 10 px padding, 70 px;
        // "now" with the current line's notes as blocks, "taps" with a
        // tick per tap, time labels every 2 s over the current line's span.
        Rectangle {
            id: lane

            // The span the lane shows: the current line with 1 s on both
            // sides.
            readonly property real startMs:
                root.currentLine && root.currentLine.starts.length > 0
                ? root.currentLine.starts[0] - 1000 : 0
            readonly property real endMs:
                root.currentLine && root.currentLine.ends.length > 0
                ? root.currentLine.ends[root.currentLine.ends.length - 1] + 1000 : 1000
            readonly property real labelWidth: Kirigami.Units.gridUnit * 2
            readonly property real trackWidth: width - 2 * metrics.tapLanePadding - labelWidth

            function xOf(ms) {
                return metrics.tapLanePadding + labelWidth
                    + (ms - startMs) / Math.max(1, endMs - startMs) * trackWidth;
            }

            objectName: "tapLane"
            Layout.fillWidth: true
            implicitHeight: metrics.tapLaneHeight
            color: windowColors.Kirigami.Theme.backgroundColor
            clip: true

            Rectangle {
                width: parent.width
                height: 1
                color: Kirigami.ColorUtils.linearInterpolation(
                           Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.10)
            }

            QQC2.Label {
                x: metrics.tapLanePadding
                y: metrics.tapLanePadding
                text: qsTr("now")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            Repeater {
                model: root.currentLine ? root.currentLine.starts.length : 0

                Rectangle {
                    required property int index

                    x: lane.xOf(root.currentLine.starts[index])
                    y: metrics.tapLanePadding
                    width: metrics.tapLaneNoteWidth
                    height: metrics.tapLaneNoteHeight
                    color: Kirigami.Theme.negativeTextColor
                }
            }

            QQC2.Label {
                x: metrics.tapLanePadding
                y: metrics.tapLanePadding + metrics.tapLaneNoteHeight + 4
                text: qsTr("taps")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            Repeater {
                objectName: "tapTicks"
                model: root.tapSession ? root.tapSession.taps : []

                Rectangle {
                    required property var modelData

                    visible: modelData >= lane.startMs && modelData <= lane.endMs
                    x: lane.xOf(modelData)
                    y: metrics.tapLanePadding + metrics.tapLaneNoteHeight + 4
                    width: 2
                    height: metrics.tapLaneNoteHeight
                    color: Kirigami.Theme.textColor
                }
            }

            Repeater {
                model: Math.max(0, Math.floor((lane.endMs - lane.startMs) / 2000) + 1)

                QQC2.Label {
                    required property int index

                    readonly property real ms: Math.ceil(lane.startMs / 2000) * 2000 + index * 2000

                    visible: ms <= lane.endMs
                    x: lane.xOf(ms)
                    y: lane.height - height - 4
                    text: Time.shortTime(Math.max(0, ms))
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }
    }

    footer: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: metrics.statusBarSpacing

            QQC2.Label {
                objectName: "tapCountLabel"
                text: qsTr("Taps %1 / %2")
                      .arg(root.tapSession ? root.tapSession.tapCount : 0)
                      .arg(root.tapSession ? root.tapSession.expectedCount : 0)
            }

            QQC2.Label {
                color: Kirigami.Theme.disabledTextColor
                text: qsTr("Playback %1x, pitch kept").arg(root.rate)
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                objectName: "tapStopButton"
                text: qsTr("Stop")
                enabled: root.tapSession && root.tapSession.active
                onClicked: root.tapSession.stop()
            }

            QQC2.Button {
                objectName: "tapUseButton"
                Accessible.defaultButton: true
                text: qsTr("Use taps")
                enabled: root.tapSession && root.tapSession.tapCount > 0
                onClicked: {
                    if (root.tapSession.useTaps()) {
                        root.shell.closeTapAlongPage();
                    }
                }
            }
        }
    }

    // The Measure tap latency dialog (UI_SPEC 5.7); a used latency takes
    // effect at once while no tap is recorded (the session restarts).
    Usdx.Calibration {
        id: calibration

        objectName: "calibration"
        document: root.document
    }

    Usdx.CalibrationDialog {
        id: calibrationDialog

        objectName: "calibrationDialog"
        calibration: calibration
        onUsed: {
            if (root.tapSession && root.tapSession.tapCount === 0) {
                root.restart();
            }
        }
    }
}
