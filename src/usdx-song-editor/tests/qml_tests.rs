//! About me: runs the Qt Quick Test cases in tests/qml/ (the QML-side unit
//! tests, such as the format.js helpers) with Qt 6's qmltestrunner under the
//! offscreen platform, so `cargo test` and `scripts/dev.sh test` cover them.
//! The runner is taken from the Qt 6 host binaries folder that qmake reports
//! (the `QMAKE` variable or `qmake6`, the same lookup the cxx-qt build uses),
//! because the qmltestrunner on PATH can belong to Qt 5. A missing Qt 6 runner
//! fails the test with a hint instead of skipping it silently.

use std::path::{Path, PathBuf};
use std::process::Command;

/// qt6_test_runner returns the qmltestrunner of the Qt 6 host binaries.
fn qt6_test_runner() -> PathBuf {
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| String::from("qmake6"));
    let out = Command::new(&qmake)
        .args(["-query", "QT_HOST_BINS"])
        .output()
        .unwrap_or_else(|error| panic!("{qmake} could not run (install qt6-base): {error}"));
    let bins = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let runner = Path::new(&bins).join("qmltestrunner");
    assert!(
        runner.is_file(),
        "no Qt 6 qmltestrunner at {} (install qt6-declarative)",
        runner.display()
    );
    runner
}

#[test]
fn qml_test_cases_pass() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("qml");
    let out = Command::new(qt6_test_runner())
        .env("QT_QPA_PLATFORM", "offscreen")
        // The runner prints its results through Qt logging, which goes to
        // the journal instead of stderr without this.
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .arg("-input")
        .arg(&cases)
        .output()
        .expect("the Qt 6 qmltestrunner runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the QML test cases failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("0 failed") || stdout.contains("0 failed"),
        "the QML test run reported no clean totals:\n{stdout}\n{stderr}"
    );
}
