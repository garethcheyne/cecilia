//! Playlists on disk (`<data>/playlists/<id>.playlist.json`), and the
//! service runner that steps through one during a service.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use organ_core::driver::OrganEvent;
use organ_core::live::LiveOrgan;
use organ_core::player::{PlayOptions, Player, PlayerEvent, PlayerState};
use organ_core::playlist::{Delay, Playlist, PlaylistRow, PlaylistSummary};
use organ_core::recorder::unix_ms;
use organ_core::track::{valid_id, TrackEvent};
use organ_core::MidiHub;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use utoipa::ToSchema;

use crate::library::LibraryStore;
use crate::services::AppEvent;

pub struct PlaylistStore {
    dir: PathBuf,
    all: RwLock<BTreeMap<String, Playlist>>,
}

impl PlaylistStore {
    pub fn load(data_dir: &Path) -> anyhow::Result<Self> {
        let dir = data_dir.join("playlists");
        std::fs::create_dir_all(&dir)?;
        let store = Self { dir, all: RwLock::new(BTreeMap::new()) };
        store.reload();
        Ok(store)
    }

    /// Re-scan the playlists folder.
    pub fn reload(&self) {
        let mut all = BTreeMap::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            if !path.to_string_lossy().ends_with(".playlist.json") {
                continue;
            }
            let parsed = std::fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|t| Ok(serde_json::from_str::<Playlist>(&t)?))
                .and_then(|p| p.validate().map(|_| p).map_err(anyhow::Error::msg));
            match parsed {
                Ok(p) => {
                    all.insert(p.id.clone(), p);
                }
                Err(err) => tracing::warn!(path = %path.display(), %err, "skipping playlist file"),
            }
        }
        *self.all.write().unwrap() = all;
    }

    pub fn list(&self) -> Vec<PlaylistSummary> {
        let mut v: Vec<_> = self.all.read().unwrap().values().map(Playlist::summary).collect();
        v.sort_by_key(|p| std::cmp::Reverse(p.modified));
        v
    }

    pub fn get(&self, id: &str) -> Option<Playlist> {
        self.all.read().unwrap().get(id).cloned()
    }

    pub fn save(&self, p: &Playlist) -> anyhow::Result<()> {
        p.validate().map_err(anyhow::Error::msg)?;
        let path = self.dir.join(format!("{}.playlist.json", p.id));
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(p)?)?;
        std::fs::rename(&tmp, &path)?;
        self.all.write().unwrap().insert(p.id.clone(), p.clone());
        Ok(())
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<bool> {
        if !valid_id(id) || self.all.write().unwrap().remove(id).is_none() {
            return Ok(false);
        }
        std::fs::remove_file(self.dir.join(format!("{id}.playlist.json")))?;
        Ok(true)
    }

    pub fn unique_id(&self, name: &str) -> String {
        let base: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() { "playlist".to_string() } else { base.chars().take(60).collect() };
        let taken = |id: &str| self.all.read().unwrap().contains_key(id);
        if !taken(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap()
    }
}

// ---------------------------------------------------------------------------
// Service runner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    /// No playlist running.
    Idle,
    /// A track is loaded and waits for Play.
    Ready,
    Playing,
    /// The next track starts by itself in `countdown` seconds.
    Waiting,
    /// The last row has been played.
    Finished,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ServiceStatus {
    pub state: ServiceState,
    pub playlist_id: Option<String>,
    pub name: Option<String>,
    /// Row index (into the playlist's rows) of the current track.
    pub row: Option<usize>,
    /// Row index of the track after it, if any.
    pub next_row: Option<usize>,
    /// Seconds until an automatic start.
    pub countdown: Option<u32>,
}

struct Session {
    playlist: Playlist,
    order: Vec<usize>,
    /// Position in `order` of the current track row.
    pos: usize,
    auto_start: Option<Instant>,
    finished: bool,
}

#[derive(Clone)]
pub struct Service {
    session: Arc<Mutex<Option<Session>>>,
    player: Player,
    library: Arc<LibraryStore>,
    hub: MidiHub,
    live: LiveOrgan,
    events: broadcast::Sender<AppEvent>,
}

impl Service {
    pub fn start(
        player: Player,
        library: Arc<LibraryStore>,
        hub: MidiHub,
        live: LiveOrgan,
        events: broadcast::Sender<AppEvent>,
    ) -> Self {
        let s = Self { session: Arc::new(Mutex::new(None)), player, library, hub, live, events };
        let task = s.clone();
        tokio::spawn(async move { task.run().await });
        s
    }

    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn run(self) {
        let mut rx = self.player.subscribe();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let mut last_countdown = None;
        loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Ok(PlayerEvent::Finished { track_id }) => self.on_finished(&track_id),
                    Ok(PlayerEvent::Status(_)) => {}
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
                _ = tick.tick() => {
                    let due = self.lock().as_ref().and_then(|s| s.auto_start).is_some_and(|t| Instant::now() >= t);
                    if due {
                        if let Some(s) = self.lock().as_mut() {
                            s.auto_start = None;
                        }
                        self.player.play();
                        self.publish();
                    }
                    let countdown = self.status().countdown;
                    if countdown != last_countdown {
                        last_countdown = countdown;
                        self.publish();
                    }
                }
            }
        }
    }

    pub fn status(&self) -> ServiceStatus {
        let guard = self.lock();
        let Some(s) = guard.as_ref() else {
            return ServiceStatus {
                state: ServiceState::Idle,
                playlist_id: None,
                name: None,
                row: None,
                next_row: None,
                countdown: None,
            };
        };
        let player = self.player.status();
        let state = if s.finished {
            ServiceState::Finished
        } else if s.auto_start.is_some() {
            ServiceState::Waiting
        } else if matches!(player.state, PlayerState::Playing | PlayerState::Starting | PlayerState::Paused) {
            ServiceState::Playing
        } else {
            ServiceState::Ready
        };
        let next = s.playlist.next_track(&s.order, s.pos + 1).or_else(|| {
            if s.playlist.repeat {
                s.playlist.next_track(&s.order, 0)
            } else {
                None
            }
        });
        ServiceStatus {
            state,
            playlist_id: Some(s.playlist.id.clone()),
            name: Some(s.playlist.name.clone()),
            row: s.order.get(s.pos).copied(),
            next_row: next.and_then(|p| s.order.get(p).copied()),
            countdown: s.auto_start.map(|t| t.saturating_duration_since(Instant::now()).as_secs_f32().ceil() as u32),
        }
    }

    fn publish(&self) {
        let _ = self.events.send(AppEvent::Service(self.status()));
    }

    /// Start running `playlist` from `row` (or its first track). The first
    /// track plays straight away unless `play` is false.
    pub fn begin(&self, playlist: Playlist, row: Option<usize>, play: bool) -> Result<ServiceStatus, String> {
        let order = playlist.order(unix_ms());
        let from = match row {
            Some(r) => order.iter().position(|&i| i == r).ok_or("that row isn't in the playlist")?,
            None => 0,
        };
        let pos = playlist.next_track(&order, from).ok_or("there are no tracks to play from there")?;
        *self.lock() = Some(Session { playlist, order, pos, auto_start: None, finished: false });
        self.load_current(if play { Some(Delay::After { seconds: 0 }) } else { Some(Delay::Wait) })?;
        Ok(self.status())
    }

    /// Load the current row. `how` overrides the row's own delay.
    fn load_current(&self, how: Option<Delay>) -> Result<(), String> {
        let (track_id, delay, verses, tempo) = {
            let mut guard = self.lock();
            let s = guard.as_mut().ok_or("no playlist is running")?;
            s.finished = false;
            let row = s.order[s.pos];
            match &s.playlist.rows[row] {
                PlaylistRow::Track { track_id, delay, verses, tempo, .. } => {
                    (track_id.clone(), how.unwrap_or(*delay), verses.clone(), *tempo)
                }
                PlaylistRow::Text { .. } => return Err("not a track row".into()),
            }
        };
        let track = self.library.get(&track_id).ok_or_else(|| format!("the track {track_id:?} is missing"))?;
        let start_ms = match delay {
            Delay::Cue => track
                .events
                .iter()
                .find(|e| matches!(e.event, TrackEvent::Note { velocity, .. } if velocity > 0))
                .map(|e| e.t.saturating_sub(150)),
            _ => None,
        };
        let options = PlayOptions { verses, tempo, start_ms };
        match delay {
            Delay::After { seconds: 0 } => self.player.load(track, options, true),
            Delay::After { seconds } => {
                self.player.load(track, options, false);
                if let Some(s) = self.lock().as_mut() {
                    s.auto_start = Some(Instant::now() + Duration::from_secs(seconds as u64));
                }
            }
            Delay::Wait | Delay::Cue => self.player.load(track, options, false),
        }
        self.publish();
        Ok(())
    }

    fn on_finished(&self, track_id: &str) {
        let silent = {
            let guard = self.lock();
            let Some(s) = guard.as_ref() else { return };
            let current = s.order.get(s.pos).and_then(|&r| match &s.playlist.rows[r] {
                PlaylistRow::Track { track_id, .. } => Some(track_id.clone()),
                _ => None,
            });
            if current.as_deref() != Some(track_id) {
                return; // something else was playing
            }
            s.playlist.silent
        };
        if !silent {
            for m in self.live.encode(&OrganEvent::AllStopsOff) {
                self.hub.send_quiet(m);
            }
        }
        let _ = self.advance(1, None);
    }

    /// Move `step` track rows (1 = next, -1 = previous) and load it.
    fn advance(&self, step: i32, how: Option<Delay>) -> Result<ServiceStatus, String> {
        {
            let mut guard = self.lock();
            let s = guard.as_mut().ok_or("no playlist is running")?;
            s.auto_start = None;
            let next = if step > 0 {
                s.playlist.next_track(&s.order, s.pos + 1).or_else(|| {
                    if s.playlist.repeat {
                        s.playlist.next_track(&s.order, 0)
                    } else {
                        None
                    }
                })
            } else {
                (0..s.pos).rev().find(|&p| matches!(s.playlist.rows[s.order[p]], PlaylistRow::Track { .. }))
            };
            match next {
                Some(p) => s.pos = p,
                None => {
                    if step > 0 {
                        s.finished = true;
                    }
                    drop(guard);
                    self.publish();
                    return Ok(self.status());
                }
            }
        }
        self.load_current(how)?;
        Ok(self.status())
    }

    /// Skip to the next track and play it now.
    pub fn next(&self) -> Result<ServiceStatus, String> {
        self.player.stop();
        self.advance(1, Some(Delay::After { seconds: 0 }))
    }

    pub fn previous(&self) -> Result<ServiceStatus, String> {
        self.player.stop();
        self.advance(-1, Some(Delay::After { seconds: 0 }))
    }

    /// Get `row` ready to play (without starting it).
    pub fn select(&self, row: usize) -> Result<ServiceStatus, String> {
        {
            let mut guard = self.lock();
            let s = guard.as_mut().ok_or("no playlist is running")?;
            let pos = s.order.iter().position(|&i| i == row).ok_or("that row isn't in the playlist")?;
            if !matches!(s.playlist.rows[row], PlaylistRow::Track { .. }) {
                return Err("that row is text, not a track".into());
            }
            s.pos = pos;
            s.auto_start = None;
        }
        self.player.stop();
        self.load_current(Some(Delay::Wait))?;
        Ok(self.status())
    }

    pub fn end(&self) -> ServiceStatus {
        *self.lock() = None;
        self.player.stop();
        self.publish();
        self.status()
    }

    /// The playlist was edited: keep running with the new rows if it's ours.
    pub fn playlist_saved(&self, p: &Playlist) {
        if let Some(s) = self.lock().as_mut() {
            if s.playlist.id == p.id {
                let row = s.order.get(s.pos).copied();
                s.playlist = p.clone();
                s.order = p.order(unix_ms());
                s.pos = row.and_then(|r| s.order.iter().position(|&i| i == r)).unwrap_or(0);
            }
        }
        self.publish();
    }
}
