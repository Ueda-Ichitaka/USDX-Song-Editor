// About me: the repair page's sheet column (UI_SPEC 4.6, task P7-08). It
// reuses the painted SheetBackground without the lyrics lane and the
// editor's NoteDelegate for the notes, and adds the repair-only overlays:
// the dashed ghost outlines where the notes land after Apply (from the
// Repair bridge's previewNotes, shown while the Preview toggle is on), the
// anchor lines with their 22 x 14 px flags at the pinned times through the
// full height and the dotted playhead. The block move/scale mode (UI_SPEC
// 4.7) hides the lyrics, waveform and activity lanes and uses the full
// height for the notes, and the selection band with the 2 px edges and the
// 8 x 32 px handles follows the block's new times. The column header shows
// the section's line range, the timing health pill, the vocal coverage of
// the section and the Preview toggle; under the sheet the caption explains
// the solid notes, the dashed ghosts and the flags. The sheet mirrors the
// document's shared view, so the overview strip and the lines pane move it.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

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

    // The notes of racing lines draw in the negative colour (UI_SPEC 4.6):
    // the Repair bridge's racingRows as a lookup by row id, which the
    // delegates read through racingNote.
    readonly property var racingRowSet: {
        const set = {};
        const rows = root.repair ? root.repair.racingRows : [];
        for (let index = 0; index < rows.length; index++) {
            set[rows[index]] = true;
        }
        return set;
    }

    function racingNote(noteId) {
        return root.racingRowSet[noteId] === true;
    }

    // The toolbar's Snap toggle (UI_SPEC 4.7): Beat by default (user
    // decision 2026-11-17); Shift turns the snapping off temporarily
    // while an edge is dragged.
    property bool snapBeat: true

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

    // beat_to_ms is msToBeat's inverse: the band drags map a beat
    // position back into the audio milliseconds the block fields show.
    function beatToMs(beat) {
        return document && document.bpm > 0
            ? beat * 15000.0 / document.bpm + document.gapMs : beat;
    }

    // clipX maps an absolute beat to the x the clip area uses (it starts
    // at the gutter's right edge).
    function clipX(beat) {
        return (beat - sheet.startBeat) * sheet.pxPerBeat;
    }

    // The column header and the caption belong to the anchor mode; the
    // block mode's sheet has neither (UI_SPEC 4.7 mockup).
    readonly property bool anchorMode: !root.repair || root.repair.mode === 0

    // The block mode's time view: the union of the block's old and new
    // spans with 10 % of it on both sides.
    readonly property bool blockView:
        root.repair !== null && root.repair.mode === 1 && root.repair.blockStartMs >= 0
        && root.repair.blockNewEndMs >= 0
    readonly property real blockSpanStartMs: blockView
        ? Math.min(root.repair.blockStartMs, root.repair.blockNewStartMs) : 0
    readonly property real blockSpanEndMs: blockView
        ? Math.max(root.repair.blockEndMs, root.repair.blockNewEndMs) : 0
    readonly property real blockViewStartMs:
        Math.max(0, blockSpanStartMs - (blockSpanEndMs - blockSpanStartMs) * 0.1)
    readonly property real blockViewEndMs:
        blockSpanEndMs + (blockSpanEndMs - blockSpanStartMs) * 0.1

    // Column header (UI_SPEC 4.6): the section's line range, the timing
    // health pill, the vocal coverage and the Preview toggle.
    Rectangle {
        id: header

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        color: root.Kirigami.Theme.backgroundColor
        visible: root.anchorMode
        height: visible ? headerRow.implicitHeight + 2 * metrics.columnPaddingVertical : 0

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
        anchors.bottom: caption.visible ? caption.top : parent.bottom

        Usdx.SheetBackground {
            id: sheet

            objectName: "repairSheetBackground"
            overview: false
            anchors.fill: parent
            bpm: root.document ? root.document.bpm : 0
            gapMs: root.document ? root.document.gapMs : 0
            sheetNoteStarts: root.document ? root.document.sheetNoteStarts : []
            sheetNoteTexts: root.document ? root.document.sheetNoteTexts : []
            sheetNoteCurrents: root.document ? root.document.sheetNoteCurrents : []
            sheetPitchLow: root.document ? root.document.sheetPitchLow : -1
            sheetPitchHigh: root.document ? root.document.sheetPitchHigh : -1
            // The time view: the current line's view, or in the block mode
            // the block's old and new span with 10 % on both sides, so the
            // band, its new edges and the ghosts are in view (UI_SPEC 4.7).
            startMs: root.blockView ? root.blockViewStartMs
                     : (root.document ? root.document.viewStartMs : 0)
            endMs: root.blockView ? root.blockViewEndMs
                   : (root.document ? root.document.viewEndMs : 0)
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
            // The block mode uses the full height for the notes: no
            // lyrics lane, no waveform lanes (UI_SPEC 4.7).
            lyricsHeight: root.repair && root.repair.mode === 1
                          ? 0 : metrics.sheetLyricsHeight
            gapHeight: root.repair && root.repair.mode === 1
                       ? 0 : metrics.sheetGapHeight
            waveformHeight: root.repair && root.repair.mode === 1
                            ? 0 : metrics.waveformLaneHeight
            activityHeight: root.repair && root.repair.mode === 1
                            ? 0 : metrics.activityLaneHeight
            showLyricsLane: false
            // The block mode's sheet has no waveform lane, so no lane label.
            trackName: root.repair && root.repair.mode === 1 ? ""
                       : [qsTr("Mix"), qsTr("Vocals"), qsTr("Instr.")][root.player ? root.player.track : 0]
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

            // A mode change only triggers update() for the height
            // properties; the pitch rows need an explicit re-sync when
            // the block mode hides the lanes.
            Connections {
                target: root.repair
                function onModeChanged() {
                    sheet.syncView();
                    sheet.syncLyrics();
                }
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

            // The block selection band (UI_SPEC 4.7): 2 px edges and two
        // 8 x 32 px handles, radius 2, textColor, centred vertically on
        // the edges. The band follows the block's NEW times (they start
        // at the block's old span, so a fresh block covers its own
        // notes); the drags commit on release like the editor's note
        // drag (plan 8.4).
        Item {
            id: blockBand

            objectName: "blockBand"

            // Above the note delegates (the selected one takes z 2): the
            // band's body and edge drags must win the presses over the
            // editor's note drag areas in the block mode.
            z: 50

            readonly property bool hasBlock:
                root.repair && root.repair.blockNewStartMs >= 0
                && root.repair.blockNewEndMs >= 0

            // The drag state: while a drag runs, the band shows the
            // locally computed times (localStartMs/localEndMs); the
            // release commits them through setBlockTimes, so the
            // preview's ghosts follow afterwards (the note drag's
            // commit-on-release pattern, plan 8.4). 1 = body move,
            // 2 = start edge, 3 = end edge.
            property int dragMode: 0
            property real pressX: 0
            property real baseStartMs: 0
            property real baseEndMs: 0
            property real localStartMs: -1
            property real localEndMs: -1

            readonly property real msPerBeat:
                root.document && root.document.bpm > 0
                ? 15000.0 / root.document.bpm : 0.0

            readonly property real shownStartMs:
                dragMode !== 0 ? localStartMs
                : (root.repair ? root.repair.blockNewStartMs : -1)
            readonly property real shownEndMs:
                dragMode !== 0 ? localEndMs
                : (root.repair ? root.repair.blockNewEndMs : -1)

            // snapOn reports whether the drag's modifier set allows
            // snapping: the toolbar's Beat toggle, and Shift only turns
            // the snapping off (UI_SPEC 4.7).
            function snapOn(modifiers) {
                return root.snapBeat && !(modifiers & Qt.ShiftModifier);
            }

            // pressAt records the pointer's x in the clip area's
            // coordinates: the band moves with the drag, so the MouseArea
            // locals shift under the pointer and the delta must be
            // measured in a coordinate space that stands still.
            function pressAt(area, mouse) {
                pressX = area.mapToItem(clipArea, mouse.x, mouse.y).x;
            }

            // deltaPx returns the pointer's x travel since the press, in
            // the clip area's coordinates.
            function deltaPx(area, mouse) {
                return area.mapToItem(clipArea, mouse.x, mouse.y).x - pressX;
            }

            // edgeMs maps a drag position into a snapped edge time: the
            // base edge's beat plus the pixel delta in beats, rounded to
            // whole beats while snapping.
            function edgeMs(baseMs, delta, modifiers) {
                let beat = root.msToBeat(baseMs) + sheet.dragDeltaBeats(delta);
                if (snapOn(modifiers)) {
                    beat = Math.round(beat);
                }
                return root.beatToMs(beat);
            }

            readonly property real startBeat:
                root.msToBeat(shownStartMs >= 0 ? shownStartMs : 0)
            readonly property real endBeat:
                root.msToBeat(shownEndMs >= 0 ? shownEndMs : 0)

            x: root.clipX(startBeat)
            y: 0
            width: Math.max(0, root.clipX(endBeat) - root.clipX(startBeat))
            height: sheet.height
            // The band belongs to the block mode (UI_SPEC 4.7); the block
            // keeps while another mode shows.
            visible: hasBlock && root.repair !== null && root.repair.mode === 1
                     && sheet.pxPerBeat > 0

            // The body drag moves the whole block by the delta (snapped
            // to whole beats while snapping), both times together.
            MouseArea {
                id: bodyDrag

                objectName: "blockBandBody"
                anchors.fill: parent
                cursorShape: Qt.SizeAllCursor
                // The note delegates' DragHandlers sit under the band and
                // would steal the grab once the drag threshold is passed;
                // the band owns the drag in the block mode.
                preventStealing: true

                onPressed: function(mouse) {
                    blockBand.baseStartMs = blockBand.shownStartMs;
                    blockBand.baseEndMs = blockBand.shownEndMs;
                    blockBand.localStartMs = blockBand.baseStartMs;
                    blockBand.localEndMs = blockBand.baseEndMs;
                    blockBand.pressAt(bodyDrag, mouse);
                    blockBand.dragMode = 1;
                }

                onPositionChanged: function(mouse) {
                    let deltaBeats =
                        sheet.dragDeltaBeats(blockBand.deltaPx(bodyDrag, mouse));
                    if (blockBand.snapOn(mouse.modifiers)) {
                        deltaBeats = Math.round(deltaBeats);
                    }
                    const deltaMs = deltaBeats * blockBand.msPerBeat;
                    blockBand.localStartMs = blockBand.baseStartMs + deltaMs;
                    blockBand.localEndMs = blockBand.baseEndMs + deltaMs;
                }

                onReleased: function(mouse) {
                    root.repair.setBlockTimes(
                        blockBand.localStartMs, blockBand.localEndMs);
                    blockBand.dragMode = 0;
                }
            }

            // The edge drags scale the block from the opposite edge:
            // the dragged edge follows the pointer (snapped), the other
            // edge keeps its time. 12 px wide hit areas centred on the
            // edges.
            MouseArea {
                id: startEdgeDrag

                objectName: "blockBandStartDrag"
                anchors.verticalCenter: parent.verticalCenter
                x: -6
                width: 12
                height: parent.height
                cursorShape: Qt.SplitHCursor
                preventStealing: true

                onPressed: function(mouse) {
                    blockBand.baseStartMs = blockBand.shownStartMs;
                    blockBand.baseEndMs = blockBand.shownEndMs;
                    blockBand.localStartMs = blockBand.baseStartMs;
                    blockBand.localEndMs = blockBand.baseEndMs;
                    blockBand.pressAt(startEdgeDrag, mouse);
                    blockBand.dragMode = 2;
                }

                onPositionChanged: function(mouse) {
                    const ms = blockBand.edgeMs(
                        blockBand.baseStartMs,
                        blockBand.deltaPx(startEdgeDrag, mouse), mouse.modifiers);
                    blockBand.localStartMs = Math.min(
                        ms, blockBand.localEndMs - blockBand.msPerBeat);
                }

                onReleased: function(mouse) {
                    root.repair.setBlockTimes(
                        blockBand.localStartMs, blockBand.localEndMs);
                    blockBand.dragMode = 0;
                }
            }

            MouseArea {
                id: endEdgeDrag

                objectName: "blockBandEndDrag"
                anchors.verticalCenter: parent.verticalCenter
                x: parent.width - 6
                width: 12
                height: parent.height
                cursorShape: Qt.SplitHCursor
                preventStealing: true

                onPressed: function(mouse) {
                    blockBand.baseStartMs = blockBand.shownStartMs;
                    blockBand.baseEndMs = blockBand.shownEndMs;
                    blockBand.localStartMs = blockBand.baseStartMs;
                    blockBand.localEndMs = blockBand.baseEndMs;
                    blockBand.pressAt(endEdgeDrag, mouse);
                    blockBand.dragMode = 3;
                }

                onPositionChanged: function(mouse) {
                    const ms = blockBand.edgeMs(
                        blockBand.baseEndMs,
                        blockBand.deltaPx(endEdgeDrag, mouse), mouse.modifiers);
                    blockBand.localEndMs = Math.max(
                        ms, blockBand.localStartMs + blockBand.msPerBeat);
                }

                onReleased: function(mouse) {
                    root.repair.setBlockTimes(
                        blockBand.localStartMs, blockBand.localEndMs);
                    blockBand.dragMode = 0;
                }
            }

            // The selection band's fill: the rubber band's style
            // (UI_SPEC 3.5), highlightColor at 15 %, under the edges.
            Rectangle {
                objectName: "blockBandFill"
                anchors.fill: parent
                z: -1
                color: Qt.alpha(root.Kirigami.Theme.highlightColor, 0.15)
            }

            // The 2 px edges in the highlight colour.
            Rectangle {
                objectName: "blockBandStartEdge"
                anchors.left: parent.left
                anchors.leftMargin: -1
                width: 2
                height: parent.height
                color: root.Kirigami.Theme.highlightColor
            }

            Rectangle {
                objectName: "blockBandEndEdge"
                anchors.right: parent.right
                anchors.rightMargin: -1
                width: 2
                height: parent.height
                color: root.Kirigami.Theme.highlightColor
            }

            // The 8 x 32 px handles, centred vertically on the edges.
            Rectangle {
                objectName: "blockBandStartHandle"
                anchors.verticalCenter: parent.verticalCenter
                x: -4
                width: 8
                height: 32
                radius: 2
                color: root.Kirigami.Theme.textColor
            }

            Rectangle {
                objectName: "blockBandEndHandle"
                anchors.verticalCenter: parent.verticalCenter
                x: parent.width - 4
                width: 8
                height: 32
                radius: 2
                color: root.Kirigami.Theme.textColor
            }
        }

        // The edge drag's arrow (UI_SPEC 4.7): while an edge is dragged, a
        // 2 px highlight arrow from the edge's old to its new position near
        // the top of the pitch rows, with "drag edge: 1:12.00 → 1:28.89"
        // (defaultFont DemiBold) above it.
        Item {
            id: edgeArrow

            readonly property bool active:
                blockBand.visible && (blockBand.dragMode === 2 || blockBand.dragMode === 3)
            readonly property real fromMs:
                blockBand.dragMode === 2 ? blockBand.baseStartMs : blockBand.baseEndMs
            readonly property real toMs:
                blockBand.dragMode === 2 ? blockBand.localStartMs : blockBand.localEndMs
            readonly property real fromX: root.clipX(root.msToBeat(fromMs))
            readonly property real toX: root.clipX(root.msToBeat(toMs))

            objectName: "blockEdgeArrow"
            visible: active && Math.abs(toX - fromX) >= 1
            z: 51
            x: Math.min(fromX, toX)
            y: sheet.rulerHeight + Kirigami.Units.largeSpacing + arrowLabel.implicitHeight
            width: Math.abs(toX - fromX)
            height: 10

            Rectangle {
                y: (parent.height - height) / 2
                width: parent.width
                height: 2
                color: root.Kirigami.Theme.highlightColor
            }

            // The arrow head at the new position.
            Canvas {
                id: arrowHead

                readonly property bool pointsRight: edgeArrow.toX >= edgeArrow.fromX
                readonly property color fill: root.Kirigami.Theme.highlightColor

                x: pointsRight ? parent.width - width : 0
                width: 8
                height: parent.height
                onPointsRightChanged: requestPaint()
                onFillChanged: requestPaint()
                onPaint: {
                    const context = getContext("2d");
                    context.reset();
                    context.fillStyle = arrowHead.fill;
                    context.beginPath();
                    if (arrowHead.pointsRight) {
                        context.moveTo(0, 0);
                        context.lineTo(width, height / 2);
                        context.lineTo(0, height);
                    } else {
                        context.moveTo(width, 0);
                        context.lineTo(0, height / 2);
                        context.lineTo(width, height);
                    }
                    context.closePath();
                    context.fill();
                }
            }

            QQC2.Label {
                id: arrowLabel

                objectName: "blockEdgeArrowLabel"
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.top
                font.weight: Font.DemiBold
                text: qsTr("drag edge: %1 \u2192 %2")
                      .arg(Time.longTime(edgeArrow.fromMs))
                      .arg(Time.longTime(edgeArrow.toMs))
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
        visible: root.anchorMode
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
