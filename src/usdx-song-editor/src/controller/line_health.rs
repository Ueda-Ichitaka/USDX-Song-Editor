//! The editor's line health (task P7-09, UI_SPEC 4.3).
//!
//! About me: this module turns the health analysis of usdx-edit (plan 6.10)
//! into what the lines pane and the line header show: per line of every
//! voice, in the lines pane's global order, the verdict (0 ok, 1 suspicious,
//! 2 racing) and the tooltip reason of UI_SPEC 4.3 named after the first
//! deciding signal ("Racing: 11.9 syllables per second", "Racing: 3.2 times
//! faster than the lines around it", "Suspicious: vocals heard during 34 %
//! of the note time", "Racing: starts after the audio ends"), or "Timing
//! looks plausible" for a calm line. The thresholds come from the settings;
//! the vocal activity of the #VOCALS track, when the analysis cache has it,
//! feeds the coverage signal, and the song's audio length, when known, the
//! past-end check (task P7-10).

use usdx_audio::Activity;
use usdx_edit::health::{Health, HealthConfig, LineHealth, Signal, analyze_with_end};
use usdx_format::rows::Document;
use usdx_format::song::Song;

use crate::controller::settings::HealthThresholds;

/// One line's mark: the verdict (0 ok, 1 suspicious, 2 racing) and the
/// tooltip reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineMark {
    pub health: i32,
    pub reason: String,
}

/// health_config maps the settings' thresholds onto the analysis.
pub fn health_config(thresholds: &HealthThresholds) -> HealthConfig {
    HealthConfig {
        rate_warn: thresholds.rate_warn,
        rate_bad: thresholds.rate_bad,
        neighbour_factor_warn: thresholds.neighbour_factor_warn,
        coverage_warn: thresholds.coverage_warn,
        coverage_bad: thresholds.coverage_bad,
        neighbourhood: thresholds.neighbourhood,
    }
}

/// mark_of writes one line's verdict and reason.
fn mark_of(line: &LineHealth) -> LineMark {
    let (health, prefix) = match line.health {
        Health::Ok => {
            return LineMark {
                health: 0,
                reason: String::from("Timing looks plausible"),
            };
        }
        Health::Suspicious => (1, "Suspicious"),
        Health::Racing => (2, "Racing"),
    };
    let detail = match line.signals.first() {
        Some(Signal::Rate) | None => format!("{:.1} syllables per second", line.syllables_per_s),
        Some(Signal::NeighbourFactor) => format!(
            "{:.1} times faster than the lines around it",
            line.neighbour_factor
        ),
        Some(Signal::Coverage) => format!(
            "vocals heard during {} % of the note time",
            (line.vocal_coverage.unwrap_or(0.0) * 100.0).round()
        ),
        Some(Signal::PastEnd) => String::from("starts after the audio ends"),
    };
    LineMark {
        health,
        reason: format!("{prefix}: {detail}"),
    }
}

/// line_marks analyses every voice and returns one mark per line in the
/// lines pane's global order (the voices one after another); `audio_end_ms`
/// is the length of the song's audio, `None` while unknown.
pub fn line_marks(
    doc: &Document,
    song: &Song,
    cfg: &HealthConfig,
    activity: Option<&Activity>,
    audio_end_ms: Option<f64>,
) -> Vec<LineMark> {
    (0..song.voices.len())
        .flat_map(|voice| {
            let voice = u8::try_from(voice).unwrap_or(u8::MAX);
            let healths = analyze_with_end(doc, song, voice, activity, audio_end_ms, cfg);
            healths.iter().map(mark_of).collect::<Vec<_>>()
        })
        .collect()
}
