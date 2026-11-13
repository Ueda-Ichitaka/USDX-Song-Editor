//! About me: the logic of the Player bridge (plan section 8.3). The QObject
//! declaration lives in the SongDocument bridge module (same-bridge nested
//! pointers, like the LinesModel); this file holds the plain-Rust playback
//! state: the pure event application (`apply_event`, unit-testable without
//! GStreamer, the same pattern the P5-02 sheet layout follows) and the
//! `PlayerCore` that wraps the usdx-audio player. The QObject impls here
//! mirror the core state into the QML properties after every invokable and
//! run the blocking track switch on a worker thread (`impl Threading`), so
//! the UI never stalls and Vocals and Instrumental resume where Mix stood.

use crate::controller::settings::TrackDefault;
use crate::controller::shell::{self, TrackKind};
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

    /// Clears the playback state after the song changed: the player is
    /// gone, the playhead stands at 0 and the next play loads the new
    /// song's audio. The track choice is re-resolved by the caller.
    pub fn reset(&mut self) {
        self.player = None;
        self.playing = false;
        self.position_ms = 0.0;
        self.duration_ms = 0.0;
        self.range_end_ms = None;
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

    /// Plays the whole song from the current position; an active play
    /// range is dropped by a flush seek to the current position, so Play
    /// always runs to the end of the song. Without a range the pipeline
    /// simply resumes: the seek would make the position unknown for a
    /// moment and is not needed.
    pub fn play(&mut self) -> Vec<CoreSignal> {
        let Some(player) = self.player.as_mut() else {
            return Vec::new();
        };
        let mut signals = Vec::new();
        if self.range_end_ms.is_some() {
            let position = player.position_ms().unwrap_or(self.position_ms);
            if let Err(error) = player.seek_ms(position) {
                signals.push(error_signal(&error));
            }
            self.position_ms = position;
        }
        player.play();
        // Optimistic: the Playing event confirms it; the suppression in
        // usdx-audio keeps the seek's transient states from clobbering it.
        self.playing = true;
        self.range_end_ms = None;
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
                    // Optimistic like play(); the Playing event confirms.
                    self.playing = true;
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
        if signals.contains(&CoreSignal::RangeDone) {
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

/// The track property representation (0 mix, 1 vocals, 2 instrumental) as
/// the shell track kind the file lookup uses.
pub fn track_kind(repr: i32) -> Option<TrackKind> {
    match repr {
        0 => Some(TrackKind::Mix),
        1 => Some(TrackKind::Vocals),
        2 => Some(TrackKind::Instrumental),
        _ => None,
    }
}

/// The track the settings default picks for a song with the given track
/// availability (UI_SPEC 4.10: the audio track the player picks when a
/// song opens); the result is the property representation.
pub fn initial_track(default_track: TrackDefault, has_vocals: bool, has_instrumental: bool) -> i32 {
    let prefer_stem = match default_track {
        TrackDefault::Mix => None,
        TrackDefault::VocalsIfPresentElseMix => Some((has_vocals, 1)),
        TrackDefault::InstrumentalIfPresentElseMix => Some((has_instrumental, 2)),
    };
    match prefer_stem {
        Some((true, repr)) => repr,
        _ => 0,
    }
}

/// The Track enum as its qenum representation for the property.
pub fn track_repr(track: Track) -> i32 {
    match track {
        Track::Mix => 0,
        Track::Vocals => 1,
        Track::Instrumental => 2,
    }
}

// --- the QObject impls ---

use core::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::bridge::song_document::qobject;
use crate::bridge::song_document::qobject::Player;

impl qobject::Player {
    /// play starts the whole song from the current position; without a
    /// loaded track it loads first and plays when the load is through.
    pub fn play(mut self: Pin<&mut Self>) {
        self.as_mut().sync_document();
        if self.as_ref().loading {
            self.as_mut().rust_mut().play_requested = true;
            return;
        }
        if !self.core.has_player() {
            self.as_mut().start_load();
            return;
        }
        let signals = self.as_mut().rust_mut().core.play();
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// pause stops the playback; a parked play request is dropped.
    pub fn pause(mut self: Pin<&mut Self>) {
        self.as_mut().sync_document();
        self.as_mut().rust_mut().play_requested = false;
        let signals = self.as_mut().rust_mut().core.pause();
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// toggle plays or pauses, whatever does the opposite of the state.
    pub fn toggle(mut self: Pin<&mut Self>) {
        if self.core.playing {
            self.as_mut().pause();
        } else {
            self.as_mut().play();
        }
    }

    /// seek_ms moves the playhead at once (optimistic); the next ticks
    /// confirm the position.
    pub fn seek_ms(mut self: Pin<&mut Self>, ms: f64) {
        self.as_mut().sync_document();
        if self.as_ref().loading {
            return;
        }
        let signals = self.as_mut().rust_mut().core.seek_ms(ms);
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// play_range plays only the range; without a loaded track it loads
    /// first and plays the range when the load is through.
    pub fn play_range(mut self: Pin<&mut Self>, start_ms: f64, end_ms: f64) {
        self.as_mut().sync_document();
        if self.as_ref().loading || !self.core.has_player() {
            self.as_mut().rust_mut().play_requested = true;
            self.as_mut().rust_mut().pending_range = Some((start_ms, end_ms));
            if !self.as_ref().loading {
                self.as_mut().start_load();
            }
            return;
        }
        let signals = self.as_mut().rust_mut().core.play_range(start_ms, end_ms);
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// set_rate changes the playback rate (0.5, 0.75, 1.0); it doubles as
    /// the WRITE setter of the rate property.
    pub fn set_rate(mut self: Pin<&mut Self>, rate: f64) {
        let signals = self.as_mut().rust_mut().core.set_rate(rate);
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// set_track switches the track on a worker thread; the switch keeps
    /// the position, the playing state and the rate. It doubles as the
    /// WRITE setter of the track property. While a load runs the request
    /// is ignored; the property bounces back when the load attaches.
    pub fn set_track(mut self: Pin<&mut Self>, track: i32) {
        if self.as_ref().loading || track == self.core.track {
            return;
        }
        self.as_mut().rust_mut().track_resolved = true;
        let old_track = self.core.track;
        self.as_mut().rust_mut().core.track = track;
        let Some(target) = self.track_path_for(track) else {
            // No file for the new track: the old playback continues and
            // the property bounces back to the previous track.
            self.as_mut().rust_mut().core.track = old_track;
            self.as_mut()
                .error_occurred(QString::from("the audio track is not in the song folder"));
            self.as_mut().publish();
            return;
        };
        let Some(player) = self.as_mut().rust_mut().core.detach_player() else {
            // Nothing loaded: the choice applies to the next play.
            self.as_mut().publish();
            return;
        };
        let was_playing = self.core.playing;
        let position = self.core.position_ms;
        let song_path = self.loaded_song_path.clone();
        self.as_mut().rust_mut().loading = true;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let mut player = player;
            let outcome = player.switch_file(&target, position, was_playing);
            let _ = thread.queue(move |qobject: Pin<&mut Player>| {
                qobject.switch_finished(outcome, player, track, old_track, was_playing, song_path);
            });
        });
        self.as_mut().publish();
    }

    /// tick drains the bus events into the properties and follows the
    /// playhead; the FrameAnimation calls it every frame while playing.
    pub fn tick(mut self: Pin<&mut Self>) {
        self.as_mut().sync_document();
        if self.as_ref().loading {
            return;
        }
        let signals = self.as_mut().rust_mut().core.tick();
        self.as_mut().emit_signals(signals);
        self.as_mut().publish();
    }

    /// load_finished attaches the freshly loaded player to the core; a
    /// parked play request starts the playback now.
    fn load_finished(
        mut self: Pin<&mut Self>,
        result: Result<AudioPlayer, AudioError>,
        track: i32,
        song_path: String,
    ) {
        self.as_mut().rust_mut().loading = false;
        self.as_mut().sync_document();
        if self.loaded_song_path != song_path {
            // The song changed while the load ran: the audio belongs to
            // the old song and is dropped.
            drop(result);
            self.as_mut().publish();
            return;
        }
        match result {
            Ok(player) => {
                let signals = self
                    .as_mut()
                    .rust_mut()
                    .core
                    .attach_player(player, track, false);
                self.as_mut().emit_signals(signals);
            }
            Err(error) => {
                self.as_mut()
                    .error_occurred(QString::from(error.to_string()));
            }
        }
        if self.play_requested {
            self.as_mut().rust_mut().play_requested = false;
            let range = self.as_mut().rust_mut().pending_range.take();
            let signals = match range {
                Some((start, end)) => self.as_mut().rust_mut().core.play_range(start, end),
                None => self.as_mut().rust_mut().core.play(),
            };
            self.as_mut().emit_signals(signals);
        }
        self.as_mut().publish();
    }

    /// switch_finished attaches the switched player back to the core; on a
    /// failure the old playback continues and reports the error.
    fn switch_finished(
        mut self: Pin<&mut Self>,
        outcome: Result<(), AudioError>,
        player: AudioPlayer,
        track: i32,
        old_track: i32,
        was_playing: bool,
        song_path: String,
    ) {
        self.as_mut().rust_mut().loading = false;
        self.as_mut().sync_document();
        if self.loaded_song_path != song_path {
            // The switch belongs to the old song; the player is dropped.
            drop(player);
            self.as_mut().publish();
            return;
        }
        match outcome {
            Ok(()) => {
                let signals =
                    self.as_mut()
                        .rust_mut()
                        .core
                        .attach_player(player, track, was_playing);
                self.as_mut().emit_signals(signals);
            }
            Err(error) => {
                // The old file continues where it stood.
                self.as_mut().rust_mut().core.track = old_track;
                let signals =
                    self.as_mut()
                        .rust_mut()
                        .core
                        .attach_player(player, old_track, was_playing);
                self.as_mut().emit_signals(signals);
                self.as_mut()
                    .error_occurred(QString::from(error.to_string()));
            }
        }
        self.as_mut().publish();
    }

    /// start_load loads the audio of the resolved track on a worker
    /// thread; the queued handler attaches it to the core.
    fn start_load(mut self: Pin<&mut Self>) {
        self.as_mut().resolve_initial_track();
        let track = self.core.track;
        let song_path = self.loaded_song_path.clone();
        let Some(path) = self.track_path_for(track) else {
            self.as_mut()
                .error_occurred(QString::from("the audio track is not in the song folder"));
            return;
        };
        self.as_mut().rust_mut().loading = true;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result =
                AudioPlayer::new().and_then(|mut player| player.load(&path).map(|_| player));
            let _ = thread.queue(move |qobject: Pin<&mut Player>| {
                qobject.load_finished(result, track, song_path);
            });
        });
    }

    /// sync_document follows the document: a song change unloads the
    /// player (the audio belongs to the old song), the track choice is
    /// re-resolved on the next play and the availability flags refresh.
    fn sync_document(mut self: Pin<&mut Self>) {
        let song_path = match self.song_document() {
            Some(document) => String::from(document.file_path()),
            None => return,
        };
        if song_path == self.loaded_song_path {
            return;
        }
        let (has_vocals, has_instrumental) = match self.song_document() {
            Some(document) => {
                let controller = document.rust().editor();
                (
                    shell::has_track(controller, TrackKind::Vocals),
                    shell::has_track(controller, TrackKind::Instrumental),
                )
            }
            None => (false, false),
        };
        self.as_mut().rust_mut().loaded_song_path = song_path;
        self.as_mut().rust_mut().core.reset();
        self.as_mut().rust_mut().track_resolved = false;
        self.as_mut().rust_mut().play_requested = false;
        self.as_mut().rust_mut().pending_range = None;
        self.as_mut().rust_mut().core.has_vocals = has_vocals;
        self.as_mut().rust_mut().core.has_instrumental = has_instrumental;
    }

    /// resolve_initial_track picks the track from the settings default and
    /// the track availability, once per song.
    fn resolve_initial_track(mut self: Pin<&mut Self>) {
        if self.track_resolved {
            return;
        }
        self.as_mut().rust_mut().track_resolved = true;
        let (default_track, has_vocals, has_instrumental) = match self.song_document() {
            Some(document) => {
                let rust = document.rust();
                (
                    rust.editor_settings().default_track,
                    shell::has_track(rust.editor(), TrackKind::Vocals),
                    shell::has_track(rust.editor(), TrackKind::Instrumental),
                )
            }
            None => (TrackDefault::Mix, false, false),
        };
        self.as_mut().rust_mut().core.track =
            initial_track(default_track, has_vocals, has_instrumental);
    }

    /// track_path_for resolves the audio file of the track, when the song
    /// has it.
    fn track_path_for(&self, track: i32) -> Option<std::path::PathBuf> {
        let kind = track_kind(track)?;
        let document = self.song_document()?;
        shell::track_path(document.rust().editor(), kind)
    }

    /// song_document returns the document the player follows, when QML set
    /// it.
    fn song_document(&self) -> Option<&qobject::SongDocument> {
        // SAFETY: the pointer comes from the document property QML set and
        // the document outlives the player.
        unsafe { self.document().as_ref() }
    }

    /// emit_signals turns the core signals into the Qt signals.
    fn emit_signals(mut self: Pin<&mut Self>, signals: Vec<CoreSignal>) {
        for signal in signals {
            match signal {
                CoreSignal::RangeDone => self.as_mut().range_done(),
                CoreSignal::Error(message) => {
                    self.as_mut().error_occurred(QString::from(message));
                }
            }
        }
    }

    /// publish mirrors the core state into the Q_PROPERTY storage; the
    /// rate and track properties have the setRate and setTrack invokables
    /// as their WRITE setters, so their notify signals are emitted after
    /// the field write.
    fn publish(mut self: Pin<&mut Self>) {
        let (playing, position_ms, duration_ms) = (
            self.core.playing,
            self.core.position_ms,
            self.core.duration_ms,
        );
        let (rate, track, has_vocals, has_instrumental) = (
            self.core.rate,
            self.core.track,
            self.core.has_vocals,
            self.core.has_instrumental,
        );
        self.as_mut().set_position_ms(position_ms);
        self.as_mut().set_duration_ms(duration_ms);
        self.as_mut().set_playing(playing);
        self.as_mut().set_has_vocals(has_vocals);
        self.as_mut().set_has_instrumental(has_instrumental);
        if self.rate != rate {
            self.as_mut().rust_mut().rate = rate;
            self.as_mut().rate_changed();
        }
        if self.track != track {
            self.as_mut().rust_mut().track = track;
            self.as_mut().track_changed();
        }
    }
}
