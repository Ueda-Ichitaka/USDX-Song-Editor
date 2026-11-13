// About me: the editor page (UI_SPEC 4.3): the toolbar with the playback and
// edit controls, the horizontal split of the lines pane and the editor
// column with the banners, the line header and the note sheet area, and the
// status bar in the page footer. Without a song the page shows the start
// screen with the recent songs list (UI_SPEC 4.1). The playback controls
// show their final enablement but are inert until the player of P4 exists;
// the note sheet area stays an empty view background until P4, the
// inspector arrives with P5.

import QtQuick
import QtQuick.Controls
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.Page {
    id: root

    // The document the page edits and the application window, which owns
    // the dialogs and the global drawer.
    property Usdx.SongDocument document
    property QtObject shell

    title: ""
    padding: 0

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            QQC2.ToolButton {
                icon.name: "application-menu"
                display: QQC2.AbstractButton.IconOnly
                onClicked: root.shell && root.shell.globalDrawer.open()
                QQC2.ToolTip.text: qsTr("Main menu")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.ToolButton {
                icon.name: "document-open"
                text: qsTr("Open")
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: root.shell && root.shell.openFileDialog()
                QQC2.ToolTip.text: qsTr("Open song")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.ToolButton {
                icon.name: "document-save"
                text: qsTr("Save")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canSave
                onClicked: root.shell && root.shell.saveSong()
            }

            Kirigami.Separator {}

            QQC2.ToolButton {
                icon.name: "edit-undo"
                text: qsTr("Undo")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canUndo
                onClicked: root.document.undo()
                QQC2.ToolTip.text: root.document && root.document.undoText.length > 0
                                   ? qsTr("Undo") + " " + root.document.undoText
                                   : qsTr("Undo")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.ToolButton {
                icon.name: "edit-redo"
                text: qsTr("Redo")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canRedo
                onClicked: root.document.redo()
                QQC2.ToolTip.text: root.document && root.document.redoText.length > 0
                                   ? qsTr("Redo") + " " + root.document.redoText
                                   : qsTr("Redo")
                QQC2.ToolTip.visible: hovered
            }

            Kirigami.Separator {}

            // The playback controls are enabled with the audio file in the
            // song folder and do nothing until the player of P4 arrives.
            QQC2.ToolButton {
                icon.name: "media-playback-start"
                text: qsTr("Play")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canPlay
            }

            QQC2.ToolButton {
                icon.name: "view-media-lyrics"
                text: qsTr("Play line")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canPlayLine
            }

            QQC2.ToolButton {
                icon.name: "media-playlist-repeat"
                text: qsTr("Loop")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canLoop
            }

            QQC2.Label {
                text: qsTr("Track")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.ComboBox {
                id: trackCombo

                // Every track is listed; the ones whose file is missing are
                // disabled (UI_SPEC 4.3). The player of P4 uses the choice.
                textRole: "label"
                model: [
                    { label: qsTr("Mix"), available: root.document ? root.document.audioPresent : false },
                    { label: qsTr("Vocals"), available: root.document ? root.document.hasVocals : false },
                    { label: qsTr("Instrumental"), available: root.document ? root.document.hasInstrumental : false }
                ]
                enabled: root.document && root.document.canTrack
                delegate: QQC2.ItemDelegate {
                    required property var modelData
                    required property int index

                    text: modelData.label
                    highlighted: trackCombo.highlightedIndex === index
                    enabled: modelData.available
                    width: trackCombo.width
                }
            }

            QQC2.Label {
                text: qsTr("Speed")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.ComboBox {
                model: ["0.5x", "0.75x", "1x"]
                enabled: root.document && root.document.canSpeed
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.ToolButton {
                icon.name: "tools-wizard"
                text: qsTr("Repair timing")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canRepair
                onClicked: root.shell && root.shell.showNotYetAvailable()
            }

            QQC2.ToolButton {
                icon.name: "document-properties"
                text: qsTr("Song properties")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canProperties
                onClicked: root.shell && root.shell.showNotYetAvailable()
            }
        }
    }

    footer: Usdx.StatusBar {
        document: root.document
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Without a song the page shows the start screen (UI_SPEC 4.1).
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !root.document || !root.document.isLoaded

            Kirigami.PlaceholderMessage {
                id: startMessage

                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                anchors.topMargin: metrics.startPlaceholderTop
                icon.name: "audio-x-generic"
                text: qsTr("No song open")
                explanation: qsTr("Open an UltraStar song file (.txt), or drop one onto this window.")
                helpfulAction: Kirigami.Action {
                    icon.name: "document-open"
                    text: qsTr("Open song…")
                    onTriggered: root.shell && root.shell.openFileDialog()
                }
            }

            ColumnLayout {
                id: recentColumn

                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: startMessage.bottom
                anchors.topMargin: metrics.recentListGap
                width: metrics.recentListWidth
                visible: root.document && root.document.recentCount > 0
                spacing: Kirigami.Units.smallSpacing

                QQC2.Label {
                    text: qsTr("Recent songs")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.bold: true
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.5
                    color: Kirigami.Theme.disabledTextColor
                }

                ListView {
                    id: recentList

                    Layout.fillWidth: true
                    Layout.preferredHeight: contentHeight
                    interactive: false
                    clip: true
                    model: root.document ? root.document.recentModel : null

                    delegate: QQC2.ItemDelegate {
                        id: recentRow

                        required property int index
                        required property string title
                        required property string openedText
                        required property string displayPath
                        required property bool exists
                        required property string pathUrl

                        width: ListView.view.width
                        enabled: recentRow.exists

                        contentItem: ColumnLayout {
                            spacing: 0

                            RowLayout {
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    text: recentRow.title
                                    elide: Text.ElideRight
                                    color: recentRow.enabled
                                           ? Kirigami.Theme.textColor
                                           : Kirigami.Theme.disabledTextColor
                                }

                                QQC2.Label {
                                    text: recentRow.openedText
                                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                                    color: Kirigami.Theme.disabledTextColor
                                }
                            }

                            QQC2.Label {
                                Layout.fillWidth: true
                                text: recentRow.exists
                                      ? recentRow.displayPath
                                      : qsTr("File not found")
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                elide: Text.ElideMiddle
                                color: recentRow.exists
                                       ? Kirigami.Theme.disabledTextColor
                                       : Kirigami.Theme.negativeTextColor
                            }
                        }

                        // The 1 px grid line below every row except the last.
                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: metrics.borderThickness
                            color: Kirigami.ColorUtils.linearInterpolation(
                                       Kirigami.Theme.backgroundColor,
                                       Kirigami.Theme.textColor, 0.10)
                            visible: recentRow.index
                                     < (root.document ? root.document.recentCount : 0) - 1
                        }

                        onClicked: root.shell && root.shell.openRecentSong(recentRow.pathUrl)
                    }
                }
            }
        }

        // The editor column: the banners, the line header and the note sheet
        // area, left of the lines pane in the split.
        SplitView {
            id: split

            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.document && root.document.isLoaded
            orientation: Qt.Horizontal

            Usdx.LinesPane {
                SplitView.preferredWidth: metrics.linesPaneWidth
                SplitView.minimumWidth: metrics.linesPaneMinimumWidth
                SplitView.maximumWidth: metrics.linesPaneMaximumWidth
                document: root.document
                linesModel: root.document ? root.document.linesModel : null
            }

            ColumnLayout {
                SplitView.fillWidth: true
                spacing: 0

                Kirigami.InlineMessage {
                    id: readOnlyBanner

                    position: Kirigami.InlineMessage.Position.Header
                    Layout.fillWidth: true
                    visible: root.document && root.document.readOnly
                    type: Kirigami.MessageType.Error
                    text: root.document && root.document.formatVersion.length > 0
                          ? qsTr("This song uses format version %1. The editor can show and play it, but cannot change or save it.").arg(root.document.formatVersion)
                          : qsTr("This file cannot be saved back without changing parts you did not edit. The editor shows it read-only.")
                }

                Kirigami.InlineMessage {
                    id: audioBanner

                    position: Kirigami.InlineMessage.Position.Header
                    Layout.fillWidth: true
                    visible: root.document && root.document.isLoaded
                             && !root.document.readOnly && !root.document.audioPresent
                    type: Kirigami.MessageType.Warning
                    text: root.document && root.document.audioFileName.length > 0
                          ? qsTr("The audio file \"%1\" is not in the song folder. You can edit the song, but playback and the waveform are off.").arg(root.document.audioFileName)
                          : qsTr("This song names no audio file. You can edit the song, but playback and the waveform are off.")
                    actions: [
                        Kirigami.Action {
                            text: qsTr("Song properties")
                            onTriggered: root.shell && root.shell.showNotYetAvailable()
                        }
                    ]
                }

                Kirigami.InlineMessage {
                    id: saveErrorBanner

                    // Stays until a save succeeds (UI_SPEC 6); the document
                    // clears saveError on the next successful save.
                    position: Kirigami.InlineMessage.Position.Header
                    Layout.fillWidth: true
                    visible: root.document && root.document.saveError.length > 0
                    type: Kirigami.MessageType.Error
                    text: root.document
                          ? qsTr("Could not save: %1. Your changes are still here.").arg(root.document.saveError)
                          : ""
                    actions: [
                        Kirigami.Action {
                            icon.name: "document-save"
                            text: qsTr("Try again")
                            onTriggered: root.shell && root.shell.saveSong()
                        },
                        Kirigami.Action {
                            icon.name: "document-save-as"
                            text: qsTr("Save as…")
                            onTriggered: root.shell && root.shell.openSaveAsDialog()
                        }
                    ]
                }

                Usdx.LineHeader {
                    Layout.fillWidth: true
                    document: root.document
                }

                Rectangle {
                    // The note sheet area: the sheet, ruler and lanes arrive
                    // with the viewport task P4.
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: Kirigami.Theme.backgroundColor
                }
            }
        }
    }

    Usdx.Metrics {
        id: metrics
    }
}
