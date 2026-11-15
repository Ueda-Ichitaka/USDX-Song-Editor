// About me: one note rectangle of the note sheet (UI_SPEC 3.5, task P5-04),
// a delegate of the SongDocument's NotesModel: the fill is the note type
// colour, a selected note carries a 2 px text-colour border and 4 x 12 px
// edge handles, and the notes of the global neighbour lines draw at 35 %
// opacity. A click selects (Ctrl adds, Shift extends between the anchor and
// the click, plan 8.4), a click on a neighbour note first makes its line
// current. One DragHandler serves the whole note and picks its mode at the
// press: within 5 px of an edge of a selected note (the edge handles, from
// 12 px width) it resizes from that edge through previewResize and
// resizeNote, everywhere else it moves the note through previewMove and
// moveSelection. A single handler means no second handler competes for the
// press. The note (or the dragged edge) follows the pointer while the ghost
// outline shows the snapped, committed result, and the release commits
// once, so one drag is one undo step; Shift locks the first axis moved and
// Alt turns the edge snapping off. The pointer deltas are taken in scene
// coordinates because the note moves under the pointer. A double click
// opens the sheet's inline syllable field for the note.

import QtQuick
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Rectangle {
    id: note

    // The NotesModel roles (plan 8.3).
    required property int noteId
    required property real start
    required property int length
    required property int pitch
    required property string type
    required property string text
    required property int lineOffset
    required property bool selected
    required property real ghostStart
    required property int ghostLength

    // The sheet this note sits on and the painted background that owns the
    // beat axis; NoteSheet binds them. The names avoid the NoteSheet.qml
    // ids: the delegate's own properties shadow them.
    property Usdx.NoteSheet noteSheet
    property Usdx.SheetBackground bg

    // The committed deltas of the active drag: the previewMove /
    // previewResize return values. The ghost outline and the release commit
    // use them, so the ghost and the commit agree (plan 8.4, user decision
    // 2026-11-14: the rounding happens after the snap in the controller).
    property int pendingDBeat: 0
    property int pendingDPitch: 0
    // The raw pointer offsets: the note follows the pointer while the ghost
    // shows the committed result (UI_SPEC 3.5).
    property real dragOffsetX: 0
    property real dragOffsetY: 0
    property real startEdgeOffsetX: 0
    property real endEdgeOffsetX: 0

    // The mode of the active drag, chosen at the press: "move", "start"
    // (resize from the start edge) or "end" (resize from the end edge).
    property string dragMode: ""

    // The width of the edge grab zones of a selected note (UI_SPEC 3.5).
    readonly property real edgeZone: 5
    readonly property bool edgesActive: selected && width >= 12

    // The note geometry on the sheet: x right of the gutter, the pitch row
    // top plus the 2 px row padding (UI_SPEC 3.5).
    readonly property real baseX:
        bg ? bg.gutterWidth + (start - bg.startBeat) * bg.pxPerBeat : 0
    readonly property real baseY:
        bg ? bg.rulerHeight
             + (Math.round(bg.pitchTop) - pitch) * bg.pxPerSemitone + 2 : 0

    readonly property bool dragging: bodyDrag.active && dragMode === "move"
    readonly property bool startResizing: bodyDrag.active && dragMode === "start"
    readonly property bool endResizing: bodyDrag.active && dragMode === "end"
    readonly property bool resizing: startResizing || endResizing
    // The whole selection previews the committed delta while one of its
    // notes is dragged (plan 8.4: the commit moves the selection by one
    // delta; the dragged note itself follows the pointer).
    readonly property bool moveShifted:
        !dragging && !resizing && selected && noteSheet.moveActive

    // The ghost outline position of the active drag: the committed result.
    readonly property real ghostX:
        endResizing ? baseX
                       : baseX + (bg ? pendingDBeat * bg.pxPerBeat : 0)
    readonly property real ghostY: baseY + pendingDPitch * (bg ? bg.pxPerSemitone : 0)
    readonly property real ghostWidth:
        startResizing
            ? Math.max(1, length - pendingDBeat) * bg.pxPerBeat - 1.5
            : endResizing
                ? Math.max(1, length + pendingDBeat) * bg.pxPerBeat - 1.5
                : bg ? length * bg.pxPerBeat - 1.5 : 0

    // The note position in sheet coordinates; x is the same minus the clip
    // area's gutter offset (the delegate sits in the clip area that starts
    // at the gutter, y stays in sheet coordinates because the clip area
    // starts at y 0).
    readonly property real sheetX:
        baseX
        + (dragging ? dragOffsetX : 0)
        + (startResizing ? startEdgeOffsetX : 0)
        + (moveShifted && bg ? noteSheet.moveDBeat * bg.pxPerBeat : 0)
    readonly property real sheetY:
        baseY
        + (dragging ? dragOffsetY : 0)
        + (moveShifted && bg ? noteSheet.moveDPitch * bg.pxPerSemitone : 0)

    x: bg ? sheetX - bg.gutterWidth : 0
    y: sheetY
    // A dragged edge follows the pointer, the other edge stays.
    width: Math.max(2, bg ? length * bg.pxPerBeat - 1.5
                            - (startResizing ? startEdgeOffsetX : 0)
                            + (endResizing ? endEdgeOffsetX : 0)
                          : 0)
    height: bg ? bg.pxPerSemitone - 4 : 0
    radius: 3
    color: typeColor
    // The notes of the global neighbour lines draw at 35 % opacity.
    opacity: lineOffset !== 0 ? 0.35 : 1.0
    border.width: selected ? 2 : 0
    border.color: Kirigami.Theme.textColor
    // Selected notes above the line's notes, the line's notes above the
    // neighbour notes.
    z: selected ? 2 : (lineOffset === 0 ? 1 : 0)
    // A note whose pitch row lies outside the visible pitch rows hides
    // (the x binding subtracts the clip area's gutter offset, the y stays
    // in sheet coordinates because the clip area starts at y 0).
    visible: bg !== null
             && baseY >= bg.rulerHeight
             && baseY + height <= pitchBottom
    readonly property real pitchBottom: bg
        ? bg.height - bg.lyricsHeight - bg.gapHeight
          - bg.waveformHeight - bg.activityHeight
        : 0

    // The app colours (UI_SPEC 2.3); Palette derives them, one instance per
    // file needs no state.
    Usdx.Palette {
        id: appPalette
    }

    readonly property color typeColor:
        type === "*" ? appPalette.noteGolden
        : type === "F" ? appPalette.noteFreestyle
        : type === "R" ? appPalette.noteRap
        : type === "G" ? appPalette.noteRapGolden
        : appPalette.noteNormal

    HoverHandler {
        // SizeAll on the body, SizeHor within 5 px of an edge of a selected
        // note (UI_SPEC 3.5).
        cursorShape:
            note.edgesActive
            && (point.position.x < note.edgeZone
                || point.position.x > note.width - note.edgeZone)
            ? Qt.SizeHorCursor : Qt.SizeAllCursor
    }

    TapHandler {
        id: noteTap

        acceptedButtons: Qt.LeftButton

        onTapped: (eventPoint, button) => {
            // A neighbour note first makes its line current (UI_SPEC 3.5).
            if (note.lineOffset !== 0) {
                noteSheet.document.goToLine(noteSheet.document.currentLine + note.lineOffset);
            }
            // The modifiers live on the handler's point; the signal's
            // QEventPoint has none.
            const modifiers = noteTap.point.modifiers;
            const mode = modifiers & Qt.ControlModifier
                ? Usdx.SongDocument.SelectionKind.Add
                : modifiers & Qt.ShiftModifier
                    ? Usdx.SongDocument.SelectionKind.Extend
                    : Usdx.SongDocument.SelectionKind.Set;
            noteSheet.document.selectNote(note.noteId, mode);
        }

        // A double click edits the note's syllable over its lyric label
        // (UI_SPEC 3.6, task P6-02).
        onDoubleTapped: noteSheet.editSyllable(note.noteId)
    }

    DragHandler {
        id: bodyDrag

        acceptedButtons: Qt.LeftButton
        target: null

        property string axisLocked: ""

        onActiveChanged: {
            if (active) {
                // The mode comes from the press point; the note has not
                // moved yet at activation.
                const pressX = note.mapFromItem(null, centroid.scenePressPosition).x;
                note.dragMode = !note.edgesActive ? "move"
                    : pressX < note.edgeZone ? "start"
                    : pressX > note.width - note.edgeZone ? "end"
                    : "move";
                note.pendingDBeat = 0;
                note.pendingDPitch = 0;
                note.dragOffsetX = 0;
                note.dragOffsetY = 0;
                note.startEdgeOffsetX = 0;
                note.endEdgeOffsetX = 0;
                bodyDrag.axisLocked = "";
                return;
            }
            // A commit that changes the note order (or the note count)
            // resets the notes model, which destroys this delegate's QML
            // context; other commits update the rows in place. Everything
            // after the call goes through references resolved beforehand,
            // and the delegate's own state is only touched when nothing was
            // committed.
            const document = noteSheet.document;
            const sheet = noteSheet;
            const noteId = note.noteId;
            const mode = note.dragMode;
            const dBeat = note.pendingDBeat;
            const dPitch = note.pendingDPitch;
            const wasSelected = note.selected;
            let committed = false;
            if (mode === "move") {
                sheet.moveActive = false;
                if (dBeat !== 0 || dPitch !== 0) {
                    if (!wasSelected) {
                        // Drag selects implicitly (user decision
                        // 2026-11-15): a drag on an unselected note makes
                        // it the selection, so the commit moves it. The
                        // select happens at release, not at drag
                        // activation, because a selection change rebuilds
                        // the notes model and would kill the live gesture.
                        document.selectNote(noteId, Usdx.SongDocument.SelectionKind.Set);
                    }
                    committed = document.moveSelection(dBeat, dPitch);
                }
            } else if (dBeat !== 0) {
                committed = document.resizeNote(
                    noteId,
                    mode === "start" ? Usdx.SongDocument.ResizeEdge.Start
                                     : Usdx.SongDocument.ResizeEdge.End,
                    dBeat);
            }
            if (!committed) {
                note.dragMode = "";
                note.dragOffsetX = 0;
                note.dragOffsetY = 0;
                note.startEdgeOffsetX = 0;
                note.endEdgeOffsetX = 0;
                note.pendingDBeat = 0;
                note.pendingDPitch = 0;
            }
        }

        onTranslationChanged: {
            const dx = centroid.scenePosition.x - centroid.scenePressPosition.x;
            const dy = centroid.scenePosition.y - centroid.scenePressPosition.y;
            // Alt turns the edge snapping off (plan 8.4).
            const snapOn = !(centroid.modifiers & Qt.AltModifier);
            if (note.dragMode === "start" || note.dragMode === "end") {
                const startEdge = note.dragMode === "start";
                note.pendingDBeat = noteSheet.document.previewResize(
                    note.noteId,
                    startEdge ? Usdx.SongDocument.ResizeEdge.Start
                              : Usdx.SongDocument.ResizeEdge.End,
                    bg.dragDeltaBeats(dx), snapOn, bg.pxPerBeat);
                if (startEdge) {
                    note.startEdgeOffsetX = dx;
                } else {
                    note.endEdgeOffsetX = dx;
                }
                return;
            }
            // Shift locks the first axis moved (plan 8.4); DragHandler's
            // event point is `centroid` (multi-point handler), not `point`.
            if (bodyDrag.axisLocked === "") {
                bodyDrag.axisLocked = Math.abs(dx) >= Math.abs(dy) ? "x" : "y";
            }
            const tx = bodyDrag.axisLocked === "y" ? 0 : dx;
            const ty = bodyDrag.axisLocked === "x" ? 0 : dy;
            const result = noteSheet.document.previewMove(
                note.noteId, bg.dragDeltaBeats(tx), bg.dragDeltaPitch(ty),
                snapOn, bg.pxPerBeat);
            note.pendingDBeat = result["dBeat"];
            note.pendingDPitch = result["dPitch"];
            note.dragOffsetX = tx;
            note.dragOffsetY = ty;
            noteSheet.moveActive = true;
            noteSheet.moveDBeat = note.pendingDBeat;
            noteSheet.moveDPitch = note.pendingDPitch;
        }
    }

    // The ghost outline of the active drag: the committed result (the
    // snapped position for a move, the snapped length for a resize).
    Rectangle {
        visible: note.dragging || note.resizing
        x: note.ghostX - note.sheetX
        y: note.ghostY - note.sheetY
        width: Math.max(2, note.ghostWidth)
        height: note.height
        radius: 3
        color: "transparent"
        border.width: 1
        border.color: Kirigami.Theme.textColor
    }

    // The edge handles of a selected note (UI_SPEC 3.5): 4 x 12 px, radius
    // 1, text colour, centred on the left and right edge, from 12 px width.
    // They only show where the edge grab zones are; bodyDrag does the
    // resizing.
    Rectangle {
        visible: note.edgesActive
        x: 0
        y: (note.height - height) / 2
        width: 4
        height: 12
        radius: 1
        color: Kirigami.Theme.textColor
    }

    Rectangle {
        visible: note.edgesActive
        x: note.width - width
        y: (note.height - height) / 2
        width: 4
        height: 12
        radius: 1
        color: Kirigami.Theme.textColor
    }
}
