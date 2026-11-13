// About me: the main menu of the shell (UI_SPEC 4.2): an isMenu
// Kirigami.GlobalDrawer owned by the application window and opened by the
// toolbar's Main menu button. It carries the file, edit and navigation
// actions with their shortcuts and the Open Recent submenu, which rebuilds
// itself from the recent songs model whenever the list changes. The
// not-yet-built pages (Repair timing, Song properties, Settings, Export
// anchors) show a temporary passive notification until their tasks land.

import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Kirigami.GlobalDrawer {
    id: drawer

    // The application window, which owns the document, the dialogs and the
    // page stack.
    property QtObject shell

    // One Open Recent entry: created for each recent song when the submenu
    // rebuilds (an inline component, so createObject works on it).
    component RecentAction: Kirigami.Action {
        required property var entry

        objectName: "recentSongAction"
        text: entry.title
        icon.name: "audio-x-generic"
        enabled: entry.exists
        onTriggered: drawer.shell.openSong(entry.pathUrl)
    }

    // The submenu refills whenever the recent list changes.
    Connections {
        target: drawer.shell ? drawer.shell.songDocument : null
        function onRecentCountChanged() {
            openRecentAction.rebuild();
        }
    }

    isMenu: true

    actions: [
        Kirigami.Action {
            text: qsTr("Open…")
            icon.name: "document-open"
            shortcut: "Ctrl+O"
            onTriggered: drawer.shell.openFileDialog()
        },
        Kirigami.Action {
            id: openRecentAction

            text: qsTr("Open Recent")
            icon.name: "document-open-recent"
            visible: drawer.shell && drawer.shell.songDocument.recentCount > 0

            // The submenu holds one action per recent song, rebuilt whenever
            // the list changes; the entries below are the static separator
            // and the Clear List action.
            function rebuild() {
                const stale = children.filter(
                    child => child.objectName === "recentSongAction");
                children = children.filter(
                    child => child.objectName !== "recentSongAction");
                for (const child of stale) {
                    child.destroy();
                }
                if (!drawer.shell) {
                    return;
                }
                const model = drawer.shell.songDocument.recentModel;
                const count = drawer.shell.songDocument.recentCount;
                for (let index = 0; index < count; index++) {
                    const entry = model.entry(index);
                    children.push(
                        RecentAction.createObject(drawer, { entry: entry }));
                }
            }

            Kirigami.Action {
                separator: true
            }

            Kirigami.Action {
                icon.name: "edit-delete"
                text: qsTr("Clear List")
                onTriggered: drawer.shell.songDocument.clearRecentList()
            }
        },
        Kirigami.Action {
            text: qsTr("Save")
            icon.name: "document-save"
            shortcut: "Ctrl+S"
            enabled: drawer.shell && drawer.shell.songDocument.canSave
            onTriggered: drawer.shell.saveSong()
        },
        Kirigami.Action {
            text: qsTr("Save As…")
            icon.name: "document-save-as"
            shortcut: "Ctrl+Shift+S"
            enabled: drawer.shell && drawer.shell.songDocument.canSaveAs
            onTriggered: drawer.shell.openSaveAsDialog()
        },
        Kirigami.Action {
            text: qsTr("Reload from Disk")
            icon.name: "view-refresh"
            enabled: drawer.shell && drawer.shell.songDocument.isLoaded
            onTriggered: drawer.shell.requestReload()
        },
        Kirigami.Action {
            separator: true
        },
        Kirigami.Action {
            text: qsTr("Song Properties")
            icon.name: "document-properties"
            enabled: drawer.shell && drawer.shell.songDocument.canProperties
            onTriggered: drawer.shell.showNotYetAvailable()
        },
        Kirigami.Action {
            text: qsTr("Repair Timing")
            icon.name: "tools-wizard"
            shortcut: "Ctrl+R"
            enabled: drawer.shell && drawer.shell.songDocument.canRepair
            onTriggered: drawer.shell.showNotYetAvailable()
        },
        Kirigami.Action {
            text: qsTr("Export Anchors for the Factory…")
            icon.name: "document-export"
            enabled: drawer.shell && drawer.shell.songDocument.canExportAnchors
            onTriggered: drawer.shell.showNotYetAvailable()
        },
        Kirigami.Action {
            separator: true
        },
        Kirigami.Action {
            text: qsTr("Settings")
            icon.name: "settings-configure"
            shortcut: "Ctrl+,"
            onTriggered: drawer.shell.showNotYetAvailable()
        },
        Kirigami.Action {
            text: qsTr("About USDX Song Editor")
            icon.name: "help-about"
            onTriggered: drawer.shell.showAboutPage()
        },
        Kirigami.Action {
            text: qsTr("Quit")
            icon.name: "application-exit"
            shortcut: "Ctrl+Q"
            onTriggered: drawer.shell.requestQuit()
        }
    ]
}
