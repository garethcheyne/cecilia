//! The playback engine: turns a track into live MIDI for the connected organ.
//!
//! It runs on its own thread and owns the timing. MIDI is generated as it
//! plays, through the [`Organist`], so a track recorded on one instrument
//! plays on another, and seeking re-creates the registration of the new
//! position ("reverse regeneration"). Transport is tape-recorder style:
//! play, latching pause, stop, seek, tempo, verse control and fade-out.
//!
//! Before the opening registration the player clears the organ: with the
//! organ's general cancel if it has one, otherwise one "off" per drawn stop.
//! On organs whose stops only toggle (e.g. Makin) it then waits for the
//! console to settle.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use utoipa::ToSchema;

use crate::console::ConsoleState;
use crate::driver::{Driver, OrganEvent};
use crate::organ::{DivisionRole, OrganDefinition, StopBinding};
use crate::organist::Organist;
use crate::track::{Registration, Span, Track, TrackEvent};

/// Where the player's MIDI goes, and what the organ currently shows.
pub trait Link: Send + 'static {
    fn send(&self, msg: Vec<u8>);
    /// The organ's state as last observed (it may lag our own sends a little).
    fn console(&self) -> ConsoleState;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerState {
    /// Nothing loaded, or stopped.
    Stopped,
    /// Clearing the organ and setting the opening registration.
    Starting,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlayerStatus {
    pub state: PlayerState,
    pub track_id: Option<String>,
    pub title: Option<String>,
    /// Position in the track, in ms.
    pub position_ms: u32,
    pub duration_ms: u32,
    /// Progress through the verses being played (repeats included).
    pub elapsed_ms: u32,
    pub total_ms: u32,
    /// Playback speed in percent (100 = as recorded).
    pub tempo: u16,
    /// 1-based verse being played, out of `verses`.
    pub verse: usize,
    pub verses: usize,
    /// Recorded verse numbers to be played, in order.
    pub plan: Vec<usize>,
    pub stop_after_verse: bool,
    pub skip_to_last: bool,
    pub fading: bool,
    pub organ_id: String,
}

/// Which verses to play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum VersePlan {
    /// Play this many verses: the first ones, then the last recorded verse
    /// (repeating the middle when asking for more than were recorded).
    Count(usize),
    /// Exactly these recorded verse numbers, e.g. [1, 2, 3, 5, 7].
    List(Vec<usize>),
}

impl VersePlan {
    /// Recorded verse numbers (1-based) for a track with `recorded` verses.
    pub fn resolve(&self, recorded: usize) -> Vec<usize> {
        let recorded = recorded.max(1);
        match self {
            Self::List(list) => list.iter().copied().filter(|v| (1..=recorded).contains(v)).collect(),
            Self::Count(0) => Vec::new(),
            Self::Count(n) if recorded == 1 => vec![1; *n],
            Self::Count(1) => vec![1],
            Self::Count(n) => {
                let middle: Vec<usize> = if recorded > 2 { (2..recorded).collect() } else { vec![1] };
                let mut plan = vec![1];
                plan.extend(middle.iter().cycle().take(n - 2));
                plan.push(recorded);
                plan
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct PlayOptions {
    /// Default: every recorded verse once.
    #[serde(default)]
    pub verses: Option<VersePlan>,
    /// Percent, 25-400. Default 100.
    #[serde(default)]
    pub tempo: Option<u16>,
    /// Start here instead of at the beginning (track ms).
    #[serde(default)]
    pub start_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerEvent {
    Status(PlayerStatus),
    /// A track played to its end (not stopped by hand).
    Finished {
        track_id: String,
    },
}

enum Cmd {
    Load(Box<Track>, PlayOptions, bool),
    Play,
    Pause,
    Stop,
    Seek(u32),
    Tempo(u16),
    StopAfterVerse(bool),
    SkipToLast(bool),
    Fade(bool),
    Organ(Box<OrganDefinition>),
    Settings(PlayerSettings),
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerSettings {
    pub fade_ms: u32,
    /// Wait after clearing a toggle-only organ before setting stops.
    pub toggle_settle_ms: u32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self { fade_ms: 5000, toggle_settle_ms: 400 }
    }
}

/// Cloneable handle to the player thread.
#[derive(Clone)]
pub struct Player {
    tx: mpsc::Sender<Cmd>,
    status: Arc<Mutex<PlayerStatus>>,
    events: broadcast::Sender<PlayerEvent>,
}

impl Player {
    pub fn start(link: impl Link, organ: OrganDefinition, settings: PlayerSettings) -> Self {
        let (tx, rx) = mpsc::channel();
        let (events, _) = broadcast::channel(256);
        let status = Arc::new(Mutex::new(idle_status(&organ)));
        let engine = Engine::new(Box::new(link), organ, settings, Arc::clone(&status), events.clone());
        std::thread::Builder::new()
            .name("player".into())
            .spawn(move || engine.run(rx))
            .expect("failed to spawn player thread");
        Self { tx, status, events }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PlayerEvent> {
        self.events.subscribe()
    }

    pub fn status(&self) -> PlayerStatus {
        self.status.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn cmd(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }

    /// Load a track; with `play`, start it straight away.
    pub fn load(&self, track: Track, options: PlayOptions, play: bool) {
        self.cmd(Cmd::Load(Box::new(track), options, play));
    }
    pub fn play(&self) {
        self.cmd(Cmd::Play);
    }
    pub fn pause(&self) {
        self.cmd(Cmd::Pause);
    }
    pub fn stop(&self) {
        self.cmd(Cmd::Stop);
    }
    pub fn seek(&self, ms: u32) {
        self.cmd(Cmd::Seek(ms));
    }
    pub fn set_tempo(&self, percent: u16) {
        self.cmd(Cmd::Tempo(percent));
    }
    pub fn stop_after_verse(&self, on: bool) {
        self.cmd(Cmd::StopAfterVerse(on));
    }
    pub fn skip_to_last_verse(&self, on: bool) {
        self.cmd(Cmd::SkipToLast(on));
    }
    /// Start (or with false, cancel) a fade-out.
    pub fn fade(&self, on: bool) {
        self.cmd(Cmd::Fade(on));
    }
    pub fn set_organ(&self, organ: OrganDefinition) {
        self.cmd(Cmd::Organ(Box::new(organ)));
    }
    pub fn set_settings(&self, settings: PlayerSettings) {
        self.cmd(Cmd::Settings(settings));
    }
}

fn idle_status(organ: &OrganDefinition) -> PlayerStatus {
    PlayerStatus {
        state: PlayerState::Stopped,
        track_id: None,
        title: None,
        position_ms: 0,
        duration_ms: 0,
        elapsed_ms: 0,
        total_ms: 0,
        tempo: 100,
        verse: 0,
        verses: 0,
        plan: Vec::new(),
        stop_after_verse: false,
        skip_to_last: false,
        fading: false,
        organ_id: organ.id.clone(),
    }
}

// ---------------------------------------------------------------------------
// Engine (player thread)
// ---------------------------------------------------------------------------

struct Fade {
    started: Instant,
    /// Volume before the fade, restored afterwards.
    from: u8,
    last_sent: u8,
}

struct Engine {
    link: Box<dyn Link>,
    driver: Driver,
    organist: Organist,
    settings: PlayerSettings,
    status: Arc<Mutex<PlayerStatus>>,
    events: broadcast::Sender<PlayerEvent>,

    track: Option<Arc<Track>>,
    spans: Vec<Span>,
    /// Indexes into `spans`, in playing order.
    plan: Vec<usize>,
    step: usize,
    state: PlayerState,
    /// Track time at `anchor_at`, and playback rate.
    anchor_pos: f64,
    anchor_at: Instant,
    rate: f64,
    start_at: Option<Instant>,
    next: usize,

    /// The registration the track asks for right now.
    recorded: Registration,
    /// What we believe the organ has drawn.
    drawn: BTreeSet<String>,
    touched: HashMap<String, Instant>,
    /// Keys we hold down, counted (two recorded divisions may share one).
    held: BTreeMap<(DivisionRole, u8), u32>,
    /// Last values sent, to avoid repeats.
    expression: BTreeMap<DivisionRole, u8>,
    master: Option<u8>,

    stop_after_verse: bool,
    skip_to_last: bool,
    fade: Option<Fade>,
    last_status: Instant,
    last_sync: Instant,
}

impl Engine {
    fn new(
        link: Box<dyn Link>,
        organ: OrganDefinition,
        settings: PlayerSettings,
        status: Arc<Mutex<PlayerStatus>>,
        events: broadcast::Sender<PlayerEvent>,
    ) -> Self {
        let now = Instant::now();
        Self {
            link,
            driver: Driver::new(organ.clone()),
            organist: Organist::new(organ),
            settings,
            status,
            events,
            track: None,
            spans: Vec::new(),
            plan: Vec::new(),
            step: 0,
            state: PlayerState::Stopped,
            anchor_pos: 0.0,
            anchor_at: now,
            rate: 1.0,
            start_at: None,
            next: 0,
            recorded: Registration::default(),
            drawn: BTreeSet::new(),
            touched: HashMap::new(),
            held: BTreeMap::new(),
            expression: BTreeMap::new(),
            master: None,
            stop_after_verse: false,
            skip_to_last: false,
            fade: None,
            last_status: now,
            last_sync: now,
        }
    }

    fn organ(&self) -> &OrganDefinition {
        self.driver.organ()
    }

    fn run(mut self, rx: mpsc::Receiver<Cmd>) {
        loop {
            let wait = self.time_to_next().min(Duration::from_millis(50));
            match rx.recv_timeout(wait) {
                Ok(cmd) => self.handle(cmd),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let before = (self.state, self.step);
            self.pump();
            if before != (self.state, self.step) || self.last_status.elapsed() >= Duration::from_millis(200) {
                self.publish();
            }
        }
    }

    // -- clock -----------------------------------------------------------------

    fn position(&self) -> f64 {
        match self.state {
            PlayerState::Playing => self.anchor_pos + self.anchor_at.elapsed().as_secs_f64() * 1000.0 * self.rate,
            _ => self.anchor_pos,
        }
    }

    fn set_position(&mut self, pos: f64) {
        self.anchor_pos = pos;
        self.anchor_at = Instant::now();
    }

    fn span(&self) -> Option<Span> {
        self.plan.get(self.step).and_then(|i| self.spans.get(*i)).copied()
    }

    fn time_to_next(&self) -> Duration {
        match self.state {
            PlayerState::Starting => {
                self.start_at.map_or(Duration::ZERO, |t| t.saturating_duration_since(Instant::now()))
            }
            PlayerState::Playing => {
                let Some(track) = &self.track else { return Duration::from_millis(50) };
                let span_end = self.span().map_or(0, |s| s.end) as f64;
                let due = track.events.get(self.next).map_or(span_end, |e| (e.t as f64).min(span_end));
                let ms = ((due - self.position()) / self.rate).max(0.0);
                Duration::from_micros((ms * 1000.0) as u64)
            }
            _ => Duration::from_millis(50),
        }
    }

    // -- commands --------------------------------------------------------------

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Load(track, options, play) => self.load(*track, options, play),
            Cmd::Play => match self.state {
                PlayerState::Paused => self.resume(),
                // From where it was loaded or cued (the verse start after a stop).
                PlayerState::Stopped if self.track.is_some() => self.begin(self.anchor_pos as u32),
                _ => {}
            },
            Cmd::Pause => match self.state {
                PlayerState::Playing => {
                    let pos = self.position();
                    self.set_position(pos);
                    self.state = PlayerState::Paused;
                    self.release_all();
                }
                PlayerState::Paused => self.resume(),
                _ => {}
            },
            Cmd::Stop => self.stop(false),
            Cmd::Seek(ms) => self.seek(ms),
            Cmd::Tempo(p) => {
                let pos = self.position();
                self.set_position(pos);
                self.rate = p.clamp(25, 400) as f64 / 100.0;
            }
            Cmd::StopAfterVerse(on) => self.stop_after_verse = on,
            Cmd::SkipToLast(on) => self.skip_to_last = on,
            Cmd::Fade(true) => {
                if self.fade.is_none() && matches!(self.state, PlayerState::Playing | PlayerState::Paused) {
                    let from = self.master.unwrap_or(127);
                    self.fade = Some(Fade { started: Instant::now(), from, last_sent: from });
                }
            }
            Cmd::Fade(false) => {
                if let Some(f) = self.fade.take() {
                    self.restore_volume(f.from);
                }
            }
            Cmd::Organ(organ) => {
                self.stop(false);
                self.driver = Driver::new((*organ).clone());
                self.organist = Organist::new(*organ);
                self.drawn.clear();
            }
            Cmd::Settings(s) => self.settings = s,
        }
        self.publish();
    }

    fn load(&mut self, track: Track, options: PlayOptions, play: bool) {
        self.stop(false);
        self.spans = track.verse_spans();
        let plan = options.verses.unwrap_or(VersePlan::Count(self.spans.len())).resolve(self.spans.len());
        self.plan = if plan.is_empty() { vec![0] } else { plan.into_iter().map(|v| v - 1).collect() };
        // The piece's own usual speed, unless asked for another.
        self.rate = options.tempo.unwrap_or(track.info.tempo).clamp(25, 400) as f64 / 100.0;
        self.track = Some(Arc::new(track));
        self.step = 0;
        let start = options.start_ms.unwrap_or_else(|| self.span().map_or(0, |s| s.start));
        if let Some(i) = self.plan.iter().position(|i| {
            let s = self.spans[*i];
            (s.start..s.end).contains(&start)
        }) {
            self.step = i;
        }
        self.set_position(start as f64);
        if play {
            self.begin(start);
        }
    }

    /// Clear the organ, then (after it settles) set the registration at `pos`
    /// and start the clock.
    fn begin(&mut self, pos: u32) {
        self.release_all();
        self.fade = None;
        self.stop_after_verse = false;
        self.skip_to_last = false;
        self.clear_organ();
        let toggles = self.organ().stops.iter().any(|s| matches!(s.midi.first(), Some(StopBinding::Toggle { .. })));
        let settle = if toggles { self.settings.toggle_settle_ms } else { 30 };
        self.start_at = Some(Instant::now() + Duration::from_millis(settle as u64));
        self.set_position(pos as f64);
        self.state = PlayerState::Starting;
    }

    fn clear_organ(&mut self) {
        let console = self.link.console();
        let msgs = self.driver.encode(&OrganEvent::AllStopsOff, |id| console.is_on(id));
        for m in msgs {
            self.link.send(m);
        }
        self.drawn.clear();
        self.touched.clear();
        self.expression.clear();
        self.master = None;
    }

    fn resume(&mut self) {
        let pos = self.position();
        self.state = PlayerState::Playing;
        self.set_position(pos);
        // Tape-recorder style: keys held at this point sound again.
        if let Some(track) = self.track.clone() {
            let (_, held) = track.state_at(pos as u32 + 1);
            for (d, k) in held {
                self.note(d, k, 80);
            }
        }
    }

    fn stop(&mut self, finished: bool) {
        if self.state == PlayerState::Stopped {
            return;
        }
        self.release_all();
        if let Some(f) = self.fade.take() {
            // Fade done (or abandoned): put the volume back for next time.
            self.restore_volume(f.from);
        }
        self.state = PlayerState::Stopped;
        self.start_at = None;
        self.step = 0;
        self.set_position(self.span().map_or(0, |s| s.start) as f64);
        self.next = 0;
        if finished {
            if let Some(t) = &self.track {
                let _ = self.events.send(PlayerEvent::Finished { track_id: t.id.clone() });
            }
        }
    }

    fn seek(&mut self, ms: u32) {
        let Some(track) = self.track.clone() else { return };
        let ms = ms.min(track.duration_ms);
        if let Some(i) = self.plan.iter().position(|i| {
            let s = self.spans[*i];
            (s.start..s.end).contains(&ms)
        }) {
            self.step = i;
        }
        self.set_position(ms as f64);
        match self.state {
            PlayerState::Playing | PlayerState::Paused => {
                self.release_all();
                self.regenerate(&track, ms);
                if self.state == PlayerState::Playing {
                    let (_, held) = track.state_at(ms + 1);
                    for (d, k) in held {
                        self.note(d, k, 80);
                    }
                }
            }
            _ => {}
        }
    }

    /// Put the organ into the state the track has at `ms` and continue from
    /// there.
    fn regenerate(&mut self, track: &Track, ms: u32) {
        let (reg, _) = track.state_at(ms);
        self.recorded = reg;
        self.apply_registration();
        let expression = self.recorded.expression.clone();
        for (d, v) in expression {
            self.expression(d, v);
        }
        if let Some(v) = self.recorded.master_volume {
            self.recorded_master(v);
        } else if self.organ().master_volume.is_some() {
            self.recorded_master(127);
        }
        self.next = track.events.partition_point(|e| e.t < ms);
    }

    // -- the clock running ------------------------------------------------------

    fn pump(&mut self) {
        if self.state == PlayerState::Starting && self.start_at.is_some_and(|t| Instant::now() >= t) {
            self.start_at = None;
            if let Some(track) = self.track.clone() {
                let pos = self.anchor_pos as u32;
                self.regenerate(&track, pos);
                self.state = PlayerState::Playing;
                self.set_position(pos as f64);
            }
        }
        if self.state != PlayerState::Playing {
            return;
        }
        let Some(track) = self.track.clone() else { return };
        self.sync_manual_changes();
        let pos = self.position();
        let span = self.span().unwrap_or(Span { start: 0, end: track.duration_ms });

        let mut registration_changed = false;
        while let Some(e) = track.events.get(self.next) {
            if e.t as f64 > pos || e.t >= span.end {
                break;
            }
            self.next += 1;
            match &e.event {
                TrackEvent::Note { division, key, velocity } => {
                    if registration_changed {
                        // Draw stops before the chord they colour.
                        self.apply_registration();
                        registration_changed = false;
                    }
                    self.note(*division, *key, *velocity);
                }
                TrackEvent::Stop { stop, on } => {
                    if *on {
                        self.recorded.stops.insert(stop.clone());
                    } else {
                        self.recorded.stops.remove(stop);
                    }
                    registration_changed = true;
                }
                TrackEvent::Expression { division, value } => {
                    self.recorded.expression.insert(*division, *value);
                    self.expression(*division, *value);
                }
                TrackEvent::MasterVolume { value } => {
                    self.recorded.master_volume = Some(*value);
                    self.recorded_master(*value);
                }
            }
        }
        if registration_changed {
            self.apply_registration();
        }
        self.pump_fade();
        if self.state == PlayerState::Playing && pos >= span.end as f64 {
            self.end_of_span(&track);
        }
    }

    fn end_of_span(&mut self, track: &Track) {
        self.release_all();
        let last = self.plan.len().saturating_sub(1);
        if self.stop_after_verse || self.step >= last {
            self.stop(true);
            return;
        }
        self.step = if self.skip_to_last { last } else { self.step + 1 };
        self.skip_to_last = false;
        let start = self.span().map_or(0, |s| s.start);
        self.set_position(start as f64);
        self.regenerate(track, start);
    }

    fn pump_fade(&mut self) {
        let Some(f) = &self.fade else { return };
        let fade_ms = self.settings.fade_ms.max(1) as f64;
        let progress = (f.started.elapsed().as_secs_f64() * 1000.0 / fade_ms).min(1.0);
        let value = (f.from as f64 * (1.0 - progress)).round() as u8;
        if value != f.last_sent {
            self.send_volume_raw(value);
            if let Some(f) = &mut self.fade {
                f.last_sent = value;
            }
        }
        if progress >= 1.0 {
            self.stop(false);
        }
    }

    /// Pick up stops the organist changed by hand while we play.
    fn sync_manual_changes(&mut self) {
        if self.last_sync.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_sync = Instant::now();
        let live = self.link.console();
        let quiet = Duration::from_millis(600);
        for s in &self.organ().stops.clone() {
            if self.touched.get(&s.id).is_some_and(|t| t.elapsed() < quiet) {
                continue;
            }
            let on = live.is_on(&s.id);
            if on != self.drawn.contains(&s.id) {
                if on {
                    self.drawn.insert(s.id.clone());
                } else {
                    self.drawn.remove(&s.id);
                }
            }
        }
    }

    // -- output --------------------------------------------------------------------

    fn send_event(&mut self, e: OrganEvent) {
        let drawn = &self.drawn;
        let msgs = self.driver.encode(&e, |id| drawn.contains(id));
        for m in msgs {
            self.link.send(m);
        }
    }

    fn note(&mut self, division: DivisionRole, key: u8, velocity: u8) {
        let Some(d) = self.organist.division(division) else { return };
        let count = self.held.entry((d, key)).or_insert(0);
        if velocity > 0 {
            *count += 1;
            if *count > 1 {
                return; // already sounding from another recorded keyboard
            }
        } else {
            if *count == 0 {
                return;
            }
            *count -= 1;
            if *count > 0 {
                return;
            }
            self.held.remove(&(d, key));
        }
        self.send_event(OrganEvent::Note { division: d, key, velocity });
    }

    fn release_all(&mut self) {
        let keys: Vec<_> = std::mem::take(&mut self.held).into_keys().collect();
        for (d, k) in keys {
            self.send_event(OrganEvent::Note { division: d, key: k, velocity: 0 });
        }
    }

    fn apply_registration(&mut self) {
        let target = self.organist.stops_for(&self.recorded);
        let off: Vec<String> = self.drawn.difference(&target).cloned().collect();
        let on: Vec<String> = target.difference(&self.drawn).cloned().collect();
        let now = Instant::now();
        for id in off {
            self.send_event(OrganEvent::Stop { stop: id.clone(), on: false });
            self.drawn.remove(&id);
            self.touched.insert(id, now);
        }
        for id in on {
            self.send_event(OrganEvent::Stop { stop: id.clone(), on: true });
            self.drawn.insert(id.clone());
            self.touched.insert(id, now);
        }
    }

    fn expression(&mut self, division: DivisionRole, value: u8) {
        let Some(d) = self.organist.expression_division(division) else { return };
        if self.expression.insert(d, value) != Some(value) {
            self.send_event(OrganEvent::Expression { division: d, value });
        }
    }

    /// A master volume from the track, scaled by the track's volume setting.
    fn recorded_master(&mut self, value: u8) {
        let volume = self.track.as_ref().map_or(100, |t| t.info.volume) as u32;
        let v = (value as u32 * volume / 100) as u8;
        if self.fade.is_some() {
            if let Some(f) = &mut self.fade {
                f.from = v;
            }
            return;
        }
        self.send_volume(v);
    }

    fn send_volume(&mut self, v: u8) {
        if self.organ().master_volume.is_some() {
            self.master = Some(v);
            self.send_event(OrganEvent::MasterVolume { value: v });
        }
    }

    /// Undo a fade: the master volume, or the swell boxes as they were.
    fn restore_volume(&mut self, from: u8) {
        if self.organ().master_volume.is_some() {
            self.send_volume(from);
        } else {
            let expression: Vec<_> = self.expression.iter().map(|(d, v)| (*d, *v)).collect();
            for (d, v) in expression {
                self.send_event(OrganEvent::Expression { division: d, value: v });
            }
        }
    }

    /// During a fade: master volume if the organ has one, else the swell boxes.
    fn send_volume_raw(&mut self, v: u8) {
        if self.organ().master_volume.is_some() {
            self.send_event(OrganEvent::MasterVolume { value: v });
        } else {
            let divisions: Vec<DivisionRole> =
                self.organ().divisions.iter().filter(|d| d.expression.is_some()).map(|d| d.role).collect();
            for d in divisions {
                let base = self.expression.get(&d).copied().unwrap_or(127) as u32;
                let scaled = (base * v as u32 / 127) as u8;
                self.send_event(OrganEvent::Expression { division: d, value: scaled });
            }
        }
    }

    fn publish(&mut self) {
        self.last_status = Instant::now();
        let status = self.snapshot();
        *self.status.lock().unwrap_or_else(|p| p.into_inner()) = status.clone();
        let _ = self.events.send(PlayerEvent::Status(status));
    }

    fn snapshot(&self) -> PlayerStatus {
        let mut s = idle_status(self.organ());
        s.state = self.state;
        s.tempo = (self.rate * 100.0).round() as u16;
        s.stop_after_verse = self.stop_after_verse;
        s.skip_to_last = self.skip_to_last;
        s.fading = self.fade.is_some();
        let Some(t) = &self.track else { return s };
        let pos = self.position().clamp(0.0, t.duration_ms as f64) as u32;
        s.track_id = Some(t.id.clone());
        s.title = Some(t.info.title.clone());
        s.position_ms = pos;
        s.duration_ms = t.duration_ms;
        s.plan = self.plan.iter().map(|i| i + 1).collect();
        s.verses = self.plan.len();
        s.verse = self.step + 1;
        let len = |i: &usize| self.spans.get(*i).map_or(0, |sp| sp.end - sp.start);
        s.total_ms = self.plan.iter().map(len).sum();
        let before: u32 = self.plan.iter().take(self.step).map(len).sum();
        let into = self.span().map_or(0, |sp| pos.saturating_sub(sp.start).min(sp.end - sp.start));
        s.elapsed_ms = before + into;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::builtin_organs;
    use crate::track::{StopRef, TimedEvent, TrackInfo, TrackSource, TRACK_SCHEMA};

    /// Records what was sent, decoded with the organ's driver, with times.
    #[derive(Clone)]
    struct Probe {
        start: Instant,
        sent: Arc<Mutex<crate::smf::Timed<u64>>>,
        console: Arc<Mutex<ConsoleState>>,
        driver: Arc<Driver>,
    }

    impl Link for Probe {
        fn send(&self, msg: Vec<u8>) {
            for e in self.driver.decode(&msg) {
                self.console.lock().unwrap().apply(&e);
            }
            self.sent.lock().unwrap().push((self.start.elapsed().as_millis() as u64, msg));
        }
        fn console(&self) -> ConsoleState {
            self.console.lock().unwrap().clone()
        }
    }

    impl Probe {
        fn new(organ: &OrganDefinition) -> Self {
            Self {
                start: Instant::now(),
                sent: Default::default(),
                console: Arc::new(Mutex::new(ConsoleState::new(organ))),
                driver: Arc::new(Driver::new(organ.clone())),
            }
        }
        fn events(&self) -> Vec<(u64, OrganEvent)> {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .flat_map(|(t, m)| self.driver.decode(m).into_iter().map(move |e| (*t, e)))
                .collect()
        }
        fn note_ons(&self) -> Vec<(u64, u8)> {
            self.events()
                .into_iter()
                .filter_map(|(t, e)| match e {
                    OrganEvent::Note { key, velocity, .. } if velocity > 0 => Some((t, key)),
                    _ => None,
                })
                .collect()
        }
    }

    fn organ(id: &str) -> OrganDefinition {
        builtin_organs().into_iter().find(|o| o.id == id).unwrap()
    }

    fn sref(o: &OrganDefinition, id: &str) -> StopRef {
        StopRef::of(o, id).unwrap()
    }

    /// Three notes 100 ms apart, in two "verses".
    fn track(o: &OrganDefinition) -> Track {
        let note =
            |t, key, v| TimedEvent { t, event: TrackEvent::Note { division: DivisionRole::Great, key, velocity: v } };
        let mut initial = Registration::default();
        initial.stops.insert(sref(o, "gt-opendiap"));
        let mut t = Track {
            schema: TRACK_SCHEMA,
            id: "t1".into(),
            info: TrackInfo::titled("Test"),
            source: TrackSource::Recorded,
            organ_id: o.id.clone(),
            organ_name: o.name.clone(),
            created: 0,
            modified: 0,
            initial,
            duration_ms: 400,
            verses: vec![150],
            meter: None,
            events: vec![
                note(10, 60, 80),
                note(90, 60, 0),
                note(110, 62, 80),
                note(140, 62, 0),
                TimedEvent { t: 160, event: TrackEvent::Stop { stop: sref(o, "gt-trumpet"), on: true } },
                note(200, 64, 80),
                note(290, 64, 0),
            ],
        };
        t.sort();
        t
    }

    fn wait_until(p: &Player, secs: f64, pred: impl Fn(&PlayerStatus) -> bool) -> PlayerStatus {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        loop {
            let s = p.status();
            if pred(&s) {
                return s;
            }
            assert!(Instant::now() < deadline, "timed out; status {s:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn start(o: &OrganDefinition) -> (Player, Probe) {
        let probe = Probe::new(o);
        let p = Player::start(probe.clone(), o.clone(), PlayerSettings { fade_ms: 200, toggle_settle_ms: 100 });
        (p, probe)
    }

    #[test]
    fn plays_notes_in_time_with_the_registration() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        p.load(track(&o), PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        let ons = probe.note_ons();
        assert_eq!(ons.iter().map(|(_, k)| *k).collect::<Vec<_>>(), [60, 62, 64]);
        let gap = |a: usize, b: usize| ons[b].0 as i64 - ons[a].0 as i64;
        assert!((gap(0, 1) - 100).abs() <= 25, "{ons:?}");
        assert!((gap(1, 2) - 90).abs() <= 25, "{ons:?}");
        // Opening registration, then the trumpet before the last note.
        let c = probe.console();
        assert!(c.is_on("gt-opendiap") && c.is_on("gt-trumpet"));
        assert!(c.notes.is_empty(), "no stuck notes");
    }

    #[test]
    fn double_tempo_halves_the_gaps() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        p.load(track(&o), PlayOptions { tempo: Some(200), ..Default::default() }, true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        let ons = probe.note_ons();
        assert!((ons[1].0 as i64 - ons[0].0 as i64 - 50).abs() <= 25, "{ons:?}");
    }

    #[test]
    fn verse_plans_skip_and_repeat() {
        assert_eq!(VersePlan::Count(2).resolve(4), [1, 4]);
        assert_eq!(VersePlan::Count(4).resolve(4), [1, 2, 3, 4]);
        assert_eq!(VersePlan::Count(6).resolve(3), [1, 2, 2, 2, 2, 3]);
        assert_eq!(VersePlan::Count(3).resolve(1), [1, 1, 1]);
        assert_eq!(VersePlan::List(vec![1, 2, 9, 2]).resolve(3), [1, 2, 2]);
    }

    #[test]
    fn playing_only_the_last_verse_regenerates_its_registration() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        p.load(track(&o), PlayOptions { verses: Some(VersePlan::List(vec![2])), ..Default::default() }, true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        assert_eq!(probe.note_ons().iter().map(|(_, k)| *k).collect::<Vec<_>>(), [64]);
        assert!(probe.console().is_on("gt-opendiap"), "verse 2 starts with verse 1's stops");
    }

    #[test]
    fn stop_after_this_verse() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        p.load(track(&o), PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing);
        p.stop_after_verse(true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        assert_eq!(probe.note_ons().len(), 2);
    }

    #[test]
    fn pause_releases_keys_and_holds_position() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        p.load(track(&o), PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing && s.position_ms > 30);
        p.pause();
        let s = wait_until(&p, 1.0, |s| s.state == PlayerState::Paused);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(p.status().position_ms, s.position_ms);
        assert!(probe.console().notes.is_empty());
        p.pause(); // latching: press again to continue
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        assert_eq!(probe.note_ons().last().unwrap().1, 64);
    }

    #[test]
    fn toggle_organs_get_a_general_cancel_first() {
        let o = organ("makin-2m");
        let (p, probe) = start(&o);
        // Something left drawn on the organ.
        probe.send(vec![0xCB, 0x0F]);
        let mut t = track(&o);
        t.events.clear();
        t.events.push(TimedEvent {
            t: 10,
            event: TrackEvent::Note { division: DivisionRole::Great, key: 60, velocity: 80 },
        });
        t.events.push(TimedEvent {
            t: 20,
            event: TrackEvent::Note { division: DivisionRole::Great, key: 60, velocity: 0 },
        });
        p.load(t, PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped && s.track_id.is_some());
        let sent = probe.sent.lock().unwrap().clone();
        assert_eq!(sent[1].1, vec![0xF0, 0x00, 0x4A, 0x4F, 0x48, 0x41, 0x53, 0x7F, 0xF7]);
        // The opening stop comes only after the settle time.
        let draw = sent[2..].iter().find(|(_, m)| m[0] == 0xCB).unwrap();
        assert!(draw.0 >= sent[1].0 + 90, "{sent:?}");
        let c = probe.console();
        assert_eq!(c.stops.iter().collect::<Vec<_>>(), ["gt-opendiap"]);
    }

    #[test]
    fn fade_stops_then_restores_the_volume() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        let mut t = track(&o);
        t.duration_ms = 5000;
        t.verses.clear();
        p.load(t, PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Playing);
        p.fade(true);
        wait_until(&p, 3.0, |s| s.state == PlayerState::Stopped);
        // The demo organ has a master volume, so the fade uses it.
        let volume: Vec<u8> = probe
            .events()
            .into_iter()
            .filter_map(|(_, e)| match e {
                OrganEvent::MasterVolume { value } => Some(value),
                _ => None,
            })
            .collect();
        assert!(volume.contains(&0), "faded to silence: {volume:?}");
        assert_eq!(volume.last(), Some(&127), "and put back for next time: {volume:?}");
    }

    #[test]
    fn seeking_back_redraws_earlier_stops() {
        let o = organ("demo");
        let (p, probe) = start(&o);
        let mut t = track(&o);
        t.duration_ms = 3000;
        t.verses.clear();
        p.load(t, PlayOptions::default(), true);
        wait_until(&p, 3.0, |s| s.position_ms > 220);
        assert!(probe.console().is_on("gt-trumpet"));
        p.seek(50);
        std::thread::sleep(Duration::from_millis(30));
        let c = probe.console();
        assert!(!c.is_on("gt-trumpet") && c.is_on("gt-opendiap"), "{c:?}");
        p.stop();
    }
}
