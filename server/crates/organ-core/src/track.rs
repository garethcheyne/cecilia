//! Tracks: neutral, organ-independent recordings.
//!
//! A track never stores MIDI. It stores what the organist did: keys,
//! stop changes (as [`StopRef`]s, not stop ids), expression and master
//! volume, timed in milliseconds from the start of the track. That is what
//! lets the same recording play on any organ (see [`crate::organist`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::console::ConsoleState;
use crate::driver::OrganEvent;
use crate::organ::{DivisionRole, Equivalent, OrganDefinition};

pub const TRACK_SCHEMA: u32 = 1;

/// A stop as a track sees it: which keyboard, and what it *is*.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct StopRef {
    pub division: DivisionRole,
    pub equivalent: Equivalent,
}

impl StopRef {
    /// The track's view of a stop on `organ`.
    pub fn of(organ: &OrganDefinition, stop_id: &str) -> Option<Self> {
        organ
            .stop(stop_id)
            .map(|s| Self { division: s.division, equivalent: s.equivalent.clone() })
    }

    /// The stop on `organ` that is exactly this one, if any.
    pub fn find_in<'a>(&self, organ: &'a OrganDefinition) -> Option<&'a str> {
        organ
            .stops
            .iter()
            .find(|s| s.division == self.division && s.equivalent == self.equivalent)
            .map(|s| s.id.as_str())
    }
}

/// Something the organist did, in organ-neutral terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrackEvent {
    Note {
        division: DivisionRole,
        key: u8,
        /// 0 = note off.
        velocity: u8,
    },
    Stop {
        stop: StopRef,
        on: bool,
    },
    Expression {
        division: DivisionRole,
        value: u8,
    },
    MasterVolume {
        value: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TimedEvent {
    /// Milliseconds from the start of the track.
    pub t: u32,
    #[serde(flatten)]
    pub event: TrackEvent,
}

/// Stops drawn plus pedal positions at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Ceciliation {
    #[serde(default)]
    pub stops: BTreeSet<StopRef>,
    /// Expression per division, 0-127.
    #[serde(default)]
    pub expression: BTreeMap<DivisionRole, u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_volume: Option<u8>,
}

impl Ceciliation {
    /// The ceciliation showing on `console` of `organ`.
    pub fn from_console(organ: &OrganDefinition, console: &ConsoleState) -> Self {
        Self {
            stops: console.stops.iter().filter_map(|id| StopRef::of(organ, id)).collect(),
            expression: console.expression.clone(),
            master_volume: organ.master_volume.as_ref().map(|_| console.master_volume),
        }
    }

    fn apply(&mut self, event: &TrackEvent) {
        match event {
            TrackEvent::Stop { stop, on: true } => {
                self.stops.insert(stop.clone());
            }
            TrackEvent::Stop { stop, on: false } => {
                self.stops.remove(stop);
            }
            TrackEvent::Expression { division, value } => {
                self.expression.insert(*division, *value);
            }
            TrackEvent::MasterVolume { value } => self.master_volume = Some(*value),
            TrackEvent::Note { .. } => {}
        }
    }
}

/// Descriptive fields, all editable in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrackInfo {
    pub title: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub performer: String,
    #[serde(default)]
    pub publication: String,
    #[serde(default)]
    pub copyright: String,
    /// Hymn tune name, e.g. "OLD 100TH".
    #[serde(default)]
    pub melody: String,
    /// Playback volume 1-100, applied through the master volume.
    #[serde(default = "full_volume")]
    pub volume: u8,
}

fn full_volume() -> u8 {
    100
}

impl TrackInfo {
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            composer: String::new(),
            performer: String::new(),
            publication: String::new(),
            copyright: String::new(),
            melody: String::new(),
            volume: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackSource {
    /// Played on an organ (or the simulator) and recorded.
    Recorded,
    /// One of the bundled sample pieces.
    Sample,
    /// Imported from a MIDI or MusicXML file.
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Major,
    Minor,
}

/// Musical framing for the score view. Exact for samples and imports;
/// estimated for live recordings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Meter {
    /// Crotchet (quarter-note) beats per minute at 100 % tempo.
    pub bpm: f32,
    /// Time signature, e.g. 4/4, 3/4, 6/8.
    pub beats: u8,
    pub beat_unit: u8,
    /// Key signature as sharps (+) or flats (-).
    #[serde(default)]
    pub fifths: i8,
    #[serde(default = "major")]
    pub mode: Mode,
    /// Track time of the first downbeat (after any pick-up).
    #[serde(default)]
    pub offset_ms: u32,
}

fn major() -> Mode {
    Mode::Major
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Track {
    pub schema: u32,
    pub id: String,
    pub info: TrackInfo,
    pub source: TrackSource,
    /// Organ it was recorded on (empty for samples).
    #[serde(default)]
    pub organ_id: String,
    #[serde(default)]
    pub organ_name: String,
    /// Unix time in ms.
    pub created: u64,
    pub modified: u64,
    /// Ceciliation and pedals when the track starts.
    pub initial: Ceciliation,
    /// Where the track ends (the last note plus the end margin).
    pub duration_ms: u32,
    /// Boundaries between hymn verses, in ms. Empty = a single verse.
    #[serde(default)]
    pub verses: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meter: Option<Meter>,
    /// Sorted by time; at equal times, stop and pedal changes come first.
    pub events: Vec<TimedEvent>,
}

/// A library listing row.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrackSummary {
    pub id: String,
    pub info: TrackInfo,
    pub source: TrackSource,
    pub organ_name: String,
    pub created: u64,
    pub modified: u64,
    pub duration_ms: u32,
    pub verses: usize,
    pub notes: usize,
    /// Divisions with notes in them.
    pub divisions: Vec<DivisionRole>,
}

/// A span of the track, used for verses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Track {
    pub fn summary(&self) -> TrackSummary {
        let divisions: BTreeSet<_> = self
            .events
            .iter()
            .filter_map(|e| match e.event {
                TrackEvent::Note { division, .. } => Some(division),
                _ => None,
            })
            .collect();
        TrackSummary {
            id: self.id.clone(),
            info: self.info.clone(),
            source: self.source,
            organ_name: self.organ_name.clone(),
            created: self.created,
            modified: self.modified,
            duration_ms: self.duration_ms,
            verses: self.verses.len() + 1,
            notes: self.note_count(),
            divisions: divisions.into_iter().collect(),
        }
    }

    pub fn note_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e.event, TrackEvent::Note { velocity, .. } if velocity > 0))
            .count()
    }

    /// Sort events by time, settings before notes at the same instant so a
    /// chord sounds with the new ceciliation, and note-offs before note-ons.
    pub fn sort(&mut self) {
        fn rank(e: &TrackEvent) -> u8 {
            match e {
                TrackEvent::Note { velocity: 0, .. } => 0,
                TrackEvent::Stop { .. } | TrackEvent::Expression { .. } | TrackEvent::MasterVolume { .. } => 1,
                TrackEvent::Note { .. } => 2,
            }
        }
        self.events.sort_by_key(|e| (e.t, rank(&e.event)));
    }

    /// Verse spans in order (one span covering the whole track if there are
    /// no markers).
    pub fn verse_spans(&self) -> Vec<Span> {
        let mut bounds = vec![0];
        bounds.extend(self.verses.iter().copied().filter(|&v| v > 0 && v < self.duration_ms));
        bounds.sort_unstable();
        bounds.dedup();
        bounds.push(self.duration_ms);
        bounds.windows(2).map(|w| Span { start: w[0], end: w[1] }).collect()
    }

    /// Ceciliation and held keys just before time `t` (events at exactly `t`
    /// are not applied).
    pub fn state_at(&self, t: u32) -> (Ceciliation, BTreeSet<(DivisionRole, u8)>) {
        let mut reg = self.initial.clone();
        let mut held = BTreeSet::new();
        for e in self.events.iter().take_while(|e| e.t < t) {
            match &e.event {
                TrackEvent::Note { division, key, velocity } => {
                    if *velocity > 0 {
                        held.insert((*division, *key));
                    } else {
                        held.remove(&(*division, *key));
                    }
                }
                other => reg.apply(other),
            }
        }
        (reg, held)
    }

    /// Check the invariants the player and editor rely on.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TRACK_SCHEMA {
            return Err(format!("unsupported track schema {}", self.schema));
        }
        if !valid_id(&self.id) {
            return Err(format!("track id {:?} must be letters, digits, '-' or '_'", self.id));
        }
        if self.info.title.trim().is_empty() {
            return Err("a track needs a title".into());
        }
        if !(1..=100).contains(&self.info.volume) {
            return Err("volume must be 1-100".into());
        }
        if self.events.windows(2).any(|w| w[0].t > w[1].t) {
            return Err("events are not in time order".into());
        }
        if self.events.last().is_some_and(|e| e.t > self.duration_ms) {
            return Err("an event falls after the end of the track".into());
        }
        Ok(())
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The organ-neutral form of a console event (`None` for things a track
/// doesn't store, like toggles of unknown stops).
pub fn to_track_event(organ: &OrganDefinition, console: &ConsoleState, e: &OrganEvent) -> Vec<TrackEvent> {
    match e {
        OrganEvent::Note { division, key, velocity } => {
            vec![TrackEvent::Note { division: *division, key: *key, velocity: *velocity }]
        }
        OrganEvent::Stop { stop, on } => StopRef::of(organ, stop)
            .map(|s| vec![TrackEvent::Stop { stop: s, on: *on }])
            .unwrap_or_default(),
        // Resolve a toggle against the state before it was applied.
        OrganEvent::StopToggle { stop } => StopRef::of(organ, stop)
            .map(|s| vec![TrackEvent::Stop { stop: s, on: !console.is_on(stop) }])
            .unwrap_or_default(),
        OrganEvent::Expression { division, value } => {
            vec![TrackEvent::Expression { division: *division, value: *value }]
        }
        OrganEvent::MasterVolume { value } => vec![TrackEvent::MasterVolume { value: *value }],
        OrganEvent::AllStopsOff => console
            .stops
            .iter()
            .filter_map(|id| StopRef::of(organ, id))
            .map(|s| TrackEvent::Stop { stop: s, on: false })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::{builtin_organs, Family};

    fn open_diapason() -> StopRef {
        StopRef {
            division: DivisionRole::Great,
            equivalent: Equivalent::Voice {
                family: Family::Principal,
                footage: "8".into(),
                ranks: None,
                tag: String::new(),
            },
        }
    }

    fn track(events: Vec<TimedEvent>) -> Track {
        Track {
            schema: TRACK_SCHEMA,
            id: "t".into(),
            info: TrackInfo::titled("Test"),
            source: TrackSource::Recorded,
            organ_id: "demo".into(),
            organ_name: "Demo".into(),
            created: 0,
            modified: 0,
            initial: Ceciliation::default(),
            duration_ms: 10_000,
            verses: vec![4000, 8000],
            meter: None,
            events,
        }
    }

    fn at(t: u32, event: TrackEvent) -> TimedEvent {
        TimedEvent { t, event }
    }

    #[test]
    fn stop_refs_find_the_same_stop_on_the_same_organ() {
        let organ = builtin_organs().remove(0);
        for s in &organ.stops {
            let r = StopRef::of(&organ, &s.id).unwrap();
            assert_eq!(r.find_in(&organ), Some(s.id.as_str()));
        }
        assert_eq!(StopRef::of(&organ, "gt-opendiap"), Some(open_diapason()));
    }

    #[test]
    fn state_at_replays_ceciliation_and_held_keys() {
        let note = |t, v| at(t, TrackEvent::Note { division: DivisionRole::Great, key: 60, velocity: v });
        let t = track(vec![
            at(100, TrackEvent::Stop { stop: open_diapason(), on: true }),
            note(200, 80),
            at(300, TrackEvent::Expression { division: DivisionRole::Swell, value: 40 }),
            note(500, 0),
        ]);
        let (reg, held) = t.state_at(400);
        assert!(reg.stops.contains(&open_diapason()));
        assert_eq!(reg.expression[&DivisionRole::Swell], 40);
        assert_eq!(held.into_iter().collect::<Vec<_>>(), [(DivisionRole::Great, 60)]);
        let (_, held) = t.state_at(600);
        assert!(held.is_empty());
        let (reg, _) = t.state_at(100);
        assert!(reg.stops.is_empty(), "events at exactly t are not applied yet");
    }

    #[test]
    fn verses_split_the_track() {
        let t = track(vec![]);
        assert_eq!(
            t.verse_spans(),
            [Span { start: 0, end: 4000 }, Span { start: 4000, end: 8000 }, Span { start: 8000, end: 10_000 }]
        );
        assert_eq!(t.summary().verses, 3);
    }

    #[test]
    fn sort_puts_ceciliation_before_the_chord_it_colours() {
        let mut t = track(vec![
            at(100, TrackEvent::Note { division: DivisionRole::Great, key: 60, velocity: 80 }),
            at(100, TrackEvent::Stop { stop: open_diapason(), on: true }),
            at(100, TrackEvent::Note { division: DivisionRole::Great, key: 59, velocity: 0 }),
        ]);
        t.sort();
        assert!(matches!(t.events[0].event, TrackEvent::Note { velocity: 0, .. }));
        assert!(matches!(t.events[1].event, TrackEvent::Stop { .. }));
        assert!(t.validate().is_ok());
    }

    #[test]
    fn json_shape_is_flat_and_readable() {
        let e = at(5, TrackEvent::Note { division: DivisionRole::Pedal, key: 36, velocity: 64 });
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v, serde_json::json!({ "t": 5, "type": "note", "division": "pedal", "key": 36, "velocity": 64 }));
        let back: TimedEvent = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }
}
