// About me: the Edit line dialog (UI_SPEC 5.4, task P6-02). It edits one
// line's whole text in the line text syntax (a space separates words, |
// splits a word into syllables, ~ continues the previous syllable on a new
// note). Every keystroke asks the document for a preview (previewLineText,
// JSON): the syllable chips on their notes with the start beat below
// (changed text: highlight border on highlight at 20 %, new note: positive
// border on the positive background), the count message when the syllable
// count of a changed stretch differs from its note count, and a refusal
// that disables Apply. "When counts differ" switches between the automatic
// split and merge and "Ask me per note", which shows one note pick per
// count change (user decision 2026-11-15, "Pick note per change"). Apply
// commits the text as one undo step; Cancel and Esc close without change.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.Dialog {
    id: dialog

    // The document whose line the dialog edits.
    property Usdx.SongDocument document

    // The global line index being edited.
    property int line: -1

    // "Ask me per note": the count changes show their note picks.
    property bool askPerNote: false

    // The picked line note per count change (-1: automatic).
    property var picks: []

    // The last preview of the field's text.
    property var preview: ({
        chips: [],
        changes: [],
        message: "",
        error: "",
        changed: false,
        canApply: false
    })

    // openFor shows the dialog for a line with its text, or with a given
    // text (the screenshot demo state).
    function openFor(lineIndex, text) {
        dialog.line = lineIndex;
        dialog.picks = [];
        lineField.text = text !== undefined && text !== null
                         ? text : dialog.document.lineText(lineIndex);
        dialog.refresh();
        dialog.open();
        lineField.forceActiveFocus();
    }

    // refresh asks the document for the preview of the field's text.
    function refresh() {
        if (!dialog.document || dialog.line < 0) {
            return;
        }
        dialog.preview = JSON.parse(dialog.document.previewLineText(
            dialog.line, lineField.text, dialog.askPerNote ? dialog.picks.join(",") : ""));
    }

    // apply commits the text and closes; an unchanged text just closes.
    function apply() {
        if (!dialog.preview.canApply) {
            return;
        }
        if (dialog.preview.changed && !dialog.document.applyLineText(
                dialog.line, lineField.text,
                dialog.askPerNote ? dialog.picks.join(",") : "")) {
            return;
        }
        dialog.close();
    }

    title: qsTr("Edit line %1").arg(dialog.line + 1)
    preferredWidth: metrics.lineTextDialogWidth
    padding: metrics.dialogPadding
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            id: applyAction

            text: qsTr("Apply")
            icon.name: "dialog-ok-apply"
            enabled: dialog.preview.canApply
            onTriggered: dialog.apply()
        },
        Kirigami.Action {
            text: qsTr("Cancel")
            icon.name: "dialog-cancel"
            onTriggered: dialog.close()
        }
    ]

    // Apply is the default button (UI_SPEC 5.4): the desktop style draws
    // the default look from Accessible.defaultButton; Enter in the field
    // triggers it.
    onAboutToShow: {
        const applyButton = dialog.customFooterButton(applyAction);
        if (applyButton) {
            applyButton.Accessible.defaultButton = true;
        }
    }

    Usdx.Metrics {
        id: metrics
    }

    ColumnLayout {
        spacing: metrics.dialogSpacing

        QQC2.Label {
            Layout.fillWidth: true
            text: qsTr("Line text. A space separates words, | splits a word into syllables, ~ continues the previous syllable on a new note.")
            wrapMode: Text.WordWrap
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        QQC2.TextField {
            id: lineField

            objectName: "lineTextField"
            Layout.fillWidth: true
            font.pixelSize: metrics.lineTextFieldPixelSize
            onTextEdited: {
                dialog.picks = [];
                dialog.refresh();
            }
            onAccepted: dialog.apply()
        }

        QQC2.Label {
            text: qsTr("Syllables on notes")
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.bold: true
            font.capitalization: Font.AllUppercase
            font.letterSpacing: 0.5
            color: Kirigami.Theme.disabledTextColor
        }

        Flow {
            objectName: "lineTextChips"
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: dialog.preview.chips

                Rectangle {
                    id: chip

                    required property var modelData

                    readonly property bool isChanged: modelData.state === "changed"
                    readonly property bool isNew: modelData.state === "new"

                    width: Math.max(metrics.syllableChipMinimumWidth,
                                    chipColumn.implicitWidth + 2 * metrics.syllableChipPaddingHorizontal)
                    height: chipColumn.implicitHeight + 2 * metrics.syllableChipPaddingVertical
                    radius: metrics.syllableChipRadius
                    border.width: metrics.borderThickness
                    border.color: chip.isNew ? Kirigami.Theme.positiveTextColor
                                  : chip.isChanged ? Kirigami.Theme.highlightColor
                                  : Kirigami.ColorUtils.linearInterpolation(
                                        Kirigami.Theme.backgroundColor,
                                        Kirigami.Theme.textColor, 0.25)
                    color: chip.isNew ? Kirigami.Theme.positiveBackgroundColor
                           : chip.isChanged ? Qt.alpha(Kirigami.Theme.highlightColor, 0.2)
                           : Kirigami.Theme.backgroundColor

                    ColumnLayout {
                        id: chipColumn

                        anchors.centerIn: parent
                        spacing: 0

                        QQC2.Label {
                            Layout.alignment: Qt.AlignHCenter
                            text: chip.modelData.text
                        }

                        QQC2.Label {
                            Layout.alignment: Qt.AlignHCenter
                            text: chip.modelData.beat
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                        }
                    }
                }
            }
        }

        Kirigami.InlineMessage {
            objectName: "lineTextMessage"
            Layout.fillWidth: true
            type: Kirigami.MessageType.Information
            text: dialog.preview.message
            visible: text.length > 0
        }

        Kirigami.InlineMessage {
            objectName: "lineTextError"
            Layout.fillWidth: true
            type: Kirigami.MessageType.Error
            text: dialog.preview.error
            visible: text.length > 0
        }

        // The note picks of "Ask me per note": one row per count change.
        Repeater {
            model: dialog.askPerNote ? dialog.preview.changes : []

            RowLayout {
                id: pickRow

                required property var modelData
                required property int index

                spacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    text: pickRow.modelData.kind === "split"
                          ? qsTr("Split note") : qsTr("Merge notes")
                    color: Kirigami.Theme.disabledTextColor
                }

                QQC2.ComboBox {
                    model: pickRow.modelData.options
                    textRole: "label"
                    valueRole: "note"
                    Component.onCompleted: currentIndex = indexOfValue(pickRow.modelData.chosen)
                    onActivated: (index) => {
                        const picks = dialog.picks.slice();
                        while (picks.length <= pickRow.index) {
                            picks.push(-1);
                        }
                        picks[pickRow.index] = valueAt(index);
                        dialog.picks = picks;
                        dialog.refresh();
                    }
                }
            }
        }

        RowLayout {
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                Layout.leftMargin: Kirigami.Units.gridUnit * 2
                text: qsTr("When counts differ")
                color: Kirigami.Theme.disabledTextColor
            }

            RowLayout {
                spacing: 0

                QQC2.ButtonGroup {
                    id: policyGroup
                }

                QQC2.Button {
                    text: qsTr("Split / merge longest notes")
                    checkable: true
                    checked: true
                    QQC2.ButtonGroup.group: policyGroup
                    onToggled: {
                        if (checked) {
                            dialog.askPerNote = false;
                            dialog.picks = [];
                            dialog.refresh();
                        }
                    }
                }

                QQC2.Button {
                    objectName: "lineTextAskPerNote"
                    text: qsTr("Ask me per note")
                    checkable: true
                    QQC2.ButtonGroup.group: policyGroup
                    onToggled: {
                        if (checked) {
                            dialog.askPerNote = true;
                            dialog.refresh();
                        }
                    }
                }
            }
        }
    }
}
