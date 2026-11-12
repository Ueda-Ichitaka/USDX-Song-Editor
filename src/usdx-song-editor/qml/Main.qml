// About me: the root window of the USDX Song Editor. It is a Kirigami
// ApplicationWindow that instantiates the SongDocument bridge and the LinesModel
// for the lines pane (task P3-04); the editor page, toolbar, note sheet and dialogs
// are added by the later tasks following Plan/UI_SPEC.md (the sizes here are the
// window rules of UI_SPEC 2.6).

import QtQuick
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.ApplicationWindow {
    id: root

    // The application state, exposed for the self-test helper and the pages.
    property Usdx.SongDocument songDocument: Usdx.SongDocument {
        objectName: "songDocument"
        linesModel: root.linesModel
    }
    property Usdx.LinesModel linesModel: Usdx.LinesModel {
        objectName: "linesModel"
        document: root.songDocument
    }

    width: 1280
    height: 800
    minimumWidth: 960
    minimumHeight: 600

    title: "USDX Song Editor"
}
