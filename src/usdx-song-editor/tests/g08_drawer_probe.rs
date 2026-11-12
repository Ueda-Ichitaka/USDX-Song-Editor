//! About me: gate G-08 probe (the first step of task P3-05, UI_SPEC 4.2). It
//! loads a minimal QML scene offscreen that owns a `Kirigami.GlobalDrawer`
//! with `isMenu: true` and a plain `QQC2.ToolBar` with our own button, clicks
//! the button the way Qt itself does (through the invokable `click()` method),
//! and asserts that the drawer reported itself visible. Pass: the P3-05 shell
//! uses the GlobalDrawer as planned. Fail: gate G-08 stops the build for a
//! decision about the main menu component.

use std::fs;

use cxx_qt_lib::{QQuickStyle, QString, QUrl};

/// The probe scene: an isMenu GlobalDrawer opened by our own toolbar button,
/// with the resulting visibility mirrored into a plain bool property.
const PROBE_QML: &str = r#"
import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root

    property bool drawerVisible: false

    globalDrawer: Kirigami.GlobalDrawer {
        isMenu: true
        onVisibleChanged: root.drawerVisible = visible
    }

    header: Controls.ToolBar {
        Controls.ToolButton {
            objectName: "menuButton"
            text: "Menu"
            onClicked: root.globalDrawer.open()
        }
    }
}
"#;

#[test]
fn g08_is_menu_global_drawer_opens_from_our_own_toolbar_button() {
    // The offscreen platform must be set before the application object is
    // constructed; set_var is unsafe in the 2024 edition because it can race
    // with concurrent readers.
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("probe.qml");
    fs::write(&path, PROBE_QML).expect("the probe scene is written");

    // The same style the application uses, so the probe tests the real
    // conditions rather than the fallback style.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&"org.kde.desktop".into());
    }
    // The application is declared before the engine so that the scope end
    // destroys the QML objects first; the other order crashes in the
    // QQuickShortcut destructors. The object must stay alive for the whole
    // probe, hence the underscore name without a drop.
    let _app = cxx_qt_lib_extras::QApplication::new();

    let mut engine = cxx_qt_lib::QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from_local_file(&QString::from(
            path.to_str().expect("the path is valid unicode"),
        )));
    }

    let window_visible = |name: &str| {
        engine.as_ref().is_some_and(|engine| {
            usdx_song_editor::bridge::selftest::usdx_qml_root_property_bool(
                engine,
                &QString::from(name),
            )
        })
    };
    assert!(!window_visible("drawerVisible"), "the drawer starts closed");

    // Click the toolbar button through its invokable click() method, the way
    // a real interaction reaches the onClicked handler.
    let clicked = engine.as_ref().is_some_and(|engine| {
        usdx_song_editor::bridge::selftest::usdx_qml_click_child(
            engine,
            &QString::from("menuButton"),
        )
    });
    assert!(clicked, "the toolbar button exists and accepted the click");

    // The visibility is mirrored by the onVisibleChanged handler.
    let visible = window_visible("drawerVisible");
    assert!(
        visible,
        "an isMenu GlobalDrawer must open from our own toolbar button (gate G-08)"
    );
}
