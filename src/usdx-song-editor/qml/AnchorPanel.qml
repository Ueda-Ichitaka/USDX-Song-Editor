// About me: the anchor + stretch mode's side panel (UI_SPEC 4.6, task
// P7-08). It shows the warning InlineMessage while a racing section is
// selected with fewer than two anchors (the pin-both-ends advice with the
// section's line numbers and neighbour factor), one card per pinned anchor
// (badge, line number and syllable text, the old and pinned times in the
// fixed-width font with the offset, a remove button through
// repair.removeAnchor), the dashed "+" card with the pin instructions, and
// the BETWEEN ANCHORS form: the Note lengths toggle (Scale or Keep, through
// repair.noteLengths / setNoteLengths, default Keep per user decision
// 2026-11-17), the section's stretch factor and the collisions pill from
// repair.shortenedNotes ("none after rounding" or "n notes shortened").
// Cancel and Apply stretch forward to the repair bridge's cancel and apply
// invokables, so one Apply is one undo step (plan 8.4).

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

    Usdx.Metrics {
        id: metrics
    }

    color: Kirigami.Theme.backgroundColor

    // The racing section advice shows while a section is selected (the
    // span is set) and fewer than two anchors are pinned.
    readonly property bool showWarning:
        repair && repair.sectionFirst >= 0 && repair.anchors.length < 2

    // offsetText renders the pinned time's offset: "(unchanged)" or
    // "(+15.20 s)" with a signed second value.
    function offsetText(oldMs, targetMs) {
        if (Math.abs(targetMs - oldMs) < 0.5) {
            return qsTr("(unchanged)");
        }
        const seconds = (targetMs - oldMs) / 1000;
        return "(" + (seconds >= 0 ? "+" : "-") + Math.abs(seconds).toFixed(2) + " s)";
    }

    // factorText renders the neighbour factor: "3.9x".
    function factorText(factor) {
        return factor.toFixed(2) + "x";
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 10
        spacing: 10

        Kirigami.InlineMessage {
            objectName: "anchorWarning"
            Layout.fillWidth: true
            visible: root.showWarning
            type: Kirigami.MessageType.Warning
            text: root.repair && root.repair.sectionFirst >= 0
                  ? qsTr("Lines %1-%2 run %3 times faster than the lines around them. Pin the true start of the first line and of the first line after the section, then Apply.")
                      .arg(root.repair.sectionFirst + 1)
                      .arg(root.repair.sectionLast + 1)
                      .arg(root.repair.sectionFactor.toFixed(1))
                  : ""
        }

        QQC2.Label {
            objectName: "anchorsLabel"
            Layout.fillWidth: true
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.weight: Font.DemiBold
            color: root.Kirigami.Theme.disabledTextColor
            text: qsTr("ANCHORS")
        }

        // The pinned anchors' cards, newest at the bottom.
        Repeater {
            objectName: "anchorCards"
            model: root.repair ? root.repair.anchors : []

            Rectangle {
                id: card

                required property var modelData
                required property int index

                Layout.fillWidth: true
                implicitHeight: cardColumn.implicitHeight + 2 * 6
                radius: 4
                color: root.Kirigami.Theme.backgroundColor
                border.width: 1
                border.color: root.Kirigami.Theme.alternateBackgroundColor

                ColumnLayout {
                    id: cardColumn

                    anchors.fill: parent
                    anchors.margins: 6
                    spacing: 2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        QQC2.Label {
                            text: "A" + (card.modelData.badge + 1)
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            font.weight: Font.DemiBold
                            color: appPalette.anchor
                        }

                        QQC2.Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: qsTr("Line %1 %2")
                                      .arg(card.modelData.line)
                                      .arg(card.modelData.text)
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: root.Kirigami.Theme.textColor
                        }

                        QQC2.ToolButton {
                            objectName: "removeAnchorButton" + card.index
                            icon.name: "edit-delete-remove"
                            text: qsTr("Remove")
                            display: QQC2.AbstractButton.IconOnly
                            onClicked: root.repair.removeAnchor(card.index)
                            QQC2.ToolTip.text: qsTr("Remove this anchor")
                            QQC2.ToolTip.visible: hovered
                        }
                    }

                    QQC2.Label {
                        objectName: "anchorTimes" + card.index
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        font.family: Kirigami.Theme.fixedWidthFont.family
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: root.Kirigami.Theme.disabledTextColor
                        text: Time.longTime(card.modelData.old)
                              + " -> " + Time.longTime(card.modelData.target)
                              + " " + root.offsetText(card.modelData.old,
                                                      card.modelData.target)
                    }
                }
            }
        }

        // The dashed "+" card: how to pin the next anchor.
        Rectangle {
            objectName: "anchorAddCard"
            Layout.fillWidth: true
            implicitHeight: addColumn.implicitHeight + 2 * 6
            radius: 4
            color: "transparent"
            border.width: 1
            border.color: root.Kirigami.Theme.alternateBackgroundColor

            ColumnLayout {
                id: addColumn

                anchors.fill: parent
                anchors.margins: 6
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    QQC2.Label {
                        text: "+"
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        font.weight: Font.DemiBold
                        color: root.Kirigami.Theme.disabledTextColor
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: qsTr("Next anchor")
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: root.Kirigami.Theme.disabledTextColor
                    }
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    text: qsTr("Play, then press A at a line start")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.Kirigami.Theme.disabledTextColor
                    wrapMode: Text.Wrap
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    text: qsTr("or drag a line onto the waveform")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.Kirigami.Theme.disabledTextColor
                    wrapMode: Text.Wrap
                }
            }
        }

        QQC2.Label {
            objectName: "betweenAnchorsLabel"
            Layout.fillWidth: true
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.weight: Font.DemiBold
            color: root.Kirigami.Theme.disabledTextColor
            text: qsTr("BETWEEN ANCHORS")
        }

        // The form (UI_SPEC 4.6 item 3): Note lengths [Scale | Keep] (Keep
        // checked by default, user decision 2026-11-17), the stretch factor
        // of the section and the collisions pill.
        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                text: qsTr("Note lengths")
                color: root.Kirigami.Theme.disabledTextColor
            }

            RowLayout {
                spacing: 0

                QQC2.Button {
                    objectName: "noteLengthsScale"
                    checkable: true
                    autoExclusive: true
                    checked: root.repair && root.repair.noteLengths === 0
                    text: qsTr("Scale")
                    onToggled: if (checked) root.repair.setNoteLengths(0)
                }

                QQC2.Button {
                    objectName: "noteLengthsKeep"
                    checkable: true
                    autoExclusive: true
                    checked: !root.repair || root.repair.noteLengths === 1
                    text: qsTr("Keep")
                    onToggled: if (checked) root.repair.setNoteLengths(1)
                }
            }

            QQC2.Label {
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                visible: stretchFactor.visible
                text: qsTr("Stretch factor")
                color: root.Kirigami.Theme.disabledTextColor
            }

            QQC2.Label {
                id: stretchFactor

                objectName: "stretchFactor"
                visible: root.repair && root.repair.sectionFirst >= 0
                Layout.fillWidth: true
                elide: Text.ElideRight
                font.family: Kirigami.Theme.fixedWidthFont.family
                color: root.Kirigami.Theme.textColor
                text: root.repair && root.repair.sectionFirst >= 0
                      ? root.factorText(root.repair.sectionFactor)
                        + qsTr(" (lines %1-%2)")
                          .arg(root.repair.sectionFirst + 1)
                          .arg(root.repair.sectionLast + 1)
                      : ""
            }

            QQC2.Label {
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                text: qsTr("Collisions")
                color: root.Kirigami.Theme.disabledTextColor
            }

            // The collisions pill (the house pill: smallFont bold, the
            // text colour on the matching background, fully rounded, 1 / 7
            // px padding): positive "none after rounding", neutral when the
            // rounding pass shortened notes.
            Rectangle {
                objectName: "collisionsPill"

                readonly property bool shortened:
                    root.repair !== null && root.repair.shortenedNotes > 0

                Layout.alignment: Qt.AlignLeft | Qt.AlignVCenter
                implicitWidth: collisionsLabel.implicitWidth + 2 * metrics.pillPaddingHorizontal
                implicitHeight: collisionsLabel.implicitHeight + 2 * metrics.pillPaddingVertical
                radius: height / 2
                color: shortened ? root.Kirigami.Theme.neutralBackgroundColor
                                 : root.Kirigami.Theme.positiveBackgroundColor

                QQC2.Label {
                    id: collisionsLabel

                    anchors.centerIn: parent
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.bold: true
                    color: parent.shortened ? root.Kirigami.Theme.neutralTextColor
                                            : root.Kirigami.Theme.positiveTextColor
                    text: parent.shortened
                          ? qsTr("%1 notes shortened").arg(root.repair.shortenedNotes)
                          : qsTr("none after rounding")
                }
            }
        }

        Item {
            Layout.fillHeight: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                objectName: "repairCancelButton"
                text: qsTr("Cancel")
                onClicked: root.repair.cancel()
            }

            QQC2.Button {
                objectName: "repairApplyButton"
                Accessible.defaultButton: true
                text: qsTr("Apply stretch")
                enabled: root.repair && root.repair.hasPreview
                onClicked: root.repair.apply()
            }
        }
    }

    Usdx.Palette {
        id: appPalette
    }
}
