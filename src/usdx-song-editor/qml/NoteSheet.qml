// About me: the note sheet area (UI_SPEC 3): the painted SheetBackground
// fills it with the ruler, the pitch rows, the lyrics lane, the gap and the
// waveform and activity lanes including the gutter (the lane labels are
// painted, not QML elements). On top sit the note delegates of the current
// line and its global neighbours (task P5-04, fed by the SongDocument
// NotesModel), the phrase end handles of the PhraseEndsModel (task P5-05,
// draggable through movePhraseEnd), the rubber band of a drag on the empty
// pitch area and the playhead. A click or drag in the ruler seeks live
// (UI_SPEC 3.4, user decision 2026-11-14). A drag on a note body previews
// through the SongDocument previewMove (the ghost outline shows the snapped
// result) and commits once on release, so one drag is one undo step (plan
// 8.4); an edge drag resizes through previewResize and resizeNote. A
// refused edit arrives through the document's editRefused signal, which
// EditorPage routes to a passive notification. Wheel, zoom and pan arrive
// when the sheet gets a real viewport bridge object (deferred from P5-04).

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Rectangle {
    id: root

    // The song document, the player and whether a song is open; EditorPage
    // binds them to its own properties.
    property Usdx.SongDocument document
    property Usdx.Player player
    property bool songOpen: false

    // The active body-drag preview: while one selected note is dragged,
    // every selected note shifts by the committed deltas (plan 8.4: the
    // commit moves the whole selection by one delta).
    property bool moveActive: false
    property real moveDBeat: 0
    property int moveDPitch: 0

    Layout.fillWidth: true
    Layout.fillHeight: true
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false
    color: Kirigami.Theme.backgroundColor

    Usdx.Metrics {
        id: metrics
    }

    Usdx.Palette {
        id: appPalette
    }

    // The window and header colour sets, for the lane and ruler backgrounds
    // (UI_SPEC 2.3).
    Item {
        id: windowColors

        Kirigami.Theme.colorSet: Kirigami.Theme.Window
        Kirigami.Theme.inherit: false
    }

    Item {
        id: headerColors

        Kirigami.Theme.colorSet: Kirigami.Theme.Header
        Kirigami.Theme.inherit: false
    }

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

    Usdx.SheetBackground {
        id: sheet

        objectName: "sheetBackground"
        anchors.fill: parent
        // The song timing the beat axis maps through.
        bpm: root.document ? root.document.bpm : 0
        gapMs: root.document ? root.document.gapMs : 0
        // The note data of the current line and its global neighbours,
        // which SongDocument computes.
        sheetNoteStarts: root.document ? root.document.sheetNoteStarts : []
        sheetNoteTexts: root.document ? root.document.sheetNoteTexts : []
        sheetNoteCurrents: root.document ? root.document.sheetNoteCurrents : []
        sheetPitchLow: root.document ? root.document.sheetPitchLow : -1
        sheetPitchHigh: root.document ? root.document.sheetPitchHigh : -1
        startMs: root.document ? root.document.viewStartMs : 0
        endMs: root.document ? root.document.viewEndMs : 0
        // The selected track draws the waveform, the vocals track the
        // activity (UI_SPEC 3.7). filePath takes part so a new song re-reads
        // the paths.
        audioPath: root.songOpen && root.document.filePath.length > 0
                   ? root.document.audioPath(root.player ? root.player.track : 0) : ""
        vocalsPath: root.songOpen && root.document.filePath.length > 0
                    ? root.document.audioPath(1) : ""
        // The lane stack and the gutter (UI_SPEC 3.1).
        gutterWidth: metrics.sheetGutterWidth
        rulerHeight: metrics.sheetRulerHeight
        lyricsHeight: metrics.sheetLyricsHeight
        gapHeight: metrics.sheetGapHeight
        waveformHeight: metrics.waveformLaneHeight
        activityHeight: metrics.activityLaneHeight
        showLyricsLane: true
        trackName: [qsTr("Mix"), qsTr("Vocals"), qsTr("Instr.")][root.player ? root.player.track : 0]
        // The theme colours (UI_SPEC 2.3); the grid lines interpolate
        // between the view background and text.
        viewBackgroundColor: root.Kirigami.Theme.backgroundColor
        alternateColor: root.Kirigami.Theme.alternateBackgroundColor
        headerBackgroundColor: headerColors.Kirigami.Theme.backgroundColor
        windowBackgroundColor: windowColors.Kirigami.Theme.backgroundColor
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
        // The sheet fonts (UI_SPEC 2.2, 3.3, 3.6), complete because the
        // bridge cannot change size or weight.
        smallFont: Kirigami.Theme.smallFont
        smallDemiFont: Qt.font({
            family: Kirigami.Theme.smallFont.family,
            pointSize: Kirigami.Theme.smallFont.pointSize,
            weight: Font.DemiBold
        })
        lyricsFont: Qt.font({
            family: Kirigami.Theme.defaultFont.family,
            pointSize: 9,
            weight: Font.DemiBold
        })
        lyricsNeighbourFont: Qt.font({
            family: Kirigami.Theme.defaultFont.family,
            pointSize: 9
        })

        // "Reading the waveform… 42 %" with its progress bar, over the
        // waveform lane at the sheet's bottom.
        ColumnLayout {
            objectName: "waveformLoading"
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height - metrics.activityLaneHeight
               - metrics.waveformLaneHeight
               + (metrics.waveformLaneHeight - height) / 2
            visible: sheet.laneStatus === Usdx.SheetBackground.LaneStatus.Loading
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Reading the waveform… %1 %").arg(Math.round(sheet.analysisProgress * 100))
                color: root.Kirigami.Theme.disabledTextColor
            }

            // The Breeze bar draws its 6 px track centred in its own
            // implicit height; a forced 6 px item height would draw nothing.
            QQC2.ProgressBar {
                Layout.preferredWidth: metrics.waveformProgressWidth
                from: 0
                to: 1
                value: sheet.analysisProgress
            }
        }

        QQC2.Label {
            objectName: "waveformNoAudio"
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height - metrics.activityLaneHeight
               - metrics.waveformLaneHeight
               + (metrics.waveformLaneHeight - height) / 2
            visible: sheet.laneStatus === Usdx.SheetBackground.LaneStatus.NoAudio
            text: qsTr("No audio")
            color: root.Kirigami.Theme.disabledTextColor
        }

        QQC2.Label {
            objectName: "waveformNoVocals"
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height - metrics.activityLaneHeight - height
            visible: sheet.laneStatus === Usdx.SheetBackground.LaneStatus.Ready
                     && !sheet.showActivity
            text: qsTr("No vocals track: vocal activity and the coverage check are off. Vocals is disabled in the track list.")
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: root.Kirigami.Theme.disabledTextColor
        }
    }

    // The bottom of the pitch rows: everything under it is the lyrics lane,
    // the gap strip and the waveform and activity lanes (UI_SPEC 3.1).
    readonly property real pitchBottom:
        sheet.height - sheet.lyricsHeight - sheet.gapHeight
        - sheet.waveformHeight - sheet.activityHeight

    // The band target and the note delegates over the painted background.
    // A press on a note goes to the note's handlers, a press on the empty
    // pitch area to the rubber band handler (the ruler area is excluded by
    // the press y check). The clip area starts at the gutter's right edge
    // and spans the sheet: notes left of the view start cut at the gutter
    // edge like the painted labels (UI_SPEC 3.5); the delegates hide
    // themselves when their pitch row lies outside the visible rows.
    Item {
        id: pitchArea

        anchors.fill: parent

        // The rubber band (UI_SPEC 3.5): highlightColour at 15 % opacity
        // fill, 1 px border; a release selects every note intersecting the
        // band and replaces the previous selection.
        property bool bandValid: false

        DragHandler {
            id: bandDrag

            acceptedButtons: Qt.LeftButton
            target: null

            onActiveChanged: {
                if (active) {
                    pitchArea.bandValid = point.pressPosition.y >= sheet.rulerHeight
                                          && point.pressPosition.y <= root.pitchBottom;
                    band.visible = pitchArea.bandValid;
                    if (pitchArea.bandValid) {
                        band.x = point.pressPosition.x;
                        band.y = point.pressPosition.y;
                        band.width = 0;
                        band.height = 0;
                    }
                } else {
                    if (pitchArea.bandValid && band.width > 2 && band.height > 2) {
                        root.document.selectRange(
                            sheet.xToBeat(band.x),
                            sheet.xToBeat(band.x + band.width),
                            sheet.yToPitch(band.y),
                            sheet.yToPitch(band.y + band.height));
                    }
                    band.visible = false;
                    pitchArea.bandValid = false;
                }
            }

            onTranslationChanged: {
                if (!pitchArea.bandValid) {
                    return;
                }
                band.x = Math.min(point.pressPosition.x, point.position.x);
                band.y = Math.min(point.pressPosition.y, point.position.y);
                band.width = Math.abs(point.position.x - point.pressPosition.x);
                band.height = Math.abs(point.position.y - point.pressPosition.y);
            }
        }

        Item {
            id: clipArea

            x: sheet.gutterWidth
            y: 0
            width: sheet.width - sheet.gutterWidth
            height: sheet.height
            clip: true

            Repeater {
                model: root.document ? root.document.notesModel : null

                Usdx.NoteDelegate {
                    noteSheet: root
                    bg: sheet
                }
            }

            // The end-of-phrase markers (UI_SPEC 3.5): dashed lines with
            // square caps, draggable through PhraseEndHandle.
            Repeater {
                model: root.document ? root.document.phraseEndsModel : null

                Usdx.PhraseEndHandle {
                    noteSheet: root
                    bg: sheet
                }
            }
        }

        Rectangle {
            id: band

            visible: false
            z: 10
            color: Qt.alpha(Kirigami.Theme.highlightColor, 0.15)
            border.color: Kirigami.Theme.highlightColor
            border.width: 1
        }
    }

    // The ruler seek strip (UI_SPEC 3.4, task P5-05): a click and a drag
    // both move the playhead live (user decision 2026-11-14). It sits above
    // the pitch area so a press in the ruler never starts a rubber band or
    // a note drag; the strip is enabled only with an open song, a player
    // and a mapped beat axis.
    Item {
        id: rulerStrip

        x: 0
        y: 0
        width: parent.width
        height: sheet.rulerHeight
        enabled: root.songOpen && root.player !== null
                 && root.document !== null && root.document.bpm > 0
        z: 6

        function seekTo(sheetX: real) {
            const beat = sheet.xToBeat(sheetX);
            root.player.seekMs(Math.max(
                0, root.document.gapMs + beat * 15000 / root.document.bpm));
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            hoverEnabled: false
            cursorShape: Qt.PointingHandCursor

            onPressed: (mouse) => rulerStrip.seekTo(mouse.x)
            onPositionChanged: (mouse) => {
                if (mouse.buttons & Qt.LeftButton) {
                    rulerStrip.seekTo(mouse.x);
                }
            }
        }
    }

    // The playhead (UI_SPEC 3.5): a 2 px line in the negative colour from
    // the ruler top to the activity lane's bottom, with a downward triangle
    // at the ruler top. Its x follows the player's position mapped through
    // the beat axis the sheet shows.
    Item {
        objectName: "playhead"
        visible: root.songOpen && root.player !== null
                 && root.document !== null && root.document.bpm > 0
        x: root.player !== null && root.document !== null
           ? sheet.gutterWidth
             + ((root.player.positionMs - root.document.gapMs)
                * root.document.bpm / 15000 - sheet.startBeat)
               * sheet.pxPerBeat - 1
           : -1
        y: 0
        width: 2
        height: parent.height

        Rectangle {
            width: 2
            height: parent.height
            color: root.Kirigami.Theme.negativeTextColor
        }

        Canvas {
            x: -5
            width: 12
            height: 8
            onPaint: {
                const context = getContext("2d");
                context.fillStyle = root.Kirigami.Theme.negativeTextColor;
                context.beginPath();
                context.moveTo(0, 0);
                context.lineTo(12, 0);
                context.lineTo(6, 8);
                context.closePath();
                context.fill();
            }
        }
    }
}
