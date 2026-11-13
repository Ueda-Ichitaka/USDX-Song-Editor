// About me: the editor page (UI_SPEC 4.3): the toolbar with the playback and
// edit controls, the horizontal split of the lines pane and the editor
// column with the banners, the line header and the note sheet area, and the
// status bar in the page footer. Without a song the page shows the start
// screen with the recent songs list (UI_SPEC 4.1). The playback controls
// drive the Player bridge: Play and Play line run the audio, Loop replays
// the current line, and the shortcuts Space, P, L, V, [ and ] follow the
// plan's key table (8.6) as Shortcut elements (they move into the
// Kirigami.Actions of the menus when those arrive). The note sheet area
// shows the waveform and activity lanes of the painted SheetBackground for
// the current line's time view, with the loading, no-audio and no-vocals
// states of UI_SPEC 3.7; the ruler, pitch rows, notes and the inspector
// arrive with P5.

import QtQuick
import QtQuick.Controls
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.Page {
    id: root

    // The document the page edits, the player of the audio playback and the
    // application window, which owns the dialogs and the global drawer.
    property Usdx.SongDocument document
    property Usdx.Player player
    property QtObject shell

    // Loop on or off; the rangeDone signal of the player replays the line
    // while it is on (UI_SPEC 4.3).
    property bool loopLine: false

    // The playback speed choices of the toolbar combo.
    readonly property var speedRates: [0.5, 0.75, 1.0]

    // The line padding of the settings (0.3 s), hard-coded until the
    // Settings bridge arrives with P8-03.
    readonly property int linePaddingMs: 300

    // The start screen's toolbar keeps only Play of the playback controls
    // (UI_SPEC 4.1); the editor shows them all (UI_SPEC 4.3).
    readonly property bool songOpen: root.document !== null && root.document.isLoaded

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

            // The playback controls run the Player bridge (UI_SPEC 4.3).
            QQC2.ToolButton {
                id: playButton

                icon.name: root.player && root.player.playing
                           ? "media-playback-pause" : "media-playback-start"
                text: root.player && root.player.playing ? qsTr("Pause") : qsTr("Play")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canPlay
                checkable: true
                checked: root.player ? root.player.playing : false
                onClicked: {
                    root.player.toggle();
                    // The click flipped checked locally; the playback state
                    // stays the source of truth.
                    playButton.checked = Qt.binding(function() {
                        return root.player && root.player.playing;
                    });
                }
                QQC2.ToolTip.text: qsTr("Play or pause (Space)")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.ToolButton {
                objectName: "playLineButton"
                visible: root.songOpen
                icon.name: "view-media-lyrics"
                text: qsTr("Play line")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canPlayLine
                onClicked: root.playCurrentLine()
                QQC2.ToolTip.text: qsTr("Play the current line (P)")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.ToolButton {
                id: loopButton

                objectName: "loopButton"
                visible: root.songOpen
                icon.name: "media-playlist-repeat"
                text: qsTr("Loop")
                display: QQC2.AbstractButton.TextBesideIcon
                enabled: root.document && root.document.canLoop
                checkable: true
                checked: root.loopLine
                onToggled: root.loopLine = checked
                QQC2.ToolTip.text: qsTr("Loop the current line (L)")
                QQC2.ToolTip.visible: hovered
            }

            QQC2.Label {
                objectName: "trackLabel"
                visible: root.songOpen
                text: qsTr("Track")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.ComboBox {
                id: trackCombo

                objectName: "trackCombo"
                visible: root.songOpen
                // Every track is listed; the ones whose file is missing are
                // disabled (UI_SPEC 4.3). The choice drives the player.
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
                    trackCombo.currentIndex = Qt.binding(function() {
                        return root.player ? root.player.track : 0;
                    });
                }
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
                objectName: "speedLabel"
                visible: root.songOpen
                text: qsTr("Speed")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.ComboBox {
                id: speedCombo

                objectName: "speedCombo"
                visible: root.songOpen
                model: ["0.5x", "0.75x", "1x"]
                enabled: root.document && root.document.canSpeed
                currentIndex: root.player
                              ? root.speedRates.indexOf(root.player.rate) : 2
                onActivated: function(index) {
                    root.player.setRate(root.speedRates[index]);
                    speedCombo.currentIndex = Qt.binding(function() {
                        return root.player ? root.speedRates.indexOf(root.player.rate) : 2;
                    });
                }
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
        player: root.player
    }

    // The playhead follows while the player plays: every frame drains the
    // GStreamer bus events into the player's properties.
    FrameAnimation {
        running: root.player ? root.player.playing : false
        onTriggered: root.player.tick()
    }

    // The rangeDone signal replays the line while Loop is on (UI_SPEC 4.3).
    Connections {
        target: root.player
        function onRangeDone() {
            if (root.loopLine) {
                root.playCurrentLine();
            }
        }
    }

    // The playback shortcuts of the key table (plan 8.6).
    Shortcut {
        sequence: "Space"
        enabled: root.document && root.document.canPlay
        onActivated: root.player.toggle()
    }

    Shortcut {
        sequence: "P"
        enabled: root.document && root.document.canPlayLine
        onActivated: root.playCurrentLine()
    }

    Shortcut {
        sequence: "L"
        enabled: root.document && root.document.canLoop
        onActivated: root.loopLine = !root.loopLine
    }

    Shortcut {
        sequence: "V"
        enabled: root.document && root.document.canTrack
        onActivated: root.nextTrack()
    }

    Shortcut {
        sequence: "["
        enabled: root.document && root.document.canSpeed
        onActivated: root.changeSpeed(-1)
    }

    Shortcut {
        sequence: "]"
        enabled: root.document && root.document.canSpeed
        onActivated: root.changeSpeed(1)
    }

    // Esc stops the playback (plan 8.6); closing dialogs and leaving pages
    // keep their precedence.
    Shortcut {
        sequence: "Esc"
        enabled: root.player && root.player.playing
        onActivated: root.player.pause()
    }

    // playCurrentLine plays the padded range of the current line; the
    // rangeDone replay of the Loop uses it as well.
    function playCurrentLine() {
        if (!root.document || !root.player) {
            return;
        }
        root.player.playRange(root.document.currentLineStartMs - root.linePaddingMs,
                              root.document.currentLineEndMs + root.linePaddingMs);
    }

    // nextTrack moves to the following track, skipping the missing ones
    // and wrapping around (shortcut V, plan 8.6).
    function nextTrack() {
        if (!root.document || !root.player) {
            return;
        }
        const available = [root.document.audioPresent,
                           root.document.hasVocals,
                           root.document.hasInstrumental];
        for (let step = 1; step <= available.length; step++) {
            const candidate = (root.player.track + step) % available.length;
            if (available[candidate]) {
                root.player.setTrack(candidate);
                return;
            }
        }
    }

    // changeSpeed steps through the speed combo in the given direction.
    function changeSpeed(step) {
        if (!root.player) {
            return;
        }
        const index = root.speedRates.indexOf(root.player.rate);
        const next = Math.min(root.speedRates.length - 1, Math.max(0, index + step));
        root.player.setRate(root.speedRates[next]);
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

                // The note sheet area (UI_SPEC 3): view background; the
                // waveform and activity lanes sit at its bottom behind the
                // gutter. The ruler, pitch rows, notes and lyrics lane above
                // them arrive with the note sheet tasks (P5).
                Rectangle {
                    id: sheetArea

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Kirigami.Theme.colorSet: Kirigami.Theme.View
                    Kirigami.Theme.inherit: false
                    color: Kirigami.Theme.backgroundColor

                    // The window colour set, for the lane backgrounds.
                    Item {
                        id: windowColors

                        Kirigami.Theme.colorSet: Kirigami.Theme.Window
                        Kirigami.Theme.inherit: false
                    }

                    // The gutter label of the waveform lane: the track name.
                    QQC2.Label {
                        x: metrics.sheetGutterLabelMargin
                        width: metrics.sheetGutterWidth - metrics.sheetGutterLabelMargin
                        y: lanes.y
                        height: metrics.waveformLaneHeight
                        verticalAlignment: Text.AlignVCenter
                        text: [qsTr("Mix"), qsTr("Vocals"), qsTr("Instr.")][root.player ? root.player.track : 0]
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: Kirigami.Theme.disabledTextColor
                    }

                    Usdx.SheetBackground {
                        id: lanes

                        objectName: "sheetBackground"
                        anchors.left: parent.left
                        anchors.leftMargin: metrics.sheetGutterWidth
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        height: metrics.waveformLaneHeight + metrics.activityLaneHeight
                        startMs: root.document ? root.document.viewStartMs : 0
                        endMs: root.document ? root.document.viewEndMs : 0
                        // The selected track draws the waveform, the vocals
                        // track the activity (UI_SPEC 3.7). filePath takes
                        // part so a new song re-reads the paths.
                        audioPath: root.songOpen && root.document.filePath.length > 0
                                   ? root.document.audioPath(root.player ? root.player.track : 0) : ""
                        vocalsPath: root.songOpen && root.document.filePath.length > 0
                                    ? root.document.audioPath(1) : ""
                        waveformHeight: metrics.waveformLaneHeight
                        activityHeight: metrics.activityLaneHeight
                        waveformColor: appPalette.waveform
                        activityColor: appPalette.vocalActivity
                        laneColor: windowColors.Kirigami.Theme.backgroundColor
                        activityLaneColor: Kirigami.Theme.backgroundColor

                        // "Reading the waveform… 42 %" with its progress bar.
                        ColumnLayout {
                            objectName: "waveformLoading"
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: (metrics.waveformLaneHeight - height) / 2
                            visible: lanes.laneStatus === Usdx.SheetBackground.LaneStatus.Loading
                            spacing: Kirigami.Units.smallSpacing

                            QQC2.Label {
                                Layout.alignment: Qt.AlignHCenter
                                text: qsTr("Reading the waveform… %1 %").arg(Math.round(lanes.analysisProgress * 100))
                                color: Kirigami.Theme.disabledTextColor
                            }

                            // The Breeze bar draws its 6 px track centred
                            // in its own implicit height; a forced 6 px
                            // item height would draw nothing.
                            QQC2.ProgressBar {
                                Layout.preferredWidth: metrics.waveformProgressWidth
                                from: 0
                                to: 1
                                value: lanes.analysisProgress
                            }
                        }

                        QQC2.Label {
                            objectName: "waveformNoAudio"
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: (metrics.waveformLaneHeight - height) / 2
                            visible: lanes.laneStatus === Usdx.SheetBackground.LaneStatus.NoAudio
                            text: qsTr("No audio")
                            color: Kirigami.Theme.disabledTextColor
                        }

                        QQC2.Label {
                            objectName: "waveformNoVocals"
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: metrics.waveformLaneHeight - height
                            visible: lanes.laneStatus === Usdx.SheetBackground.LaneStatus.Ready
                                     && !lanes.showActivity
                            text: qsTr("No vocals track: vocal activity and the coverage check are off. Vocals is disabled in the track list.")
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                        }
                    }
                }
            }
        }
    }

    Usdx.Metrics {
        id: metrics
    }

    Usdx.Palette {
        id: appPalette
    }
}
