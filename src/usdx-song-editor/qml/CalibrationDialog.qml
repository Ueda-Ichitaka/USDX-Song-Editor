// About me: the "Measure tap latency" dialog (UI_SPEC 5.7, task P7-08). It
// starts the Calibration bridge's eight clicks when it opens; every Space
// press records a tap at the playback position. A row of eight circles shows
// which clicks got a counted tap (filled) and which click plays now (the
// highlighted border); the line under it counts the clicks and lists the
// offsets measured so far with their median. "Use N ms" (the default button,
// enabled once every click has a tap) stores the median as the tap latency;
// "Start again" plays the clicks anew; Cancel and Esc close without change.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.Dialog {
    id: dialog

    // The calibration bridge; its document receives the latency.
    property Usdx.Calibration calibration

    // used: emitted after "Use" stored the latency.
    signal used()

    // The offsets measured so far, in click order.
    readonly property var measured: {
        const values = [];
        const offsets = dialog.calibration ? dialog.calibration.offsets : [];
        for (let index = 0; index < offsets.length; index++) {
            if (offsets[index] !== null && offsets[index] !== undefined) {
                values.push(Math.round(offsets[index]));
            }
        }
        return values;
    }

    title: qsTr("Measure tap latency")
    preferredWidth: metrics.calibrationDialogWidth
    padding: metrics.dialogPadding
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            id: useAction

            text: qsTr("Use %1 ms").arg(dialog.calibration && dialog.calibration.hasMedian
                                        ? dialog.calibration.medianMs : 0)
            enabled: dialog.calibration !== null && dialog.calibration.complete
            onTriggered: {
                if (dialog.calibration.useLatency()) {
                    dialog.calibration.stop();
                    dialog.used();
                    dialog.close();
                }
            }
        },
        Kirigami.Action {
            text: qsTr("Start again")
            icon.name: "view-refresh"
            onTriggered: dialog.calibration.start()
        },
        Kirigami.Action {
            text: qsTr("Cancel")
            icon.name: "dialog-cancel"
            onTriggered: dialog.close()
        }
    ]

    onAboutToShow: {
        const button = dialog.customFooterButton(useAction);
        if (button) {
            button.Accessible.defaultButton = true;
        }
    }
    onOpened: {
        dialog.calibration.start();
        content.forceActiveFocus();
    }
    onClosed: dialog.calibration.stop()

    // The round follows the playback every frame while the clicks play.
    FrameAnimation {
        running: dialog.visible && dialog.calibration !== null && dialog.calibration.running
        onTriggered: dialog.calibration.tick()
    }

    Usdx.Metrics {
        id: metrics
    }

    ColumnLayout {
        id: content

        spacing: metrics.dialogSpacing
        focus: true

        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space) {
                if (!event.isAutoRepeat && dialog.calibration.running) {
                    dialog.calibration.tap();
                }
                event.accepted = true;
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Eight clicks play, one per second. Press Space exactly on each click.")
        }

        // One circle per click: filled when the click got a counted tap,
        // a highlighted border for the click now playing.
        Row {
            objectName: "calibrationCircles"
            Layout.alignment: Qt.AlignHCenter
            spacing: metrics.calibrationCircleSpacing

            Repeater {
                model: 8

                Rectangle {
                    required property int index

                    readonly property bool tapped:
                        dialog.calibration !== null
                        && dialog.calibration.offsets[index] !== null
                        && dialog.calibration.offsets[index] !== undefined
                    readonly property bool playing:
                        dialog.calibration !== null && dialog.calibration.running
                        && dialog.calibration.click === index

                    width: metrics.calibrationCircleSize
                    height: metrics.calibrationCircleSize
                    radius: width / 2
                    border.width: 2
                    border.color: playing ? Kirigami.Theme.highlightColor
                                          : Kirigami.ColorUtils.linearInterpolation(
                                                Kirigami.Theme.backgroundColor,
                                                Kirigami.Theme.textColor, 0.20)
                    color: tapped ? Kirigami.Theme.highlightColor : "transparent"
                }
            }
        }

        QQC2.Label {
            objectName: "calibrationStatus"
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
            text: {
                if (!dialog.calibration) {
                    return "";
                }
                let line = qsTr("Click %1 of 8").arg(dialog.calibration.click + 1);
                if (dialog.measured.length > 0) {
                    line += " · " + qsTr("measured so far: %1 ms").arg(dialog.measured.join(", "));
                }
                if (dialog.calibration.hasMedian) {
                    line += " · " + qsTr("median %1 ms").arg(dialog.calibration.medianMs);
                }
                return line;
            }
        }
    }
}
