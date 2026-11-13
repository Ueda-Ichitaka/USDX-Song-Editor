//! About me: opt-in integration test of the usdx-audio library on the real
//! USDX Song Factory songs the user approved (plan section 9.5, tasks P4-02
//! and P4-03). The tests are #[ignore]d so the regular check stays fast and
//! works without the Factory repository; they run with
//! `cargo test -p usdx-audio --test real_songs -- --ignored` and take at
//! most `USDX_AUDIO_SAMPLE` songs (default 3) from a fixed pick list written
//! here, so runs are reproducible. The corpus is read-only: the songs are
//! only opened, never modified or copied.

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use usdx_audio::{Player, analyze};

/// Approved folders (plan section 9.5); not `repaired/gap/` or anything else.
const FACTORY_NEW: &str = "/home/ueda/workspace/USDX-Song-Factory/stack/output/new";
const FACTORY_RECREATED: &str =
    "/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/recreated";

/// The fixed pick list: two opus songs from `new/` and one mp3 song from
/// `repaired/recreated/`.
const PICKED_SONGS: &[(&str, &str)] = &[
    (FACTORY_NEW, "ASP - Betteljunge"),
    (FACTORY_NEW, "Alstroemeria Records - Bad Apple!!"),
    (FACTORY_RECREATED, "Blind Guardian - Majesty"),
];

/// How many songs this run takes, capped by USDX_AUDIO_SAMPLE (default 3).
fn sample_count() -> usize {
    std::env::var("USDX_AUDIO_SAMPLE")
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(3)
}

/// Directs the player to the silent test sink, once per process, so the
/// real songs are never played out loud.
fn use_test_sink() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: written exactly once, before any player is built, and
        // never changed afterwards.
        unsafe { std::env::set_var("USDX_AUDIO_SINK", "fakesink sync=true") };
    });
}

/// The song file and the mix audio of one picked song folder.
fn song_files(folder: &str, name: &str) -> Option<(PathBuf, PathBuf)> {
    let dir = Path::new(folder).join(name);
    let entries = std::fs::read_dir(&dir).ok()?;
    let txt = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "txt")
                && !matches!(
                    path.file_stem().and_then(|stem| stem.to_str()),
                    Some("lyrics") | Some("language")
                )
        })?;
    let stem = txt.file_stem()?.to_str()?.to_string();
    let audio = ["opus", "mp3"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())?;
    Some((txt, audio))
}

/// The picked songs that exist on this machine.
fn opt_in_songs() -> Vec<(PathBuf, PathBuf)> {
    PICKED_SONGS
        .iter()
        .take(sample_count())
        .filter_map(|(folder, name)| song_files(folder, name))
        .collect()
}

/// Waits until `condition` holds; fails after WAIT.
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "the expected condition did not arrive within 10 s"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Opt-in player test on the real opus and mp3 files: load, the duration is
/// known and plausible, a paused seek to 2 s lands within 10 ms.
#[test]
#[ignore = "opt-in: needs the USDX Song Factory corpus; run with --ignored (USDX_AUDIO_SAMPLE caps the songs, default 3)"]
fn player_loads_and_seeks_real_songs() {
    use_test_sink();
    let songs = opt_in_songs();
    if songs.is_empty() {
        eprintln!("skipped: the approved Factory folders are not available");
        return;
    }
    for (_txt, audio) in songs {
        let mut player = Player::new().expect("the player is built");
        player.load(&audio).expect("the song audio loads");
        player.play();
        // The mp3 duration becomes known only after a moment of playback.
        wait_for(|| player.duration_ms().is_some());
        let duration = player.duration_ms().expect("the duration is known");
        assert!(
            duration > 30_000.0,
            "a real song lasts more than 30 s, measured {duration} ms"
        );
        player.pause();
        player.seek_ms(2000.0).expect("the seek succeeds");
        wait_for(|| player.position_ms().is_some());
        let position = player.position_ms().expect("the position is known");
        assert!(
            (position - 2000.0).abs() <= 10.0,
            "the seek lands within 10 ms of 2000 ms, measured {position} ms"
        );
    }
}

/// Opt-in analysis test on the real opus and mp3 files: the analysis runs,
/// the bucket arrays align, and one activity window spans two peak buckets.
/// The semantic quality of the activity curve is NOT asserted here: measured
/// on the real songs, the plan's starting thresholds (floor at the 10th
/// percentile, active above floor + 12 dB) yield 0 active windows for the
/// heavily mastered Bad Apple!! (whole dynamic range about 9 dB) and 99 %
/// active for the mp3 with long digital silence. Tuning those values is task
/// P7-10 with gate G-06.
#[test]
#[ignore = "opt-in: needs the USDX Song Factory corpus; run with --ignored (USDX_AUDIO_SAMPLE caps the songs, default 3)"]
fn analysis_runs_on_real_songs() {
    let songs = opt_in_songs();
    if songs.is_empty() {
        eprintln!("skipped: the approved Factory folders are not available");
        return;
    }
    for (_txt, audio) in songs {
        let cancel = AtomicBool::new(false);
        let (peaks, activity) = analyze(&audio, 10.0, &cancel, &|_| {})
            .expect("the analysis of the real song succeeds");
        assert_eq!(peaks.min.len(), peaks.max.len(), "min and max align");
        assert_eq!(peaks.min.len(), peaks.rms.len(), "min and rms align");
        assert_eq!(
            activity.active.len(),
            peaks.rms.len().div_ceil(2),
            "one activity window spans two peak buckets"
        );
        assert!(activity.floor_db.is_finite(), "the noise floor is finite");
    }
}
