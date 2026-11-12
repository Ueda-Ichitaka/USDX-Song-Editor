//! About me: integration test for the app skeleton (task P0-03). It runs the built
//! binary with `--self-test` under `QT_QPA_PLATFORM=offscreen` and expects exit 0
//! when the QML module loads and the window root object exists, and checks that
//! `--version` prints the crate version, so the cxx-qt and Kirigami skeleton is
//! proven to start without a real display.

use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_usdx-song-editor"))
}

#[test]
fn self_test_starts_offscreen() {
    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .output()
        .expect("the binary was built and --self-test could run");
    assert!(
        out.status.success(),
        "--self-test failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn version_prints_crate_version() {
    // Reference the library so that this test links the full dependency graph;
    // the cxx-qt initializer archives of the package's build script are linked into
    // every target and resolve through the dependency rlibs bundled in the library.
    let expected = usdx_song_editor::version_line();
    let out = binary()
        .arg("--version")
        .output()
        .expect("the binary was built and --version could run");
    assert!(out.status.success(), "--version exited with an error");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), expected);
}
