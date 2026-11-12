// About me: the root window of the USDX Song Editor. It is an empty Kirigami
// ApplicationWindow for the skeleton task P0-03; the editor page, toolbar, note sheet
// and dialogs are added by the later tasks following Plan/UI_SPEC.md (the sizes here
// are the window rules of UI_SPEC 2.6).

import QtQuick
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root

    width: 1280
    height: 800
    minimumWidth: 960
    minimumHeight: 600

    title: "USDX Song Editor"
}
