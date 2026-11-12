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
fn self_test_reports_the_line_count_of_a_song() {
    // A song with two lines in one voice and one line in a second voice.
    let text = "#TITLE:T\n#BPM:240\nP1\n: 0 4 0 a\n- 4\n: 4 4 1 b\n- 8\nP2\n: 0 4 0 c\n- 4\nE\n";
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, text).expect("the fixture is written");

    let out = binary()
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("--self-test")
        .arg(&path)
        .output()
        .expect("the binary was built and --self-test could run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "--self-test with a song failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("lines: 3"),
        "the line count must be reported on stdout, got: {stdout}"
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
