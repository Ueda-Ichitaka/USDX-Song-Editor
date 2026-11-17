// About me: the "Export anchors for the Factory" dialog (UI_SPEC 5.5, task
// P7-08, plan 6.11). It shows what the anchors.txt export writes before
// anything is written: an Information message with the counts (lines whose
// timing changed, pinned anchors, the end pin), the exact file text read-only
// in the fixed-width font with the comment lines greyed, one Warning per
// check finding with "Leave it out" (drops that line from the export and
// rechecks), an Error when the content breaks the file's rules, and the
// target ("Save to", the song folder's anchors.txt by default, "Change…"
// picks another file). Export asks before replacing an existing file, writes
// through the Repair bridge and shows the passive notification "Anchors
// exported"; it is disabled while the file would hold no anchor line.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs as FileDialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Kirigami.Dialog {
    id: dialog

    // The repair bridge that builds and writes the export, and the window
    // that shows the passive notification.
    property Usdx.Repair repair
    property QtObject shell

    // The source lines "Leave it out" dropped, and the export target.
    property var excluded: []
    property url target

    // The last preview of the export.
    property var preview: ({})

    readonly property int lineCount: preview.lineCount !== undefined ? preview.lineCount : 0
    readonly property string errorText: preview.error !== undefined ? preview.error : ""

    // openDialog starts with every line and the default target.
    function openDialog() {
        dialog.excluded = [];
        dialog.target = dialog.repair.defaultExportUrl();
        dialog.refresh();
        dialog.open();
    }

    function refresh() {
        dialog.preview = dialog.repair.exportPreviewJson(dialog.excluded);
    }

    // summary writes the counts line of the Information message.
    function summary() {
        const changed = dialog.preview.changedCount !== undefined ? dialog.preview.changedCount : 0;
        const pinned = dialog.preview.pinnedCount !== undefined ? dialog.preview.pinnedCount : 0;
        const end = dialog.preview.hasEnd
            ? qsTr("End pin at %1.").arg(Time.longTime(dialog.preview.endMs))
            : qsTr("No end pin.");
        return qsTr("%1 lines: %2 whose timing you changed, %3 pinned anchors. %4")
            .arg(dialog.lineCount).arg(changed).arg(pinned).arg(end);
    }

    // richText greys the comment lines of the file text.
    function richText(text) {
        const escape = (line) => line.replace(/&/g, "&amp;").replace(/</g, "&lt;")
                                     .replace(/>/g, "&gt;").replace(/ /g, "&nbsp;");
        const comment = Kirigami.Theme.disabledTextColor;
        return text.split("\n").map(line => line.startsWith("#")
            ? "<span style=\"color:" + comment + "\">" + escape(line) + "</span>"
            : escape(line)).join("<br>");
    }

    // leaveOut drops the source line from the export and rechecks.
    function leaveOut(line) {
        const excluded = dialog.excluded.slice();
        excluded.push(line);
        dialog.excluded = excluded;
        dialog.refresh();
    }

    function exportNow() {
        if (dialog.repair.fileExists(dialog.target)) {
            replaceDialog.open();
        } else {
            dialog.write();
        }
    }

    function write() {
        if (dialog.repair.exportAnchors(dialog.target, dialog.excluded)) {
            dialog.close();
            dialog.shell.showPassiveNotification(qsTr("Anchors exported"), 2000);
        }
    }

    title: qsTr("Export anchors for the Factory")
    preferredWidth: metrics.exportDialogWidth
    padding: metrics.dialogPadding
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            id: exportAction

            text: qsTr("Export")
            icon.name: "document-export"
            enabled: dialog.lineCount > 0 && dialog.errorText.length === 0
            onTriggered: dialog.exportNow()
        },
        Kirigami.Action {
            text: qsTr("Cancel")
            icon.name: "dialog-cancel"
            onTriggered: dialog.close()
        }
    ]

    // Export is the default button (UI_SPEC 5.5).
    onAboutToShow: {
        const button = dialog.customFooterButton(exportAction);
        if (button) {
            button.Accessible.defaultButton = true;
        }
    }

    Usdx.Metrics {
        id: metrics
    }

    ColumnLayout {
        spacing: metrics.dialogSpacing

        Kirigami.InlineMessage {
            objectName: "exportSummary"
            Layout.fillWidth: true
            type: Kirigami.MessageType.Information
            visible: true
            text: dialog.summary()
        }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 10

            QQC2.TextArea {
                objectName: "exportText"
                readOnly: true
                wrapMode: TextEdit.NoWrap
                textFormat: TextEdit.RichText
                font.family: Kirigami.Theme.fixedWidthFont.family
                font.pointSize: 9
                text: dialog.richText(dialog.preview.text !== undefined ? dialog.preview.text : "")
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Error
            visible: dialog.errorText.length > 0
            text: dialog.errorText
        }

        Repeater {
            model: dialog.preview.problems !== undefined ? dialog.preview.problems : []

            Kirigami.InlineMessage {
                required property var modelData

                objectName: "exportProblem"
                Layout.fillWidth: true
                type: Kirigami.MessageType.Warning
                visible: true
                text: modelData.message
                actions: [
                    Kirigami.Action {
                        text: qsTr("Leave it out")
                        onTriggered: dialog.leaveOut(modelData.sourceLine)
                    }
                ]
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: qsTr("Save to")
                color: Kirigami.Theme.disabledTextColor
            }

            // The path, elided in the middle, in a read-only field frame.
            QQC2.TextField {
                id: pathField

                objectName: "exportPath"
                Layout.fillWidth: true
                readOnly: true
                text: ""
                QQC2.Label {
                    anchors.fill: parent
                    anchors.leftMargin: pathField.leftPadding
                    anchors.rightMargin: pathField.rightPadding
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideMiddle
                    text: decodeURIComponent(dialog.target.toString().replace(/^file:\/\//, ""))
                }
            }

            QQC2.Button {
                icon.name: "folder-open"
                text: qsTr("Change…")
                onClicked: {
                    saveDialog.selectedFile = dialog.target;
                    saveDialog.open();
                }
            }
        }
    }

    FileDialogs.FileDialog {
        id: saveDialog

        title: qsTr("Export anchors for the Factory")
        fileMode: FileDialogs.FileDialog.SaveFile
        nameFilters: [qsTr("Anchor files (*.txt)"), qsTr("All files (*)")]
        onAccepted: dialog.target = saveDialog.selectedFile
    }

    Kirigami.PromptDialog {
        id: replaceDialog

        title: qsTr("Replace the existing anchors.txt?")
        dialogType: Kirigami.PromptDialog.Warning
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: qsTr("Replace")
                onTriggered: {
                    replaceDialog.close();
                    dialog.write();
                }
            },
            Kirigami.Action {
                text: qsTr("Cancel")
                onTriggered: replaceDialog.close()
            }
        ]
    }
}
