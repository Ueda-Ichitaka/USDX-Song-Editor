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
//! A second opt-in report compares the analysis with the user's labels of
//! known good and known bad songs (tests/health_labels.tsv, 2026-11-18): per
//! song the racing and suspicious lines with the defaults (the past-end
//! check included, with the main audio's length from the usdx-audio probe),
//! the signal extremes, the whole-note-time coverage and the lines past the
//! audio end, then a threshold grid ranked by flagged bad minus flagged good
//! songs; it runs with `USDX_FACTORY_OUTPUT=<Factory>/stack/output`.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use usdx_edit::health::{Health, HealthConfig, Signal, analyze, analyze_with_end};
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
                                // This report runs without the audio end.
                                Signal::PastEnd => continue,
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
    songs_racing.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    println!("songs with the most racing lines:");
    for (racing_lines, lines, name) in songs_racing.iter().take(15) {
        println!("  {racing_lines:3} of {lines:3}  {name}");
    }
    let clean = songs_racing
        .iter()
        .filter(|(count, _, _)| *count == 0)
        .count();
    println!(
        "songs without a racing line: {clean} of {}",
        songs_racing.len()
    );
}

/// One labelled song of tests/health_labels.tsv.
struct Labelled {
    good: bool,
    note: String,
    name: String,
    doc: Document,
    song: Song,
    activity: Option<usdx_audio::Activity>,
    audio_end_ms: Option<f64>,
}

/// racing_lines counts a song's racing lines under a configuration.
fn racing_lines(entry: &Labelled, cfg: &HealthConfig) -> usize {
    (0..entry.song.voices.len())
        .map(|voice| {
            analyze_with_end(
                &entry.doc,
                &entry.song,
                voice as u8,
                entry.activity.as_ref(),
                entry.audio_end_ms,
                cfg,
            )
            .iter()
            .filter(|line| line.health == Health::Racing)
            .count()
        })
        .sum()
}

#[test]
#[ignore = "run by hand with USDX_FACTORY_OUTPUT for the threshold tuning (gate G-06)"]
fn the_health_signals_against_the_users_labels() {
    let Some(output) = std::env::var_os("USDX_FACTORY_OUTPUT") else {
        eprintln!("skipped: USDX_FACTORY_OUTPUT is not set");
        return;
    };
    let output = PathBuf::from(output);
    let labels = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/health_labels.tsv"),
    )
    .expect("the labels file reads");
    let cancel = AtomicBool::new(false);
    let mut songs = Vec::new();
    for line in labels
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let path = output.join(fields[2]);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("missing: {}", path.display());
            continue;
        };
        let doc = classify(decode(&bytes).expect("the song decodes"));
        let song = Song::from_document(&doc);
        let activity = song
            .headers
            .get("VOCALS")
            .map(|vocals| path.with_file_name(vocals.trim()))
            .filter(|vocals| vocals.is_file())
            .and_then(|vocals| usdx_audio::analyze(&vocals, BUCKET_MS, &cancel, &|_| {}).ok())
            .map(|(_, activity)| activity);
        let audio_end_ms = song
            .headers
            .get("AUDIO")
            .or_else(|| song.headers.get("MP3"))
            .map(|audio| path.with_file_name(audio.trim()))
            .and_then(|audio| usdx_audio::duration_ms(&audio).ok());
        let name = path
            .parent()
            .and_then(|dir| dir.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        songs.push(Labelled {
            good: fields[0] == "good",
            note: fields[1].to_owned(),
            name,
            doc,
            song,
            activity,
            audio_end_ms,
        });
    }
    println!(
        "{} labelled songs: {} good, {} bad",
        songs.len(),
        songs.iter().filter(|s| s.good).count(),
        songs.iter().filter(|s| !s.good).count()
    );

    // Per song with the defaults: racing / suspicious lines, the signal
    // extremes and the notes after the end of the main audio.
    let cfg = HealthConfig::default();
    println!("label  racing susp lines maxrate maxfactor mincov songcov afterend  song (note)");
    for entry in &songs {
        let (mut racing, mut suspicious, mut lines) = (0, 0, 0);
        let (mut max_rate, mut max_factor, mut min_cov) = (0.0f64, 0.0f64, f64::INFINITY);
        for voice in 0..entry.song.voices.len() {
            for line in analyze_with_end(
                &entry.doc,
                &entry.song,
                voice as u8,
                entry.activity.as_ref(),
                entry.audio_end_ms,
                &cfg,
            ) {
                lines += 1;
                match line.health {
                    Health::Racing => racing += 1,
                    Health::Suspicious => suspicious += 1,
                    Health::Ok => {}
                }
                max_rate = max_rate.max(line.syllables_per_s);
                max_factor = max_factor.max(line.neighbour_factor);
                if let Some(coverage) = line.vocal_coverage {
                    min_cov = min_cov.min(coverage);
                }
            }
        }
        // Notes that start after the main audio (#AUDIO, else #MP3) ends.
        let audio_end_ms = entry.audio_end_ms;
        // The share of the song's whole note time with active vocals.
        let song_coverage = match (&entry.activity, entry.song.timing) {
            (Some(activity), Some(timing)) => {
                let (mut total, mut heard) = (0.0f64, 0.0f64);
                for note in entry
                    .song
                    .voices
                    .iter()
                    .flat_map(|voice| voice.lines.iter())
                    .flat_map(|line| line.notes.iter())
                    .filter_map(|id| entry.song.note(&entry.doc, *id))
                {
                    let start = timing.beat_to_ms(note.start);
                    let end = timing.beat_to_ms(note.start + note.length as f64);
                    let mut t = start;
                    while t < end {
                        let window = (t / activity.bucket_ms) as usize;
                        total += 1.0;
                        if activity.active.get(window).copied().unwrap_or(false) {
                            heard += 1.0;
                        }
                        t += activity.bucket_ms;
                    }
                }
                if total > 0.0 {
                    format!("{:.2}", heard / total)
                } else {
                    "-".to_owned()
                }
            }
            _ => "-".to_owned(),
        };
        let after_end = match (audio_end_ms, entry.song.timing) {
            (Some(audio_end), Some(timing)) => entry
                .song
                .voices
                .iter()
                .flat_map(|voice| voice.lines.iter())
                .flat_map(|line| line.notes.iter())
                .filter_map(|id| entry.song.note(&entry.doc, *id))
                .filter(|note| timing.beat_to_ms(note.start) > audio_end)
                .count()
                .to_string(),
            _ => "-".to_owned(),
        };
        println!(
            "{:5} {:6} {:4} {:5} {:7.1} {:9.1} {:6} {:>7} {:>8}  {} {}",
            if entry.good { "good" } else { "bad" },
            racing,
            suspicious,
            lines,
            max_rate,
            max_factor,
            if min_cov.is_finite() {
                format!("{min_cov:.2}")
            } else {
                "-".to_owned()
            },
            song_coverage,
            after_end,
            entry.name,
            if entry.note.is_empty() {
                String::new()
            } else {
                format!("({})", entry.note)
            }
        );
    }

    // The grid: a song is flagged when at least `min_lines` of its lines
    // race. Ranked by flagged bad songs minus flagged good songs.
    let mut results = Vec::new();
    for rate_bad in [8.0, 9.0, 10.0, 11.0, 12.0, 14.0] {
        for factor in [2.0, 2.5, 3.0, 3.5, 4.0, 100.0] {
            for coverage_bad in [0.0, 0.1, 0.2, 0.3] {
                for min_lines in [1usize, 2, 3] {
                    let cfg = HealthConfig {
                        rate_bad,
                        rate_warn: rate_bad * 0.7,
                        neighbour_factor_warn: factor,
                        coverage_bad,
                        coverage_warn: coverage_bad.max(0.4),
                        ..HealthConfig::default()
                    };
                    let (mut bad_hit, mut good_hit) = (0, 0);
                    for entry in &songs {
                        if racing_lines(entry, &cfg) >= min_lines {
                            if entry.good {
                                good_hit += 1;
                            } else {
                                bad_hit += 1;
                            }
                        }
                    }
                    results.push((
                        bad_hit as i64 - good_hit as i64,
                        bad_hit,
                        good_hit,
                        rate_bad,
                        factor,
                        coverage_bad,
                        min_lines,
                    ));
                }
            }
        }
    }
    results.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));
    println!(
        "best grid points (bad flagged / good flagged; rate_bad, factor, coverage_bad, min racing lines):"
    );
    for (_, bad_hit, good_hit, rate_bad, factor, coverage_bad, min_lines) in results.iter().take(12)
    {
        println!(
            "  {bad_hit:2} / {good_hit:2}  rate {rate_bad}, factor {factor}, coverage {coverage_bad}, lines {min_lines}"
        );
    }
    for (label, factor, coverage_bad) in [("untuned", 2.5, 0.2), ("tuned", 2.0, 0.0)] {
        let point = results
            .iter()
            .find(|r| r.3 == 10.0 && r.4 == factor && r.5 == coverage_bad && r.6 == 1)
            .expect("the defaults are on the grid");
        println!("the {label} defaults: {} / {}", point.1, point.2);
    }
}
