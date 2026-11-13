// About me: the status bar of the editor page (UI_SPEC 4.3). It shows the
// song facts BPM, GAP and the format version, and the state pills for
// unsaved changes, read-only and missing audio. The playhead label is added
// by the playback task P4-04.

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.ueda_ichitaka.UsdxSongEditor as Usdx

Rectangle {
    id: root

    // The document the bar mirrors; set by the editor page.
    property Usdx.SongDocument document

    color: Kirigami.Theme.backgroundColor
    implicitHeight: metrics.statusBarHeight
    border.width: 0

    Usdx.Metrics {
        id: metrics
    }

    component FactLabel: RowLayout {
        id: fact

        required property string label
        required property string value

        spacing: Kirigami.Units.smallSpacing
        visible: value.length > 0

        QQC2.Label {
            text: fact.label
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        QQC2.Label {
            text: fact.value
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.bold: true
        }
    }

    component StatePill: Rectangle {
        id: pill

        required property string text
        required property color textColor
        required property color fillColor

        radius: height / 2
        color: pill.fillColor
        implicitWidth: pillLabel.implicitWidth + 2 * metrics.pillPaddingHorizontal
        implicitHeight: pillLabel.implicitHeight + 2 * metrics.pillPaddingVertical
        Layout.alignment: Qt.AlignVCenter
        visible: pill.text.length > 0

        QQC2.Label {
            id: pillLabel

            anchors.centerIn: parent
            text: pill.text
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.bold: true
            color: pill.textColor
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: metrics.statusBarPadding
        anchors.rightMargin: metrics.statusBarPadding
        spacing: metrics.statusBarSpacing

        FactLabel {
            label: qsTr("BPM")
            value: document && document.isLoaded
                   ? document.bpm.toFixed(2) : ""
        }

        FactLabel {
            label: qsTr("GAP")
            value: document && document.isLoaded
                   ? Math.round(document.gapMs) + " " + qsTr("ms") : ""
        }

        FactLabel {
            label: qsTr("Format")
            value: !document || !document.isLoaded
                   ? ""
                   : document.formatVersion.length > 0
                     ? document.formatVersion
                     : qsTr("unversioned")
        }

        Item {
            Layout.fillWidth: true
        }

        StatePill {
            text: document && document.unsavedSteps > 0
                  ? qsTr("%n unsaved change(s)", "", document.unsavedSteps) : ""
            textColor: Kirigami.Theme.highlightedTextColor
            fillColor: Qt.alpha(Kirigami.Theme.highlightColor, 0.2)
        }

        StatePill {
            text: document && document.readOnly ? qsTr("read-only") : ""
            textColor: Kirigami.Theme.negativeTextColor
            fillColor: Kirigami.Theme.negativeBackgroundColor
        }

        StatePill {
            text: document && document.isLoaded && !document.audioPresent
                  ? qsTr("audio missing") : ""
            textColor: Kirigami.Theme.neutralTextColor
            fillColor: Kirigami.Theme.neutralBackgroundColor
        }
    }
}
