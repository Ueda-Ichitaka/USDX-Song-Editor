//! About me: tests for the editor's line health (task P7-09, UI_SPEC 4.3):
//! every line of every voice, in the lines pane's global order, gets the
//! health analysis verdict (0 ok, 1 suspicious, 2 racing) and the tooltip
//! reason of UI_SPEC 4.3 named after the first deciding signal - "Racing:
//! 11.9 syllables per second", "Racing: 3.2 times faster than the lines
//! around it", "Suspicious: vocals heard during 34 % of the note time" - or
//! "Timing looks plausible" for a calm line; a line that starts after the
//! end of the song's audio reads "Racing: starts after the audio ends" (task
//! P7-10). The thresholds come from the settings. The fixtures use BPM 300
//! (one beat is 50 ms) and GAP 1000.

use usdx_audio::Activity;
use usdx_format::bytes::decode;
use usdx_format::rows::classify;
use usdx_format::song::Song;
use usdx_song_editor::controller::line_health::{LineMark, health_config, line_marks};
use usdx_song_editor::controller::settings::HealthThresholds;

/// marks analyses a song text with the default thresholds.
fn marks(text: &str, activity: Option<&Activity>) -> Vec<LineMark> {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    line_marks(
        &doc,
        &song,
        &health_config(&HealthThresholds::default()),
        activity,
        None,
    )
}

fn mark(health: i32, reason: &str) -> LineMark {
    LineMark {
        health,
        reason: reason.to_owned(),
    }
}

#[test]
fn the_thresholds_come_from_the_settings() {
    let thresholds = HealthThresholds {
        rate_warn: 6.0,
        rate_bad: 9.0,
        neighbour_factor_warn: 2.0,
        neighbourhood: 4,
        coverage_warn: 0.5,
        coverage_bad: 0.1,
    };
    let cfg = health_config(&thresholds);
    assert_eq!(cfg.rate_warn, 6.0);
    assert_eq!(cfg.rate_bad, 9.0);
    assert_eq!(cfg.neighbour_factor_warn, 2.0);
    assert_eq!(cfg.neighbourhood, 4);
    assert_eq!(cfg.coverage_warn, 0.5);
    assert_eq!(cfg.coverage_bad, 0.1);
}

#[test]
fn a_racing_line_names_its_rate_and_a_calm_line_looks_plausible() {
    // Line 1: two 1-beat notes, 20 notes per second against an equally
    // fast line 2; line 3 is calm (4 notes per second).
    let text = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        ": 0 1 0 a \n: 1 1 0 b \n- 4\n",
        ": 12 1 0 c \n: 13 1 0 d \n- 16\n",
        ": 40 4 0 e \n: 46 4 0 f \nE\n",
    );
    let lines = marks(text, None);
    assert_eq!(lines[0], mark(2, "Racing: 20.0 syllables per second"));
    assert_eq!(lines[2], mark(0, "Timing looks plausible"));
}

#[test]
fn a_line_fast_only_against_its_neighbours_names_the_factor() {
    let text = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        ": 0 8 0 a \n- 10\n",
        ": 12 2 0 b \n: 15 2 0 c \n- 20\n",
        ": 24 8 0 d \nE\n",
    );
    let lines = marks(text, None);
    assert_eq!(
        lines[1],
        mark(2, "Racing: 3.2 times faster than the lines around it")
    );
}

#[test]
fn a_line_with_little_vocal_activity_names_the_coverage() {
    // Two 10-beat notes (audio ms 1000..1500 and 1600..2100, windows
    // 20..29 and 32..41 of 50 ms); five active windows: 25 %.
    let text = "#TITLE:T\n#BPM:300\n#GAP:1000\n: 0 10 0 a \n: 12 10 0 b \nE\n";
    let mut active = vec![false; 100];
    active[20..25].fill(true);
    let activity = Activity {
        bucket_ms: 50.0,
        active,
        floor_db: -60.0,
    };
    let lines = marks(text, Some(&activity));
    assert_eq!(
        lines[0],
        mark(1, "Suspicious: vocals heard during 25 % of the note time")
    );
}

#[test]
fn every_voice_follows_in_the_global_line_order() {
    let text = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        "P1\n: 40 4 0 a \n: 46 4 0 b \n",
        "P2\n: 0 1 0 c \n: 1 1 0 d \nE\n",
    );
    let lines = marks(text, None);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].health, 0);
    assert_eq!(lines[1], mark(2, "Racing: 20.0 syllables per second"));
}

#[test]
fn a_line_after_the_audio_end_names_the_end() {
    // Line 1 starts at 1000 ms, line 2 at 3000 ms; the audio ends at
    // 2500 ms.
    let text = concat!(
        "#TITLE:T\n#BPM:300\n#GAP:1000\n",
        ": 0 4 0 a \n: 6 4 0 b \n- 12\n",
        ": 40 4 0 c \n: 46 4 0 d \nE\n",
    );
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    let cfg = health_config(&HealthThresholds::default());
    let lines = line_marks(&doc, &song, &cfg, None, Some(2500.0));
    assert_eq!(lines[0], mark(0, "Timing looks plausible"));
    assert_eq!(lines[1], mark(2, "Racing: starts after the audio ends"));
}
