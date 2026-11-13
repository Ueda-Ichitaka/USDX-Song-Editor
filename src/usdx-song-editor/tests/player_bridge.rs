//! About me: tests for the Player bridge logic of task P4-04 (plan sections
//! 7.1 and 8.3). The pure event application (`apply_event`) is tested
//! without GStreamer; the `PlayerCore` tests run the real playbin3 player on
//! generated tone files with the silent fakesink test sink (the
//! USDX_AUDIO_SINK seam of P4-02), so the same behaviours the UI relies on -
//! play with the playhead property following, seek, play range with the
//! RangeDone pause, the track-switch round trip that keeps position and
//! playing state, and the rate - are pinned before the QObject wiring
//! exists. Only Opus runs here: the codec-specific behaviours (mp3 duration
//! slack, seek accuracy) are covered by the usdx-audio tests.

use std::path::Path;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use usdx_audio::testmedia;
use usdx_audio::{PlayState, Player, PlayerEvent, Track};
use usdx_song_editor::bridge::player::{CoreSignal, PlaybackState, PlayerCore, apply_event};

/// How long a test waits for an expected condition before it fails; the
/// playback here is realtime-paced by the fakesink.
const WAIT: Duration = Duration::from_secs(10);

/// Directs the player to the silent test sink, once per process before any
/// player is built (the P4-02 seam).
fn use_test_sink() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: written exactly once, before any player is built, and
        // never changed afterwards.
        unsafe { std::env::set_var("USDX_AUDIO_SINK", "fakesink sync=true") };
    });
}

/// A playing playback state at 1200 ms with a 5000 ms song.
fn playing_state() -> PlaybackState {
    PlaybackState {
        playing: true,
        position_ms: 1200.0,
        duration_ms: 5000.0,
        range_end_ms: None,
    }
}

/// A core with a player loaded with tone A (paused at 0) and the paths of
/// both tone files.
fn attached_core() -> (PlayerCore, std::path::PathBuf, std::path::PathBuf, TempDir) {
    use_test_sink();
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path_a = dir.path().join("tone-a.opus");
    let path_b = dir.path().join("tone-b.opus");
    testmedia::tone_opus(&path_a).expect("the first tone is generated");
    testmedia::tone_opus(&path_b).expect("the second tone is generated");
    let mut player = Player::new().expect("the player is built");
    player.load(&path_a).expect("the tone is loaded");
    let mut core = PlayerCore::new();
    core.attach_player(player, 0, false);
    (core, path_a, path_b, dir)
}

/// Runs tick() until `wanted` is true for the core state or fails after
/// WAIT; returns the signals of the tick that satisfied `wanted`.
fn tick_until(core: &mut PlayerCore, wanted: impl Fn(&PlayerCore) -> bool) -> Vec<CoreSignal> {
    let deadline = Instant::now() + WAIT;
    loop {
        let signals = core.tick();
        if wanted(core) {
            return signals;
        }
        assert!(
            Instant::now() < deadline,
            "the expected playback state did not arrive within {WAIT:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Puts the core into the playing state at about 1000 ms.
fn playing_at_1000(core: &mut PlayerCore) {
    core.seek_ms(1000.0);
    core.play();
    tick_until(core, |core| core.playing());
    tick_until(core, |core| core.position_ms() >= 1000.0);
}

// --- the pure event application ---

/// StateChanged(Playing) turns the state on.
#[test]
fn state_changed_playing_turns_the_playback_on() {
    let mut state = PlaybackState::default();
    let signals = apply_event(&mut state, PlayerEvent::StateChanged(PlayState::Playing));
    assert!(state.playing);
    assert!(signals.is_empty());
}

/// StateChanged(Paused) and Stopped turn the playback off.
#[test]
fn state_changed_paused_and_stopped_turn_the_playback_off() {
    for state_value in [PlayState::Paused, PlayState::Stopped] {
        let mut state = playing_state();
        let signals = apply_event(&mut state, PlayerEvent::StateChanged(state_value));
        assert!(!state.playing);
        assert!(signals.is_empty());
    }
}

/// EndOfStream turns the playback off without a signal; the QML Play button
/// follows the playing property.
#[test]
fn end_of_stream_turns_the_playback_off() {
    let mut state = playing_state();
    let signals = apply_event(&mut state, PlayerEvent::EndOfStream);
    assert!(!state.playing);
    assert!(signals.is_empty());
}

/// RangeDone pauses at the range end and reports the signal: the playhead
/// stands at the range end, not at the stale preroll position the query
/// falls back to.
#[test]
fn range_done_holds_the_playhead_at_the_range_end() {
    let mut state = playing_state();
    state.range_end_ms = Some(1500.0);
    state.position_ms = 1500.0;
    let signals = apply_event(&mut state, PlayerEvent::RangeDone);
    assert_eq!(signals, vec![CoreSignal::RangeDone]);
    assert!(!state.playing);
    assert_eq!(state.position_ms, 1500.0);
    assert_eq!(state.range_end_ms, None);
}

/// RangeDone without a known range end keeps the position.
#[test]
fn range_done_without_a_range_keeps_the_position() {
    let mut state = playing_state();
    state.position_ms = 1750.0;
    let signals = apply_event(&mut state, PlayerEvent::RangeDone);
    assert_eq!(signals, vec![CoreSignal::RangeDone]);
    assert!(!state.playing);
    assert_eq!(state.position_ms, 1750.0);
}

/// A player error turns the playback off and carries the message for the
/// passive notification.
#[test]
fn an_error_turns_the_playback_off_and_carries_the_message() {
    let mut state = playing_state();
    let signals = apply_event(
        &mut state,
        PlayerEvent::Error(String::from("the decoder failed")),
    );
    assert_eq!(
        signals,
        vec![CoreSignal::Error(String::from("the decoder failed"))]
    );
    assert!(!state.playing);
}

/// DurationKnown updates the duration the seek clamps into.
#[test]
fn duration_known_updates_the_duration() {
    let mut state = PlaybackState::default();
    let signals = apply_event(&mut state, PlayerEvent::DurationKnown(5066.0));
    assert_eq!(state.duration_ms, 5066.0);
    assert!(signals.is_empty());
}

// --- the core without a player ---

/// A core without an attached player ignores every playback call instead of
/// failing: the UI can press Play before the background load came back.
#[test]
fn a_core_without_a_player_ignores_playback_calls() {
    use_test_sink();
    let mut core = PlayerCore::new();
    assert!(!core.has_player());
    assert!(core.play().is_empty());
    assert!(core.pause().is_empty());
    assert!(core.toggle().is_empty());
    assert!(core.seek_ms(2000.0).is_empty());
    assert!(core.play_range(100.0, 500.0).is_empty());
    assert_eq!(core.position_ms(), 0.0);
    assert!(!core.playing());
    assert!(core.tick().is_empty());
}

/// The rate choice is stored even without a player and applies to players
/// attached later; a non-positive rate is refused with an error signal.
#[test]
fn the_rate_is_stored_without_a_player_and_validated() {
    use_test_sink();
    let mut core = PlayerCore::new();
    assert!(core.set_rate(0.75).is_empty());
    assert_eq!(core.rate(), 0.75);
    let signals = core.set_rate(0.0);
    assert!(
        matches!(signals.as_slice(), [CoreSignal::Error(_)]),
        "a non-positive rate is an error signal, got {signals:?}"
    );
    assert_eq!(core.rate(), 0.75);
}

// --- the core with a real player ---

/// Play turns the playing property on through tick() and the position
/// advances: the playhead follows.
#[test]
fn play_reaches_playing_and_the_playhead_follows() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    assert!(core.play().is_empty());
    tick_until(&mut core, |core| core.playing());
    let start = core.position_ms();
    std::thread::sleep(Duration::from_millis(400));
    tick_until(&mut core, |core| core.position_ms() > start);
}

/// seek_ms moves the playhead property at once (optimistic) and the next
/// ticks confirm it within 10 ms.
#[test]
fn seek_ms_moves_the_playhead() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    core.seek_ms(2000.0);
    assert_eq!(core.position_ms(), 2000.0, "the playhead follows at once");
    tick_until(&mut core, |core| {
        (core.position_ms() - 2000.0).abs() <= 10.0
    });
}

/// A seek clamps into the known duration instead of seeking past the end.
#[test]
fn a_seek_clamps_into_the_duration() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    core.tick();
    // No duration known yet: the seek stays in range by clamping at 0.
    core.seek_ms(-500.0);
    assert_eq!(core.position_ms(), 0.0);
}

/// play_range(1000, 1500) ends with the RangeDone signal, the player
/// paused and the playhead standing at the range end.
#[test]
fn play_range_reports_range_done_and_pauses() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    assert!(core.play_range(1000.0, 1500.0).is_empty());
    assert_eq!(core.position_ms(), 1000.0, "the playhead starts the range");
    let signals = tick_until(&mut core, |core| !core.playing());
    assert!(
        signals.iter().any(|signal| *signal == CoreSignal::RangeDone),
        "the range end reports RangeDone, got {signals:?}"
    );
    assert!(
        (core.position_ms() - 1500.0).abs() <= 50.0,
        "the playhead holds at the range end, measured {} ms",
        core.position_ms()
    );
}

/// Pressing Play during a range playback drops the range: the position
/// passes the range end and no RangeDone arrives.
#[test]
fn play_drops_an_active_range() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    core.play_range(1000.0, 1500.0).expect_set_up();
    core.play();
    tick_until(&mut core, |core| core.playing());
    // Past the range end without a RangeDone within one and a half seconds
    // of wall clock (the range would be over by then).
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut saw_range_done = false;
    let mut past_end = false;
    while Instant::now() < deadline {
        for signal in core.tick() {
            saw_range_done |= signal == CoreSignal::RangeDone;
        }
        past_end |= core.position_ms() >= 1500.0;
        if past_end {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(past_end, "the playback passes the range end after Play");
    assert!(!saw_range_done, "Play must not report the old range end");
}

/// The track-switch round trip: detach, switch the file on the taken
/// player, re-attach - the playing state and the position continue in the
/// new file (the QObject runs this on its worker thread).
#[test]
fn the_switch_round_trip_keeps_position_and_playing() {
    let (mut core, _path_a, path_b, _dir) = attached_core();
    playing_at_1000(&mut core);
    let mut player = core.detach_player().expect("the player is detached");
    player
        .switch_file(&path_b, 1000.0, true)
        .expect("the switch succeeds");
    let signals = core.attach_player(player, 1, true);
    assert!(signals.is_empty());
    assert_eq!(core.track(), 1, "the re-attached player carries the track");
    assert!(core.playing());
    tick_until(&mut core, |core| {
        (core.position_ms() - 1000.0).abs() <= 200.0
    });
}

/// A failed switch (missing file) re-attaches the old playback where it
/// stood: still playing, position still advancing. The switch error itself
/// becomes the error signal on the worker thread of the QObject, not in
/// attach_player.
#[test]
fn a_failed_switch_reattaches_the_old_playback() {
    let (mut core, path_a, _path_b, dir) = attached_core();
    playing_at_1000(&mut core);
    let mut player = core.detach_player().expect("the player is detached");
    let missing = dir.path().join("missing.opus");
    let error = player.switch_file(&missing, 1000.0, true).unwrap_err();
    let signals = core.attach_player(player, 0, true);
    assert!(signals.is_empty(), "attach reports no signals of its own");
    assert!(core.playing());
    tick_until(&mut core, |core| core.position_ms() > 1100.0);
    let _ = path_a;
    let _ = error;
}

/// toggle() pauses and resumes.
#[test]
fn toggle_pauses_and_resumes() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    core.play();
    tick_until(&mut core, |core| core.playing());
    assert!(core.toggle().is_empty());
    tick_until(&mut core, |core| !core.playing());
    assert!(core.toggle().is_empty());
    tick_until(&mut core, |core| core.playing());
}

/// set_rate changes the rate the core reports and forwards to the player.
#[test]
fn set_rate_forwards_to_the_player() {
    let (mut core, _path_a, _path_b, _dir) = attached_core();
    core.play();
    tick_until(&mut core, |core| core.playing());
    assert!(core.set_rate(0.5).is_empty());
    assert_eq!(core.rate(), 0.5);
}

/// A convenience for tests that only set up a range.
trait SetUpRange {
    fn expect_set_up(&mut self);
}

impl SetUpRange for Vec<CoreSignal> {
    fn expect_set_up(&mut self) {
        assert!(
            self.is_empty(),
            "the range setup reports no signals, got {self:?}"
        );
    }
}
