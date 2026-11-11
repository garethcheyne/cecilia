//! The auto-recorder: "record a whole service with one click".
//!
//! Once armed it waits for the first note, then records a track that starts
//! with the registration showing at that moment. After a stretch of silence
//! the track is finished and handed back, and the recorder waits for the
//! next piece. Stop and pedal changes made between pieces aren't lost: they
//! become the next track's starting registration.
//!
//! This is a pure state machine fed with timestamped input MIDI, so it is
//! deterministic and easy to test. The server drives it from the engine's
//! event stream and a timer.

use serde::Serialize;
use utoipa::ToSchema;

use crate::console::ConsoleState;
use crate::driver::{Driver, OrganEvent};
use crate::organ::OrganDefinition;
use crate::track::{to_track_event, Registration, TimedEvent, Track, TrackEvent, TrackInfo, TrackSource, TRACK_SCHEMA};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecorderState {
    /// Not recording.
    Off,
    /// Armed: the next note starts a track.
    Waiting,
    /// A piece is being recorded.
    Recording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecorderConfig {
    /// Silence after the last key is released that ends a piece.
    pub silence_ms: u32,
    /// Silence kept before the first note and after the last.
    pub margin_start_ms: u32,
    pub margin_end_ms: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self { silence_ms: 8000, margin_start_ms: 2000, margin_end_ms: 2000 }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecorderStatus {
    pub state: RecorderState,
    /// Length of the piece being recorded so far.
    pub elapsed_ms: u32,
    /// Notes in the piece being recorded.
    pub notes: usize,
    /// Tracks saved since the recorder was armed.
    pub saved: usize,
}

struct Take {
    /// Engine time (µs) of the first note.
    first_us: u64,
    initial: Registration,
    events: Vec<TimedEvent>,
    held: usize,
    /// Engine time of the last note-off (or note-on while keys are held).
    last_sound_us: u64,
    notes: usize,
}

pub struct AutoRecorder {
    driver: Driver,
    /// The organ's state as seen from its MIDI out, always kept current.
    console: ConsoleState,
    config: RecorderConfig,
    armed: bool,
    take: Option<Take>,
    saved: usize,
    last_us: u64,
}

impl AutoRecorder {
    pub fn new(organ: OrganDefinition, config: RecorderConfig) -> Self {
        Self {
            console: ConsoleState::new(&organ),
            driver: Driver::new(organ),
            config,
            armed: false,
            take: None,
            saved: 0,
            last_us: 0,
        }
    }

    pub fn organ(&self) -> &OrganDefinition {
        self.driver.organ()
    }

    pub fn set_config(&mut self, config: RecorderConfig) {
        self.config = config;
    }

    /// Switch organs. Anything being recorded is finished first.
    pub fn load_organ(&mut self, organ: OrganDefinition, now_us: u64) -> Option<Track> {
        let done = self.finish(now_us);
        self.console = ConsoleState::new(&organ);
        self.driver = Driver::new(organ);
        done
    }

    pub fn state(&self) -> RecorderState {
        match (&self.take, self.armed) {
            (Some(_), _) => RecorderState::Recording,
            (None, true) => RecorderState::Waiting,
            (None, false) => RecorderState::Off,
        }
    }

    pub fn status(&self) -> RecorderStatus {
        RecorderStatus {
            state: self.state(),
            elapsed_ms: self
                .take
                .as_ref()
                .map_or(0, |t| ((self.last_us.saturating_sub(t.first_us)) / 1000) as u32 + self.config.margin_start_ms),
            notes: self.take.as_ref().map_or(0, |t| t.notes),
            saved: self.saved,
        }
    }

    /// Start listening for the next piece.
    pub fn arm(&mut self) {
        if !self.armed {
            self.armed = true;
            self.saved = 0;
        }
    }

    /// Stop recording, returning the piece in progress (if it has notes).
    pub fn disarm(&mut self, now_us: u64) -> Option<Track> {
        let done = self.finish(now_us);
        self.armed = false;
        done
    }

    /// Feed one MIDI message received from the organ.
    pub fn on_midi(&mut self, t_us: u64, msg: &[u8]) {
        self.last_us = self.last_us.max(t_us);
        for e in self.driver.decode(msg) {
            self.on_event(t_us, &e);
        }
    }

    /// Feed an already-decoded console event (used by imports).
    pub fn on_event(&mut self, t_us: u64, e: &OrganEvent) {
        let starts_piece =
            self.armed && self.take.is_none() && matches!(e, OrganEvent::Note { velocity, .. } if *velocity > 0);
        if starts_piece {
            self.take = Some(Take {
                first_us: t_us,
                initial: Registration::from_console(self.driver.organ(), &self.console),
                events: Vec::new(),
                held: 0,
                last_sound_us: t_us,
                notes: 0,
            });
        }
        if let Some(take) = &mut self.take {
            let t = ((t_us.saturating_sub(take.first_us)) / 1000) as u32 + self.config.margin_start_ms;
            for event in to_track_event(self.driver.organ(), &self.console, e) {
                if let TrackEvent::Note { velocity, .. } = event {
                    if velocity > 0 {
                        take.held += 1;
                        take.notes += 1;
                    } else {
                        take.held = take.held.saturating_sub(1);
                    }
                    take.last_sound_us = t_us;
                }
                take.events.push(TimedEvent { t, event });
            }
        }
        self.console.apply(e);
    }

    /// Call regularly. Returns a finished track once the organ has been
    /// silent for long enough.
    pub fn tick(&mut self, now_us: u64) -> Option<Track> {
        self.last_us = self.last_us.max(now_us);
        let take = self.take.as_ref()?;
        let silent_for = now_us.saturating_sub(take.last_sound_us) / 1000;
        if take.held == 0 && silent_for >= self.config.silence_ms as u64 {
            self.finish(now_us)
        } else {
            None
        }
    }

    fn finish(&mut self, now_us: u64) -> Option<Track> {
        let take = self.take.take()?;
        let organ = self.driver.organ();
        let to_ms = |us: u64| ((us.saturating_sub(take.first_us)) / 1000) as u32 + self.config.margin_start_ms;
        // Release anything still held so the track never leaves a note on.
        let last_sound = if take.held > 0 { now_us.max(take.last_sound_us) } else { take.last_sound_us };
        let end_of_sound = to_ms(last_sound);
        let mut events: Vec<TimedEvent> = take.events.into_iter().filter(|e| e.t <= end_of_sound).collect();
        let mut held = std::collections::BTreeSet::new();
        for e in &events {
            if let TrackEvent::Note { division, key, velocity } = e.event {
                if velocity > 0 {
                    held.insert((division, key));
                } else {
                    held.remove(&(division, key));
                }
            }
        }
        for (division, key) in held {
            events.push(TimedEvent { t: end_of_sound, event: TrackEvent::Note { division, key, velocity: 0 } });
        }
        if take.notes == 0 {
            return None;
        }
        let created = unix_ms();
        let mut track = Track {
            schema: TRACK_SCHEMA,
            id: format!("rec-{created}"),
            info: TrackInfo::titled(recording_title(created)),
            source: TrackSource::Recorded,
            organ_id: organ.id.clone(),
            organ_name: organ.name.clone(),
            created,
            modified: created,
            initial: take.initial,
            duration_ms: end_of_sound + self.config.margin_end_ms,
            verses: Vec::new(),
            meter: None,
            events,
        };
        track.sort();
        self.saved += 1;
        Some(track)
    }
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// "Recording 11 Nov 2026 10:32" in UTC (the UI shows local times from
/// `created`; this is only a default title the organist can change).
fn recording_title(unix_ms: u64) -> String {
    let secs = unix_ms / 1000;
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    let hm = secs % 86_400;
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("Recording {d} {} {y} {:02}:{:02}", MONTHS[(m - 1) as usize], hm / 3600, hm % 3600 / 60)
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::{builtin_organs, DivisionRole};

    const S: u64 = 1_000_000; // one second in µs

    fn recorder() -> AutoRecorder {
        let organ = builtin_organs().into_iter().find(|o| o.id == "demo").unwrap();
        AutoRecorder::new(organ, RecorderConfig { silence_ms: 3000, margin_start_ms: 1000, margin_end_ms: 500 })
    }

    // Demo organ: Great on channel 1, Trumpet on = B0 50 7F? Use the driver instead.
    fn encode(r: &AutoRecorder, e: OrganEvent) -> Vec<u8> {
        Driver::new(r.organ().clone()).encode(&e, |_| false).remove(0)
    }

    fn key(r: &mut AutoRecorder, t: u64, k: u8, v: u8) {
        let m = encode(r, OrganEvent::Note { division: DivisionRole::Great, key: k, velocity: v });
        r.on_midi(t, &m);
    }

    #[test]
    fn records_from_the_first_note_with_the_registration_at_that_moment() {
        let mut r = recorder();
        // Drawn before arming: still part of the starting registration.
        let m = encode(&r, OrganEvent::Stop { stop: "gt-opendiap".into(), on: true });
        r.on_midi(0, &m);
        r.arm();
        assert_eq!(r.state(), RecorderState::Waiting);
        let m = encode(&r, OrganEvent::Stop { stop: "gt-principal".into(), on: true });
        r.on_midi(S, &m);
        assert_eq!(r.state(), RecorderState::Waiting, "stop changes don't start a piece");

        key(&mut r, 2 * S, 60, 90);
        assert_eq!(r.state(), RecorderState::Recording);
        key(&mut r, 3 * S, 60, 0);
        assert!(r.tick(5 * S).is_none(), "2 s of silence isn't enough");
        let t = r.tick(6 * S).expect("3 s of silence ends the piece");

        assert_eq!(r.state(), RecorderState::Waiting, "and waits for the next");
        assert_eq!(t.initial.stops.len(), 2);
        assert_eq!(t.events.len(), 2);
        assert_eq!(t.events[0].t, 1000, "first note after the start margin");
        assert_eq!(t.events[1].t, 2000);
        assert_eq!(t.duration_ms, 2500, "last note-off plus the end margin");
        assert_eq!(t.source, TrackSource::Recorded);
        t.validate().unwrap();
    }

    #[test]
    fn changes_between_pieces_belong_to_the_next_piece() {
        let mut r = recorder();
        r.arm();
        key(&mut r, 0, 60, 90);
        key(&mut r, S, 60, 0);
        // Pushed in after the last note, before the silence ran out.
        let m = encode(&r, OrganEvent::Stop { stop: "gt-trumpet".into(), on: true });
        r.on_midi(2 * S, &m);
        let first = r.tick(5 * S).unwrap();
        assert!(first.events.iter().all(|e| matches!(e.event, TrackEvent::Note { .. })));

        key(&mut r, 10 * S, 62, 90);
        key(&mut r, 11 * S, 62, 0);
        let second = r.disarm(11 * S).unwrap();
        assert_eq!(second.initial.stops.len(), 1, "the trumpet starts the next piece");
        assert_eq!(r.state(), RecorderState::Off);
    }

    #[test]
    fn held_keys_keep_the_piece_open_and_are_released_on_stop() {
        let mut r = recorder();
        r.arm();
        key(&mut r, 0, 48, 90);
        assert!(r.tick(60 * S).is_none(), "a held chord is not silence");
        let t = r.disarm(61 * S).unwrap();
        let last = t.events.last().unwrap();
        assert!(matches!(last.event, TrackEvent::Note { velocity: 0, key: 48, .. }));
    }

    #[test]
    fn nothing_is_saved_without_notes() {
        let mut r = recorder();
        r.arm();
        assert!(r.disarm(10 * S).is_none());
    }

    #[test]
    fn toggle_organs_record_absolute_stop_changes() {
        let organ = builtin_organs().into_iter().find(|o| o.id == "makin-2m").unwrap();
        let mut r = AutoRecorder::new(organ, RecorderConfig::default());
        r.arm();
        r.on_midi(0, &[0xCB, 0x12]); // Open Diapason toggled on
        r.on_midi(1, &[0x90, 60, 90]);
        r.on_midi(2 * S, &[0xCB, 0x12]); // and off again
        r.on_midi(3 * S, &[0x80, 60, 0]);
        let t = r.disarm(3 * S).unwrap();
        assert_eq!(t.initial.stops.len(), 1);
        assert!(t.events.iter().any(|e| matches!(&e.event, TrackEvent::Stop { on: false, .. })));
    }

    #[test]
    fn titles_read_as_dates() {
        // 2026-11-11 10:32 UTC
        assert_eq!(recording_title(1_794_393_120_000), "Recording 11 Nov 2026 10:32");
    }
}
