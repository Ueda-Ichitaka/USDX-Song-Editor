//! The application settings.
//!
//! About me: this module holds the persisted user preferences (plan section
//! 8.8): the recent songs, the playback and tap-along values, the timing
//! health thresholds and the analysis thresholds, as a plain-Rust struct
//! with a TOML file backend. It exists so the QML settings page and the
//! controllers read one tested source of defaults: a missing or broken
//! settings file loads the defaults with a log warning, changes are written
//! back at once, and restoring the defaults keeps the recent songs and the
//! window size (UI_SPEC 4.10).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The reverse-DNS application id, also the settings folder name.
pub const APP_ID: &str = "io.github.ueda_ichitaka.UsdxSongEditor";

/// How many songs the recent list keeps (plan section 8.8).
pub const RECENT_CAPACITY: usize = 10;

/// One entry of the recent songs list: the file and when it was last
/// opened, as Unix seconds (UI_SPEC 4.1 shows the relative date).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    pub opened: u64,
}

/// The audio track the player picks when a song opens (UI_SPEC 4.10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackDefault {
    /// Prefer a `#VOCALS` track, fall back to the mix.
    #[default]
    VocalsIfPresentElseMix,
    /// Always the mix.
    Mix,
    /// Prefer an instrumental track, fall back to the mix.
    InstrumentalIfPresentElseMix,
}

/// The timing health thresholds (plan section 6.10, UI_SPEC 4.10).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthThresholds {
    /// Syllables per second above which a line is suspicious.
    pub rate_warn: f64,
    /// Syllables per second above which a line races.
    pub rate_bad: f64,
    /// The factor against the neighbourhood median above which a line is
    /// suspicious.
    pub neighbour_factor_warn: f64,
    /// How many surrounding lines form the neighbourhood.
    pub neighbourhood: usize,
    /// Share of note time with active vocals below which a line is
    /// suspicious.
    pub coverage_warn: f64,
    /// Share of note time with active vocals below which a line races.
    pub coverage_bad: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        HealthThresholds {
            rate_warn: 7.0,
            rate_bad: 10.0,
            neighbour_factor_warn: 2.5,
            neighbourhood: 6,
            coverage_warn: 0.40,
            coverage_bad: 0.20,
        }
    }
}

/// The audio analysis thresholds. The first entry is the level in dB above
/// the noise floor at which the vocals track counts as heard; later tuning
/// (task P7-10, behind gate G-06) adds its values here.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisThresholds {
    pub activity_floor_db: f64,
}

impl Default for AnalysisThresholds {
    fn default() -> Self {
        AnalysisThresholds {
            activity_floor_db: 12.0,
        }
    }
}

/// The persisted user preferences. Every field not present in the file keeps
/// its default, so a settings file from an older version loads cleanly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The recently opened songs, newest first.
    pub recent_files: Vec<RecentFile>,
    /// The tap latency in milliseconds (UI_SPEC 4.10: 0-300).
    pub latency_ms: u32,
    /// Seconds of pre-roll before the first line in tap-along (UI_SPEC 4.10:
    /// 0.0-10.0).
    pub preroll_s: f64,
    /// The playback speed when tap-along starts (0.5, 0.75 or 1.0).
    pub tap_speed: f64,
    /// Time in seconds added before and after a played line (UI_SPEC 4.10:
    /// 0.0-2.0).
    pub line_padding_s: f64,
    /// The track the player picks when a song opens.
    pub default_track: TrackDefault,
    /// Whether moving to a line plays it at once.
    pub autoplay: bool,
    pub health: HealthThresholds,
    pub analysis: AnalysisThresholds,
    /// The last window size; `None` leaves the 1280 x 800 default
    /// (UI_SPEC 2.6).
    pub window_size: Option<[i32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            recent_files: Vec::new(),
            latency_ms: 35,
            preroll_s: 2.0,
            tap_speed: 0.5,
            line_padding_s: 0.3,
            default_track: TrackDefault::VocalsIfPresentElseMix,
            autoplay: false,
            health: HealthThresholds::default(),
            analysis: AnalysisThresholds::default(),
            window_size: None,
        }
    }
}

impl Settings {
    /// load reads the settings file; a missing or broken file gives the
    /// defaults and a log warning.
    pub fn load(path: &Path) -> Settings {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => {
                log::warn!(
                    "No settings file at {}; using the defaults.",
                    path.display()
                );
                return Settings::default();
            }
        };
        match Self::from_toml(&String::from_utf8_lossy(&bytes)) {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!(
                    "The settings file at {} is broken ({error}); using the defaults.",
                    path.display()
                );
                Settings::default()
            }
        }
    }

    /// save writes the settings to the file, creating the folder when it is
    /// missing.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(path, self.to_toml())
    }

    /// from_toml parses the settings text; unknown fields are ignored and
    /// missing fields keep their defaults.
    pub fn from_toml(text: &str) -> Result<Settings, toml::de::Error> {
        toml::from_str(text)
    }

    /// to_toml renders the settings as TOML text.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// remember_recent_file moves `path` to the front of the recent list
    /// with the `opened` timestamp, adding it when it is not there yet, and
    /// caps the list.
    pub fn remember_recent_file(&mut self, path: &Path, opened: u64) {
        self.recent_files
            .retain(|recent| recent.path.as_path() != path);
        self.recent_files.insert(
            0,
            RecentFile {
                path: path.to_owned(),
                opened,
            },
        );
        self.recent_files.truncate(RECENT_CAPACITY);
    }

    /// drop_missing_recent_files removes entries whose file no longer exists
    /// (UI_SPEC 4.1: a missing song is removed from the list on the next
    /// start) and tells whether anything was removed.
    pub fn drop_missing_recent_files(&mut self) -> bool {
        let before = self.recent_files.len();
        self.recent_files.retain(|recent| recent.path.exists());
        self.recent_files.len() != before
    }

    /// restore_defaults resets every preference to its default, keeping the
    /// recent songs and the window size (UI_SPEC 4.10).
    pub fn restore_defaults(&mut self) {
        let recent_files = std::mem::take(&mut self.recent_files);
        let window_size = self.window_size;
        *self = Settings {
            recent_files,
            window_size,
            ..Settings::default()
        };
    }
}

/// settings_path returns the settings file location,
/// `config_dir()/APP_ID/settings.toml`; `None` when the platform has no
/// config directory.
pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|config| config.join(APP_ID).join("settings.toml"))
}
