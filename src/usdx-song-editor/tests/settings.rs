//! About me: tests for the settings module (task P3-03, plan section 8.8,
//! UI_SPEC 4.10). They assert that the defaults match the spec's values,
//! that a missing or broken settings file loads the defaults, that a
//! partial file fills the rest with defaults, that a save and load round
//! trip preserves every field, that recent files are deduplicated and
//! capped at ten with the newest first, that restoring the defaults keeps
//! the recent songs and the window size, and that the real settings path
//! lives in the application's config folder.

use std::fs;
use std::path::{Path, PathBuf};
use usdx_song_editor::controller::settings::{
    AnalysisThresholds, HealthThresholds, Settings, TrackDefault,
};

/// a settings value with every field off its default.
fn non_default() -> Settings {
    Settings {
        latency_ms: 120,
        preroll_s: 1.5,
        tap_speed: 1.0,
        line_padding_s: 0.8,
        default_track: TrackDefault::Mix,
        autoplay: true,
        health: HealthThresholds {
            rate_warn: 8.0,
            rate_bad: 11.0,
            neighbour_factor_warn: 3.0,
            neighbourhood: 8,
            coverage_warn: 0.5,
            coverage_bad: 0.3,
        },
        analysis: AnalysisThresholds {
            activity_floor_db: 10.0,
        },
        window_size: Some([1024, 768]),
        recent_files: vec![PathBuf::from("/music/a.txt")],
    }
}

#[test]
fn the_defaults_match_the_ui_spec() {
    let settings = Settings::default();
    assert_eq!(settings.recent_files, Vec::<PathBuf>::new());
    assert_eq!(settings.latency_ms, 35);
    assert_eq!(settings.preroll_s, 2.0);
    assert_eq!(settings.tap_speed, 0.5);
    assert_eq!(settings.line_padding_s, 0.3);
    assert_eq!(settings.default_track, TrackDefault::VocalsIfPresentElseMix);
    assert!(!settings.autoplay);
    assert_eq!(
        settings.health,
        HealthThresholds {
            rate_warn: 7.0,
            rate_bad: 10.0,
            neighbour_factor_warn: 2.5,
            neighbourhood: 6,
            coverage_warn: 0.40,
            coverage_bad: 0.20,
        }
    );
    assert_eq!(settings.analysis.activity_floor_db, 12.0);
    assert_eq!(
        settings.window_size, None,
        "the window size is unset until saved"
    );
}

#[test]
fn a_missing_file_loads_the_defaults() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("settings.toml");
    assert_eq!(Settings::load(&path), Settings::default());
}

#[test]
fn a_broken_file_loads_the_defaults() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("settings.toml");
    fs::write(&path, "not [valid toml").expect("the broken file is written");
    assert_eq!(Settings::load(&path), Settings::default());
}

#[test]
fn a_partial_file_fills_the_rest_with_defaults() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("settings.toml");
    fs::write(&path, "latency_ms = 60\nautoplay = true\n").expect("the file is written");
    let settings = Settings::load(&path);
    assert_eq!(settings.latency_ms, 60);
    assert!(settings.autoplay);
    assert_eq!(settings.health, Settings::default().health);
    assert_eq!(settings.recent_files, Vec::<PathBuf>::new());
}

#[test]
fn save_and_load_round_trip_preserves_every_field() {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("settings.toml");
    let written = non_default();
    written.save(&path).expect("the settings are written");
    assert_eq!(Settings::load(&path), written);
}

#[test]
fn remember_recent_file_deduplicates_and_caps_at_ten() {
    let mut settings = Settings::default();
    for index in 0..12 {
        settings.remember_recent_file(Path::new(&format!("/music/song{index}.txt")));
    }
    assert_eq!(settings.recent_files.len(), 10);
    assert_eq!(
        settings.recent_files[0],
        PathBuf::from("/music/song11.txt"),
        "the newest file is first"
    );
    // Re-adding an old entry moves it to the front instead of duplicating.
    settings.remember_recent_file(Path::new("/music/song5.txt"));
    assert_eq!(settings.recent_files.len(), 10);
    assert_eq!(settings.recent_files[0], PathBuf::from("/music/song5.txt"));
    assert_eq!(
        settings
            .recent_files
            .iter()
            .filter(|path| path.as_path() == Path::new("/music/song5.txt"))
            .count(),
        1
    );
}

#[test]
fn restore_defaults_keeps_recent_files_and_window_size() {
    let mut settings = non_default();
    settings.restore_defaults();
    assert_eq!(settings.recent_files, vec![PathBuf::from("/music/a.txt")]);
    assert_eq!(settings.window_size, Some([1024, 768]));
    assert_eq!(settings.latency_ms, 35);
    assert_eq!(settings.health, Settings::default().health);
    assert!(!settings.autoplay);
    assert_eq!(settings.default_track, TrackDefault::VocalsIfPresentElseMix);
}

#[test]
fn the_settings_path_lives_in_the_app_config_folder() {
    let Some(path) = usdx_song_editor::controller::settings::settings_path() else {
        eprintln!("config_dir is not available in this environment; skipping");
        return;
    };
    let segments: Vec<_> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        segments.last().map(String::as_str),
        Some("settings.toml"),
        "the path is {path:?}"
    );
    assert!(
        segments
            .iter()
            .any(|segment| segment == "io.github.ueda_ichitaka.UsdxSongEditor"),
        "the path is {path:?}"
    );
}
