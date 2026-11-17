// About me: the repair page's sheet column (UI_SPEC 4.6, task P7-08). It
// reuses the painted SheetBackground without the lyrics lane and the
// editor's NoteDelegate for the notes, and adds the repair-only overlays:
// the dashed ghost outlines where the notes land after Apply (from the
// Repair bridge's previewNotes, shown while the Preview toggle is on), the
// anchor lines with their 22 x 14 px flags at the pinned times through the
// full height and the dotted playhead. The column header shows the
// section's line range, the timing health pill, the vocal coverage of the
// section and the Preview toggle; under the sheet the caption explains
// the solid notes, the dashed ghosts and the flags. The sheet mirrors the
// document's shared view, so the overview strip and the lines pane move it.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Rectangle {
    id: root

    // The song document, the player, the repair bridge and whether a song
    // is open; RepairPage binds them to its own properties.
    property Usdx.SongDocument document
    property Usdx.Player player
    property Usdx.Repair repair
    property bool songOpen: false

    // The header's preview toggle (UI_SPEC 4.6, checked by default).
    property bool preview: true

    // The host surface NoteDelegate reads: the repair sheet never moves
    // notes, so the drag deltas stay zero and the syllable field never
    // opens.
    property bool moveActive: false
    property real moveDBeat: 0
    property int moveDPitch: 0
    function editSyllable(noteId) {
    }

    Usdx.Metrics {
        id: metrics
    }

    Usdx.Palette {
        id: appPalette
    }

    color: Kirigami.Theme.backgroundColor

    // A resize recomputes the pitch rows and the view mapping from the
    // sheet item's new size.
    onWidthChanged: {
        sheet.syncView();
        sheet.syncLyrics();
    }
    onHeightChanged: {
        sheet.syncView();
        sheet.syncLyrics();
    }

    // The health colours (the lines pane's dot convention): 0 ok, 1
    // suspicious, 2 racing.
    readonly property color healthColor:
        repair && repair.sectionHealth === 2 ? Kirigami.Theme.negativeTextColor
        : repair && repair.sectionHealth === 1 ? Kirigami.Theme.neutralTextColor
        : Kirigami.Theme.positiveTextColor

    // ms_to_beat maps an audio millisecond through the song timing into
    // the beat axis the sheet draws.
    function msToBeat(ms) {
        return document && document.bpm > 0
            ? (ms - document.gapMs) * document.bpm / 15000.0 : 0;
    }

    // clipX maps an absolute beat to the x the clip area uses (it starts
    // at the gutter's right edge).
    function clipX(beat) {
        return (beat - sheet.startBeat) * sheet.pxPerBeat;
    }

    // Column header (UI_SPEC 4.6): the section's line range, the timing
    // health pill, the vocal coverage and the Preview toggle.
    Rectangle {
        id: header

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        color: root.Kirigami.Theme.backgroundColor
        height: headerRow.implicitHeight + 2 * metrics.columnPaddingVertical

        RowLayout {
            id: headerRow

            anchors.fill: parent
            anchors.leftMargin: metrics.columnPaddingHorizontal
            anchors.rightMargin: metrics.columnPaddingHorizontal
            anchors.topMargin: metrics.columnPaddingVertical
            anchors.bottomMargin: metrics.columnPaddingVertical
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                objectName: "repairLinesLabel"
                font.bold: true
                text: root.repair
                      && root.repair.sectionFirst >= 0
                    ? qsTr("Lines %1 - %2").arg(root.repair.sectionFirst + 1)
                                          .arg(root.repair.sectionLast + 1)
                    : root.document && root.document.isLoaded
                      ? qsTr("Line %1").arg(root.document.currentLine + 1)
                      : qsTr("No line")
            }

            Rectangle {
                objectName: "repairHealthPill"
                visible: root.repair && root.repair.sectionHealth >= 0
                radius: height / 2
                implicitWidth: pillLabel.implicitWidth + 2 * Kirigami.Units.smallSpacing
                implicitHeight: pillLabel.implicitHeight + 2
                color: root.healthColor

                QQC2.Label {
                    id: pillLabel

                    anchors.centerIn: parent
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.Kirigami.Theme.backgroundColor
                    text: root.repair && root.repair.sectionHealth === 2
                          ? qsTr("Racing")
                          : root.repair && root.repair.sectionHealth === 1
                            ? qsTr("Suspicious") : qsTr("Timing ok")
                }
            }

            QQC2.Label {
                objectName: "repairCoverage"
                text: root.repair && root.repair.sectionCoverage >= 0
                      ? qsTr("vocals heard during %1 % of the note time")
                          .arg(Math.round(root.repair.sectionCoverage * 100))
                      : ""
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: root.Kirigami.Theme.disabledTextColor
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.ToolButton {
                objectName: "repairPreviewToggle"
                checkable: true
                checked: root.preview
                icon.name: "view-visible"
                text: qsTr("Preview")
                onToggled: root.preview = checked
                QQC2.ToolTip.text: qsTr("Show where the notes land after Apply")
                QQC2.ToolTip.visible: hovered
            }
        }
    }

    // The sheet (UI_SPEC section 3 without the lyrics lane), below the
    // header and above the caption.
    Item {
        id: sheetArea

        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: caption.top

        Usdx.SheetBackground {
            id: sheet

            objectName: "repairSheetBackground"
            anchors.fill: parent
            overview: false
            bpm: root.document ? root.document.bpm : 0
            gapMs: root.document ? root.document.gapMs : 0
            sheetNoteStarts: root.document ? root.document.sheetNoteStarts : []
            sheetNoteTexts: root.document ? root.document.sheetNoteTexts : []
            sheetNoteCurrents: root.document ? root.document.sheetNoteCurrents : []
            sheetPitchLow: root.document ? root.document.sheetPitchLow : -1
            sheetPitchHigh: root.document ? root.document.sheetPitchHigh : -1
            startMs: root.document ? root.document.viewStartMs : 0
            endMs: root.document ? root.document.viewEndMs : 0
            songEndBeat: root.document ? root.document.songEndBeat : 0
            endMarkMs: root.document ? root.document.endMs : -1
            audioEndMs: root.player ? root.player.durationMs : 0
            audioPath: root.songOpen && root.document && root.document.filePath.length > 0
                       && root.player
                       ? root.document.audioPath(root.player.track) : ""
            vocalsPath: root.songOpen && root.document
                        && root.document.filePath.length > 0
                        ? root.document.audioPath(1) : ""
            gutterWidth: metrics.sheetGutterWidth
            rulerHeight: metrics.sheetRulerHeight
            lyricsHeight: metrics.sheetLyricsHeight
            gapHeight: metrics.sheetGapHeight
            waveformHeight: metrics.waveformLaneHeight
            activityHeight: metrics.activityLaneHeight
            showLyricsLane: false
            trackName: [qsTr("Mix"), qsTr("Vocals"), qsTr("Instr.")][root.player ? root.player.track : 0]
            viewBackgroundColor: root.Kirigami.Theme.backgroundColor
            alternateColor: root.Kirigami.Theme.alternateBackgroundColor
            headerBackgroundColor: root.Kirigami.Theme.backgroundColor
            windowBackgroundColor: root.Kirigami.Theme.backgroundColor
            gridLineColor: Kirigami.ColorUtils.linearInterpolation(
                root.Kirigami.Theme.backgroundColor,
                root.Kirigami.Theme.textColor, 0.10)
            strongGridLineColor: Kirigami.ColorUtils.linearInterpolation(
                root.Kirigami.Theme.backgroundColor,
                root.Kirigami.Theme.textColor, 0.20)
            textColor: root.Kirigami.Theme.textColor
            disabledTextColor: root.Kirigami.Theme.disabledTextColor
            waveformColor: appPalette.waveform
            activityColor: appPalette.vocalActivity
            positiveColor: Kirigami.Theme.positiveTextColor
            neutralColor: Kirigami.Theme.neutralTextColor
            negativeColor: Kirigami.Theme.negativeTextColor
            anchorColor: appPalette.anchor
            anchorTextColor: appPalette.anchorText
            smallFont: Kirigami.Theme.smallFont
            smallDemiFont: Qt.font({
                family: Kirigami.Theme.smallFont.family,
                pointSize: Kirigami.Theme.smallFont.pointSize,
                weight: Font.DemiBold
            })
            lyricsFont: Kirigami.Theme.smallFont
            lyricsNeighbourFont: Kirigami.Theme.smallFont

            Component.onCompleted: {
                syncView();
                syncLyrics();
            }
        }

        // The notes (the editor's delegate) and the repair overlays, right
        // of the gutter.
        Item {
            id: clipArea

            x: sheet.gutterWidth
            y: 0
            width: Math.max(0, sheet.width - sheet.gutterWidth)
            height: sheet.height
            clip: true

            Repeater {
                objectName: "repairNoteRepeater"
                model: root.document ? root.document.notesModel : null

                Usdx.NoteDelegate {
                    noteSheet: root
                    bg: sheet
                }
            }

            // The dashed ghost outlines (UI_SPEC 4.6): where the notes
            // land after Apply, while the Preview toggle is on.
            Repeater {
                objectName: "repairGhostRepeater"
                model: root.preview && root.repair ? root.repair.previewNotes : []

                Rectangle {
                    id: ghost

                    required property var modelData

                    readonly property real startBeat:
                        root.msToBeat(modelData.start)
                    readonly property real endBeat:
                        root.msToBeat(modelData.end)
                    readonly property real pitchRow:
                        sheet.rulerHeight
                        + (Math.round(sheet.pitchTop) - modelData.pitch)
                          * sheet.pxPerSemitone + 2

                    x: root.clipX(startBeat)
                    y: pitchRow
                    width: Math.max(2, (endBeat - startBeat) * sheet.pxPerBeat - 1.5)
                    height: sheet.pxPerSemitone - 4
                    radius: 3
                    color: "transparent"
                    border.width: 1
                    visible: sheet.pxPerBeat > 0

                    Shape {
                        anchors.fill: parent

                        ShapePath {
                            strokeColor: root.Kirigami.Theme.textColor
                            strokeWidth: 1
                            strokeStyle: ShapePath.DashLine
                            dashPattern: [4, 3]
                            capStyle: ShapePath.FlatCap
                            fillColor: "transparent"
                            startX: 0
                            startY: 0
                            PathLine { x: ghost.width; y: 0 }
                            PathLine { x: ghost.width; y: ghost.height }
                            PathLine { x: 0; y: ghost.height }
                            PathLine { x: 0; y: 0 }
                        }
                    }
                }
            }

            // The anchor lines and flags (UI_SPEC 4.6): 22 x 14 px flags
            // with the A1 badge at the top, the 2 px anchor line through
            // the full height.
            Repeater {
                objectName: "repairAnchorRepeater"
                model: root.repair ? root.repair.anchors : []

                Item {
                    id: anchorMark

                    required property var modelData

                    readonly property real beat:
                        root.msToBeat(modelData.target)

                    x: root.clipX(beat) - 1
                    y: 0
                    width: 2
                    height: sheet.height
                    visible: sheet.pxPerBeat > 0

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 2
                        height: parent.height
                        color: appPalette.anchor
                    }

                    Rectangle {
                        x: -11
                        y: 0
                        width: 22
                        height: 14
                        radius: 2
                        color: appPalette.anchor

                        QQC2.Label {
                            anchors.centerIn: parent
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            font.weight: Font.DemiBold
                            color: appPalette.anchorText
                            text: "A" + (anchorMark.modelData.badge + 1)
                        }
                    }
                }
            }

            // The dotted playhead (the editor sheet's), through the full
            // height.
            Item {
                id: playhead

                objectName: "repairPlayhead"
                visible: root.songOpen && root.player !== null
                         && root.document !== null && root.document.bpm > 0
                x: root.player !== null && root.document !== null
                   ? root.clipX(
                         (root.player.positionMs - root.document.gapMs)
                         * root.document.bpm / 15000.0) - 1
                   : -1
                y: 0
                width: 2
                height: parent.height

                Shape {
                    anchors.fill: parent

                    ShapePath {
                        strokeColor: root.Kirigami.Theme.negativeTextColor
                        strokeWidth: 2
                        strokeStyle: ShapePath.DashLine
                        dashPattern: [1, 2]
                        capStyle: ShapePath.FlatCap
                        fillColor: "transparent"
                        startX: 1
                        startY: 0

                        PathLine {
                            x: 1
                            y: playhead.height
                        }
                    }
                }
            }
        }
    }

    // The caption under the sheet (UI_SPEC 4.6).
    QQC2.Label {
        id: caption

        objectName: "repairSheetCaption"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: metrics.columnPaddingHorizontal
        anchors.rightMargin: metrics.columnPaddingHorizontal
        elide: Text.ElideRight
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        color: root.Kirigami.Theme.disabledTextColor
        text: qsTr("Solid red: the notes as they are now. Dashed: where they land after Apply. Flags: anchors.")
    }
}
