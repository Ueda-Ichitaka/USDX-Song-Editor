// About me: the line text editor in the lyrics lane (UI_SPEC 5.4, round R-06,
// user decision 2026-11-18 "Inline line field"). It edits one line's whole
// text in the line text syntax (a space separates words, | splits a word
// into syllables, ~ continues the previous syllable on a new note). It is a
// non-modal popup of the note sheet spanning its width: the text field lies
// over the lyrics lane right of the gutter, the panel under it (over the
// gutter too) shows the preview chips at the sheet x of their
// beats (so they line up with the notes above; changed text: highlight
// border on highlight at 20 %, new note: positive border on the positive
// background), the count message, the refusal, the per-change note picks of
// "Ask me per note" and the "When counts differ" choice with Apply and
// Cancel. Every keystroke asks the document for a preview (previewLineText,
// JSON); Apply (or Enter) commits the text as one undo step, Cancel and Esc
// close without change, and a change of the current line closes it too.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "syllables.js" as Syllables

QQC2.Popup {
    id: editor

    // The document whose line the editor edits.
    property Usdx.SongDocument document

    // The sheet background whose beat-to-x mapping places the chips.
    property Usdx.SheetBackground sheet

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

    // The chip placement of the preview strip (UI_SPEC 5.4).
    readonly property var chipLayout: editor.sheet
        ? Syllables.stripLayout(editor.preview.chips.map((chip) => chip.beat),
                                editor.sheet.startBeat, editor.sheet.pxPerBeat,
                                editor.sheet.gutterWidth)
        : []

    // openFor shows the editor for a line with its text, or with a given
    // text (the screenshot demo state).
    function openFor(lineIndex, text) {
        editor.line = lineIndex;
        editor.picks = [];
        lineField.text = text !== undefined && text !== null
                         ? text : editor.document.lineText(lineIndex);
        editor.refresh();
        editor.open();
        lineField.forceActiveFocus();
        lineField.cursorPosition = lineField.text.length;
    }

    // refresh asks the document for the preview of the field's text.
    function refresh() {
        if (!editor.document || editor.line < 0) {
            return;
        }
        editor.preview = JSON.parse(editor.document.previewLineText(
            editor.line, lineField.text, editor.askPerNote ? editor.picks.join(",") : ""));
    }

    // apply commits the text and closes; an unchanged text just closes.
    function apply() {
        if (!editor.preview.canApply) {
            return;
        }
        if (editor.preview.changed && !editor.document.applyLineText(
                editor.line, lineField.text,
                editor.askPerNote ? editor.picks.join(",") : "")) {
            return;
        }
        editor.close();
    }

    modal: false
    focus: true
    padding: 0
    closePolicy: QQC2.Popup.CloseOnEscape

    // The editor belongs to one line: another current line closes it.
    Connections {
        target: editor.document

        function onCurrentLineChanged() {
            if (editor.visible && editor.document.currentLine !== editor.line) {
                editor.close();
            }
        }
    }

    Usdx.Metrics {
        id: metrics
    }

    background: Item {}

    contentItem: ColumnLayout {
        spacing: 0

        // Row 1: the field over the lyrics lane.
        QQC2.TextField {
            id: lineField

            objectName: "lineTextField"
            Layout.fillWidth: true
            Layout.preferredHeight: editor.sheet ? editor.sheet.lyricsHeight - 2 : implicitHeight
            Layout.topMargin: 1
            Layout.leftMargin: editor.sheet ? editor.sheet.gutterWidth : 0
            placeholderText: qsTr("A space separates words, | splits a word into syllables, ~ continues the previous syllable on a new note.")
            onTextEdited: {
                editor.picks = [];
                editor.refresh();
            }
            onAccepted: editor.apply()
        }

        // The panel under the field.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: panel.implicitHeight + 2 * metrics.lineTextPanelPadding
            Kirigami.Theme.colorSet: Kirigami.Theme.Window
            Kirigami.Theme.inherit: false
            color: Kirigami.Theme.backgroundColor

            Rectangle {
                width: parent.width
                height: metrics.borderThickness
                color: Kirigami.ColorUtils.linearInterpolation(
                    Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.2)
            }

            ColumnLayout {
                id: panel

                x: 0
                y: metrics.lineTextPanelPadding
                width: parent.width
                spacing: metrics.lineTextPanelPadding

                // 1. The preview strip: one chip per syllable under its note.
                Item {
                    objectName: "lineTextChips"
                    Layout.fillWidth: true
                    Layout.preferredHeight: metrics.lineTextStripHeight
                    clip: true

                    Repeater {
                        model: editor.preview.chips

                        Rectangle {
                            id: chip

                            required property var modelData
                            required property int index

                            readonly property real beat: modelData.beat
                            readonly property bool isChanged: modelData.state === "changed"
                            readonly property bool isNew: modelData.state === "new"
                            readonly property var place: editor.chipLayout[index]

                            objectName: "lineTextChip"
                            x: place ? place.x : 0
                            width: Math.min(chipLabel.implicitWidth
                                            + 2 * metrics.syllableChipPaddingHorizontal,
                                            place ? place.maxWidth : 0)
                            height: parent.height
                            visible: width >= metrics.lineTextChipMinimumWidth
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

                            QQC2.Label {
                                id: chipLabel

                                x: metrics.syllableChipPaddingHorizontal
                                width: parent.width - 2 * metrics.syllableChipPaddingHorizontal
                                anchors.verticalCenter: parent.verticalCenter
                                text: chip.modelData.text
                                elide: Text.ElideRight
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                        }
                    }
                }

                // 2. The count message and the refusal.
                Kirigami.InlineMessage {
                    objectName: "lineTextMessage"
                    Layout.fillWidth: true
                    Layout.leftMargin: metrics.lineTextPanelPadding
                    Layout.rightMargin: metrics.lineTextPanelPadding
                    type: Kirigami.MessageType.Information
                    text: editor.preview.message
                    visible: text.length > 0
                }

                Kirigami.InlineMessage {
                    objectName: "lineTextError"
                    Layout.fillWidth: true
                    Layout.leftMargin: metrics.lineTextPanelPadding
                    Layout.rightMargin: metrics.lineTextPanelPadding
                    type: Kirigami.MessageType.Error
                    text: editor.preview.error
                    visible: text.length > 0
                }

                // 3. The note picks of "Ask me per note": one row per count
                // change.
                Repeater {
                    model: editor.askPerNote ? editor.preview.changes : []

                    RowLayout {
                        id: pickRow

                        required property var modelData
                        required property int index

                        Layout.leftMargin: metrics.lineTextPanelPadding
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
                                const picks = editor.picks.slice();
                                while (picks.length <= pickRow.index) {
                                    picks.push(-1);
                                }
                                picks[pickRow.index] = valueAt(index);
                                editor.picks = picks;
                                editor.refresh();
                            }
                        }
                    }
                }

                // 4. "When counts differ", Apply and Cancel.
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: metrics.lineTextPanelPadding
                    Layout.rightMargin: metrics.lineTextPanelPadding
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
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
                            checked: !editor.askPerNote
                            QQC2.ButtonGroup.group: policyGroup
                            onToggled: {
                                if (checked) {
                                    editor.askPerNote = false;
                                    editor.picks = [];
                                    editor.refresh();
                                }
                            }
                        }

                        QQC2.Button {
                            objectName: "lineTextAskPerNote"
                            text: qsTr("Ask me per note")
                            checkable: true
                            checked: editor.askPerNote
                            QQC2.ButtonGroup.group: policyGroup
                            onToggled: {
                                if (checked) {
                                    editor.askPerNote = true;
                                    editor.refresh();
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    QQC2.Button {
                        objectName: "lineTextApply"
                        text: qsTr("Apply")
                        icon.name: "dialog-ok-apply"
                        enabled: editor.preview.canApply
                        Accessible.defaultButton: true
                        onClicked: editor.apply()
                    }

                    QQC2.Button {
                        text: qsTr("Cancel")
                        icon.name: "dialog-cancel"
                        onClicked: editor.close()
                    }
                }
            }
        }
    }
}
