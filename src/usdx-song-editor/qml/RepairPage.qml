// About me: the repair timing page (UI_SPEC 4.6, task P7-08), pushed from
// the editor with the Repair timing button or Ctrl+R. The toolbar carries
// the way back to the editor, the exclusive mode buttons (anchor + stretch,
// block move/scale, tap-along; the block and tap modes arrive with steps
// 3d and 3e), Play, the pin button (pins the current line's start at the
// playhead), the track combo and the export button. The overview strip is
// the SheetBackground in overview mode (UI_SPEC 4.6): one block per line of
// the current voice coloured by the health verdict, the racing underlays,
// the vocal activity lane, the anchor flags and the time labels; a click
// makes the line under the pointer current and scrolls the sheet there.
// Below sit the three columns (lines pane, repair sheet, anchor panel) and
// the status bar with the section's old and previewed end, the one-undo
// note and the preview pill.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Kirigami.Page {
    id: root

    // The document, the player, the repair bridge and the application
    // window; Main.qml binds them (the shell owns the dialogs).
    property Usdx.SongDocument document
    property Usdx.Player player
    property Usdx.Repair repair
    property QtObject shell

    readonly property bool songOpen: root.document !== null && root.document.isLoaded

    // The whole-song duration the overview strip draws over: the audio's
    // length when known, else the song's end.
    readonly property real overviewDurationMs:
        root.player && root.player.durationMs > 0
        ? root.player.durationMs
        : (root.document ? root.document.endMs : 0)

    title: qsTr("Repair timing")

    Usdx.Metrics {
        id: metrics
    }

    Usdx.Palette {
        id: appPalette
    }

    // The mode buttons set the repair bridge's mode (0 anchor, 1 block,
    // 2 tap); the checked state follows the bridge so a mode change from
    // elsewhere keeps the buttons honest.
    function setMode(mode) {
        if (root.repair) {
            root.repair.setMode(mode);
        }
    }

    // lineAt maps a strip x in pixels to the global line whose span
    // contains the time, or -1.
    function lineAt(x) {
        if (!root.repair || root.overviewDurationMs <= 0 || strip.width <= 0) {
            return -1;
        }
        const ms = x / strip.width * root.overviewDurationMs;
        const lines = root.repair.overviewLines;
        for (let index = 0; index < lines.length; index++) {
            const row = lines[index];
            if (row.startMs <= ms && ms < row.endMs) {
                return row.line;
            }
        }
        return -1;
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The toolbar (UI_SPEC 4.6).
        RowLayout {
            objectName: "repairToolbar"

            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            QQC2.ToolButton {
                objectName: "repairBackButton"
                icon.name: "go-previous"
                text: qsTr("Editor")
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: root.shell && root.shell.showEditorPage()
                QQC2.ToolTip.text: qsTr("Back to the editor")
                QQC2.ToolTip.visible: hovered
            }

            Kirigami.Separator {}

            QQC2.ToolButton {
                objectName: "modeAnchorButton"
                checkable: true
                autoExclusive: true
                checked: !root.repair || root.repair.mode === 0
                text: qsTr("Anchor + stretch")
                display: QQC2.AbstractButton.TextBesideIcon
                onToggled: if (checked) root.setMode(0)
            }

            QQC2.ToolButton {
                objectName: "modeBlockButton"
                checkable: true
                autoExclusive: true
                checked: root.repair && root.repair.mode === 1
                text: qsTr("Block move/scale")
                display: QQC2.AbstractButton.TextBesideIcon
                onToggled: if (checked) root.setMode(1)
            }

            QQC2.ToolButton {
                objectName: "modeTapButton"
                checkable: true
                autoExclusive: true
                checked: root.repair && root.repair.mode === 2
                text: qsTr("Tap-along")
                display: QQC2.AbstractButton.TextBesideIcon
                onToggled: if (checked) root.setMode(2)
            }

            Kirigami.Separator {}

            QQC2.ToolButton {
                objectName: "repairPlayButton"
                icon.name: "media-playback-start"
                text: qsTr("Play")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.songOpen && root.player
                onClicked: root.player.toggle()
            }

            QQC2.ToolButton {
                objectName: "pinLineStartButton"
                icon.name: "pin"
                text: qsTr("Pin line start here")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.songOpen && root.player && root.repair
                         && root.repair.mode === 0
                onClicked: root.repair.pinCurrentLineAt(root.player.positionMs)
                QQC2.ToolTip.text: qsTr("Pin the current line's start at the playhead")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.Label {
                objectName: "repairTrackLabel"
                visible: root.songOpen
                text: qsTr("Track")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: root.Kirigami.Theme.disabledTextColor
            }

            QQC2.ComboBox {
                id: repairTrackCombo

                objectName: "repairTrackCombo"
                visible: root.songOpen
                textRole: "label"
                model: [
                    { label: qsTr("Mix"), available: root.document ? root.document.audioPresent : false },
                    { label: qsTr("Vocals"), available: root.document ? root.document.hasVocals : false },
                    { label: qsTr("Instrumental"), available: root.document ? root.document.hasInstrumental : false }
                ]
                enabled: root.document && root.document.canTrack
                currentIndex: root.player ? root.player.track : 0
                onActivated: function(index) {
                    root.player.setTrack(index);
                    repairTrackCombo.currentIndex = Qt.binding(function() {
                        return root.player ? root.player.track : 0;
                    });
                }
                delegate: QQC2.ItemDelegate {
                    required property var modelData
                    required property int index

                    text: modelData.label
                    highlighted: repairTrackCombo.highlightedIndex === index
                    enabled: modelData.available
                    width: repairTrackCombo.width
                }
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.ToolButton {
                objectName: "exportAnchorsButton"
                icon.name: "document-export"
                text: qsTr("Export anchors.txt…")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.songOpen && root.repair
                onClicked: root.shell && root.shell.showNotYetAvailable()
            }
        }

        // The overview strip (UI_SPEC 4.6): the SheetBackground in
        // overview mode with the caption above, 8 px padding and a
        // 1 px border below.
        Rectangle {
            objectName: "repairOverviewStrip"

            Layout.fillWidth: true
            implicitHeight: stripCaption.implicitHeight + strip.height
                            + 2 * 8 + 1
            color: root.Kirigami.Theme.backgroundColor

            QQC2.Label {
                id: stripCaption

                objectName: "repairOverviewCaption"
                x: 8
                y: 8
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: root.Kirigami.Theme.disabledTextColor
                text: qsTr("Whole song: one block per line, the vocal activity of the #VOCALS track below")
            }

            Item {
                id: strip

                objectName: "repairOverviewItem"
                x: 8
                y: stripCaption.implicitHeight + 8
                width: parent.width - 2 * 8
                height: 64

                Usdx.SheetBackground {
                    id: overviewSheet

                    objectName: "repairOverviewBackground"
                    anchors.fill: parent
                    overview: true
                    audioPath: root.songOpen && root.player && root.document
                               ? root.document.audioPath(root.player.track) : ""
                    vocalsPath: root.songOpen && root.document
                                ? root.document.audioPath(1) : ""
                    overviewDurationMs: root.overviewDurationMs
                    overviewLines: root.repair ? root.repair.overviewLines : []
                    overviewRacing: root.repair ? root.repair.racingSections : []
                    overviewAnchors: root.repair ? root.repair.overviewAnchors : []
                    viewBackgroundColor: root.Kirigami.Theme.backgroundColor
                    headerBackgroundColor: root.Kirigami.Theme.backgroundColor
                    windowBackgroundColor: root.Kirigami.Theme.backgroundColor
                    textColor: root.Kirigami.Theme.textColor
                    disabledTextColor: root.Kirigami.Theme.disabledTextColor
                    positiveColor: root.Kirigami.Theme.positiveTextColor
                    neutralColor: root.Kirigami.Theme.neutralTextColor
                    negativeColor: root.Kirigami.Theme.negativeTextColor
                    anchorColor: appPalette.anchor
                    anchorTextColor: appPalette.anchorText
                    waveformColor: appPalette.waveform
                    activityColor: appPalette.vocalActivity
                    smallFont: Kirigami.Theme.smallFont
                    smallDemiFont: Qt.font({
                        family: Kirigami.Theme.smallFont.family,
                        pointSize: Kirigami.Theme.smallFont.pointSize,
                        weight: Font.DemiBold
                    })

                    // A click makes the line under the pointer current and
                    // scrolls the sheet there (the sheet follows the
                    // document's shared view).
                    MouseArea {
                        objectName: "repairOverviewMouse"
                        anchors.fill: parent
                        enabled: root.songOpen && root.document
                        onClicked: function(mouse) {
                            const line = root.lineAt(mouse.x);
                            if (line >= 0) {
                                root.document.goToLine(line);
                            }
                        }
                    }
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 1
                color: root.Kirigami.Theme.alternateBackgroundColor
            }
        }

        // The three columns (UI_SPEC 4.6): lines pane 260 px, the repair
        // sheet, the anchor panel 280 px.
        RowLayout {
            objectName: "repairColumns"

            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            Item {
                objectName: "repairLinesPaneHost"

                Layout.preferredWidth: 260
                Layout.fillHeight: true

                Usdx.LinesPane {
                    anchors.fill: parent
                    document: root.document
                    linesModel: root.document ? root.document.linesModel : null
                    visible: root.repair && root.repair.mode === 0
                }
            }

            Usdx.RepairSheet {
                objectName: "repairSheetHost"

                Layout.fillWidth: true
                Layout.fillHeight: true
                document: root.document
                player: root.player
                repair: root.repair
                songOpen: root.songOpen
            }

            Usdx.AnchorPanel {
                objectName: "anchorPanelHost"

                Layout.preferredWidth: 280
                Layout.fillHeight: true
                visible: root.repair && root.repair.mode === 0
                repair: root.repair
            }
        }

        // The status bar (UI_SPEC 4.6): the section's old and previewed
        // end, the one-undo note and the preview pill.
        RowLayout {
            objectName: "repairStatusBar"

            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                objectName: "repairSectionStatus"
                visible: root.repair && root.repair.sectionStartMs >= 0
                elide: Text.ElideRight
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                text: root.repair && root.repair.sectionStartMs >= 0
                      ? qsTr("Section %1 - %2 becomes %3 - %4")
                          .arg(Time.longTime(root.repair.sectionStartMs))
                          .arg(Time.longTime(root.repair.sectionEndMs))
                          .arg(Time.longTime(root.repair.sectionStartMs))
                          .arg(Time.longTime(root.repair.sectionNewEndMs))
                      : ""
            }

            QQC2.Label {
                objectName: "repairUndoNote"
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: root.Kirigami.Theme.disabledTextColor
                text: qsTr("Apply is one undo step")
            }

            Item {
                Layout.fillWidth: true
            }

            Rectangle {
                objectName: "repairPreviewPill"
                visible: root.repair && root.repair.hasPreview
                radius: height / 2
                implicitWidth: previewPillLabel.implicitWidth + 2 * Kirigami.Units.smallSpacing
                implicitHeight: previewPillLabel.implicitHeight + 4
                color: root.Kirigami.Theme.highlightColor

                QQC2.Label {
                    id: previewPillLabel

                    anchors.centerIn: parent
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.Kirigami.Theme.highlightedTextColor
                    text: qsTr("preview")
                }
            }
        }
    }
}
