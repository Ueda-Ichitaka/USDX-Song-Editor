// About me: the root window of the USDX Song Editor (UI_SPEC 2.6). It owns
// the SongDocument bridge with its models, the Player bridge, the main menu
// drawer and the editor page, and orchestrates the guarded transitions of
// UI_SPEC 5.1 and 5.2: before opening another song, reloading or closing, a
// dirty song asks through the Save changes? dialog; a save that hits an
// external change asks through the Song changed on disk dialog; open and
// reload failures show the Cannot open song dialog. It also restores and
// remembers the window size, checks the file on disk when the window becomes
// active again, and accepts .txt drops on the window. The note sheet and the
// remaining dialogs arrive with the later tasks.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs as FileDialogs
import QtQuick.Layouts
import QtCore
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx
import "format.js" as Time

Kirigami.ApplicationWindow {
    id: root

    // The application state, exposed for the self-test helper and the pages.
    property Usdx.SongDocument songDocument: Usdx.SongDocument {
        objectName: "songDocument"
        linesModel: root.linesModel
        recentModel: root.recentModel
    }
    property Usdx.LinesModel linesModel: Usdx.LinesModel {
        objectName: "linesModel"
        document: root.songDocument
    }
    property Usdx.RecentSongsModel recentModel: Usdx.RecentSongsModel {
        objectName: "recentModel"
        document: root.songDocument
    }
    property Usdx.Player player: Usdx.Player {
        objectName: "player"
        document: root.songDocument
    }

    // The action waiting behind a Save changes? or Song changed on disk
    // dialog; null while no transition is pending.
    property var pendingAction: null

    width: root.songDocument.windowWidth
    height: root.songDocument.windowHeight
    minimumWidth: 960
    minimumHeight: 600
    title: root.songDocument.windowTitle

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.initialPage: Usdx.EditorPage {
        document: root.songDocument
        player: root.player
        shell: root
    }

    globalDrawer: Usdx.MainMenu {
        shell: root
    }

    // One shell instance per QML file would be wasteful; the dialogs need
    // the theme only through Kirigami itself, so no palette lives here.

    // The guarded transition helpers. A transition starts with the
    // document's decision, parks the follow-up action in pendingAction when
    // a dialog asks first, and runs it when the dialog lets it continue.

    function runPending() {
        const action = root.pendingAction;
        root.pendingAction = null;
        if (action) {
            action();
        }
    }

    // continueTransition reacts to a dialog outcome: a continue runs the
    // pending action, a changed-on-disk ask opens that dialog and keeps the
    // pending action, a stop drops it.
    function continueTransition(kind) {
        if (kind === Usdx.TransitionKind.Continue) {
            root.runPending();
        } else if (kind === Usdx.TransitionKind.AskChangedOnDisk) {
            changedOnDiskDialog.open();
        } else {
            root.pendingAction = null;
        }
    }

    // requestTransition guards `action` with the Save changes? dialog when
    // the song has unsaved changes (UI_SPEC 5.1).
    function requestTransition(action) {
        const kind = root.songDocument.transitionStart();
        if (kind === Usdx.TransitionKind.Continue) {
            action();
        } else if (kind === Usdx.TransitionKind.AskSaveChanges) {
            root.pendingAction = action;
            saveChangesDialog.open();
        }
    }

    function requestOpen(url) {
        root.requestTransition(() => root.songDocument.open(url));
    }

    function openSong(url) {
        // Recent songs were already guarded by the menu or the start screen
        // clicks; the entry still goes through the guard so both behave the
        // same.
        root.requestOpen(url);
    }

    function requestReload() {
        root.requestTransition(() => {
            const kind = root.songDocument.transitionReload();
            root.continueTransition(kind);
        });
    }

    function openFileDialog() {
        // Start in the folder of the current song, else of the most recent
        // song, else the home folder (UI_SPEC 5.3); the FileDialog has no
        // opened signal, so the folder is set right before it opens.
        const document = root.songDocument;
        let folder = "";
        if (document.isLoaded) {
            folder = Time.folderUrlOf(document.filePath);
        } else if (document.recentCount > 0) {
            folder = Time.folderUrlOf(document.recentModel.entry(0).pathUrl);
        }
        if (folder.length === 0) {
            folder = StandardPaths.writableLocation(
                StandardPaths.HomeLocation).toString();
        }
        openDialog.currentFolder = folder;
        openDialog.open();
    }

    function openSaveAsDialog() {
        saveAsDialog.open();
    }

    function saveSong() {
        if (root.songDocument.save()) {
            root.notifySaved();
        }
    }

    function notifySaved() {
        const backup = root.songDocument.takeLastBackup();
        const message = backup.length > 0
            ? qsTr("Saved. Backup written to \"%1\".").arg(backup)
            : qsTr("Saved");
        root.showPassiveNotification(message, 2000);
    }

    function showNotYetAvailable() {
        root.showPassiveNotification(
            qsTr("This page arrives in a later phase."), 4000);
    }

    function showAboutPage() {
        root.pageStack.push(aboutPageComponent);
    }

    function rememberWindowSize() {
        root.songDocument.windowWidth = root.width;
        root.songDocument.windowHeight = root.height;
        root.songDocument.rememberWindowSize();
    }

    function quitNow() {
        root.rememberWindowSize();
        Qt.quit();
    }

    function requestQuit() {
        root.songDocument.windowWidth = root.width;
        root.songDocument.windowHeight = root.height;
        root.requestTransition(() => root.quitNow());
    }

    Component.onCompleted: root.songDocument.refreshRecents()

    onClosing: (close) => {
        // The close guard of UI_SPEC 5.1; the size is remembered only when
        // the window really closes.
        close.accepted = false;
        root.requestTransition(() => root.quitNow());
    }

    onActiveChanged: {
        if (!active) {
            return;
        }
        const kind = root.songDocument.checkActivation();
        if (kind === Usdx.ActivationKind.ReloadSilently) {
            root.showPassiveNotification(
                qsTr("Reloaded: the file changed on disk."), 4000);
        } else if (kind === Usdx.ActivationKind.AskChangedOnDisk) {
            changedOnDiskDialog.open();
        }
    }

    DropArea {
        anchors.fill: parent
        onDropped: (drop) => {
            const url = drop.hasUrls
                ? drop.urls.find(candidate => candidate.toString().endsWith(".txt"))
                : null;
            if (url !== undefined) {
                root.requestOpen(url);
            } else {
                root.showPassiveNotification(
                    qsTr("Only UltraStar song files (.txt) can be opened."), 4000);
            }
        }
    }

    Connections {
        target: root.songDocument
        function onErrorOccurred(message) {
            openErrorDialog.subtitle = message;
            openErrorDialog.open();
        }
        function onExternalChangeDetected() {
            changedOnDiskDialog.open();
        }
    }

    FileDialogs.FileDialog {
        id: openDialog

        title: qsTr("Open song")
        nameFilters: [qsTr("UltraStar songs (*.txt)"), qsTr("All files (*)")]
        fileMode: FileDialogs.FileDialog.OpenFile
        onAccepted: root.requestOpen(selectedFile)
    }

    FileDialogs.FileDialog {
        id: saveAsDialog

        title: qsTr("Save as")
        nameFilters: [qsTr("UltraStar songs (*.txt)"), qsTr("All files (*)")]
        fileMode: FileDialogs.FileDialog.SaveFile
        onAccepted: {
            if (root.songDocument.saveAs(selectedFile)) {
                root.notifySaved();
            }
        }
    }

    Kirigami.PromptDialog {
        id: saveChangesDialog

        dialogType: Kirigami.PromptDialog.Warning
        title: qsTr("Save changes?")
        standardButtons: Kirigami.Dialog.NoButton
        subtitle: {
            const document = root.songDocument;
            const name = document.title.length > 0
                ? document.title
                : Time.fileName(document.filePath);
            const steps = document.unsavedSteps;
            return steps === 1
                ? qsTr("\"%1\" has 1 unsaved change. If you continue without saving, it is lost.").arg(name)
                : qsTr("\"%1\" has %2 unsaved changes. If you continue without saving, they are lost.").arg(name).arg(steps);
        }
        customFooterActions: [
            Kirigami.Action {
                icon.name: "document-save"
                text: qsTr("Save")
                onTriggered: {
                    saveChangesDialog.close();
                    const kind = root.songDocument.transitionSaveChanges(
                        Usdx.SaveChangesKind.Save);
                    root.continueTransition(kind);
                }
            },
            Kirigami.Action {
                icon.name: "edit-delete"
                text: qsTr("Discard")
                onTriggered: {
                    saveChangesDialog.close();
                    const kind = root.songDocument.transitionSaveChanges(
                        Usdx.SaveChangesKind.Discard);
                    root.continueTransition(kind);
                }
            },
            Kirigami.Action {
                text: qsTr("Cancel")
                onTriggered: {
                    saveChangesDialog.close();
                    const kind = root.songDocument.transitionSaveChanges(
                        Usdx.SaveChangesKind.Cancel);
                    root.continueTransition(kind);
                }
            }
        ]
        onRejected: root.pendingAction = null
    }

    Kirigami.PromptDialog {
        id: changedOnDiskDialog

        dialogType: Kirigami.PromptDialog.Warning
        title: qsTr("Song changed on disk")
        standardButtons: Kirigami.Dialog.NoButton
        subtitle: root.songDocument.isLoaded
            ? qsTr("Another program changed \"%1\" after you opened it.").arg(Time.fileName(root.songDocument.filePath))
            : ""

        QQC2.Label {
            text: {
                const document = root.songDocument;
                if (document.unsavedSteps === 1) {
                    return qsTr("Overwrite replaces those changes with yours. Reload discards your 1 unsaved change and opens the file as it is now.");
                }
                if (document.unsavedSteps > 1) {
                    return qsTr("Overwrite replaces those changes with yours. Reload discards your %1 unsaved changes and opens the file as it is now.").arg(document.unsavedSteps);
                }
                return qsTr("Overwrite replaces those changes with yours. Reload opens the file as it is now.");
            }
            color: Kirigami.Theme.disabledTextColor
            wrapMode: Text.WordWrap
        }
        customFooterActions: [
            Kirigami.Action {
                icon.name: "document-save"
                text: qsTr("Overwrite")
                onTriggered: {
                    changedOnDiskDialog.close();
                    if (root.songDocument.saveConfirmed()) {
                        root.notifySaved();
                        root.runPending();
                    } else {
                        root.pendingAction = null;
                    }
                }
            },
            Kirigami.Action {
                icon.name: "view-refresh"
                text: qsTr("Reload")
                onTriggered: {
                    changedOnDiskDialog.close();
                    const kind = root.songDocument.transitionChangedOnDisk(
                        Usdx.ChangedOnDiskKind.Reload);
                    root.continueTransition(kind);
                }
            },
            Kirigami.Action {
                text: qsTr("Cancel")
                onTriggered: {
                    changedOnDiskDialog.close();
                    root.pendingAction = null;
                }
            }
        ]
        onRejected: root.pendingAction = null
    }

    Kirigami.PromptDialog {
        id: openErrorDialog

        dialogType: Kirigami.PromptDialog.Error
        title: qsTr("Cannot open song")
        subtitle: ""
        standardButtons: Kirigami.Dialog.Ok
    }

    Component {
        id: aboutPageComponent

        Kirigami.AboutPage {
            title: qsTr("About")

            header: QQC2.ToolBar {
                RowLayout {
                    anchors.fill: parent

                    QQC2.ToolButton {
                        icon.name: "go-previous"
                        text: qsTr("Editor")
                        display: QQC2.AbstractButton.TextBesideIcon
                        onClicked: root.pageStack.pop()
                    }
                }
            }

            // The KAboutData-shaped object (AboutItem.qml reads componentName,
            // displayName, version, licenses and the rest); a binding value
            // cannot be a bare object literal because QML would parse it as
            // grouped properties.
            aboutData: root.aboutData
        }
    }

    property var aboutData: ({
        componentName: "usdx-song-editor",
        displayName: qsTr("USDX Song Editor"),
        version: root.songDocument.appVersion,
        shortDescription: qsTr("An editor for UltraStar Deluxe songs."),
        homepage: "",
        bugAddress: "",
        copyrightStatement: "",
        productName: "usdx-song-editor",
        desktopFileName: "io.github.ueda_ichitaka.UsdxSongEditor",
        licenses: [
            {
                name: "MIT",
                text: qsTr("Licensed under the MIT licence."),
                spdx: "MIT"
            }
        ],
        authors: [],
        credits: [],
        translators: []
    })
}
