// About me: the Keyboard Shortcuts page (UI_SPEC 4.11, round R-08). It lists
// every key binding and mouse gesture of the editor window from the one data
// source shortcuts.js, grouped by area: each row shows the keys as key caps
// (the parts of a combination as separate caps joined by "+", alternatives
// joined by " / ") or the gesture as text, and the action beside them. The
// main menu's Keyboard Shortcuts, F1 and Ctrl+? open it; the header's back
// button and Esc return to the editor. shortcutSequences hands the window
// shortcut entries to the self-test, which compares them with the code.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "shortcuts.js" as Shortcuts

Kirigami.ScrollablePage {
    id: page

    // The window shortcut sequences the page lists (for the self-test).
    readonly property var shortcutSequences: Shortcuts.shortcutSequences()
    readonly property int groupCount: Shortcuts.groups.length
    readonly property int entryCount: Shortcuts.entryCount()

    objectName: "shortcutsPage"
    title: qsTr("Keyboard Shortcuts")

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent

            QQC2.ToolButton {
                icon.name: "go-previous"
                text: qsTr("Editor")
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: applicationWindow().pageStack.pop()
            }

            Kirigami.Heading {
                Layout.fillWidth: true
                level: 2
                text: page.title
            }
        }
    }

    // Esc returns to the editor while this page is on top of the page
    // stack (Kirigami's isCurrentPage reads true once the page is popped).
    Shortcut {
        sequence: "Esc"
        enabled: applicationWindow().pageStack.currentItem === page
        onActivated: applicationWindow().pageStack.pop()
    }

    Usdx.Metrics {
        id: metrics
    }

    ColumnLayout {
        spacing: metrics.shortcutsGroupSpacing

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
            text: qsTr("The keys work in the editor window. They stand back while a text field has the focus; the text field keys are listed under Text.")
        }

        Repeater {
            model: Shortcuts.groups

            ColumnLayout {
                id: group

                required property var modelData

                Layout.fillWidth: true
                spacing: metrics.shortcutsRowSpacing

                // The section label style of the start screen (UI_SPEC 4.1).
                QQC2.Label {
                    text: group.modelData.title
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.bold: true
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.5
                    color: Kirigami.Theme.disabledTextColor
                }

                Repeater {
                    model: group.modelData.entries

                    RowLayout {
                        id: entry

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing

                        // The keys: caps, or the gesture as text.
                        Flow {
                            Layout.preferredWidth: metrics.shortcutsKeyColumnWidth
                            Layout.alignment: Qt.AlignTop
                            spacing: metrics.shortcutsCapSpacing

                            Repeater {
                                model: entry.modelData.keys

                                Row {
                                    id: alternative

                                    required property string modelData
                                    required property int index

                                    spacing: metrics.shortcutsCapSpacing

                                    QQC2.Label {
                                        visible: alternative.index > 0
                                        text: "/"
                                        color: Kirigami.Theme.disabledTextColor
                                        anchors.verticalCenter: parent.verticalCenter
                                    }

                                    QQC2.Label {
                                        visible: entry.modelData.mouse === true
                                        text: alternative.modelData
                                        anchors.verticalCenter: parent.verticalCenter
                                    }

                                    Repeater {
                                        // "Ctrl++" style sequences do not occur;
                                        // "+" separates the parts of a combination.
                                        model: entry.modelData.mouse === true
                                               ? [] : alternative.modelData.split("+")

                                        Row {
                                            id: part

                                            required property string modelData
                                            required property int index

                                            spacing: metrics.shortcutsCapSpacing

                                            QQC2.Label {
                                                visible: part.index > 0
                                                text: "+"
                                                color: Kirigami.Theme.disabledTextColor
                                                anchors.verticalCenter: parent.verticalCenter
                                            }

                                            Rectangle {
                                                width: capLabel.implicitWidth
                                                       + 2 * metrics.shortcutsCapPaddingHorizontal
                                                height: capLabel.implicitHeight
                                                        + 2 * metrics.shortcutsCapPaddingVertical
                                                radius: metrics.shortcutsCapRadius
                                                Kirigami.Theme.colorSet: Kirigami.Theme.View
                                                Kirigami.Theme.inherit: false
                                                color: Kirigami.Theme.backgroundColor
                                                border.width: metrics.borderThickness
                                                border.color: Qt.alpha(Kirigami.Theme.textColor, 0.3)

                                                QQC2.Label {
                                                    id: capLabel

                                                    anchors.centerIn: parent
                                                    text: part.modelData
                                                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        QQC2.Label {
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                            wrapMode: Text.WordWrap
                            text: entry.modelData.action
                        }
                    }
                }
            }
        }
    }
}
