// About me: the lines pane on the left side of the editor (UI_SPEC 4.3). A
// search field filters the rows case-insensitively and the list shows one
// 29 px row per line of all voices with the 1-based global number, the
// start time, the optional type chip, the text and the health dot. The
// current line is highlighted and the view follows it; the health stays
// positive with no reason until the analysis of P7 fills the roles.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

ColumnLayout {
    id: root

    // The document and its lines model; set by the editor page.
    property Usdx.SongDocument document
    property Usdx.LinesModel linesModel

    spacing: 0

    Usdx.Metrics {
        id: metrics
    }

    Usdx.Palette {
        id: palette
    }

    Kirigami.SearchField {
        id: searchField

        Layout.fillWidth: true
        Layout.margins: metrics.searchMargin
        placeholderText: qsTr("Search lyrics…")
        onTextChanged: {
            if (root.linesModel) {
                root.linesModel.searchText = text;
                root.linesModel.rebuild();
            }
        }
    }

    ListView {
        id: listView

        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: root.linesModel
        currentIndex: 0

        // Keep the current line visible: the model tracks where the current
        // line sits in the filtered rows.
        Connections {
            target: root.linesModel
            function onCurrentRowIndexChanged() {
                if (root.linesModel.currentRowIndex >= 0) {
                    listView.currentIndex = root.linesModel.currentRowIndex;
                    listView.positionViewAtIndex(
                        root.linesModel.currentRowIndex, ListView.Contain);
                }
            }
        }

        delegate: ItemDelegate {
            id: row

            required property int index
            required property int lineNumber
            required property real startMs
            required property string text
            required property string typeSummary
            required property int health
            required property string healthReason
            required property bool isCurrent

            width: ListView.view.width
            height: metrics.lineRowHeight

            background: Rectangle {
                color: row.isCurrent
                       ? Kirigami.Theme.highlightColor
                       : row.index % 2 === 1
                         ? Kirigami.Theme.alternateBackgroundColor
                         : Kirigami.Theme.backgroundColor
            }

            // The 3 px bar marks a racing line (UI_SPEC 4.3); the analysis
            // arrives with P7.
            Rectangle {
                visible: row.health === 2
                width: metrics.racingBarWidth
                height: parent.height
                color: Kirigami.Theme.negativeTextColor
            }

            contentItem: RowLayout {
                spacing: 0

                QQC2.Label {
                    Layout.preferredWidth: metrics.lineNumberWidth
                    horizontalAlignment: Text.AlignRight
                    text: row.lineNumber
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: row.isCurrent
                           ? Kirigami.Theme.highlightedTextColor
                           : Kirigami.Theme.disabledTextColor
                }

                QQC2.Label {
                    Layout.preferredWidth: metrics.lineTimeWidth
                    Layout.leftMargin: metrics.lineColumnGap
                    text: Time.shortTime(row.startMs)
                    font.family: Kirigami.Theme.fixedWidthFont.family
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: row.isCurrent
                           ? Kirigami.Theme.highlightedTextColor
                           : Kirigami.Theme.disabledTextColor
                }

                Rectangle {
                    id: typeChip

                    // The type chip before the text, only when the line is
                    // not all normal notes (UI_SPEC 4.3).
                    visible: row.typeSummary !== ":"
                    Layout.alignment: Qt.AlignVCenter
                    radius: metrics.chipRadius
                    color: row.typeSummary === "mixed" ? palette.noteMixed
                           : row.typeSummary === "*" ? palette.noteGolden
                           : row.typeSummary === "F" ? palette.noteFreestyle
                           : row.typeSummary === "R" ? palette.noteRap
                           : palette.noteRapGolden
                    implicitWidth: typeChipLabel.implicitWidth
                        + 2 * metrics.chipPaddingHorizontal
                    implicitHeight: typeChipLabel.implicitHeight
                        + 2 * metrics.chipPaddingVertical

                    QQC2.Label {
                        id: typeChipLabel

                        anchors.centerIn: parent
                        text: row.typeSummary
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        font.bold: true
                        color: palette.chipText
                    }
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: row.typeSummary !== ":"
                                       ? metrics.lineColumnGap : 0
                    text: row.text
                    elide: Text.ElideRight
                    color: row.isCurrent
                           ? Kirigami.Theme.highlightedTextColor
                           : Kirigami.Theme.textColor
                }

                Item {
                    Layout.preferredWidth: metrics.lineHealthWidth

                    Rectangle {
                        // The health dot with the reason as its tooltip;
                        // positive while the analysis of P7 is pending.
                        anchors.centerIn: parent
                        width: metrics.healthDotSize
                        height: width
                        radius: width / 2
                        color: row.health === 2
                               ? Kirigami.Theme.negativeTextColor
                               : row.health === 1
                                 ? Kirigami.Theme.neutralTextColor
                                 : Kirigami.Theme.positiveTextColor

                        QQC2.ToolTip.visible: healthDotArea.containsMouse
                        QQC2.ToolTip.text: row.health === 0
                                           ? qsTr("Timing looks plausible")
                                           : row.healthReason
                    }

                    MouseArea {
                        id: healthDotArea

                        anchors.fill: parent
                        hoverEnabled: true
                    }
                }
            }

            onClicked: {
                if (root.document) {
                    // The row number is the 1-based global line index.
                    root.document.goToLine(row.lineNumber - 1);
                }
            }
        }

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
    }
}
