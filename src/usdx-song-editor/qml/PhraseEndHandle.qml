// About me: one end-of-phrase marker of the note sheet (UI_SPEC 3.5, task
// P5-05), a delegate of the SongDocument's PhraseEndsModel: a 1.4 px dashed
// vertical line in disabledTextColor through the pitch rows with an 8 x 8 px
// square cap (radius 2) 2 px below the ruler. A drag moves the marker live,
// clamped to its bounds between the surrounding notes (the model's min and
// max roles); the release commits the rounded beat once through
// movePhraseEnd, so one drag is one undo step and a refused move arrives as
// the document's editRefused notification.

import QtQuick
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Item {
    id: handle

    // The PhraseEndsModel roles (plan 8.3): the marker row, its beat and
    // the drag bounds between the surrounding notes.
    required property int rowId
    required property real beat
    required property real min
    required property real max

    // The sheet this marker sits on and the painted background that owns
    // the beat axis; NoteSheet binds them (the names avoid the NoteSheet
    // ids, which a delegate property would shadow).
    property Usdx.NoteSheet noteSheet
    property Usdx.SheetBackground bg

    // The rounded beat the active drag previews; the release commits it.
    property real previewBeat: beat

    readonly property bool dragging: drag.active
    readonly property real shownBeat: dragging ? previewBeat : beat

    // The grab area is 12 px wide, centred on the 1.4 px line; x is
    // clip-area-relative (the clip area starts at the gutter), y spans the
    // pitch rows.
    readonly property real lineX:
        bg ? bg.gutterWidth + (shownBeat - bg.startBeat) * bg.pxPerBeat : 0

    x: bg ? lineX - bg.gutterWidth - width / 2 : 0
    y: bg ? bg.rulerHeight : 0
    width: 12
    height: bg ? noteSheet.pitchBottom - bg.rulerHeight : 0
    z: 3

    HoverHandler {
        cursorShape: Qt.SizeHorCursor
    }

    DragHandler {
        id: drag

        acceptedButtons: Qt.LeftButton
        target: null

        onTranslationChanged: {
            // The line centre follows the pointer; the beat rounds to a
            // whole beat and clamps to the marker's bounds, so the preview
            // and the commit agree.
            const sheetX = handle.lineX + translation.x;
            const raw = handle.bg ? handle.bg.xToBeat(sheetX) : 0;
            handle.previewBeat = Math.min(
                handle.max, Math.max(handle.min, Math.round(raw)));
        }

        onActiveChanged: {
            if (active) {
                handle.previewBeat = handle.beat;
            } else if (handle.previewBeat !== handle.beat) {
                handle.noteSheet.document.movePhraseEnd(
                    handle.rowId, handle.previewBeat);
            }
        }
    }

    // The dashed 1.4 px line (dash 5, gap 4) through the pitch rows.
    Canvas {
        x: (parent.width - width) / 2
        width: 2
        height: parent.height

        onPaint: {
            const context = getContext("2d");
            context.clearRect(0, 0, width, height);
            context.strokeStyle = handle.Kirigami.Theme.disabledTextColor;
            context.lineWidth = 1.4;
            context.setLineDash([5, 4]);
            context.beginPath();
            context.moveTo(width / 2, 0);
            context.lineTo(width / 2, height);
            context.stroke();
        }
        onHeightChanged: requestPaint()
        onVisibleChanged: requestPaint()
    }

    // The 8 x 8 px square cap 2 px below the ruler (the handle starts at
    // the ruler height, so the cap sits at y 2).
    Rectangle {
        x: (parent.width - width) / 2
        y: 2
        width: 8
        height: 8
        radius: 2
        color: handle.Kirigami.Theme.disabledTextColor
    }
}
