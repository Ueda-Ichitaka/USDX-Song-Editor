// About me: the line header of the editor column (UI_SPEC 4.3, item 1). It
// shows the line position, the time span with duration, note count and
// syllable rate, the timing health pill and the line type combo with the
// Edit text button. The health stays "timing ok" (the analysis arrives with
// P7) and the line type selection is a no-op until the line operations of
// P6 exist; both render their final texts and enablement already.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Rectangle {
    id: root

    // The document the header mirrors; set by the editor page.
    property Usdx.SongDocument document

    color: Kirigami.Theme.backgroundColor
    implicitHeight: headerRow.implicitHeight + 2 * metrics.columnPaddingVertical

    Usdx.Metrics {
        id: metrics
    }

    RowLayout {
        id: headerRow

        anchors.fill: parent
        anchors.leftMargin: metrics.columnPaddingHorizontal
        anchors.rightMargin: metrics.columnPaddingHorizontal
        anchors.topMargin: metrics.columnPaddingVertical
        anchors.bottomMargin: metrics.columnPaddingVertical
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            text: document && document.isLoaded
                  ? qsTr("Line %1 of %2").arg(document.currentLine + 1).arg(document.lineCount)
                  : qsTr("No line")
            font.bold: true
        }

        QQC2.Label {
            // The times say "Whole line selected" while the whole line is
            // selected (UI_SPEC 4.3); the selection arrives with P5, so the
            // times always show.
            text: {
                if (!document || !document.isLoaded) {
                    return "";
                }
                return Time.longTime(document.currentLineStartMs)
                    + " - " + Time.longTime(document.currentLineEndMs)
                    + " \u00B7 " + Time.durationText(document.currentLineEndMs - document.currentLineStartMs)
                    + " \u00B7 " + document.currentLineNoteCount
                    + " " + (document.currentLineNoteCount === 1 ? qsTr("note") : qsTr("notes"))
                    + " \u00B7 " + Time.rateText(document.currentLineRate);
            }
            font.family: Kirigami.Theme.fixedWidthFont.family
            font.pointSize: 9
            color: Kirigami.Theme.disabledTextColor
        }

        Rectangle {
            id: healthPill

            // The timing health pill: "timing ok", "suspicious" or
            // "racing · 11.9 syl/s" (UI_SPEC 4.3). Health arrives with P7.
            visible: document && document.isLoaded
            Layout.alignment: Qt.AlignVCenter
            radius: height / 2
            color: document && document.currentLineHealth === 2
                   ? Kirigami.Theme.negativeBackgroundColor
                   : document && document.currentLineHealth === 1
                     ? Kirigami.Theme.neutralBackgroundColor
                     : Kirigami.Theme.positiveBackgroundColor
            implicitWidth: healthPillLabel.implicitWidth
                + 2 * metrics.pillPaddingHorizontal
            implicitHeight: healthPillLabel.implicitHeight
                + 2 * metrics.pillPaddingVertical

            QQC2.Label {
                id: healthPillLabel

                anchors.centerIn: parent
                text: {
                    if (!document || !document.isLoaded) {
                        return "";
                    }
                    if (document.currentLineHealth === 2) {
                        return qsTr("racing") + " \u00B7 " + Time.rateText(document.currentLineRate);
                    }
                    return document.currentLineHealth === 1 ? qsTr("suspicious") : qsTr("timing ok");
                }
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                font.bold: true
                color: document && document.currentLineHealth === 2
                       ? Kirigami.Theme.negativeTextColor
                       : document && document.currentLineHealth === 1
                         ? Kirigami.Theme.neutralTextColor
                         : Kirigami.Theme.positiveTextColor
            }
        }

        Item {
            Layout.fillWidth: true
        }

        QQC2.Label {
            text: qsTr("Line type")
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        QQC2.ComboBox {
            id: lineTypeCombo

            // The combo lists the note types; a line with mixed types shows
            // "Mixed" (UI_SPEC 4.3). The selection is a no-op until P6.
            textRole: "label"
            valueRole: "type"
            model: [
                { label: qsTr("Normal"), type: ":" },
                { label: qsTr("Golden"), type: "*" },
                { label: qsTr("Freestyle"), type: "F" },
                { label: qsTr("Rap"), type: "R" },
                { label: qsTr("Rap golden"), type: "G" }
            ]
            enabled: document && document.isLoaded && !document.readOnly
            displayText: document && document.currentLineType === "mixed"
                         ? qsTr("Mixed") : currentText
        }

        QQC2.Button {
            icon.name: "document-edit"
            text: qsTr("Edit text…")
            enabled: document && document.isLoaded && !document.readOnly
            onClicked: {
                // The Edit line dialog arrives with P5.
            }
        }
    }
}
