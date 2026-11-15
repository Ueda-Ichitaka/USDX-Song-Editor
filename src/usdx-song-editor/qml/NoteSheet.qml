// About me: the note sheet area (UI_SPEC 3): the painted SheetBackground
// fills it with the ruler, the pitch rows, the lyrics lane, the gap and the
// waveform and activity lanes including the gutter (the lane labels are
// painted, not QML elements). On top sit the note delegates of every line
// of the song (task P5-04, fed by the SongDocument NotesModel), the phrase
// end handles of the PhraseEndsModel (task P5-05, draggable through
// movePhraseEnd), the rubber band of a drag on the empty pitch area and the
// dotted playhead. A click or drag in the ruler or in the waveform lanes
// seeks live (UI_SPEC 3.4, user decisions 2026-11-14 and 2026-11-15). A
// drag on a note body previews through the SongDocument previewMove (the
// ghost outline shows the snapped result) and commits once on release, so
// one drag is one undo step (plan 8.4); an edge drag resizes through
// previewResize and resizeNote. A refused edit arrives through the
// document's editRefused signal, which EditorPage routes to a passive
// notification. The navigation gestures of UI_SPEC 3.2 run through the
// SheetBackground's viewport invokables: a plain wheel step scrolls time,
// Ctrl+wheel zooms around the cursor, Shift+wheel scrolls the pitch rows, a
// middle drag pans, and a visible horizontal scrollbar (recorded spec
// deviation, user decision 2026-11-15) spans the scroll bounds: 0:00 to the
// song or audio end plus 2 s (user decision 2026-11-15). The area after
// #END is shaded with a button that removes #END, the playhead carries the
// "End song here" button in the ruler while paused, and showMs scrolls a
// time into view for the song start and end jumps. The sheet
// paints every line of the song (user decision 2026-11-15); the current
// line's notes draw opaque, all other lines at 35 % opacity. The inline
// syllable field (task P6-02, user decision 2026-11-15) opens over a note's
// lyric label in the lyrics lane after a double click on the note or the
// label, or through editSyllable (the Enter key): Enter commits one undo
// step, Esc cancels, Tab and Shift+Tab commit and move to the next or
// previous note of the line, and leaving the field commits.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Shapes
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

    // True while a press in the ruler or the waveform lanes seeks: the
    // playback follow of the current line pauses meanwhile, so the view
    // does not move under the pointer.
    readonly property bool seeking: rulerSeek.pressed || waveformSeek.pressed

    // seekTo moves the playback to the time under a sheet x (the ruler and
    // the waveform lanes, UI_SPEC 3.4 and user decision 2026-11-15).
    function seekTo(sheetX: real) {
        const beat = sheet.xToBeat(sheetX);
        root.player.seekMs(Math.max(
            0, root.document.gapMs + beat * 15000 / root.document.bpm));
    }

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
        // The note data of every line of the song, which SongDocument
        // computes (user decision 2026-11-15: the sheet paints the whole
        // song).
        sheetNoteStarts: root.document ? root.document.sheetNoteStarts : []
        sheetNoteTexts: root.document ? root.document.sheetNoteTexts : []
        sheetNoteCurrents: root.document ? root.document.sheetNoteCurrents : []
        sheetPitchLow: root.document ? root.document.sheetPitchLow : -1
        sheetPitchHigh: root.document ? root.document.sheetPitchHigh : -1
        startMs: root.document ? root.document.viewStartMs : 0
        endMs: root.document ? root.document.viewEndMs : 0
        // The ends the time scroll is limited by (user decision
        // 2026-11-15): the last note, #END and the player's audio length.
        songEndBeat: root.document ? root.document.songEndBeat : 0
        endMarkMs: root.document ? root.document.endMs : -1
        audioEndMs: root.player ? root.player.durationMs : 0
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

    // editSyllable opens the inline syllable field over the note's lyric
    // label: the note becomes the selection, the field shows its syllable
    // without the word space.
    function editSyllable(noteId: int) {
        if (!root.document || root.document.readOnly || noteId < 0) {
            return;
        }
        root.document.selectNote(noteId, Usdx.SongDocument.SelectionKind.Set);
        const info = root.document.inspector;
        if (!info.single) {
            return;
        }
        syllableField.beat = info.start;
        syllableField.text = root.document.syllableForEditing(noteId);
        syllableField.noteId = noteId;
        syllableField.forceActiveFocus();
        syllableField.selectAll();
    }

    // closeSyllableField hides the inline field. Closed by a key, it hands
    // the keyboard focus back to the sheet, so the window shortcuts (Ctrl+Z,
    // the arrows) work again at once; closed because the focus went
    // elsewhere, the focus stays there.
    function closeSyllableField(refocus: bool) {
        syllableField.noteId = -1;
        if (refocus) {
            root.forceActiveFocus();
        }
    }

    // commitSyllable commits the inline field; `step` 1 or -1 then opens
    // the field on the next or previous note of the line. `refocus` hands
    // the focus back to the sheet (see closeSyllableField).
    function commitSyllable(step: int, refocus: bool) {
        const id = syllableField.noteId;
        if (id < 0) {
            return;
        }
        root.closeSyllableField(refocus);
        root.document.setNoteSyllable(id, syllableField.text);
        if (step === 0) {
            return;
        }
        if (step > 0) {
            root.document.selectNextNote();
        } else {
            root.document.selectPreviousNote();
        }
        const info = root.document.inspector;
        if (info.single && info.noteId !== id) {
            root.editSyllable(info.noteId);
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
            // The band never steals an exclusive grab from a deeper handler:
            // a press on a note must stay with the note's bodyDrag (or an
            // edge handle), while a press on empty area grabs freely because
            // a fresh grab is not a takeover.
            grabPermissions: PointerHandler.TakeOverForbidden

            onActiveChanged: {
                if (active) {
                    // DragHandler is a multi-point handler: the event point
                    // is `centroid`; `point` exists only on the single-point
                    // handlers (TapHandler, HoverHandler).
                    pitchArea.bandValid = centroid.pressPosition.y >= sheet.rulerHeight
                                          && centroid.pressPosition.y <= root.pitchBottom;
                    band.visible = pitchArea.bandValid;
                    if (pitchArea.bandValid) {
                        band.x = centroid.pressPosition.x;
                        band.y = centroid.pressPosition.y;
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
                band.x = Math.min(centroid.pressPosition.x, centroid.position.x);
                band.y = Math.min(centroid.pressPosition.y, centroid.position.y);
                band.width = Math.abs(centroid.position.x - centroid.pressPosition.x);
                band.height = Math.abs(centroid.position.y - centroid.pressPosition.y);
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
                objectName: "noteRepeater"
                model: root.document ? root.document.notesModel : null

                Usdx.NoteDelegate {
                    noteSheet: root
                    bg: sheet
                }
            }

            // The end-of-phrase markers (UI_SPEC 3.5): dashed lines with
            // square caps, draggable through PhraseEndHandle.
            Repeater {
                objectName: "phraseEndRepeater"
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

        // The navigation gestures of UI_SPEC 3.2, over the whole sheet: a
        // plain wheel step scrolls time (10 % of the visible width), wheel
        // down moves forward; Ctrl+wheel zooms by 1.25 per step around the
        // beat under the cursor; Shift+wheel scrolls the pitch range one
        // row per step, wheel down moves it down.
        WheelHandler {
            id: sheetWheel

            // KeyboardModifierMask accepts every modifier (the handler
            // compares them for exact equality otherwise); the dispatch
            // below reads the modifiers itself.
            acceptedModifiers: Qt.KeyboardModifierMask
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

            onWheel: (wheel) => {
                if (wheel.modifiers & Qt.ControlModifier) {
                    sheet.zoomAround(wheel.angleDelta.y > 0 ? 1.25 : 0.8, wheel.x);
                } else if (wheel.modifiers & Qt.ShiftModifier) {
                    sheet.wheelPitch(wheel.angleDelta.y > 0 ? -1 : 1);
                } else {
                    sheet.wheelTime(-wheel.angleDelta.y / 120.0);
                }
                wheel.accepted = true;
            }
        }

        // The middle-drag pan (UI_SPEC 3.2): dragging the sheet with the
        // middle button moves the view with the pointer, dragging right
        // shows earlier beats; each move applies its own delta.
        DragHandler {
            id: panDrag

            acceptedButtons: Qt.MiddleButton
            target: null

            property real lastX: 0

            onActiveChanged: {
                if (active) {
                    lastX = centroid.position.x;
                }
            }

            onTranslationChanged: {
                const dx = centroid.position.x - lastX;
                if (dx !== 0) {
                    sheet.panTime(dx);
                    lastX = centroid.position.x;
                }
            }
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

        MouseArea {
            id: rulerSeek

            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            hoverEnabled: false
            cursorShape: Qt.PointingHandCursor

            onPressed: (mouse) => root.seekTo(mouse.x)
            onPositionChanged: (mouse) => {
                if (mouse.buttons & Qt.LeftButton) {
                    root.seekTo(mouse.x);
                }
            }
        }
    }

    // The waveform seek strip (user decision 2026-11-15): a press anywhere
    // in the waveform and activity lanes right of the gutter moves the
    // playhead there, a drag scrubs live, while playing or paused. It sits
    // above the pitch area like the ruler strip; the scrollbar overlay
    // (z 12) keeps its own bottom edge.
    Item {
        id: waveformStrip

        objectName: "waveformSeekStrip"
        x: 0
        y: root.height - sheet.waveformHeight - sheet.activityHeight
        width: parent.width
        height: sheet.waveformHeight + sheet.activityHeight
        enabled: rulerStrip.enabled
        z: 6

        MouseArea {
            id: waveformSeek

            x: sheet.gutterWidth
            width: parent.width - sheet.gutterWidth
            height: parent.height
            acceptedButtons: Qt.LeftButton
            hoverEnabled: false
            cursorShape: Qt.SizeHorCursor

            onPressed: (mouse) => root.seekTo(waveformSeek.x + mouse.x)
            onPositionChanged: (mouse) => {
                if (mouse.buttons & Qt.LeftButton) {
                    root.seekTo(waveformSeek.x + mouse.x);
                }
            }
        }
    }

    // The lyrics lane's double click (UI_SPEC 3.6): it edits the syllable
    // of the note whose label lies under the pointer.
    MouseArea {
        id: lyricsLane

        objectName: "lyricsLaneArea"
        x: sheet.gutterWidth
        y: root.pitchBottom
        width: Math.max(0, parent.width - sheet.gutterWidth)
        height: sheet.lyricsHeight
        enabled: root.songOpen && root.document !== null && !root.document.readOnly
        acceptedButtons: Qt.LeftButton
        z: 6

        onDoubleClicked: (mouse) => root.editSyllable(
            root.document.noteAtLyricBeat(sheet.xToBeat(lyricsLane.x + mouse.x)))
    }

    // The inline syllable field over the lyric label of the note it edits.
    QQC2.TextField {
        id: syllableField

        // The row id of the edited note, -1 while closed, and its start
        // beat, which places the field.
        property int noteId: -1
        property real beat: 0

        objectName: "inlineSyllableField"
        visible: noteId >= 0
        z: 9
        x: Math.max(sheet.gutterWidth,
                    sheet.gutterWidth + (beat - sheet.startBeat) * sheet.pxPerBeat)
        y: root.pitchBottom + 1
        width: Math.max(metrics.inlineSyllableMinimumWidth, implicitWidth)
        height: sheet.lyricsHeight - 2

        onAccepted: root.commitSyllable(0, true)
        Keys.onEscapePressed: (event) => {
            root.closeSyllableField(true);
            event.accepted = true;
        }
        Keys.onTabPressed: (event) => {
            root.commitSyllable(1, true);
            event.accepted = true;
        }
        Keys.onBacktabPressed: (event) => {
            root.commitSyllable(-1, true);
            event.accepted = true;
        }
        onActiveFocusChanged: {
            if (!activeFocus && noteId >= 0) {
                root.commitSyllable(0, false);
            }
        }
    }

    // showMs scrolls the view so the time is visible: near the left edge,
    // or near the right edge with `atEnd` (the song start and end jumps);
    // the viewport keeps the view inside the scroll bounds.
    function showMs(ms: real, atEnd: bool) {
        if (!root.document || root.document.bpm <= 0) {
            return;
        }
        const beat = (ms - root.document.gapMs) * root.document.bpm / 15000;
        const visible = (sheet.width - sheet.gutterWidth) / Math.max(sheet.pxPerBeat, 0.001);
        sheet.scrollToBeat(atEnd ? beat - visible * 0.8 : beat - visible * 0.1);
    }

    // The area after #END (user decision 2026-11-15): shaded in the
    // neutral colour with a 2 px line at the song end, a label and a
    // button that removes #END. The shade passes every press through.
    Item {
        id: songEndArea

        readonly property real endBeat:
            root.document && root.document.endMs > 0 && root.document.bpm > 0
            ? (root.document.endMs - root.document.gapMs) * root.document.bpm / 15000 : 0
        readonly property real endX:
            (endBeat - sheet.startBeat) * sheet.pxPerBeat

        objectName: "songEndArea"
        x: sheet.gutterWidth
        width: Math.max(0, parent.width - sheet.gutterWidth)
        height: parent.height
        clip: true
        z: 7
        visible: root.songOpen && root.document.endMs > 0 && endX < width

        Rectangle {
            x: Math.max(0, songEndArea.endX)
            width: songEndArea.width - x
            height: parent.height
            color: Qt.alpha(Kirigami.Theme.neutralTextColor, 0.12)
        }

        Rectangle {
            x: songEndArea.endX - 1
            width: 2
            height: parent.height
            color: Kirigami.Theme.neutralTextColor
        }

        RowLayout {
            x: Math.max(0, songEndArea.endX) + Kirigami.Units.smallSpacing
            y: sheet.rulerHeight + Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                text: qsTr("Song end (#END)")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.neutralTextColor
            }

            QQC2.ToolButton {
                objectName: "removeSongEndButton"
                icon.name: "edit-delete-remove"
                text: qsTr("Remove song end")
                display: QQC2.AbstractButton.IconOnly
                enabled: root.document && !root.document.readOnly
                onClicked: root.document.removeSongEnd()
                QQC2.ToolTip.text: qsTr("Remove song end (#END)")
                QQC2.ToolTip.visible: hovered
            }
        }
    }

    // The horizontal scrollbar (recorded deviation from UI_SPEC 3.2: the
    // spec asks for wheel, zoom and pan only; the user asked 2026-11-15 for
    // a visible bar as well). It overlays the bottom edge of the activity
    // lane and spans the scroll bounds: 0:00 to the song or audio end plus
    // the margin (user decision 2026-11-15).
    QQC2.ScrollBar {
        id: sheetHScroll

        objectName: "sheetScrollBar"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 12
        z: 12
        orientation: Qt.Horizontal
        visible: root.songOpen && sheet.maxBeat > sheet.minBeat
        enabled: visible
        // The handle covers the visible beats of the bounds; a 2 %
        // minimum keeps the extreme zoom levels draggable.
        readonly property real contentBeats: Math.max(1.0, sheet.maxBeat - sheet.minBeat)
        readonly property real viewBeats:
            (sheet.width - sheet.gutterWidth) / Math.max(sheet.pxPerBeat, 0.001)
        size: Math.min(1.0, Math.max(0.02, viewBeats / contentBeats))
        position: Math.min(Math.max((sheet.startBeat - sheet.minBeat) / contentBeats, 0.0),
                           1.0 - size)
        onPositionChanged: {
            // The user's handle drag scrolls the view; the binding above
            // also fires this on view changes, so only a pressed handle
            // feeds back into the viewport (programmatic position changes
            // never emit while pressed).
            if (pressed) {
                sheet.scrollToBeat(sheet.minBeat + position * contentBeats);
            }
        }
    }

    // The playhead (UI_SPEC 3.5, user decision 2026-11-15: dotted): a 2 px
    // dotted line in the negative colour from the ruler top through the
    // pitch rows, the lyrics lane and the waveform lanes to the activity
    // lane's bottom, with a downward triangle at the ruler top. Its x
    // follows the player's position mapped through the beat axis the sheet
    // shows; the clip area starts at the gutter's right edge, so a playhead
    // outside the view never paints over the gutter or the lines pane.
    Item {
        id: playheadClip

        x: sheet.gutterWidth
        y: 0
        width: Math.max(0, parent.width - sheet.gutterWidth)
        height: parent.height
        clip: true
        z: 8

        Item {
            id: playhead

            objectName: "playhead"
            visible: root.songOpen && root.player !== null
                     && root.document !== null && root.document.bpm > 0
            x: root.player !== null && root.document !== null
               ? ((root.player.positionMs - root.document.gapMs)
                  * root.document.bpm / 15000 - sheet.startBeat)
                 * sheet.pxPerBeat - 1
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
                    // In units of the stroke width: 2 px dots, 4 px gaps.
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

            // "End song here" (user decision 2026-11-15) rides the playhead
            // in the ruler, right of the triangle: it writes #END at the
            // playhead and removes the notes after it (one undo step).
            QQC2.ToolButton {
                objectName: "endSongButton"
                x: 8
                y: 0
                width: sheet.rulerHeight
                height: sheet.rulerHeight
                padding: 2
                icon.name: "edit-cut"
                icon.width: Kirigami.Units.iconSizes.small
                icon.height: Kirigami.Units.iconSizes.small
                text: qsTr("End song here")
                display: QQC2.AbstractButton.IconOnly
                visible: root.document !== null && !root.document.readOnly
                         && root.player !== null && !root.player.playing
                         && root.player.positionMs > 0
                onClicked: root.document.endSongAt(root.player.positionMs)
                QQC2.ToolTip.text: qsTr("End the song here: sets #END at the playhead and deletes the notes after it")
                QQC2.ToolTip.visible: hovered
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
}
