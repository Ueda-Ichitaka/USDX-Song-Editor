// About me: one note rectangle of the note sheet (UI_SPEC 3.5, task P5-04),
// a delegate of the SongDocument's NotesModel: the fill is the note type
// colour, a selected note carries a 2 px text-colour border and 4 x 12 px
// edge handles, and the notes of the global neighbour lines draw at 35 %
// opacity. A click selects (Ctrl adds, Shift extends between the anchor and
// the click, plan 8.4), a click on a neighbour note first makes its line
// current. A body drag previews through the SongDocument previewMove (the
// ghost outline shows the snapped, committed result while the note itself
// follows the pointer) and commits once on release, so one drag is one undo
// step; Shift locks the first axis moved and Alt turns the edge snapping
// off. An edge drag resizes through previewResize and resizeNote.

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

    // The note geometry on the sheet: x right of the gutter, the pitch row
    // top plus the 2 px row padding (UI_SPEC 3.5).
    readonly property real baseX:
        bg ? bg.gutterWidth + (start - bg.startBeat) * bg.pxPerBeat : 0
    readonly property real baseY:
        bg ? bg.rulerHeight
             + (Math.round(bg.pitchTop) - pitch) * bg.pxPerSemitone + 2 : 0

    readonly property bool dragging: bodyDrag.active
    readonly property bool resizing: startDrag.active || endDrag.active
    // The whole selection previews the committed delta while one of its
    // notes is dragged (plan 8.4: the commit moves the selection by one
    // delta; the dragged note itself follows the pointer).
    readonly property bool moveShifted:
        !dragging && !resizing && selected && noteSheet.moveActive

    // The ghost outline position of the active drag: the committed result.
    readonly property real ghostX:
        endDrag.active ? baseX
                       : baseX + (bg ? pendingDBeat * bg.pxPerBeat : 0)
    readonly property real ghostY: baseY + pendingDPitch * (bg ? bg.pxPerSemitone : 0)
    readonly property real ghostWidth:
        startDrag.active
            ? Math.max(1, length - pendingDBeat) * bg.pxPerBeat - 1.5
            : endDrag.active
                ? Math.max(1, length + pendingDBeat) * bg.pxPerBeat - 1.5
                : bg ? length * bg.pxPerBeat - 1.5 : 0

    // The note position in sheet coordinates; x is the same minus the clip
    // area's gutter offset (the delegate sits in the clip area that starts
    // at the gutter, y stays in sheet coordinates because the clip area
    // starts at y 0).
    readonly property real sheetX:
        baseX
        + (dragging ? dragOffsetX : 0)
        + (startDrag.active ? startEdgeOffsetX : 0)
        + (moveShifted && bg ? noteSheet.moveDBeat * bg.pxPerBeat : 0)
    readonly property real sheetY:
        baseY
        + (dragging ? dragOffsetY : 0)
        + (moveShifted && bg ? noteSheet.moveDPitch * bg.pxPerSemitone : 0)

    x: bg ? sheetX - bg.gutterWidth : 0
    y: sheetY
    width: Math.max(2, bg ? length * bg.pxPerBeat - 1.5 : 0)
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
            note.selected
            && (point.position.x < 5 || point.position.x > note.width - 5)
            ? Qt.SizeHorCursor : Qt.SizeAllCursor
    }

    TapHandler {
        acceptedButtons: Qt.LeftButton

        onTapped: (eventPoint, button) => {
            // A neighbour note first makes its line current (UI_SPEC 3.5).
            if (note.lineOffset !== 0) {
                noteSheet.document.goToLine(noteSheet.document.currentLine + note.lineOffset);
            }
            const modifiers = eventPoint.modifiers;
            const mode = modifiers & Qt.ControlModifier
                ? Usdx.SongDocument.SelectionKind.Add
                : modifiers & Qt.ShiftModifier
                    ? Usdx.SongDocument.SelectionKind.Extend
                    : Usdx.SongDocument.SelectionKind.Set;
            noteSheet.document.selectNote(note.noteId, mode);
        }
    }

    DragHandler {
        id: bodyDrag

        acceptedButtons: Qt.LeftButton
        target: null

        onActiveChanged: {
            if (active) {
                note.pendingDBeat = 0;
                note.pendingDPitch = 0;
                note.dragOffsetX = 0;
                note.dragOffsetY = 0;
                bodyDrag.axisLocked = "";
            } else {
                // A successful moveSelection rebuilds the notes model, which
                // destroys this delegate's QML context: everything after the
                // call goes through references resolved beforehand, and the
                // delegate's own state is only touched when no reset
                // happened.
                const document = noteSheet.document;
                const sheet = noteSheet;
                const noteId = note.noteId;
                const dBeat = note.pendingDBeat;
                const dPitch = note.pendingDPitch;
                let moved = false;
                if (dBeat !== 0 || dPitch !== 0) {
                    if (!note.selected) {
                        // Drag selects implicitly (user decision
                        // 2026-11-15): a drag on an unselected note makes it
                        // the selection, so the commit moves it. The select
                        // happens at release, not at drag activation,
                        // because a selection change rebuilds the notes
                        // model and would kill the live gesture.
                        document.selectNote(noteId, Usdx.SongDocument.SelectionKind.Set);
                    }
                    moved = document.moveSelection(dBeat, dPitch);
                }
                document.notesModel.setGhost(-1, 0, -1);
                sheet.moveActive = false;
                if (!moved) {
                    note.dragOffsetX = 0;
                    note.dragOffsetY = 0;
                    note.pendingDBeat = 0;
                    note.pendingDPitch = 0;
                }
            }
        }

        onTranslationChanged: {
            // Shift locks the first axis moved (plan 8.4); DragHandler's
            // event point is `centroid` (multi-point handler), not `point`.
            if (bodyDrag.axisLocked === "") {
                bodyDrag.axisLocked = Math.abs(centroid.position.x - centroid.pressPosition.x)
                                      >= Math.abs(centroid.position.y - centroid.pressPosition.y)
                    ? "x" : "y";
            }
            const tx = bodyDrag.axisLocked === "y"
                ? 0 : centroid.position.x - centroid.pressPosition.x;
            const ty = bodyDrag.axisLocked === "x"
                ? 0 : centroid.position.y - centroid.pressPosition.y;
            // Alt turns the edge snapping off (plan 8.4).
            const snapOn = !(centroid.modifiers & Qt.AltModifier);
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

        property string axisLocked: ""
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
    Rectangle {
        id: startHandle

        visible: note.selected && note.width >= 12
        x: 0
        y: (note.height - height) / 2
        width: 4
        height: 12
        radius: 1
        color: Kirigami.Theme.textColor

        DragHandler {
            id: startDrag

            acceptedButtons: Qt.LeftButton
            target: null

            onActiveChanged: {
                if (active) {
                    note.pendingDBeat = 0;
                    note.startEdgeOffsetX = 0;
                } else {
                    // A successful resizeNote rebuilds the notes model, which
                    // destroys this delegate's QML context: everything after
                    // the call goes through references resolved beforehand,
                    // and the delegate's own state is only touched when no
                    // reset happened.
                    const document = noteSheet.document;
                    const noteId = note.noteId;
                    const dBeat = note.pendingDBeat;
                    const resized = dBeat !== 0
                        ? document.resizeNote(
                              noteId, Usdx.SongDocument.ResizeEdge.Start, dBeat)
                        : false;
                    document.notesModel.setGhost(-1, 0, -1);
                    if (!resized) {
                        note.startEdgeOffsetX = 0;
                        note.pendingDBeat = 0;
                    }
                }
            }

            onTranslationChanged: {
                const snapOn = !(startDrag.centroid.modifiers & Qt.AltModifier);
                note.pendingDBeat = noteSheet.document.previewResize(
                    note.noteId, Usdx.SongDocument.ResizeEdge.Start,
                    bg.dragDeltaBeats(centroid.position.x - centroid.pressPosition.x),
                    snapOn, bg.pxPerBeat);
                note.startEdgeOffsetX = centroid.position.x - centroid.pressPosition.x;
            }
        }
    }

    Rectangle {
        id: endHandle

        visible: note.selected && note.width >= 12
        x: note.width - width
        y: (note.height - height) / 2
        width: 4
        height: 12
        radius: 1
        color: Kirigami.Theme.textColor

        DragHandler {
            id: endDrag

            acceptedButtons: Qt.LeftButton
            target: null

            onActiveChanged: {
                if (active) {
                    note.pendingDBeat = 0;
                } else {
                    // A successful resizeNote rebuilds the notes model, which
                    // destroys this delegate's QML context: everything after
                    // the call goes through references resolved beforehand,
                    // and the delegate's own state is only touched when no
                    // reset happened.
                    const document = noteSheet.document;
                    const noteId = note.noteId;
                    const dBeat = note.pendingDBeat;
                    const resized = dBeat !== 0
                        ? document.resizeNote(
                              noteId, Usdx.SongDocument.ResizeEdge.End, dBeat)
                        : false;
                    document.notesModel.setGhost(-1, 0, -1);
                    if (!resized) {
                        note.pendingDBeat = 0;
                    }
                }
            }

            onTranslationChanged: {
                const snapOn = !(endDrag.centroid.modifiers & Qt.AltModifier);
                note.pendingDBeat = noteSheet.document.previewResize(
                    note.noteId, Usdx.SongDocument.ResizeEdge.End,
                    bg.dragDeltaBeats(centroid.position.x - centroid.pressPosition.x),
                    snapOn, bg.pxPerBeat);
            }
        }
    }
}
