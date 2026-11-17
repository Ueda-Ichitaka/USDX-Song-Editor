//! About me: tests for the health analysis (task P7-05, plan section 6.10).
//! The analysis reports per line the syllable rate (notes per second of
//! the line's note span), the neighbour factor (the line's rate against
//! the median of its neighbourhood = up to 6 lines before and 6 after,
//! user decision 2026-11-16) and the vocal coverage (the share of the
//! line's note time where the passed `usdx-audio` `Activity` is active;
//! `None` without an activity). The worst signal decides the health
//! (`Ok`, `Suspicious`, `Racing`) and the reason names the signals at
//! that level. The fixtures use BPM 300 (one beat is 50 ms) and GAP 1000
//! (beat 0 at 1 s), so a note's audio time is `1000 + beat * 50` ms.

use usdx_audio::Activity;
use usdx_edit::health::{Health, HealthConfig, LineHealth, Signal, analyze, racing_sections};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, classify};
use usdx_format::song::Song;

/// loaded classifies a song text.
fn loaded(text: &str) -> (Document, Song) {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    (doc, song)
}

/// song_text builds a song from per-line note lists: each line is a list
/// of (start beat, length) pairs, the lines sit far apart so their
/// neighbourhoods are the lines themselves.
fn song_text(lines: &[&[(i64, i64)]]) -> String {
    let mut text = String::from("#TITLE:T\n#BPM:300\n#GAP:1000\n");
    for (i, notes) in lines.iter().enumerate() {
        for &(start, length) in *notes {
            text += &format!(": {start} {length} 0 a \n");
        }
        if i + 1 < lines.len() {
            let end = notes.iter().map(|&(s, l)| s + l).max().unwrap_or(0);
            text += &format!("- {end}\n");
        }
    }
    text += "E\n";
    text
}

/// health_of analyzes voice 0 of a song text without an activity.
fn health_of(text: &str, cfg: &HealthConfig) -> Vec<LineHealth> {
    let (doc, song) = loaded(text);
    analyze(&doc, &song, 0, None, cfg)
}

/// activity_of builds an activity with 100 windows of `window_ms` ms and
/// the listed window indexes active.
fn activity_of(window_ms: f64, active: &[usize]) -> Activity {
    let mut flags = vec![false; 100];
    for &index in active {
        flags[index] = true;
    }
    Activity {
        bucket_ms: window_ms,
        active: flags,
        floor_db: -50.0,
    }
}

#[test]
fn calm_lines_report_their_rates_and_stay_ok() {
    // Each line: 2 notes over a 10-beat span (0.5 s), so 4.0/s; the
    // neighbour rates are equal, so the factor is 1.0 and no signal
    // fires. Without an activity the coverage is None.
    let text = song_text(&[&[(0, 4), (6, 4)], &[(12, 4), (18, 4)]]);
    let health = health_of(&text, &HealthConfig::default());
    assert_eq!(health.len(), 2);
    for (line, lh) in health.iter().enumerate() {
        assert_eq!(lh.line, line);
        assert_eq!(lh.health, Health::Ok);
        assert!(
            (lh.syllables_per_s - 4.0).abs() < 1e-9,
            "{}",
            lh.syllables_per_s
        );
        assert!((lh.neighbour_factor - 1.0).abs() < 1e-9);
        assert_eq!(lh.vocal_coverage, None);
        assert_eq!(lh.reason, "ok");
    }
}

#[test]
fn the_rate_signals_warn_and_race() {
    // A 20/s line next to an equally fast line (the factor stays 1.0)
    // races by rate alone; an 8/s line next to a 4/s line (the factor 2.0
    // stays under the 2.5 warn) is only suspicious.
    let fast = song_text(&[&[(0, 1), (1, 1)], &[(12, 1), (13, 1)]]);
    let health = health_of(&fast, &HealthConfig::default());
    assert_eq!(health[0].health, Health::Racing);
    assert_eq!(health[0].reason, "rate 20.0/s");
    assert_eq!(health[0].signals, vec![Signal::Rate]);
    assert!((health[0].neighbour_factor - 1.0).abs() < 1e-9);
    assert_eq!(health[1].health, Health::Racing);

    let medium = song_text(&[&[(0, 2), (3, 2)], &[(12, 4), (18, 4)]]);
    let health = health_of(&medium, &HealthConfig::default());
    assert_eq!(health[0].health, Health::Suspicious);
    assert_eq!(health[0].reason, "rate 8.0/s");
    assert!((health[0].neighbour_factor - 2.0).abs() < 1e-9);
    assert_eq!(health[1].health, Health::Ok);
}

#[test]
fn the_neighbour_factor_races_a_line_that_is_fast_only_relative() {
    // An 8/s line between two 2.5/s lines: the rate stays under the 10
    // bad bound, but 8 / 2.5 = 3.2 crosses the 2.5 warn, so the line
    // races by the neighbour factor alone.
    let text = song_text(&[&[(0, 8)], &[(12, 2), (15, 2)], &[(24, 8)]]);
    let health = health_of(&text, &HealthConfig::default());
    assert_eq!(health[1].health, Health::Racing);
    assert_eq!(health[1].reason, "neighbour factor 3.2");
    assert_eq!(health[1].signals, vec![Signal::NeighbourFactor]);
    // The slow lines stay ok: their neighbourhood medians (5.25) pull the
    // factor under the warn.
    assert_eq!(health[0].health, Health::Ok);
    assert_eq!(health[2].health, Health::Ok);
}

#[test]
fn the_neighbourhood_spans_six_lines_before_and_after() {
    // 13 lines: the middle line 6 is 8/s, lines 0-5 and 10-12 are 2.5/s
    // (one 8-beat note), lines 7-9 are 20/s. The 6+6 window's median is
    // 2.5 (nine slow neighbours dominate), so the factor is 3.2 and the
    // line races; a 3+3 window would see the median 11.25 and stay ok.
    let text = song_text(&[
        &[(0, 8)],
        &[(30, 8)],
        &[(60, 8)],
        &[(90, 8)],
        &[(120, 8)],
        &[(150, 8)],
        &[(180, 2), (183, 2)],
        &[(210, 1), (211, 1)],
        &[(240, 1), (241, 1)],
        &[(270, 1), (271, 1)],
        &[(300, 8)],
        &[(330, 8)],
        &[(360, 8)],
    ]);
    let health = health_of(&text, &HealthConfig::default());
    assert_eq!(health[6].health, Health::Racing);
    assert_eq!(health[6].reason, "neighbour factor 3.2");
    // The outer slow lines stay ok under the 6+6 window.
    assert_eq!(health[0].health, Health::Ok);
    assert_eq!(health[10].health, Health::Ok);
    assert_eq!(health[12].health, Health::Ok);
}

#[test]
fn the_vocal_coverage_signals_from_the_activity() {
    // One line with two 10-beat notes at beats 0 and 12: note 1 covers
    // audio ms 1000..1500 (windows 20..29), note 2 ms 1600..2100
    // (windows 32..41), 1000 ms of note time over 50 ms windows.
    let text = song_text(&[&[(0, 10), (12, 10)]]);
    let (doc, song) = loaded(&text);
    let cfg = HealthConfig::default();

    // All note windows inactive: coverage 0.0 races.
    let health = analyze(&doc, &song, 0, Some(&activity_of(50.0, &[])), &cfg);
    assert_eq!(health[0].health, Health::Racing);
    assert_eq!(health[0].reason, "coverage 0.00");
    assert!((health[0].vocal_coverage.unwrap() - 0.0).abs() < 1e-9);

    // Five of the twenty note windows active: 0.25 is only suspicious.
    let health = analyze(
        &doc,
        &song,
        0,
        Some(&activity_of(50.0, &[20, 21, 22, 23, 24])),
        &cfg,
    );
    assert_eq!(health[0].health, Health::Suspicious);
    assert_eq!(health[0].reason, "coverage 0.25");
    assert_eq!(health[0].signals, vec![Signal::Coverage]);
    assert!((health[0].vocal_coverage.unwrap() - 0.25).abs() < 1e-9);

    // All note windows active: no coverage signal.
    let active: Vec<usize> = (20..30).chain(32..42).collect();
    let health = analyze(&doc, &song, 0, Some(&activity_of(50.0, &active)), &cfg);
    assert_eq!(health[0].health, Health::Ok);
    assert_eq!(health[0].reason, "ok");
    assert!(health[0].signals.is_empty(), "a calm line names no signal");
    assert!((health[0].vocal_coverage.unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn several_racing_signals_are_named_together() {
    // A 20/s line next to a 4/s line: rate (bad) and neighbour factor
    // (3.2x4 = 5.0 over the warn) both race and the reason names both.
    let text = song_text(&[&[(0, 1), (1, 1)], &[(12, 4), (18, 4)]]);
    let health = health_of(&text, &HealthConfig::default());
    assert_eq!(health[0].health, Health::Racing);
    assert_eq!(health[0].reason, "rate 20.0/s, neighbour factor 5.0");
    assert_eq!(
        health[0].signals,
        vec![Signal::Rate, Signal::NeighbourFactor]
    );
}

#[test]
fn racing_sections_merge_neighbouring_racing_lines() {
    let entry = |line: usize, health: Health| LineHealth {
        line,
        health,
        syllables_per_s: 0.0,
        neighbour_factor: 1.0,
        vocal_coverage: None,
        reason: String::new(),
        signals: Vec::new(),
    };
    let health = vec![
        entry(0, Health::Ok),
        entry(1, Health::Racing),
        entry(2, Health::Racing),
        entry(3, Health::Ok),
        entry(4, Health::Racing),
        entry(5, Health::Ok),
        entry(6, Health::Racing),
    ];
    assert_eq!(racing_sections(&health), vec![1..3, 4..5, 6..7]);
    assert!(racing_sections(&[]).is_empty());
    assert!(racing_sections(&[entry(0, Health::Ok), entry(1, Health::Suspicious)]).is_empty());
}

#[test]
fn analyze_without_a_usable_song_or_voice_is_empty() {
    let no_timing = "#TITLE:T\n: 0 4 0 a \nE\n";
    assert!(health_of(no_timing, &HealthConfig::default()).is_empty());
    let (doc, song) = loaded(&song_text(&[&[(0, 4)]]));
    assert!(analyze(&doc, &song, 5, None, &HealthConfig::default()).is_empty());
}
