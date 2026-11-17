//! The racing detection of the timing repair (plan section 6.10, task
//! P7-05).
//!
//! About me: the health analysis reads a voice's lines and reports per
//! line how race-like it looks: the syllable rate (notes per second of
//! the line's note span), the neighbour factor (the line's rate against
//! the median of its neighbourhood = up to 6 lines before and 6 after,
//! user decision 2026-11-16) and the vocal coverage (the share of the
//! line's note time where the passed `usdx-audio` `Activity` is active;
//! `None` without an activity, which the caller passes when the song has
//! no `#VOCALS` track). The worst signal decides the health and the
//! reason names the signals at that level. `racing_sections` merges
//! neighbouring Racing lines into the line ranges a repair would touch.
//! The analysis only reads: a relative song is analyzed like an absolute
//! one, and a song without a usable tempo or an unknown voice analyzes
//! to no lines.

use std::ops::Range;

use usdx_audio::Activity;
use usdx_format::rows::Document;
use usdx_format::song::Song;
use usdx_format::timing::Timing;

/// HealthConfig holds the health thresholds. The defaults are the plan's
/// first guesses.
#[derive(Clone, Copy, Debug)]
pub struct HealthConfig {
    /// The syllable rate in notes per second where a line becomes
    /// suspicious and where it races.
    pub rate_warn: f64,
    pub rate_bad: f64,
    /// The neighbour factor where a line races against its neighbourhood
    /// median.
    pub neighbour_factor_warn: f64,
    /// The vocal coverage where a line becomes suspicious and where it
    /// races (the coverage signal fires at or below the bound).
    pub coverage_warn: f64,
    pub coverage_bad: f64,
    /// How many lines on each side form a line's neighbourhood.
    pub neighbourhood: usize,
}

impl Default for HealthConfig {
    fn default() -> Self {
        HealthConfig {
            rate_warn: 7.0,
            rate_bad: 10.0,
            neighbour_factor_warn: 2.5,
            coverage_warn: 0.40,
            coverage_bad: 0.20,
            neighbourhood: 6,
        }
    }
}

/// Health is the worst verdict of a line's signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Ok,
    Suspicious,
    Racing,
}

/// One health signal (plan 6.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// The syllable rate.
    Rate,
    /// The rate against the neighbourhood median.
    NeighbourFactor,
    /// The share of the note time with active vocals.
    Coverage,
}

/// LineHealth is one line's report: its index in the voice, its verdict,
/// the raw signals, the reason naming the signals at the verdict's level
/// and those signals themselves (in the order rate, neighbour factor,
/// coverage; empty for a calm line).
#[derive(Clone, Debug, PartialEq)]
pub struct LineHealth {
    pub line: usize,
    pub health: Health,
    pub syllables_per_s: f64,
    pub neighbour_factor: f64,
    pub vocal_coverage: Option<f64>,
    pub reason: String,
    pub signals: Vec<Signal>,
}

/// severity_of maps one signal to its verdict: a value at or beyond the
/// bad bound races, at or beyond the warn bound is suspicious. The
/// coverage bounds read downward, the rate and factor bounds upward.
fn severity_of(value: f64, warn: f64, bad: f64, downward: bool) -> Health {
    let races = if downward { value <= bad } else { value >= bad };
    let suspects = if downward {
        value <= warn
    } else {
        value >= warn
    };
    if races {
        Health::Racing
    } else if suspects {
        Health::Suspicious
    } else {
        Health::Ok
    }
}

/// worst returns the worse of two verdicts.
fn worst(a: Health, b: Health) -> Health {
    match (a, b) {
        (Health::Racing, _) | (_, Health::Racing) => Health::Racing,
        (Health::Suspicious, _) | (_, Health::Suspicious) => Health::Suspicious,
        _ => Health::Ok,
    }
}

/// line_notes returns every line of the voice as its notes' beat
/// intervals in row order.
fn line_notes(doc: &Document, song: &Song, voice: usize) -> Vec<Vec<(f64, f64)>> {
    song.voices
        .get(voice)
        .map(|v| {
            v.lines
                .iter()
                .map(|line| {
                    line.notes
                        .iter()
                        .filter_map(|id| song.note(doc, *id))
                        .map(|note| (note.start, note.start + note.length as f64))
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// rate_of returns a line's notes per second over its note span (first
/// note start to last note end); a line without notes or without a span
/// has the rate 0.
fn rate_of(timing: &Timing, notes: &[(f64, f64)]) -> f64 {
    let (Some(&(first, _)), Some(&(_, last))) = (notes.first(), notes.last()) else {
        return 0.0;
    };
    let seconds = (timing.beat_to_ms(last) - timing.beat_to_ms(first)) / 1000.0;
    if seconds > 0.0 {
        notes.len() as f64 / seconds
    } else {
        0.0
    }
}

/// coverage_of returns the share of the line's note time where the
/// activity is active, or `None` without an activity or note time. Every
/// note's audio interval is walked over the activity windows it touches.
fn coverage_of(timing: &Timing, notes: &[(f64, f64)], activity: Option<&Activity>) -> Option<f64> {
    let activity = activity?;
    if notes.is_empty() {
        return None;
    }
    let window = activity.bucket_ms.max(0.001);
    let mut active_ms = 0.0;
    let mut total_ms = 0.0;
    for &(first, last) in notes {
        let start_ms = timing.beat_to_ms(first);
        let end_ms = timing.beat_to_ms(last);
        total_ms += end_ms - start_ms;
        let first_bucket = (start_ms / window).floor() as isize;
        let last_bucket = (end_ms / window).ceil() as isize;
        for bucket in first_bucket..last_bucket {
            if !activity
                .active
                .get(bucket.max(0) as usize)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let bucket_start = bucket as f64 * window;
            let overlap = (bucket_start + window).min(end_ms) - bucket_start.max(start_ms);
            if overlap > 0.0 {
                active_ms += overlap;
            }
        }
    }
    if total_ms <= 0.0 {
        return None;
    }
    Some((active_ms / total_ms).clamp(0.0, 1.0))
}

/// analyze reports every line of `voice`: its rate, neighbour factor and
/// vocal coverage with the worst verdict. A song without a usable tempo
/// or an unknown voice analyzes to no lines.
pub fn analyze(
    doc: &Document,
    song: &Song,
    voice: u8,
    activity: Option<&Activity>,
    cfg: &HealthConfig,
) -> Vec<LineHealth> {
    let Some(timing) = song.timing else {
        return Vec::new();
    };
    let voice = usize::from(voice);
    if voice >= song.voices.len() {
        return Vec::new();
    }
    let lines = line_notes(doc, song, voice);
    let rates: Vec<f64> = lines.iter().map(|notes| rate_of(&timing, notes)).collect();

    lines
        .iter()
        .enumerate()
        .map(|(index, notes)| {
            // The neighbourhood median: up to cfg.neighbourhood lines
            // before and after the line (user decision 2026-11-16).
            let before = index.saturating_sub(cfg.neighbourhood);
            let after = (index + cfg.neighbourhood + 1).min(rates.len());
            let mut neighbours: Vec<f64> = rates[before..index]
                .iter()
                .chain(rates[index + 1..after].iter())
                .copied()
                .collect();
            neighbours.sort_by(|a, b| a.total_cmp(b));
            let median = match neighbours.len() {
                0 => f64::NAN,
                odd if odd % 2 == 1 => neighbours[odd / 2],
                even => (neighbours[even / 2 - 1] + neighbours[even / 2]) / 2.0,
            };
            let factor = if median.is_finite() && median > 0.0 {
                rates[index] / median
            } else {
                1.0
            };

            let rate_signal = severity_of(rates[index], cfg.rate_warn, cfg.rate_bad, false);
            // The factor has a single bound: it races at the warn value.
            let neighbour_signal = severity_of(
                factor,
                cfg.neighbour_factor_warn,
                cfg.neighbour_factor_warn,
                false,
            );
            let coverage = coverage_of(&timing, notes, activity);
            let coverage_signal = coverage
                .map(|value| severity_of(value, cfg.coverage_warn, cfg.coverage_bad, true))
                .unwrap_or(Health::Ok);
            let verdict = worst(rate_signal, worst(neighbour_signal, coverage_signal));

            // The reason names the signals at the verdict's level; a
            // calm line stays unnamed.
            let mut named: Vec<String> = Vec::new();
            let mut signals: Vec<Signal> = Vec::new();
            if verdict != Health::Ok && rate_signal == verdict {
                named.push(format!("rate {:.1}/s", rates[index]));
                signals.push(Signal::Rate);
            }
            if verdict != Health::Ok && neighbour_signal == verdict {
                named.push(format!("neighbour factor {:.1}", factor));
                signals.push(Signal::NeighbourFactor);
            }
            if verdict != Health::Ok && coverage_signal == verdict {
                named.push(format!("coverage {:.2}", coverage.unwrap_or(0.0)));
                signals.push(Signal::Coverage);
            }
            LineHealth {
                line: index,
                health: verdict,
                syllables_per_s: rates[index],
                neighbour_factor: factor,
                vocal_coverage: coverage,
                reason: if named.is_empty() {
                    "ok".to_owned()
                } else {
                    named.join(", ")
                },
                signals,
            }
        })
        .collect()
}

/// racing_sections merges neighbouring Racing lines into line ranges
/// (half open, the end is one past the last Racing line).
pub fn racing_sections(health: &[LineHealth]) -> Vec<Range<usize>> {
    let mut sections: Vec<Range<usize>> = Vec::new();
    for entry in health {
        if entry.health != Health::Racing {
            continue;
        }
        match sections.last_mut() {
            Some(last) if last.end == entry.line => last.end = entry.line + 1,
            _ => sections.push(entry.line..entry.line + 1),
        }
    }
    sections
}
