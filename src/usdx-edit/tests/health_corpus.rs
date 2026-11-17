//! About me: the opt-in health report over the real test songs (task
//! P7-10, plan 6.10 and 9.5, gate G-06). For every song of the
//! `USDX_CORPUS` folders that names an existing #VOCALS track it analyses
//! the track's vocal activity (usdx-audio, cached under the user's cache
//! folder; the song folders are only read), runs the health analysis of
//! every voice with the default thresholds, and prints the distributions
//! of the three signals (syllable rate, neighbour factor, vocal coverage),
//! the verdict counts with the deciding signals and the songs with the
//! most racing lines, so the
//! thresholds can be tuned with the user. It asserts nothing about the
//! values; it is ignored by default and runs with
//! `USDX_CORPUS=... scripts/dev.sh test usdx-edit --test health_corpus -- --ignored --nocapture`.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use usdx_edit::health::{Health, HealthConfig, Signal, analyze};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;

/// The peak bucket of the editor's analysis (the cache key includes it).
const BUCKET_MS: f64 = 10.0;

/// song_documents walks a corpus folder for the song files.
fn song_documents(root: &Path) -> Vec<(PathBuf, Document)> {
    let helpers = ["lyrics.txt", "language.txt", "song_end.txt", "anchors.txt"];
    let mut songs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if !name.ends_with(".txt") || helpers.contains(&name.as_str()) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(file) = decode(&bytes) else {
                continue;
            };
            let doc = classify(file);
            if doc.rows.iter().any(
                |row| matches!(&row.kind, RowKind::Header { key, value } if key == "BPM" && !value.is_empty()),
            ) {
                songs.push((path, doc));
            }
        }
    }
    songs.sort_by(|left, right| left.0.cmp(&right.0));
    songs
}

/// percentile returns the value at the share `p` (0..=1) of the sorted
/// values, nearest rank.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

/// report prints one signal's distribution.
fn report(name: &str, values: &mut [f64]) {
    values.sort_by(f64::total_cmp);
    let points = [0.01, 0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99, 1.0];
    let text: Vec<String> = points
        .iter()
        .map(|p| format!("p{:.0} {:.2}", p * 100.0, percentile(values, *p)))
        .collect();
    println!("{name} ({} lines): {}", values.len(), text.join(", "));
}

#[test]
#[ignore = "run by hand with USDX_CORPUS for the threshold tuning (gate G-06)"]
fn the_health_signals_over_the_corpus() {
    let Some(corpus) = std::env::var_os("USDX_CORPUS") else {
        eprintln!("skipped: USDX_CORPUS is not set");
        return;
    };
    let cfg = HealthConfig::default();
    let cancel = AtomicBool::new(false);
    let (mut rates, mut factors, mut coverages) = (Vec::new(), Vec::new(), Vec::new());
    let (mut ok, mut suspicious, mut racing) = (0usize, 0usize, 0usize);
    // The lines whose verdict a signal decided, per verdict level.
    let mut by_signal = [[0usize; 3]; 2];
    let mut songs_racing: Vec<(usize, usize, String)> = Vec::new();
    let mut analysed = 0;
    for folder in std::env::split_paths(&corpus) {
        for (path, doc) in song_documents(&folder) {
            let song = Song::from_document(&doc);
            let Some(vocals) = song.headers.get("VOCALS") else {
                continue;
            };
            let vocals_path = path.with_file_name(vocals.trim());
            if !vocals_path.is_file() {
                continue;
            }
            let activity = match usdx_audio::analyze(&vocals_path, BUCKET_MS, &cancel, &|_| {}) {
                Ok((_, activity)) => activity,
                Err(error) => {
                    eprintln!("{}: the vocals do not analyse: {error}", path.display());
                    continue;
                }
            };
            analysed += 1;
            let (mut lines, mut racing_lines) = (0, 0);
            for voice in 0..song.voices.len() {
                for line in analyze(&doc, &song, voice as u8, Some(&activity), &cfg) {
                    lines += 1;
                    rates.push(line.syllables_per_s);
                    factors.push(line.neighbour_factor);
                    if let Some(coverage) = line.vocal_coverage {
                        coverages.push(coverage);
                    }
                    let level = match line.health {
                        Health::Suspicious => Some(0),
                        Health::Racing => Some(1),
                        Health::Ok => None,
                    };
                    if let Some(level) = level {
                        for signal in &line.signals {
                            let index = match signal {
                                Signal::Rate => 0,
                                Signal::NeighbourFactor => 1,
                                Signal::Coverage => 2,
                            };
                            by_signal[level][index] += 1;
                        }
                    }
                    match line.health {
                        Health::Ok => ok += 1,
                        Health::Suspicious => suspicious += 1,
                        Health::Racing => {
                            racing += 1;
                            racing_lines += 1;
                        }
                    }
                }
            }
            let name = path
                .parent()
                .and_then(|dir| dir.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            songs_racing.push((racing_lines, lines, name));
        }
    }
    println!("analysed {analysed} songs with a #VOCALS track");
    report("syllable rate /s", &mut rates);
    report("neighbour factor", &mut factors);
    report("vocal coverage", &mut coverages);
    println!(
        "verdicts with the defaults (rate {}/{}, factor {}, coverage {}/{}): {ok} ok, {suspicious} suspicious, {racing} racing",
        cfg.rate_warn, cfg.rate_bad, cfg.neighbour_factor_warn, cfg.coverage_warn, cfg.coverage_bad
    );
    for (level, name) in [(0, "suspicious"), (1, "racing")] {
        println!(
            "{name} lines decided by: rate {}, neighbour factor {}, coverage {}",
            by_signal[level][0], by_signal[level][1], by_signal[level][2]
        );
    }
    songs_racing.sort_by(|a, b| b.0.cmp(&a.0));
    println!("songs with the most racing lines:");
    for (racing_lines, lines, name) in songs_racing.iter().take(15) {
        println!("  {racing_lines:3} of {lines:3}  {name}");
    }
    let clean = songs_racing.iter().filter(|(count, _, _)| *count == 0).count();
    println!("songs without a racing line: {clean} of {}", songs_racing.len());
}
