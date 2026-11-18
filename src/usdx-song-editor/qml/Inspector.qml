// About me: the note inspector under the note sheet (UI_SPEC 4.3, item 3,
// task P5-05). It shows what the SongDocument's inspector hash holds for
// the current selection: the note position label with the syllable in
// DemiBold (spaces as U+2423), the five type buttons (the F button toggles
// freestyle), the Start, Length and Pitch spin boxes and the Text field.
// Several selected notes disable Start, Length and Text and transpose on a
// pitch change; nothing selected disables everything; a read-only song
// disables the fields through the hash's editable key. Start commits as a
// move of the note that keeps its length (user decision 2026-11-15), Length
// as a resize of the end edge, so the changes are one undo step each. The type buttons take their checked
// state from the hash imperatively: a click commits only when the selection's
// type differs from the button's, so pressing the current type again creates
// no empty undo step.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Rectangle {
    id: root

    // The document the inspector mirrors; set by the editor page.
    property Usdx.SongDocument document

    // The inspector hash of the current selection; before the document
    // publishes it the property reads as undefined, so an empty hash stands
    // in (everything disabled, every field empty).
    readonly property var info:
        document && document.inspector !== undefined ? document.inspector : ({})
    readonly property bool single: info.single === true
    readonly property bool editable: info.editable === true
    readonly property int noteId: info.noteId !== undefined ? info.noteId : -1

    Kirigami.Theme.colorSet: Kirigami.Theme.Window
    Kirigami.Theme.inherit: false
    color: Kirigami.Theme.backgroundColor
    implicitHeight: flow.implicitHeight + 16

    // The 1 px border above the window background (UI_SPEC 4.3).
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

    Flow {
        id: flow

        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        anchors.topMargin: 8
        anchors.bottomMargin: 8
        spacing: 12

        // The position label: "Note 3 · " in disabledTextColor, with the
        // syllable appended in DemiBold (spaces as U+2423); the multi and
        // empty states show their one label alone.
        QQC2.Label {
            text: root.info.label !== undefined ? root.info.label : ""
            color: root.Kirigami.Theme.disabledTextColor
        }

        QQC2.Label {
            visible: root.single
            text: root.single && root.info.text !== undefined
                  ? '"' + root.info.text.replace(/ /g, "\u2423") + '"' : ""
            font.bold: true
            color: root.Kirigami.Theme.textColor
        }

        // The type row: five checkable buttons; F toggles freestyle.
        RowLayout {
            spacing: 6

            GroupLabel {
                text: qsTr("Type")
            }

            component TypeButton: QQC2.Button {
                id: typeButton

                property string code
                property string tooltip

                checkable: true
                autoExclusive: true
                flat: true
                font.family: Kirigami.Theme.fixedWidthFont.family
                implicitWidth: 32
                enabled: root.editable && root.info.count > 0
                text: code
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: tooltip

                // A checkable button is already checked when clicked runs,
                // so the selection's type decides: the current type again
                // is no edit, and the checked state follows the hash after
                // the commit (or stays on the old type when it is refused).
                onClicked: {
                    if (root.info.typeCode !== typeButton.code) {
                        root.document.setSelectionType(typeButton.code);
                    }
                    for (const button of typeButton.parent.children) {
                        if (button.code !== undefined) {
                            button.checked = root.info.typeCode === button.code;
                        }
                    }
                }

                Connections {
                    target: root.document

                    function onInspectorChanged() {
                        typeButton.checked = root.info.typeCode === typeButton.code;
                    }
                }

                Component.onCompleted:
                    checked = root.info.typeCode === typeButton.code
            }

            TypeButton {
                objectName: "inspectorTypeNormal"
                code: ":"
                tooltip: qsTr("Normal")
            }
            TypeButton {
                objectName: "inspectorTypeGolden"
                code: "*"
                tooltip: qsTr("Golden")
            }
            TypeButton {
                objectName: "inspectorTypeFreestyle"
                code: "F"
                tooltip: qsTr("Freestyle")
            }
            TypeButton {
                objectName: "inspectorTypeRap"
                code: "R"
                tooltip: qsTr("Rap")
            }
            TypeButton {
                objectName: "inspectorTypeRapGolden"
                code: "G"
                tooltip: qsTr("Rap golden")
            }
        }

        // Start: the beat spin box and the time it sits at. Visible but
        // disabled in the multi and empty states (UI_SPEC 4.3: every field
        // stays put, nothing is hidden).
        RowLayout {
            enabled: root.editable && root.single
            spacing: 6

            GroupLabel {
                text: qsTr("Start")
            }

            QQC2.SpinBox {
                objectName: "inspectorStart"
                editable: true
                from: 0
                to: 99999
                value: root.info.start !== undefined ? root.info.start : 0

                // A new start moves the note and keeps its length (user
                // decision 2026-11-15); the field is enabled only for a
                // single selected note, so the selection is that note.
                onValueModified: {
                    const delta = value - root.info.start;
                    root.document.moveSelection(delta, 0);
                }
            }

            QQC2.Label {
                text: root.document && root.document.bpm > 0 && root.info.start !== undefined
                      ? Time.longTime(root.document.gapMs
                                      + root.info.start * 15000 / root.document.bpm)
                      : ""
                font.family: Kirigami.Theme.fixedWidthFont.family
                color: root.Kirigami.Theme.textColor
            }
        }

        // Length: at least 1 beat (the spin box's floor; the operation
        // refuses below anyway). Visible but disabled in the multi and
        // empty states.
        RowLayout {
            enabled: root.editable && root.single
            spacing: 6

            GroupLabel {
                text: qsTr("Length")
            }

            QQC2.SpinBox {
                editable: true
                from: 1
                to: 99999
                value: root.info.length !== undefined ? root.info.length : 1

                onValueModified: {
                    const delta = value - root.info.length;
                    root.document.resizeNote(
                        root.noteId, Usdx.SongDocument.ResizeEdge.End, delta);
                }
            }
        }

        // Pitch: disabled (value kept) for freestyle, rap and rap-golden
        // notes; a multi selection transposes by the delta.
        RowLayout {
            enabled: root.editable
                     && (root.info.pitchEditable === true
                         || (root.info.pitchCommon === true && !root.single))
            spacing: 6

            GroupLabel {
                text: qsTr("Pitch")
            }

            QQC2.SpinBox {
                editable: true
                from: 0
                to: 127
                value: root.info.pitch !== undefined ? root.info.pitch : 60

                onValueModified: {
                    const delta = value - root.info.pitch;
                    root.document.transposeSelection(delta);
                }
            }

            QQC2.Label {
                text: root.info.pitchName !== undefined ? root.info.pitchName : ""
                font.family: Kirigami.Theme.fixedWidthFont.family
                color: root.Kirigami.Theme.textColor
            }
        }

        // Text: the syllable, committed on return or focus loss. Visible
        // but disabled in the multi and empty states.
        RowLayout {
            enabled: root.editable && root.single
            spacing: 6

            GroupLabel {
                text: qsTr("Text")
            }

            QQC2.TextField {
                implicitWidth: 90
                text: root.info.text !== undefined ? root.info.text : ""

                onEditingFinished: {
                    if (text !== root.info.text) {
                        root.document.setNoteText(root.noteId, text);
                    }
                }
            }
        }
    }
}
