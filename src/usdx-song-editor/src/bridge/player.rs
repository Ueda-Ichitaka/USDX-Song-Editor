//! About me: the logic of the Player bridge (plan section 8.3). The QObject
//! declaration lives in the SongDocument bridge module (same-bridge nested
//! pointers, like the LinesModel); this file holds the plain-Rust playback
//! state: the pure event application (`apply_event`, unit-testable without
//! GStreamer, the same pattern the P5-02 sheet layout follows) and the
//! `PlayerCore` that wraps the usdx-audio player. The QObject impls here
//! mirror the core state into the QML properties after every invokable and
//! run the blocking track switch on a worker thread (`impl Threading`), so
//! the UI never stalls and Vocals and Instrumental resume where Mix stood.

use usdx_audio::{AudioError, PlayState, Player as AudioPlayer, PlayerEvent, Track};

/// A notification the core hands to the Qt side as a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSignal {
    /// The played range is over; QML replays it while Loop is on.
    RangeDone,
    /// A playback error, shown as a passive notification.
    Error(String),
}

/// The playback state the QObject mirrors into its properties. Plain data,
/// so the event handling stays unit-testable without GStreamer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackState {
    pub playing: bool,
    pub position_ms: f64,
    pub duration_ms: f64,
    pub range_end_ms: Option<f64>,
}

/// Applies one player event to the playback state and returns the signals
/// the Qt side must emit. A finished range pauses the player (the QObject
/// does it when it sees the RangeDone signal) and holds the playhead at the
/// range end, because the position query falls back to a stale preroll
/// value after a segment completes (measured in P4-02).
pub fn apply_event(state: &mut PlaybackState, event: PlayerEvent) -> Vec<CoreSignal> {
    match event {
        PlayerEvent::StateChanged(PlayState::Playing) => {
            state.playing = true;
            Vec::new()
        }
        PlayerEvent::StateChanged(_) => {
            state.playing = false;
            Vec::new()
        }
        PlayerEvent::EndOfStream => {
            state.playing = false;
            Vec::new()
        }
        PlayerEvent::RangeDone => {
            state.playing = false;
            if let Some(end) = state.range_end_ms.take() {
                state.position_ms = end;
            }
            vec![CoreSignal::RangeDone]
        }
        PlayerEvent::Error(message) => {
            state.playing = false;
            vec![CoreSignal::Error(message)]
        }
        PlayerEvent::DurationKnown(duration) => {
            state.duration_ms = duration;
            Vec::new()
        }
    }
}

/// The player bridge state: the GStreamer player plus the mirrored playback
/// state. The qproperty fields (positionMs, durationMs, playing, rate,
/// track, hasVocals, hasInstrumental and the nested document pointer) are
/// declared in the bridge; the QObject impls copy them into the Qt property
/// system after every change.
pub struct PlayerCore {
    player: Option<AudioPlayer>,
    /// The chosen track: 0 mix, 1 vocals, 2 instrumental (the qenum repr).
    track: i32,
    rate: f64,
    playing: bool,
    position_ms: f64,
    duration_ms: f64,
    /// The stop position of the active play range, if any.
    range_end_ms: Option<f64>,
    has_vocals: bool,
    has_instrumental: bool,
}

impl Default for PlayerCore {
    fn default() -> Self {
        Self {
            player: None,
            track: 0,
            rate: 1.0,
            playing: false,
            position_ms: 0.0,
            duration_ms: 0.0,
            range_end_ms: None,
            has_vocals: false,
            has_instrumental: false,
        }
    }
}

impl PlayerCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The chosen track (the qenum repr).
    pub fn track(&self) -> i32 {
        self.track
    }

    /// The chosen rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Whether playback is running (the qproperty value).
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// The playhead position in milliseconds (the qproperty value).
    pub fn position_ms(&self) -> f64 {
        self.position_ms
    }

    /// The song duration in milliseconds (the qproperty value).
    pub fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    /// Whether a GStreamer player is attached.
    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    /// Takes the player out for a background operation; playback calls are
    /// no-ops until it comes back through attach_player.
    pub fn detach_player(&mut self) -> Option<AudioPlayer> {
        self.player.take()
    }

    /// Installs a (re)loaded player for the track. The position comes from
    /// the player (a failed switch re-attaches the old playback where it
    /// stood), the playing state is what the caller knows about the player
    /// after its load or switch.
    pub fn attach_player(
        &mut self,
        player: AudioPlayer,
        track: i32,
        playing: bool,
    ) -> Vec<CoreSignal> {
        self.player = Some(player);
        self.track = track;
        self.range_end_ms = None;
        self.playing = playing;
        self.position_ms = self
            .player
            .as_ref()
            .and_then(AudioPlayer::position_ms)
            .unwrap_or(self.position_ms);
        // The bus holds the state messages from before the attach (the
        // load's transition to PAUSED, an old switch); they would report a
        // long-gone state on the next tick, so they are drained here.
        if let Some(player) = self.player.as_mut() {
            player.poll();
        }
        // A fresh playbin starts at rate 1.0; the rate chosen earlier
        // applies again (the speed combo keeps its choice across loads).
        let rate = self.rate;
        if rate != 1.0 {
            return self.set_rate(rate);
        }
        Vec::new()
    }

    /// Plays the whole song from the current position; an active play range
    /// is dropped by the flush seek, so Play always runs to the end of the
    /// song.
    pub fn play(&mut self) -> Vec<CoreSignal> {
        let Some(player) = self.player.as_mut() else {
            return Vec::new();
        };
        let position = player.position_ms().unwrap_or(self.position_ms);
        let mut signals = Vec::new();
        if let Err(error) = player.seek_ms(position) {
            signals.push(error_signal(&error));
        }
        player.play();
        self.range_end_ms = None;
        self.position_ms = position;
        signals
    }

    /// Pauses playback.
    pub fn pause(&mut self) -> Vec<CoreSignal> {
        if let Some(player) = self.player.as_mut() {
            player.pause();
        }
        self.playing = false;
        Vec::new()
    }

    /// Plays or pauses, whatever does the opposite of the current state.
    pub fn toggle(&mut self) -> Vec<CoreSignal> {
        if self.playing {
            self.pause()
        } else {
            self.play()
        }
    }

    /// Seeks to `ms`, clamped into the known duration; the playhead
    /// property follows at once and the next tick confirms it.
    pub fn seek_ms(&mut self, ms: f64) -> Vec<CoreSignal> {
        if self.player.is_none() {
            return Vec::new();
        }
        let ms = self.clamp_into_duration(ms);
        let mut signals = Vec::new();
        if let Some(player) = self.player.as_mut()
            && let Err(error) = player.seek_ms(ms)
        {
            signals.push(error_signal(&error));
        }
        self.position_ms = ms;
        self.range_end_ms = None;
        signals
    }

    /// Plays only the range from `start_ms` to `end_ms` (both clamped into
    /// the known duration); tick() reports RangeDone when it is over. The
    /// usdx-audio play_range only seeks with the segment stop, so playback
    /// starts here.
    pub fn play_range(&mut self, start_ms: f64, end_ms: f64) -> Vec<CoreSignal> {
        let end = self.clamp_into_duration(end_ms);
        let start = start_ms.max(0.0).min(end);
        let mut signals = Vec::new();
        if let Some(player) = self.player.as_mut() {
            match player.play_range(start, end) {
                Ok(()) => {
                    player.play();
                    self.position_ms = start;
                    self.range_end_ms = Some(end);
                }
                Err(error) => signals.push(error_signal(&error)),
            }
        }
        signals
    }

    /// Changes the playback rate (0.5, 0.75, 1.0); the rate survives track
    /// switches and applies to players attached later.
    pub fn set_rate(&mut self, rate: f64) -> Vec<CoreSignal> {
        if rate <= 0.0 || rate.is_nan() {
            return vec![CoreSignal::Error(format!(
                "the rate must be positive, got {rate}"
            ))];
        }
        let previous = self.rate;
        self.rate = rate;
        let mut signals = Vec::new();
        if let Some(player) = self.player.as_mut()
            && let Err(error) = player.set_rate(rate)
        {
            self.rate = previous;
            signals.push(error_signal(&error));
        }
        signals
    }

    /// Drains the bus events into the playback state; a finished range
    /// pauses the player so the playhead stands at its end.
    pub fn tick(&mut self) -> Vec<CoreSignal> {
        let Some(player) = self.player.as_mut() else {
            return Vec::new();
        };
        let mut state = PlaybackState {
            playing: self.playing,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
            range_end_ms: self.range_end_ms,
        };
        let mut signals = Vec::new();
        for event in player.poll() {
            signals.extend(apply_event(&mut state, event));
        }
        if signals.iter().any(|signal| *signal == CoreSignal::RangeDone) {
            // The segment is over; the playhead stands at the range end
            // (the position query would fall back to a stale preroll
            // value, so it is skipped for this tick).
            player.pause();
        } else if let Some(position) = player.position_ms() {
            state.position_ms = position;
        }
        self.playing = state.playing;
        self.position_ms = state.position_ms;
        self.duration_ms = state.duration_ms;
        self.range_end_ms = state.range_end_ms;
        signals
    }

    /// Clamps a time into [0, duration] when the duration is known.
    fn clamp_into_duration(&self, ms: f64) -> f64 {
        let ms = ms.max(0.0);
        if self.duration_ms > 0.0 {
            ms.min(self.duration_ms)
        } else {
            ms
        }
    }
}

/// Converts an usdx-audio error into the error signal of the bridge.
pub fn error_signal(error: &AudioError) -> CoreSignal {
    CoreSignal::Error(error.to_string())
}

/// The Track enum as its qenum representation for the property.
pub fn track_repr(track: Track) -> i32 {
    match track {
        Track::Mix => 0,
        Track::Vocals => 1,
        Track::Instrumental => 2,
    }
}
