//! Bundled sample music, written in a small symbolic format and compiled to
//! ordinary tracks.
//!
//! A score is JSON:
//!
//! ```json
//! {
//!   "id": "old-hundredth", "title": "Old Hundredth", "composer": "…",
//!   "melody": "OLD 100TH", "copyright": "Public domain",
//!   "bpm": 72, "time": "4/4", "key": "G", "pickup": 0,
//!   "verses": 4, "gap": 2,
//!   "ceciliation": ["great principal 8", "great principal 4", "pedal flute 16", "great to pedal"],
//!   "verse_ceciliation": { "4": ["+great mixture", "+swell to great"] },
//!   "expression": { "swell": 90 },
//!   "parts": [
//!     { "division": "great", "notes": "G4:1 | G4:1 F#4:1 E4:1 D4:1 | …" },
//!     { "division": "pedal", "notes": "G2:1 | …" }
//!   ]
//! }
//! ```
//!
//! Notes are `pitch:beats` where beats count crotchets (`1`, `2`, `0.5`,
//! `1/2`, `1.5`). Pitches use scientific names (`C4` is middle C, `F#3`,
//! `Bb2`); `r` is a rest; `[C4 E4 G4]:2` is a chord. A trailing `~` ties a
//! note into the next note of the same pitch. `|` bar lines are ignored but
//! must fall on bar boundaries (checked), which catches miscounted bars.
//!
//! Stops are named `<division> <family> <footage> [tag]` (family: principal,
//! flute, string, celeste, reed, mutation, mixture), `<division> tremulant`,
//! or couplers `<from> to <to>`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::organ::{CouplerPitch, DivisionRole, Equivalent, Family};
use crate::track::{
    Meter, Mode, Ceciliation, StopRef, TimedEvent, Track, TrackEvent, TrackInfo, TrackSource, TRACK_SCHEMA,
};

const START_MARGIN_MS: f64 = 1000.0;
const END_MARGIN_MS: u32 = 2500;
const VELOCITY: u8 = 80;
/// Notes are released a little early so repeated notes speak again.
const RELEASE_MS: f64 = 35.0;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub melody: String,
    #[serde(default)]
    pub copyright: String,
    #[serde(default)]
    pub publication: String,
    /// Licence of this file, e.g. "CC0-1.0" or "public-domain" (required in
    /// the shared library).
    #[serde(default)]
    pub licence: String,
    /// Where the music came from (hymnal, edition, URL).
    #[serde(default)]
    pub source: String,
    /// For browsing, e.g. ["hymn", "evening", "funeral"].
    #[serde(default)]
    pub tags: Vec<String>,
    /// Crotchets per minute.
    pub bpm: f64,
    #[serde(default = "four_four")]
    pub time: String,
    #[serde(default = "c_major")]
    pub key: String,
    /// Crotchet beats before the first bar line (anacrusis).
    #[serde(default)]
    pub pickup: f64,
    #[serde(default = "one")]
    pub verses: u32,
    /// Crotchets of silence between verses.
    #[serde(default = "two")]
    pub gap: f64,
    pub ceciliation: Vec<String>,
    /// Changes at the start of verse N: "+stop" draws, "-stop" pushes in.
    #[serde(default)]
    pub verse_ceciliation: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    pub expression: BTreeMap<DivisionRole, u8>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub division: DivisionRole,
    pub notes: String,
}

fn four_four() -> String {
    "4/4".into()
}
fn c_major() -> String {
    "C".into()
}
fn one() -> u32 {
    1
}
fn two() -> f64 {
    2.0
}

/// A note in crotchet beats from the start of one verse.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreNote {
    pub division: DivisionRole,
    pub key: u8,
    pub start: f64,
    pub beats: f64,
}

pub fn parse_pitch(s: &str) -> Result<u8, String> {
    let bad = || format!("bad pitch {s:?}");
    let mut chars = s.chars();
    let letter = chars.next().ok_or_else(bad)?;
    let base: i32 = match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return Err(bad()),
    };
    let rest: String = chars.collect();
    let (acc, octave) = match rest.chars().next() {
        Some('#') | Some('♯') => (1, &rest[rest.char_indices().nth(1).map_or(rest.len(), |(i, _)| i)..]),
        Some('b') | Some('♭') => (-1, &rest[rest.char_indices().nth(1).map_or(rest.len(), |(i, _)| i)..]),
        _ => (0, rest.as_str()),
    };
    let octave: i32 = octave.parse().map_err(|_| bad())?;
    let n = (octave + 1) * 12 + base + acc;
    u8::try_from(n).ok().filter(|n| *n <= 127).ok_or_else(bad)
}

fn parse_beats(s: &str) -> Result<f64, String> {
    let v = match s.split_once('/') {
        Some((n, d)) => n.parse::<f64>().ok().zip(d.parse::<f64>().ok()).map(|(n, d)| n / d),
        None => s.parse::<f64>().ok(),
    };
    v.filter(|b| b.is_finite() && *b > 0.0).ok_or_else(|| format!("bad duration {s:?}"))
}

fn parse_time(s: &str) -> Result<(u8, u8), String> {
    let (b, u) = s.split_once('/').ok_or_else(|| format!("bad time signature {s:?}"))?;
    let b: u8 = b.trim().parse().map_err(|_| format!("bad time signature {s:?}"))?;
    let u: u8 = u.trim().parse().map_err(|_| format!("bad time signature {s:?}"))?;
    if b == 0 || ![2, 4, 8, 16].contains(&u) {
        return Err(format!("bad time signature {s:?}"));
    }
    Ok((b, u))
}

/// Key name ("G", "Bb", "F#m") to (sharps(+)/flats(-), mode).
pub fn parse_key(s: &str) -> Result<(i8, Mode), String> {
    let (tonic, mode) = match s.strip_suffix('m') {
        Some(t) => (t, Mode::Minor),
        None => (s, Mode::Major),
    };
    const MAJOR: [(&str, i8); 15] = [
        ("Cb", -7), ("Gb", -6), ("Db", -5), ("Ab", -4), ("Eb", -3), ("Bb", -2), ("F", -1), ("C", 0),
        ("G", 1), ("D", 2), ("A", 3), ("E", 4), ("B", 5), ("F#", 6), ("C#", 7),
    ];
    const MINOR: [(&str, i8); 15] = [
        ("Ab", -7), ("Eb", -6), ("Bb", -5), ("F", -4), ("C", -3), ("G", -2), ("D", -1), ("A", 0),
        ("E", 1), ("B", 2), ("F#", 3), ("C#", 4), ("G#", 5), ("D#", 6), ("A#", 7),
    ];
    let table = if mode == Mode::Major { &MAJOR } else { &MINOR };
    table
        .iter()
        .find(|(n, _)| *n == tonic)
        .map(|(_, f)| (*f, mode))
        .ok_or_else(|| format!("unknown key {s:?}"))
}

fn parse_division(s: &str) -> Result<DivisionRole, String> {
    serde_json::from_value(serde_json::Value::String(s.to_lowercase()))
        .map_err(|_| format!("unknown division {s:?}"))
}

/// "great principal 8", "swell tremulant", "swell to great".
pub fn parse_stop(s: &str) -> Result<StopRef, String> {
    let words: Vec<&str> = s.split_whitespace().collect();
    match words.as_slice() {
        [from, "to", to] => Ok(StopRef {
            division: parse_division(to)?,
            equivalent: Equivalent::Coupler { source: parse_division(from)?, pitch: CouplerPitch::Unison },
        }),
        [d, "tremulant"] => Ok(StopRef { division: parse_division(d)?, equivalent: Equivalent::Tremulant }),
        [d, family, rest @ ..] if !rest.is_empty() => {
            let family: Family = serde_json::from_value(serde_json::Value::String(family.to_string()))
                .map_err(|_| format!("unknown stop family in {s:?}"))?;
            // Footage may be "2 2/3"; anything after it is the tag.
            let mut footage = vec![rest[0]];
            let mut i = 1;
            if rest.get(1).is_some_and(|w| w.contains('/')) {
                footage.push(rest[1]);
                i = 2;
            }
            let footage = footage.join(" ");
            crate::organ::footage_feet(&footage).ok_or_else(|| format!("bad footage in {s:?}"))?;
            Ok(StopRef {
                division: parse_division(d)?,
                equivalent: Equivalent::Voice { family, footage, ranks: None, tag: rest[i..].join(" ") },
            })
        }
        _ => Err(format!("can't read stop {s:?}")),
    }
}

/// Parse one part into notes, checking bar lines against the time signature.
fn parse_part(part: &Part, bar_beats: f64, pickup: f64) -> Result<(Vec<ScoreNote>, f64), String> {
    let mut notes: Vec<ScoreNote> = Vec::new();
    let mut t = 0.0;
    // Last note per key that ended with a tie, waiting to be extended.
    let mut tied: BTreeMap<u8, usize> = BTreeMap::new();
    let text = part.notes.replace('[', "[ ").replace(']', " ]");
    let mut tokens = text.split_whitespace().peekable();
    let mut bar = 0usize;
    while let Some(tok) = tokens.next() {
        if tok == "|" {
            bar += 1;
            let expected = pickup + (bar as f64 - if pickup > 0.0 { 1.0 } else { 0.0 }) * bar_beats;
            let expected = if pickup > 0.0 && bar == 1 { pickup } else { expected };
            if (t - expected).abs() > 1e-6 {
                return Err(format!(
                    "{:?} part: bar line {bar} at beat {t}, expected {expected}",
                    part.division
                ));
            }
            continue;
        }
        // Chord: "[" pitch… "]:beats"
        let (pitches, dur_tok): (Vec<String>, String) = if tok == "[" {
            let mut ps = Vec::new();
            loop {
                let p = tokens.next().ok_or("unclosed chord")?;
                if let Some(d) = p.strip_prefix(']') {
                    break (ps, d.to_string());
                }
                ps.push(p.to_string());
            }
        } else {
            let (p, d) = tok.split_once(':').ok_or_else(|| format!("{tok:?} needs a :duration"))?;
            (vec![p.to_string()], format!(":{d}"))
        };
        let dur_tok = dur_tok.strip_prefix(':').ok_or_else(|| format!("chord needs ]:duration near {tok:?}"))?;
        let (dur_tok, tie) = match dur_tok.strip_suffix('~') {
            Some(d) => (d, true),
            None => (dur_tok, false),
        };
        let beats = parse_beats(dur_tok)?;
        if !(pitches.len() == 1 && pitches[0] == "r") {
            let mut next_tied = BTreeMap::new();
            for p in &pitches {
                let key = parse_pitch(p)?;
                let idx = match tied.remove(&key) {
                    Some(i) => {
                        notes[i].beats += beats;
                        i
                    }
                    None => {
                        notes.push(ScoreNote { division: part.division, key, start: t, beats });
                        notes.len() - 1
                    }
                };
                if tie {
                    next_tied.insert(key, idx);
                }
            }
            tied = next_tied;
        } else {
            tied.clear();
        }
        t += beats;
    }
    Ok((notes, t))
}

impl Score {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// All notes of one verse, and its length in crotchets.
    pub fn notes(&self) -> Result<(Vec<ScoreNote>, f64), String> {
        let (beats, unit) = parse_time(&self.time)?;
        let bar_beats = beats as f64 * 4.0 / unit as f64;
        let mut all = Vec::new();
        let mut length: Option<f64> = None;
        for part in &self.parts {
            let (notes, len) = parse_part(part, bar_beats, self.pickup)?;
            if let Some(l) = length {
                if (l - len).abs() > 1e-6 {
                    return Err(format!("{:?} part lasts {len} beats, others {l}", part.division));
                }
            }
            length = Some(len);
            all.extend(notes);
        }
        Ok((all, length.ok_or("a score needs at least one part")?))
    }

    pub fn compile(&self) -> Result<Track, String> {
        if self.bpm <= 0.0 || self.verses == 0 || self.verses > 12 {
            return Err("bpm must be positive and verses 1-12".into());
        }
        let (beats, unit) = parse_time(&self.time)?;
        let (fifths, mode) = parse_key(&self.key)?;
        let (notes, verse_beats) = self.notes()?;
        let ms_per_beat = 60_000.0 / self.bpm;

        let stops = |list: &[String]| -> Result<BTreeSet<StopRef>, String> {
            list.iter().map(|s| parse_stop(s)).collect()
        };
        let initial = Ceciliation {
            stops: stops(&self.ceciliation)?,
            expression: self.expression.clone(),
            master_volume: None,
        };

        let mut events = Vec::new();
        let mut verses = Vec::new();
        let ms = |beat: f64| (START_MARGIN_MS + beat * ms_per_beat).round() as u32;
        for v in 0..self.verses {
            let verse_start = v as f64 * (verse_beats + self.gap);
            if v > 0 {
                // Marker in the middle of the silence between verses.
                verses.push(ms(verse_start - self.gap / 2.0));
            }
            for change in self.verse_ceciliation.get(&(v + 1)).into_iter().flatten() {
                let (on, name) = match change.split_at(1) {
                    ("+", rest) => (true, rest),
                    ("-", rest) => (false, rest),
                    _ => return Err(format!("verse change {change:?} must start with + or -")),
                };
                // Change stops just before the verse, in the gap.
                let t = ms(verse_start - (self.gap / 4.0).min(0.5)).max(1);
                events.push(TimedEvent { t, event: TrackEvent::Stop { stop: parse_stop(name.trim())?, on } });
            }
            for n in &notes {
                let start = verse_start + n.start;
                let on = ms(start);
                let off = (START_MARGIN_MS + (start + n.beats) * ms_per_beat - RELEASE_MS).round().max(on as f64 + 1.0) as u32;
                events.push(TimedEvent {
                    t: on,
                    event: TrackEvent::Note { division: n.division, key: n.key, velocity: VELOCITY },
                });
                events.push(TimedEvent { t: off, event: TrackEvent::Note { division: n.division, key: n.key, velocity: 0 } });
            }
        }
        let end = ms(self.verses as f64 * verse_beats + (self.verses - 1) as f64 * self.gap);
        let mut track = Track {
            schema: TRACK_SCHEMA,
            id: format!("sample-{}", self.id),
            info: TrackInfo {
                title: self.title.clone(),
                composer: self.composer.clone(),
                performer: String::new(),
                publication: self.publication.clone(),
                copyright: self.copyright.clone(),
                melody: self.melody.clone(),
                volume: 100,
                tempo: 100,
            },
            source: TrackSource::Sample,
            organ_id: String::new(),
            organ_name: "Cecilia samples".into(),
            created: 0,
            modified: 0,
            initial,
            duration_ms: end + END_MARGIN_MS,
            verses,
            meter: Some(Meter {
                bpm: self.bpm as f32,
                beats,
                beat_unit: unit,
                fifths,
                mode,
                offset_ms: ms(self.pickup),
            }),
            events,
        };
        track.sort();
        track.validate()?;
        Ok(track)
    }
}

/// The sample scores shipped inside the binary, as (file name, JSON).
pub const SAMPLES: &[(&str, &str)] = &[
    ("old-hundredth", include_str!("../samples/old-hundredth.score.json")),
    ("eventide", include_str!("../samples/eventide.score.json")),
    ("st-anne", include_str!("../samples/st-anne.score.json")),
    ("tallis-canon", include_str!("../samples/tallis-canon.score.json")),
    ("duke-street", include_str!("../samples/duke-street.score.json")),
    ("lobe-den-herren", include_str!("../samples/lobe-den-herren.score.json")),
    ("nicaea", include_str!("../samples/nicaea.score.json")),
    ("crimond", include_str!("../samples/crimond.score.json")),
    ("st-clement", include_str!("../samples/st-clement.score.json")),
    ("winchester-new", include_str!("../samples/winchester-new.score.json")),
    ("prelude-on-a-ground", include_str!("../samples/prelude-on-a-ground.score.json")),
];

/// Compile every bundled sample.
pub fn sample_tracks() -> Vec<Track> {
    SAMPLES
        .iter()
        .map(|(name, text)| {
            Score::from_json(text)
                .and_then(|s| s.compile())
                .unwrap_or_else(|e| panic!("sample {name} is invalid: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::builtin_organs;
    use crate::organist::Organist;

    #[test]
    fn pitches_parse() {
        assert_eq!(parse_pitch("C4"), Ok(60));
        assert_eq!(parse_pitch("A4"), Ok(69));
        assert_eq!(parse_pitch("F#3"), Ok(54));
        assert_eq!(parse_pitch("Bb2"), Ok(46));
        assert_eq!(parse_pitch("C-1"), Ok(0));
        assert!(parse_pitch("H2").is_err());
    }

    #[test]
    fn keys_and_stops_parse() {
        assert_eq!(parse_key("G"), Ok((1, Mode::Major)));
        assert_eq!(parse_key("Bb"), Ok((-2, Mode::Major)));
        assert_eq!(parse_key("Dm"), Ok((-1, Mode::Minor)));
        let s = parse_stop("great mutation 2 2/3 Twelfth").unwrap();
        assert!(matches!(&s.equivalent, Equivalent::Voice { footage, tag, .. } if footage == "2 2/3" && tag == "Twelfth"));
        let c = parse_stop("swell to great").unwrap();
        assert_eq!(c.division, DivisionRole::Great);
        assert!(parse_stop("great kazoo 8").is_err());
    }

    #[test]
    fn bars_ties_and_chords() {
        let part = Part { division: DivisionRole::Great, notes: "C4:1 | D4:2~ D4:1 [E4 G4]:1 | r:4".into() };
        let (notes, len) = parse_part(&part, 4.0, 1.0).unwrap();
        assert_eq!(len, 9.0);
        assert_eq!(notes.len(), 4);
        assert_eq!(notes[1], ScoreNote { division: DivisionRole::Great, key: 62, start: 1.0, beats: 3.0 });
        let bad = Part { division: DivisionRole::Great, notes: "C4:1 | D4:3 |".into() };
        assert!(parse_part(&bad, 4.0, 1.0).unwrap_err().contains("bar line"));
    }

    #[test]
    fn every_sample_compiles_fits_the_keyboards_and_plays_everywhere() {
        let tracks = sample_tracks();
        assert_eq!(tracks.len(), SAMPLES.len());
        let organs = builtin_organs();
        for t in &tracks {
            assert!(t.note_count() > 20, "{} is too short", t.id);
            for o in &organs {
                let vo = Organist::new(o.clone());
                for e in &t.events {
                    if let TrackEvent::Note { division, key, .. } = e.event {
                        let d = o.division(vo.division(division).unwrap()).unwrap();
                        assert!(
                            (d.low_key..=d.high_key).contains(&key),
                            "{}: {key} is off the {:?} keyboard of {}",
                            t.id,
                            division,
                            o.id
                        );
                    }
                }
                assert!(!vo.stops_for(&t.initial).is_empty(), "{} has no sound on {}", t.id, o.id);
            }
        }
    }
}
