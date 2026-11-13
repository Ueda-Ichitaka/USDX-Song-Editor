// About me: the app colours of UI_SPEC 2.3 that are not theme roles: the
// note type chip colours, the chip text colour, the anchor colour and the
// waveform lane colours (2.3: theme colours at 55 % opacity). The dark
// variants follow the brightness of the theme background, so the colours
// work in both Breeze themes. The file is a zero-size Item because the
// Kirigami theme attached properties need an item, and a visible one:
// Kirigami does not resolve the theme colours of a hidden item, so they
// would stay black. Every QML file that draws app colours instantiates
// `Palette {}` (the contents are derived constants, so one instance per
// file needs no state).

import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    // Visible on purpose (see above); the zero size draws nothing.
    visible: true
    width: 0
    height: 0

    // Dark means the theme background is a dark colour (UI_SPEC 2.3).
    readonly property bool dark:
        Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor)
        === Kirigami.ColorUtils.Dark

    // Note type colours: the fill of the type chip (UI_SPEC 4.3).
    readonly property color noteNormal: "#3daee9"
    readonly property color noteGolden: "#f0b429"
    readonly property color noteFreestyle: dark ? "#7d848e" : "#b8bec7"
    readonly property color noteRap: "#2fb6a6"
    readonly property color noteRapGolden: "#9b7fe6"
    // The mixed chip uses the golden colour.
    readonly property color noteMixed: noteGolden

    // The text on the type chips.
    readonly property color chipText: dark ? "#0b0e12" : "#10151c"

    // The anchor colour with the readable text colour on it.
    readonly property color anchor: dark ? "#f27fb3" : "#a3245e"
    readonly property color anchorText: dark ? "#0b0e12" : "#ffffff"

    // The waveform lane colours (UI_SPEC 2.3): theme colours at 55 %.
    readonly property color waveform: Qt.alpha(Kirigami.Theme.highlightColor, 0.55)
    readonly property color vocalActivity: Qt.alpha(Kirigami.Theme.positiveTextColor, 0.55)
}
