//! About me: the playbin3-based player of the usdx-audio library (plan
//! section 7.1, task P4-02). It exists so the editor can play a song file at
//! any position and speed: it wraps one playbin3 element with scaletempo as
//! the audio filter, exposes load/play/pause/seek/play-range/rate, and
//! reports bus messages as PlayerEvents through poll() so the caller needs
//! no GLib main loop. Tests drive it with the fakesink seam (USDX_AUDIO_SINK)
//! so no audio is played out loud.

use std::path::Path;

use gst::prelude::*;
use gstreamer as gst;

/// The audio track of a song a playback request addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Mix,
    Vocals,
    Instrumental,
}

/// The playback state the player reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Stopped,
    Paused,
    Playing,
}

/// An event the player observed on the GStreamer bus.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    StateChanged(PlayState),
    EndOfStream,
    RangeDone,
    Error(String),
    DurationKnown(f64),
}

/// The errors the player reports.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("the audio file does not exist: {0}")]
    File(String),
    #[error("the gstreamer player failed: {0}")]
    Gst(String),
    #[error("the analysis was cancelled")]
    Cancelled,
}

/// How long the player waits for a synchronous state change (load) before it
/// gives up; decoders on this machine reach PAUSED in well under a second.
const STATE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

/// Converts milliseconds into a GStreamer clock time for seek arguments.
fn clock_time_from_ms(ms: f64) -> gst::ClockTime {
    gst::ClockTime::from_mseconds(ms.round().max(0.0) as u64)
}

/// The GStreamer player: one playbin3 element with scaletempo, driven
/// synchronously through poll().
pub struct Player {
    playbin: gst::Element,
    bus: gst::Bus,
    rate: f64,
    duration_reported: bool,
    range_end: Option<f64>,
    last_position: Option<f64>,
    /// Set by a flush seek until the pipeline plays again: a seek makes
    /// playbin churn through Stopped and Paused states before Playing, and
    /// those transient messages would report a long-gone state.
    seek_pending: bool,
}

impl Player {
    /// Builds the player: initializes GStreamer once and creates the playbin3
    /// with scaletempo as its audio filter. The USDX_AUDIO_SINK environment
    /// variable, when set to a pipeline snippet, names the audio sink the
    /// tests use (for example "fakesink sync=true") instead of the system
    /// output.
    pub fn new() -> Result<Player, AudioError> {
        gst::init()
            .map_err(|error| AudioError::Gst(format!("gstreamer could not initialize: {error}")))?;
        let playbin = gst::ElementFactory::make("playbin3")
            .build()
            .map_err(|error| AudioError::Gst(format!("playbin3 does not build: {error}")))?;
        let scaletempo = gst::ElementFactory::make("scaletempo")
            .build()
            .map_err(|error| AudioError::Gst(format!("scaletempo does not build: {error}")))?;
        playbin.set_property("audio-filter", &scaletempo);
        if let Ok(sink) = std::env::var("USDX_AUDIO_SINK")
            && !sink.is_empty()
        {
            let sink = gst::parse::launch_full(&sink, None, gst::ParseFlags::empty()).map_err(
                |error| AudioError::Gst(format!("USDX_AUDIO_SINK does not build: {error}")),
            )?;
            playbin.set_property("audio-sink", &sink);
        }
        let bus = playbin
            .bus()
            .ok_or_else(|| AudioError::Gst(String::from("playbin3 has no bus")))?;
        Ok(Player {
            playbin,
            bus,
            rate: 1.0,
            duration_reported: false,
            range_end: None,
            last_position: None,
            seek_pending: false,
        })
    }

    /// Loads an audio file and waits until the decoder is ready (PAUSED).
    pub fn load(&mut self, path: &Path) -> Result<(), AudioError> {
        if !path.is_file() {
            return Err(AudioError::File(path.display().to_string()));
        }
        let uri = gst::glib::filename_to_uri(path, None).map_err(|error| {
            AudioError::Gst(format!("the audio path does not become a uri: {error}"))
        })?;
        self.playbin
            .set_state(gst::State::Null)
            .map_err(|error| AudioError::Gst(format!("the player does not stop: {error}")))?;
        self.playbin.set_property("uri", uri.as_str());
        self.playbin
            .set_state(gst::State::Paused)
            .map_err(|error| AudioError::Gst(format!("the player does not start: {error}")))?;
        let (result, state, _) = self.playbin.state(Some(STATE_TIMEOUT));
        if result.is_err() || state != gst::State::Paused {
            return Err(AudioError::Gst(String::from(
                "the player does not reach PAUSED within 10 s",
            )));
        }
        self.rate = 1.0;
        self.duration_reported = false;
        self.range_end = None;
        self.last_position = None;
        self.seek_pending = false;
        Ok(())
    }

    /// Resumes playback.
    pub fn play(&mut self) {
        let _ = self.playbin.set_state(gst::State::Playing);
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        self.seek_pending = false;
        let _ = self.playbin.set_state(gst::State::Paused);
    }

    /// Seeks to `ms` milliseconds into the song, accurately (FLUSH | ACCURATE).
    pub fn seek_ms(&mut self, ms: f64) -> Result<(), AudioError> {
        self.range_end = None;
        self.seek(
            ms,
            self.rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            None,
        )
    }

    /// Plays only the range from `start_ms` to `end_ms`; poll() reports
    /// RangeDone when the end is reached.
    pub fn play_range(&mut self, start_ms: f64, end_ms: f64) -> Result<(), AudioError> {
        self.seek(
            start_ms,
            self.rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE | gst::SeekFlags::SEGMENT,
            Some(end_ms),
        )?;
        self.range_end = Some(end_ms);
        self.last_position = None;
        Ok(())
    }

    /// The common seek: flushes to `start_ms` at `rate`, with an optional
    /// stop position that turns the segment flag into a range playback.
    fn seek(
        &mut self,
        start_ms: f64,
        rate: f64,
        flags: gst::SeekFlags,
        stop_ms: Option<f64>,
    ) -> Result<(), AudioError> {
        let (stop_type, stop) = match stop_ms {
            Some(ms) => (gst::SeekType::Set, Some(clock_time_from_ms(ms))),
            None => (gst::SeekType::None, None),
        };
        self.seek_pending = true;
        self.playbin
            .seek(
                rate,
                flags,
                gst::SeekType::Set,
                clock_time_from_ms(start_ms),
                stop_type,
                stop,
            )
            .map_err(|error| AudioError::Gst(format!("the seek fails: {error}")))
    }

    /// Switches to another audio file at `position_ms` while keeping the
    /// rate and, with `resume`, the playing state. The track switch of the
    /// bridge uses this so Vocals and Instrumental start where Mix stood.
    pub fn switch_file(
        &mut self,
        path: &Path,
        position_ms: f64,
        resume: bool,
    ) -> Result<(), AudioError> {
        if !path.is_file() {
            return Err(AudioError::File(path.display().to_string()));
        }
        let uri = gst::glib::filename_to_uri(path, None).map_err(|error| {
            AudioError::Gst(format!("the audio path does not become a uri: {error}"))
        })?;
        self.playbin
            .set_state(gst::State::Null)
            .map_err(|error| AudioError::Gst(format!("the player does not stop: {error}")))?;
        self.playbin.set_property("uri", uri.as_str());
        self.playbin
            .set_state(gst::State::Paused)
            .map_err(|error| AudioError::Gst(format!("the player does not start: {error}")))?;
        let (result, state, _) = self.playbin.state(Some(STATE_TIMEOUT));
        if result.is_err() || state != gst::State::Paused {
            return Err(AudioError::Gst(String::from(
                "the player does not reach PAUSED within 10 s",
            )));
        }
        // The rate survives the switch (the speed combo keeps its choice);
        // load() resets it, so the seek carries the rate explicitly.
        self.duration_reported = false;
        self.range_end = None;
        self.last_position = None;
        self.seek(
            position_ms.max(0.0),
            self.rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            None,
        )?;
        if resume {
            // The suppression stays armed: the state churn of the seek
            // arrives after this call and only Playing ends it.
            self.play();
        } else {
            // Without a resume the pipeline rests in Paused, whose state
            // message must stay reportable.
            self.seek_pending = false;
        }
        Ok(())
    }

    /// Changes the playback rate (0.5, 0.75, 1.0) by seeking with the new
    /// rate from the current position. An active play range keeps its stop
    /// position, so the rest of the range plays at the new rate.
    pub fn set_rate(&mut self, rate: f64) -> Result<(), AudioError> {
        if rate <= 0.0 || rate.is_nan() {
            return Err(AudioError::Gst(format!(
                "the rate must be positive, got {rate}"
            )));
        }
        let position = self.position_ms().ok_or_else(|| {
            AudioError::Gst(String::from(
                "the rate changes only while a position is known",
            ))
        })?;
        let flags = match self.range_end {
            Some(end) => {
                self.seek(
                    position,
                    rate,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE | gst::SeekFlags::SEGMENT,
                    Some(end),
                )?;
                self.last_position = None;
                self.rate = rate;
                return Ok(());
            }
            None => gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
        };
        self.seek(position, rate, flags, None)?;
        self.rate = rate;
        Ok(())
    }

    /// The current playback position in milliseconds, when known.
    pub fn position_ms(&self) -> Option<f64> {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(|time| time.mseconds() as f64)
    }

    /// The total duration of the loaded file in milliseconds, when known.
    pub fn duration_ms(&self) -> Option<f64> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(|time| time.mseconds() as f64)
    }

    /// Drains the GStreamer bus into events without a GLib main loop and
    /// reports the range end from the playback position.
    pub fn poll(&mut self) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        loop {
            let Some(message) = self.bus.pop() else {
                break;
            };
            match message.view() {
                gst::MessageView::StateChanged(state_changed) => {
                    let from_playbin =
                        message.src() == Some(self.playbin.upcast_ref::<gst::Object>());
                    if from_playbin {
                        match state_changed.current() {
                            gst::State::Playing => {
                                // The pipeline plays again: the transient
                                // states after a flush seek are over.
                                self.seek_pending = false;
                                events.push(PlayerEvent::StateChanged(PlayState::Playing));
                            }
                            gst::State::Paused if self.seek_pending => {}
                            gst::State::Paused => {
                                events.push(PlayerEvent::StateChanged(PlayState::Paused))
                            }
                            _ if self.seek_pending => {}
                            _ => events.push(PlayerEvent::StateChanged(PlayState::Stopped)),
                        }
                    }
                }
                gst::MessageView::Eos(_) => {
                    self.range_end = None;
                    events.push(PlayerEvent::EndOfStream);
                }
                // The bus SegmentDone from playbin3 comes from the demuxer
                // chain and is posted right after the range seek, long before
                // the audio sink renders the segment (measured: oggdemux
                // posts 4 ms after the seek, the sink finishes 500 ms later
                // and posts nothing). RangeDone therefore comes from the
                // position check below, not from this message.
                gst::MessageView::Error(error) => events.push(PlayerEvent::Error(format!(
                    "{} ({})",
                    error.error(),
                    error.debug().unwrap_or_default()
                ))),
                // The duration becomes known some time after PAUSED (the MP3
                // demuxer announces it late); the first announcement or the
                // async completion reports it once per load.
                gst::MessageView::AsyncDone(_) | gst::MessageView::DurationChanged(_) => {
                    if !self.duration_reported
                        && let Some(duration) = self.duration_ms()
                    {
                        self.duration_reported = true;
                        events.push(PlayerEvent::DurationKnown(duration));
                    }
                }
                _ => {}
            }
        }
        if let (Some(end), Some(position)) = (self.range_end, self.position_ms()) {
            // Reached the end, or the position went backwards after
            // playing towards it: the segment is complete and the
            // position query falls back to a stale preroll value.
            if position >= end || position < self.last_position.unwrap_or(position) {
                self.range_end = None;
                events.push(PlayerEvent::RangeDone);
            }
            self.last_position = Some(position);
        }
        events
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // GStreamer requires elements to be in the NULL state before their
        // last reference is dropped, or disposal prints a critical warning.
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testmedia;
    use std::time::{Duration, Instant};

    /// How long a test waits for the player to reach an expected condition
    /// before it fails; state changes on this machine take well under a second.
    const WAIT: Duration = Duration::from_secs(10);

    /// Directs the player to the silent test sink, once per process before
    /// any player is built; every test calls this first, so no Player::new
    /// reads the variable while it is written.
    fn use_test_sink() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            // SAFETY: written exactly once, before any player is built, and
            // never changed afterwards.
            unsafe { std::env::set_var("USDX_AUDIO_SINK", "fakesink sync=true") };
        });
    }

    /// Generates the 5 s tone in both formats and builds a player for each
    /// file path; the caller gets the temporary directory so the files live
    /// as long as the test.
    fn player_for_both_formats() -> Vec<(Player, std::path::PathBuf, tempfile::TempDir)> {
        use_test_sink();
        let mut players = Vec::new();
        for (generate, name) in [
            (
                testmedia::tone_opus as fn(&Path) -> Result<(), String>,
                "tone.opus",
            ),
            (testmedia::tone_mp3, "tone.mp3"),
        ] {
            let dir = tempfile::tempdir().expect("the temporary directory is created");
            let path = dir.path().join(name);
            generate(&path).expect("the test tone is generated");
            let mut player = Player::new().expect("the player is built");
            player.load(&path).expect("the tone is loaded");
            players.push((player, path, dir));
        }
        players
    }

    /// Waits until poll() produces `wanted`; fails after WAIT.
    fn wait_for(player: &mut Player, mut wanted: impl FnMut(&PlayerEvent) -> bool) -> PlayerEvent {
        let deadline = Instant::now() + WAIT;
        loop {
            for event in player.poll() {
                if wanted(&event) {
                    return event;
                }
            }
            assert!(
                Instant::now() < deadline,
                "the expected event did not arrive within {WAIT:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Plan test 1: load, play reaches PLAYING.
    #[test]
    fn load_then_play_reaches_playing() {
        for (player, _path, _dir) in player_for_both_formats() {
            let mut player = player;
            player.play();
            wait_for(&mut player, |event| {
                *event == PlayerEvent::StateChanged(PlayState::Playing)
            });
        }
    }

    /// Plan test 2: seek_ms(2000) gives position_ms within 10 ms after the
    /// seek completes (opus and mp3). The seek happens while paused, so the
    /// position is deterministic once the flush is through.
    #[test]
    fn seek_to_2000_ms_lands_within_10_ms() {
        for (mut player, _path, _dir) in player_for_both_formats() {
            player.seek_ms(2000.0).expect("the seek succeeds");
            let deadline = Instant::now() + WAIT;
            loop {
                if let Some(position) = player.position_ms() {
                    assert!(
                        (position - 2000.0).abs() <= 10.0,
                        "the seek lands within 10 ms of 2000 ms, measured {position} ms"
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the position is not reported within {WAIT:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Plan test 3: play_range(1000, 1500) ends with RangeDone within 100 ms
    /// of 1500 - measured as stream time, wall clock from the moment the
    /// state turns Playing plus the 1000 ms start offset.
    #[test]
    fn play_range_reports_rangedone_within_100_ms_of_1500() {
        for (mut player, _path, _dir) in player_for_both_formats() {
            player.play_range(1000.0, 1500.0).expect("the range starts");
            player.play();
            wait_for(&mut player, |event| {
                *event == PlayerEvent::StateChanged(PlayState::Playing)
            });
            let started = Instant::now();
            let event = wait_for(&mut player, |event| *event == PlayerEvent::RangeDone);
            let _ = event;
            let stream_ms = 1000.0 + started.elapsed().as_secs_f64() * 1000.0;
            assert!(
                (stream_ms - 1500.0).abs() <= 100.0,
                "the range ends within 100 ms of 1500 ms, measured {stream_ms} ms"
            );
        }
    }

    /// Plan test 4: rate 0.5 makes the position advance at about half speed,
    /// measured over 1 s with a 15 percent tolerance. The flush seek of
    /// set_rate briefly clears the position, so the measurement starts once
    /// it is reported again.
    #[test]
    fn half_rate_moves_position_at_half_speed() {
        for (mut player, _path, _dir) in player_for_both_formats() {
            player.play();
            wait_for(&mut player, |event| {
                *event == PlayerEvent::StateChanged(PlayState::Playing)
            });
            player.set_rate(0.5).expect("the rate changes");
            let deadline = Instant::now() + WAIT;
            let start = loop {
                if let Some(position) = player.position_ms() {
                    break position;
                }
                assert!(
                    Instant::now() < deadline,
                    "the position is not reported within {WAIT:?} after the rate change"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            std::thread::sleep(Duration::from_secs(1));
            let end = player
                .position_ms()
                .expect("the position is reported while playing");
            let advance = end - start;
            assert!(
                (advance - 500.0).abs() <= 75.0,
                "at rate 0.5 the position advances about 500 ms per second, measured {advance} ms"
            );
        }
    }

    /// Plan test 5: a missing file gives an error.
    #[test]
    fn missing_file_gives_an_error() {
        use_test_sink();
        let mut player = Player::new().expect("the player is built");
        let error = player
            .load(Path::new("/nonexistent/no-such-audio.opus"))
            .unwrap_err();
        assert!(
            matches!(error, AudioError::File(_)),
            "a missing file reports a file error, got {error:?}"
        );
    }

    /// Builds two tone files of one format in one temporary directory and a
    /// player loaded with the first one, so a switch has a second file to
    /// move to.
    fn player_with_two_files(
        extension: &str,
    ) -> (Player, std::path::PathBuf, std::path::PathBuf, tempfile::TempDir) {
        use_test_sink();
        let generate: fn(&Path) -> Result<(), String> = if extension == "mp3" {
            testmedia::tone_mp3
        } else {
            testmedia::tone_opus
        };
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path_a = dir.path().join(format!("tone-a.{extension}"));
        let path_b = dir.path().join(format!("tone-b.{extension}"));
        generate(&path_a).expect("the first tone is generated");
        generate(&path_b).expect("the second tone is generated");
        let mut player = Player::new().expect("the player is built");
        player.load(&path_a).expect("the first tone is loaded");
        (player, path_a, path_b, dir)
    }

    /// A playing player at about 1000 ms for the switch tests.
    fn playing_at_1000(player: &mut Player) {
        player.seek_ms(1000.0).expect("the seek succeeds");
        player.play();
        wait_for(player, |event| {
            *event == PlayerEvent::StateChanged(PlayState::Playing)
        });
        let deadline = Instant::now() + WAIT;
        while player.position_ms().is_none() {
            assert!(
                Instant::now() < deadline,
                "the position is not reported within {WAIT:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The track switch keeps the playing state and continues from the
    /// requested position in the new file (the bridge seeks the remembered
    /// Mix position back after the swap).
    #[test]
    fn switch_file_keeps_playing_and_position() {
        for extension in ["opus", "mp3"] {
            let (mut player, path_a, path_b, _dir) = player_with_two_files(extension);
            playing_at_1000(&mut player);
            player
                .switch_file(&path_b, 1000.0, true)
                .expect("the switch succeeds");
            wait_for(&mut player, |event| {
                *event == PlayerEvent::StateChanged(PlayState::Playing)
            });
            let deadline = Instant::now() + WAIT;
            loop {
                if let Some(position) = player.position_ms() {
                    // The position may have advanced a little between the
                    // resume and the query; 200 ms of wall clock slack.
                    assert!(
                        (position - 1000.0).abs() <= 200.0,
                        "the switch continues at 1000 ms in the new file, measured {position} ms"
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the position is not reported within {WAIT:?} after the switch"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = &path_a;
        }
    }

    /// The track switch keeps the rate: after a switch at rate 0.5 the
    /// position advances at about half speed.
    #[test]
    fn switch_file_keeps_the_rate() {
        for extension in ["opus", "mp3"] {
            let (mut player, _path_a, path_b, _dir) = player_with_two_files(extension);
            playing_at_1000(&mut player);
            player.set_rate(0.5).expect("the rate changes");
            let deadline = Instant::now() + WAIT;
            let start = loop {
                if let Some(position) = player.position_ms() {
                    break position;
                }
                assert!(
                    Instant::now() < deadline,
                    "the position is not reported within {WAIT:?} after the rate change"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            player
                .switch_file(&path_b, 2000.0, true)
                .expect("the switch succeeds");
            wait_for(&mut player, |event| {
                *event == PlayerEvent::StateChanged(PlayState::Playing)
            });
            let switch_start = loop {
                if let Some(position) = player.position_ms() {
                    assert!(
                        (position - 2000.0).abs() <= 200.0,
                        "the switch continues at 2000 ms in the new file, measured {position} ms"
                    );
                    break position;
                }
                assert!(
                    Instant::now() < deadline,
                    "the position is not reported within {WAIT:?} after the switch"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            std::thread::sleep(Duration::from_secs(1));
            let end = player
                .position_ms()
                .expect("the position is reported while playing");
            let advance = end - switch_start;
            assert!(
                (advance - 500.0).abs() <= 75.0,
                "after the switch the position still advances at half speed, measured {advance} ms in 1 s"
            );
            let _ = start;
        }
    }

    /// Without `resume` the switch lands paused at the requested position.
    #[test]
    fn switch_file_without_resume_stays_paused() {
        for extension in ["opus", "mp3"] {
            let (mut player, _path_a, path_b, _dir) = player_with_two_files(extension);
            playing_at_1000(&mut player);
            player
                .switch_file(&path_b, 2500.0, false)
                .expect("the switch succeeds");
            // The player never turns Playing again within a second.
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                for event in player.poll() {
                    assert_ne!(
                        event,
                        PlayerEvent::StateChanged(PlayState::Playing),
                        "the switch without resume must not start playback"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let position = player
                .position_ms()
                .expect("the position is reported while paused");
            assert!(
                (position - 2500.0).abs() <= 10.0,
                "the paused switch lands at 2500 ms within 10 ms, measured {position} ms"
            );
        }
    }

    /// A switch to a missing file fails with a file error and leaves the
    /// current playback untouched.
    #[test]
    fn switch_file_to_missing_file_keeps_playing() {
        for extension in ["opus", "mp3"] {
            let (mut player, path_a, _path_b, dir) = player_with_two_files(extension);
            playing_at_1000(&mut player);
            let missing = dir.path().join(format!("missing.{extension}"));
            let error = player.switch_file(&missing, 1000.0, true).unwrap_err();
            assert!(
                matches!(error, AudioError::File(_)),
                "a missing file reports a file error, got {error:?}"
            );
            std::thread::sleep(Duration::from_millis(300));
            let position = player
                .position_ms()
                .expect("the old playback continues");
            assert!(
                position > 1000.0,
                "the old file keeps playing after a failed switch, measured {position} ms"
            );
            let _ = &path_a;
        }
    }
}
