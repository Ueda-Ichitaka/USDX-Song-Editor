// About me: the block move/scale mode's bar under the sheet (UI_SPEC 4.7,
// task P7-08 step 3d). It mirrors the Repair bridge's block state: the
// "Selection" label with the block's 1-based global line numbers and the
// note count in DemiBold, the Start field of the block's new start, the
// block's current end read-only with an arrow to the new end field (the
// highlighted border), all as m:ss.ss in the fixed-width font (a typed time
// commits both times through repair.setBlockTimes, keeping the other
// value), the Scale field ("3.90x"; a typed factor sets the new end from
// the new start and the old span), the Following lines toggle (Stay or Move
// along through repair.follow / setFollow) and the Apply button (the
// default button), which forwards to repair.apply so one Apply is one undo
// step (plan 8.4).

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Rectangle {
    id: root

    // The repair bridge; RepairPage binds it.
    property Usdx.Repair repair

    // True while a block is selected (the line numbers are set).
    readonly property bool hasBlock: repair && repair.blockFirst >= 0

    Kirigami.Theme.colorSet: Kirigami.Theme.Window
    Kirigami.Theme.inherit: false
    color: Kirigami.Theme.backgroundColor
    implicitHeight: flow.implicitHeight + 16

    // The 1 px border above the window background (the inspector's).
    Rectangle {
        anchors.top: parent.top
        width: parent.width
        height: 1
        color: Kirigami.ColorUtils.linearInterpolation(
            root.Kirigami.Theme.backgroundColor,
            root.Kirigami.Theme.textColor, 0.10)
    }

    // An inline component cannot see this file's ids; the label inherits
    // the Window colour set from the Flow it sits in.
    component GroupLabel: QQC2.Label {
        color: Kirigami.Theme.disabledTextColor
        font.pointSize: Kirigami.Theme.smallFont.pointSize
    }

    // The new times' fields show m:ss.ss and take a typed time back
    // (Time.parseTime: m:ss.ss, m:ss or seconds); an unreadable entry
    // resets the field. Each field tracks its own time property: a change
    // from outside (a drag, a fresh block) resets the text unless the user
    // edits the field (typing broke the text binding).
    component TimeField: QQC2.TextField {
        id: timeField

        property real ms: -1
        property bool isStart: false
        property string fieldLabel

        function resetText() {
            text = ms >= 0 ? Time.longTime(ms) : "";
        }

        font.family: Kirigami.Theme.fixedWidthFont.family
        implicitWidth: 80
        selectByMouse: true
        enabled: root.hasBlock
        text: timeField.ms >= 0 ? Time.longTime(timeField.ms) : ""
        QQC2.ToolTip.text: timeField.fieldLabel
        QQC2.ToolTip.visible: hovered

        Connections {
            target: root.repair
            function onBlockNewStartMsChanged() {
                if (timeField.isStart && !timeField.activeFocus) {
                    timeField.resetText();
                }
            }
            function onBlockNewEndMsChanged() {
                if (!timeField.isStart && !timeField.activeFocus) {
                    timeField.resetText();
                }
            }
        }

        onEditingFinished: {
            const value = Time.parseTime(text);
            if (!root.hasBlock || isNaN(value)) {
                timeField.resetText();
                return;
            }
            // The commit keeps the other field's value: one
            // setBlockTimes with both.
            const newStart = timeField.isStart ? value : root.repair.blockNewStartMs;
            const newEnd = timeField.isStart ? root.repair.blockNewEndMs : value;
            root.repair.setBlockTimes(Math.max(newStart, 0), Math.max(newEnd, 0));
        }
    }

    Flow {
        id: flow

        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        anchors.topMargin: 8
        anchors.bottomMargin: 8
        spacing: 12

        GroupLabel {
            text: qsTr("Selection")
        }

        QQC2.Label {
            objectName: "blockSelectionLabel"
            text: root.hasBlock
                  ? qsTr("Lines %1 - %2 · %3 notes")
                        .arg(root.repair.blockFirst + 1)
                        .arg(root.repair.blockLast + 1)
                        .arg(root.repair.blockNotes)
                  : qsTr("No lines selected")
            font.weight: Font.DemiBold
            color: root.Kirigami.Theme.textColor
        }

        RowLayout {
            spacing: 6

            GroupLabel {
                text: qsTr("Start")
            }

            TimeField {
                id: startField

                ms: root.repair ? root.repair.blockNewStartMs : -1
                isStart: true
                fieldLabel: qsTr("The block's new start")
                objectName: "blockStartField"
            }
        }

        // End: the block's current end, read-only, then the new end
        // field with the highlighted border (UI_SPEC 4.7).
        RowLayout {
            spacing: 6

            GroupLabel {
                text: qsTr("End")
            }

            QQC2.TextField {
                objectName: "blockOldEndField"
                implicitWidth: 80
                readOnly: true
                enabled: root.hasBlock
                font.family: Kirigami.Theme.fixedWidthFont.family
                text: root.hasBlock && root.repair.blockEndMs >= 0
                      ? Time.longTime(root.repair.blockEndMs) : ""
            }

            QQC2.Label {
                text: "\u2192"
                color: root.Kirigami.Theme.disabledTextColor
            }

            TimeField {
                id: endField

                ms: root.repair ? root.repair.blockNewEndMs : -1
                isStart: false
                fieldLabel: qsTr("The block's new end")
                objectName: "blockEndField"

                background: Rectangle {
                    color: endField.Kirigami.Theme.backgroundColor
                    radius: Kirigami.Units.cornerRadius
                    border.width: 1
                    border.color: endField.Kirigami.Theme.highlightColor
                }
            }
        }

        // Scale: the new length over the old span, "3.90x"; a typed
        // factor sets the new end from the new start.
        RowLayout {
            spacing: 6

            GroupLabel {
                text: qsTr("Scale")
            }

            QQC2.TextField {
                id: scaleField

                function resetText() {
                    text = root.hasBlock ? root.repair.blockScale.toFixed(2) + "x" : "";
                }

                objectName: "blockScaleField"
                implicitWidth: 64
                selectByMouse: true
                enabled: root.hasBlock
                font.family: Kirigami.Theme.fixedWidthFont.family
                text: root.hasBlock ? root.repair.blockScale.toFixed(2) + "x" : ""

                Connections {
                    target: root.repair
                    function onBlockScaleChanged() {
                        if (!scaleField.activeFocus) {
                            scaleField.resetText();
                        }
                    }
                }

                onEditingFinished: {
                    const factor = Number(text.replace(/x\s*$/i, "").replace(",", ".").trim());
                    const oldSpan = root.hasBlock
                        ? root.repair.blockEndMs - root.repair.blockStartMs : 0;
                    if (!(factor > 0) || !(oldSpan > 0)) {
                        scaleField.resetText();
                        return;
                    }
                    root.repair.setBlockTimes(root.repair.blockNewStartMs,
                                              root.repair.blockNewStartMs + oldSpan * factor);
                }
            }
        }

        RowLayout {
            spacing: 6

            GroupLabel {
                text: qsTr("Following lines")
            }

            QQC2.Button {
                objectName: "blockFollowStay"
                checkable: true
                autoExclusive: true
                flat: true
                checked: root.repair && root.repair.follow === 0
                enabled: root.hasBlock
                text: qsTr("Stay")
                onToggled: if (checked) root.repair.setFollow(0)
            }

            QQC2.Button {
                objectName: "blockFollowMoveAlong"
                checkable: true
                autoExclusive: true
                flat: true
                checked: !root.repair || root.repair.follow === 1
                enabled: root.hasBlock
                text: qsTr("Move along")
                onToggled: if (checked) root.repair.setFollow(1)
            }
        }

        Item {
            Layout.fillWidth: true
            implicitWidth: 12
        }

        QQC2.Button {
            objectName: "blockApplyButton"
            Accessible.defaultButton: true
            text: qsTr("Apply")
            enabled: root.repair && root.repair.hasPreview
            onClicked: root.repair.apply()
        }
    }
}
