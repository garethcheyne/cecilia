//! Long-running services around the engine: the auto-recorder task, the
//! player's MIDI link, and app-level events for the WebSocket stream.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use organ_core::console::ConsoleState;
use organ_core::live::LiveOrgan;
use organ_core::organ::OrganDefinition;
use organ_core::player::{Link, Player, PlayerSettings, PlayerStatus};
use organ_core::recorder::{AutoRecorder, RecorderConfig, RecorderStatus};
use organ_core::sample::sample_tracks;
use organ_core::track::TrackSummary;
use organ_core::{Direction, EngineEvent, MidiHub};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use utoipa::ToSchema;

use crate::library::LibraryStore;
use crate::settings::{PlaybackSettings, RecordingSettings, SettingsStore};

/// Events for clients besides raw MIDI and simulator snapshots.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    Recorder(RecorderStatus),
    /// The library changed; `saved` is a newly recorded track, if any.
    Library { saved: Option<TrackSummary> },
    /// The service runner moved on.
    Service(crate::playlists::ServiceStatus),
}

pub fn recorder_config(r: &RecordingSettings) -> RecorderConfig {
    RecorderConfig {
        silence_ms: r.silence_ms.clamp(2000, 60_000),
        margin_start_ms: r.margin_start_ms.min(15_000),
        margin_end_ms: r.margin_end_ms.min(15_000),
    }
}

pub fn player_settings(p: &PlaybackSettings) -> PlayerSettings {
    PlayerSettings { fade_ms: p.fade_ms.clamp(1000, 30_000), toggle_settle_ms: p.settle_ms.min(3000) }
}

// ---------------------------------------------------------------------------
// Recorder
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Mutex<AutoRecorder>>,
    hub: MidiHub,
    library: Arc<LibraryStore>,
    events: broadcast::Sender<AppEvent>,
}

impl Recorder {
    /// Start the background task that feeds organ input to the recorder.
    pub fn start(
        hub: MidiHub,
        organ: OrganDefinition,
        config: RecorderConfig,
        library: Arc<LibraryStore>,
        events: broadcast::Sender<AppEvent>,
    ) -> Self {
        let r = Self {
            inner: Arc::new(Mutex::new(AutoRecorder::new(organ, config))),
            hub,
            library,
            events,
        };
        let task = r.clone();
        tokio::spawn(async move { task.run().await });
        r
    }

    fn lock(&self) -> MutexGuard<'_, AutoRecorder> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn run(self) {
        let mut rx = self.hub.subscribe();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let mut last_state = self.status().state;
        loop {
            tokio::select! {
                event = rx.recv() => match event {
                    Ok(EngineEvent::Midi { direction: Direction::In, t_us, bytes, .. }) => {
                        self.lock().on_midi(t_us, &bytes);
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(n)) => tracing::warn!(skipped = n, "recorder missed MIDI input"),
                    Err(RecvError::Closed) => break,
                },
                _ = tick.tick() => {
                    let done = self.lock().tick(self.hub.now_us());
                    if let Some(track) = done {
                        self.save(track);
                    }
                }
            }
            let status = self.status();
            if status.state != last_state {
                last_state = status.state;
                let _ = self.events.send(AppEvent::Recorder(status));
            }
        }
    }

    fn save(&self, track: organ_core::track::Track) {
        match self.library.save(&track) {
            Ok(summary) => {
                tracing::info!(id = %summary.id, notes = summary.notes, "recorded a track");
                let _ = self.events.send(AppEvent::Library { saved: Some(summary) });
            }
            Err(err) => tracing::error!(%err, "could not save a recording"),
        }
        let _ = self.events.send(AppEvent::Recorder(self.status()));
    }

    pub fn status(&self) -> RecorderStatus {
        self.lock().status()
    }

    pub fn arm(&self) -> RecorderStatus {
        self.lock().arm();
        let s = self.status();
        let _ = self.events.send(AppEvent::Recorder(s.clone()));
        s
    }

    pub fn disarm(&self) -> RecorderStatus {
        let done = self.lock().disarm(self.hub.now_us());
        if let Some(t) = done {
            self.save(t);
        }
        let s = self.status();
        let _ = self.events.send(AppEvent::Recorder(s.clone()));
        s
    }

    pub fn load_organ(&self, organ: OrganDefinition) {
        let done = self.lock().load_organ(organ, self.hub.now_us());
        if let Some(t) = done {
            self.save(t);
        }
    }

    pub fn set_config(&self, config: RecorderConfig) {
        self.lock().set_config(config);
    }
}

// ---------------------------------------------------------------------------
// Player link
// ---------------------------------------------------------------------------

/// Player output goes to the engine's output port; the organ's state comes
/// from the live tracker.
pub struct HubLink {
    pub hub: MidiHub,
    pub live: LiveOrgan,
}

impl Link for HubLink {
    fn send(&self, msg: Vec<u8>) {
        self.hub.send_quiet(msg);
    }
    fn console(&self) -> ConsoleState {
        self.live.console()
    }
}

pub fn start_player(hub: MidiHub, live: LiveOrgan, organ: OrganDefinition, settings: PlayerSettings) -> Player {
    Player::start(HubLink { hub, live }, organ, settings)
}

pub fn player_status(p: &Player) -> PlayerStatus {
    p.status()
}

// ---------------------------------------------------------------------------
// Samples
// ---------------------------------------------------------------------------

/// Put bundled samples into the library the first time each one is seen.
/// A sample the organist deleted is not brought back.
pub fn install_samples(library: &LibraryStore, settings: &SettingsStore) -> anyhow::Result<usize> {
    let seen = settings.get().samples_seen;
    let mut added = Vec::new();
    for track in sample_tracks() {
        if seen.contains(&track.id) {
            continue;
        }
        if !library.contains(&track.id) {
            library.save(&track)?;
        }
        added.push(track.id);
    }
    if !added.is_empty() {
        let n = added.len();
        settings.update(|s| s.samples_seen.extend(added))?;
        tracing::info!(count = n, "added sample music to the library");
        return Ok(n);
    }
    Ok(0)
}
