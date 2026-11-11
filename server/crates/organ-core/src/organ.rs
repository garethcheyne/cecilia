//! Organ definitions: divisions, stops and their MIDI bindings.
//!
//! An organ is stored as JSON (`<id>.organ.json`). Tracks never refer to a
//! stop by its id, only by `(division, equivalent)`, which is what makes a
//! recording portable between instruments.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};
use utoipa::openapi::RefOr;
use utoipa::{PartialSchema, ToSchema};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BINDINGS_PER_STOP: usize = 5;

// ---------------------------------------------------------------------------
// MIDI patterns
// ---------------------------------------------------------------------------

/// A MIDI message template such as `"B1 0B vv"`: hex bytes, with at most one
/// `vv` slot that carries a variable value (expression level, on/off value).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pattern(Vec<Option<u8>>);

impl Pattern {
    pub fn fixed(bytes: &[u8]) -> Self {
        Self(bytes.iter().copied().map(Some).collect())
    }

    pub fn has_slot(&self) -> bool {
        self.0.iter().any(Option::is_none)
    }

    /// `Some(None)` = matched a fixed pattern; `Some(Some(v))` = matched and
    /// captured `v` from the slot; `None` = no match.
    pub fn capture(&self, msg: &[u8]) -> Option<Option<u8>> {
        if msg.len() != self.0.len() {
            return None;
        }
        let mut value = None;
        for (p, b) in self.0.iter().zip(msg) {
            match p {
                Some(fixed) if fixed != b => return None,
                Some(_) => {}
                None => value = Some(*b),
            }
        }
        Some(value)
    }

    /// Could some message match both patterns?
    pub fn overlaps(&self, other: &Pattern) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|(a, b)| a.is_none() || b.is_none() || a == b)
    }

    /// Build the message, filling the slot (if any) with `value`.
    pub fn render(&self, value: u8) -> Vec<u8> {
        self.0.iter().map(|p| p.unwrap_or(value)).collect()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid MIDI pattern {0:?}: use hex bytes like \"B0 07 vv\"")]
pub struct PatternError(String);

impl FromStr for Pattern {
    type Err = PatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || PatternError(s.to_string());
        let parts: Vec<Option<u8>> = s
            .split_whitespace()
            .map(|tok| {
                if tok.eq_ignore_ascii_case("vv") {
                    Ok(None)
                } else if tok.len() == 2 {
                    u8::from_str_radix(tok, 16).map(Some).map_err(|_| err())
                } else {
                    Err(err())
                }
            })
            .collect::<Result<_, _>>()?;
        let slots = parts.iter().filter(|p| p.is_none()).count();
        if parts.is_empty() || slots > 1 || parts[0].is_none() {
            return Err(err());
        }
        Ok(Self(parts))
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for p in &self.0 {
            if !first {
                f.write_str(" ")?;
            }
            first = false;
            match p {
                Some(b) => write!(f, "{b:02X}")?,
                None => f.write_str("vv")?,
            }
        }
        Ok(())
    }
}

impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

impl PartialSchema for Pattern {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .description(Some(
                "MIDI message as hex bytes, optionally with one `vv` value slot, e.g. \"B1 0B vv\"",
            ))
            .build()
            .into()
    }
}

impl ToSchema for Pattern {}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The role of a keyboard, independent of its local name (Great, Hauptwerk,
/// Grand Orgue...). Order follows the English convention.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DivisionRole {
    Pedal,
    Great,
    Swell,
    Choir,
    Solo,
    Echo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Manufacturer {
    Generic,
    Hauptwerk,
    Allen,
    ClassicOrganWorks,
    Content,
    Johannus,
    JohannusProgrammable,
    Makin,
    Peterson,
    Rodgers,
    RodgersPhase2,
    SolidState1,
    SolidState2,
    Viscount,
    Walker,
    Wyvern,
}

/// A value-carrying control such as an expression pedal or master volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ValueBinding {
    /// Must contain a `vv` slot.
    pub pattern: Pattern,
    #[serde(default)]
    pub min: u8,
    #[serde(default = "default_max")]
    pub max: u8,
}

fn default_max() -> u8 {
    127
}

impl ValueBinding {
    /// Raw controller value -> normalised 0..=127.
    pub fn normalise(&self, raw: u8) -> u8 {
        let (lo, hi) = (self.min.min(self.max), self.min.max(self.max));
        if hi == lo {
            return 0;
        }
        let v = raw.clamp(lo, hi) - lo;
        ((v as u32 * 127 + (hi - lo) as u32 / 2) / (hi - lo) as u32) as u8
    }

    /// Normalised 0..=127 -> raw controller value.
    pub fn denormalise(&self, value: u8) -> u8 {
        let (lo, hi) = (self.min.min(self.max), self.min.max(self.max));
        lo + ((value.min(127) as u32 * (hi - lo) as u32 + 63) / 127) as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Division {
    pub role: DivisionRole,
    /// Display name, e.g. "Great", "Hauptwerk", "Grand Orgue".
    pub name: String,
    /// MIDI channel 1-16 that carries this keyboard's notes.
    pub channel: u8,
    /// Lowest and highest MIDI note on the keyboard.
    pub low_key: u8,
    pub high_key: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<ValueBinding>,
}

/// Tonal family, used to find a similar stop on another organ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Principal,
    Flute,
    String,
    Celeste,
    Reed,
    Mutation,
    Mixture,
    Percussion,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CouplerPitch {
    /// 16' (an octave down)
    Sub,
    /// 8'
    Unison,
    /// 4' (an octave up)
    Super,
}

impl CouplerPitch {
    pub fn semitones(self) -> i8 {
        match self {
            Self::Sub => -12,
            Self::Unison => 0,
            Self::Super => 12,
        }
    }
}

/// Parse a footage like "8", "2 2/3" or "1 3/5" into feet.
pub fn footage_feet(footage: &str) -> Option<f32> {
    let mut total = 0.0;
    for part in footage.split_whitespace() {
        total += match part.split_once('/') {
            Some((n, d)) => n.parse::<f32>().ok()? / d.parse::<f32>().ok().filter(|d| *d != 0.0)?,
            None => part.parse::<f32>().ok()?,
        };
    }
    (total > 0.0).then_some(total)
}

/// What a stop *is*, independent of what it's called on this organ.
/// Unique per `(division, equivalent)` within an organ.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Equivalent {
    /// A speaking stop.
    Voice {
        family: Family,
        /// Footage such as "16", "8", "4", "2 2/3", "1 3/5".
        footage: String,
        /// Number of ranks (mixtures, celestes).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ranks: Option<u8>,
        /// Distinguishes similar stops, e.g. "Open Diapason I".
        #[serde(default, skip_serializing_if = "String::is_empty")]
        tag: String,
    },
    /// Plays `source` division's stops from this stop's keyboard.
    Coupler { source: DivisionRole, pitch: CouplerPitch },
    Tremulant,
    /// Unison off for this division.
    UnisonOff,
}

/// How a stop is switched over MIDI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StopBinding {
    /// Distinct messages for on and off.
    OnOff { on: Pattern, off: Pattern },
    /// The same message flips the stop.
    Toggle { msg: Pattern },
    /// One message with a value slot carrying `on` or `off`.
    Switch { pattern: Pattern, on: u8, off: u8 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopShape {
    #[default]
    Round,
    Rectangle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StopStyle {
    #[serde(default)]
    pub shape: StopShape,
    /// CSS colour of the label, e.g. "#1d4ed8".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Swap text and background colours when the stop is on.
    #[serde(default)]
    pub invert: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Stop {
    /// Unique within the organ; stable across edits.
    pub id: String,
    pub division: DivisionRole,
    pub equivalent: Equivalent,
    /// Up to three label lines shown on the stop, e.g. ["Open", "Diapason", "8"].
    pub names: Vec<String>,
    /// Tooltip; defaults to the label lines joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    /// Up to five ways the organ can switch this stop (jamb, crescendo, pistons...).
    #[serde(default)]
    pub midi: Vec<StopBinding>,
    /// Minimum gap between conflicting on/off messages (moving drawstops).
    #[serde(default)]
    pub repeat_ms: u32,
    /// Pause after changing this stop before continuing playback.
    #[serde(default)]
    pub delay_ms: u32,
    #[serde(default)]
    pub style: StopStyle,
}

impl Stop {
    pub fn display_name(&self) -> String {
        self.full_name
            .clone()
            .unwrap_or_else(|| self.names.join(" "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OrganDefaults {
    /// Silence before the first note of a new recording.
    #[serde(default = "default_margin")]
    pub margin_start_ms: u32,
    /// Silence after the last note.
    #[serde(default = "default_margin")]
    pub margin_end_ms: u32,
}

fn default_margin() -> u32 {
    2000
}

impl Default for OrganDefaults {
    fn default() -> Self {
        Self {
            margin_start_ms: default_margin(),
            margin_end_ms: default_margin(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OrganDefinition {
    pub schema: u32,
    /// File-safe identifier, e.g. "demo".
    pub id: String,
    pub name: String,
    pub manufacturer: Manufacturer,
    /// Keyboards, in the order they appear (bottom to top is up to the UI).
    pub divisions: Vec<Division>,
    pub stops: Vec<Stop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_volume: Option<ValueBinding>,
    /// A single message that pushes in every stop (the organ's general
    /// cancel), e.g. Johannus/Makin `F0 00 4A 4F 48 41 53 7F F7`. Organs whose
    /// stops only *toggle* need it so playback can start from a known state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_stops_off: Option<Pattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<Connection>,
    #[serde(default)]
    pub defaults: OrganDefaults,
}

/// How to cable the organ, in the words printed beside its sockets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Connection {
    /// The organ's socket to record from, e.g. "MIDI SEQ".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_from: Option<String>,
    /// The organ's socket to play back into, e.g. "MIDI IN".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_to: Option<String>,
    /// Anything else the organist should know before relying on this
    /// definition (unverified numbers, settings to change on the organ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrganSummary {
    pub id: String,
    pub name: String,
    pub manufacturer: Manufacturer,
    pub divisions: usize,
    pub stops: usize,
    /// Shipped with Registra (read-only) rather than user-created.
    pub builtin: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum OrganError {
    #[error("invalid organ definition: {0}")]
    Invalid(String),
    #[error("could not parse organ definition: {0}")]
    Parse(#[from] serde_json::Error),
}

impl OrganDefinition {
    pub fn from_json(text: &str) -> Result<Self, OrganError> {
        let organ: Self = serde_json::from_str(text)?;
        organ.validate()?;
        Ok(organ)
    }

    pub fn division(&self, role: DivisionRole) -> Option<&Division> {
        self.divisions.iter().find(|d| d.role == role)
    }

    pub fn stop(&self, id: &str) -> Option<&Stop> {
        self.stops.iter().find(|s| s.id == id)
    }

    pub fn summary(&self, builtin: bool) -> OrganSummary {
        OrganSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            manufacturer: self.manufacturer,
            divisions: self.divisions.len(),
            stops: self.stops.len(),
            builtin,
        }
    }

    pub fn validate(&self) -> Result<(), OrganError> {
        let bad = |m: String| Err(OrganError::Invalid(m));
        if self.schema != SCHEMA_VERSION {
            return bad(format!("unsupported schema version {}", self.schema));
        }
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return bad(format!("id {:?} must be letters, digits, '-' or '_'", self.id));
        }

        let mut roles = HashSet::new();
        for d in &self.divisions {
            if !roles.insert(d.role) {
                return bad(format!("division {:?} appears twice", d.role));
            }
            if !(1..=16).contains(&d.channel) {
                return bad(format!("{}: channel must be 1-16", d.name));
            }
            if d.low_key > d.high_key || d.high_key > 127 {
                return bad(format!("{}: invalid key range", d.name));
            }
            if let Some(e) = &d.expression {
                if !e.pattern.has_slot() {
                    return bad(format!("{}: expression pattern needs a vv slot", d.name));
                }
            }
        }

        let mut ids = HashSet::new();
        let mut equivalents = HashSet::new();
        for s in &self.stops {
            if !ids.insert(s.id.as_str()) {
                return bad(format!("stop id {:?} appears twice", s.id));
            }
            if !roles.contains(&s.division) {
                return bad(format!("stop {:?} is on a missing division", s.id));
            }
            if let Equivalent::Coupler { source, .. } = &s.equivalent {
                if !roles.contains(source) {
                    return bad(format!("coupler {:?} couples a missing division", s.id));
                }
            }
            if !equivalents.insert((s.division, &s.equivalent)) {
                return bad(format!(
                    "stop {:?} has the same division and equivalent as another stop",
                    s.id
                ));
            }
            if s.midi.len() > MAX_BINDINGS_PER_STOP {
                return bad(format!("stop {:?} has more than {MAX_BINDINGS_PER_STOP} bindings", s.id));
            }
            for b in &s.midi {
                if let StopBinding::Switch { pattern, .. } = b {
                    if !pattern.has_slot() {
                        return bad(format!("stop {:?}: switch pattern needs a vv slot", s.id));
                    }
                }
            }
        }
        if let Some(p) = &self.all_stops_off {
            if p.has_slot() {
                return bad("all_stops_off must be a fixed message (no vv slot)".into());
            }
        }
        self.check_binding_overlaps()
    }

    /// Two controls answering to the same MIDI message would make the organ
    /// state ambiguous (e.g. a stop that also moves the master volume).
    fn check_binding_overlaps(&self) -> Result<(), OrganError> {
        let mut patterns: Vec<(String, &Pattern)> = Vec::new();
        for d in &self.divisions {
            if let Some(e) = &d.expression {
                patterns.push((format!("{} expression", d.name), &e.pattern));
            }
        }
        if let Some(m) = &self.master_volume {
            patterns.push(("master volume".into(), &m.pattern));
        }
        if let Some(p) = &self.all_stops_off {
            patterns.push(("all stops off".into(), p));
        }
        for s in &self.stops {
            for b in &s.midi {
                let owner = format!("stop {:?}", s.id);
                match b {
                    StopBinding::OnOff { on, off } => {
                        patterns.push((owner.clone(), on));
                        patterns.push((owner, off));
                    }
                    StopBinding::Toggle { msg } => patterns.push((owner, msg)),
                    StopBinding::Switch { pattern, .. } => patterns.push((owner, pattern)),
                }
            }
        }
        for (i, (a_owner, a)) in patterns.iter().enumerate() {
            // Note on/off on a keyboard's channel always means a key.
            for d in &self.divisions {
                let ch = d.channel - 1;
                if a.0.len() == 3 && matches!(a.0[0], Some(s) if s == 0x80 | ch || s == 0x90 | ch) {
                    return Err(OrganError::Invalid(format!(
                        "{a_owner} uses {}'s note channel",
                        d.name
                    )));
                }
            }
            for (b_owner, b) in &patterns[i + 1..] {
                if a_owner != b_owner && a.overlaps(b) {
                    return Err(OrganError::Invalid(format!(
                        "{a_owner} ({a}) and {b_owner} ({b}) respond to the same MIDI"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Organs shipped inside the binary.
pub fn builtin_organs() -> Vec<OrganDefinition> {
    [
        include_str!("../organs/demo.organ.json"),
        include_str!("../organs/makin-2m.organ.json"),
    ]
        .into_iter()
        .map(|text| OrganDefinition::from_json(text).expect("built-in organ is valid"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_round_trip_and_capture() {
        let p: Pattern = "b1 0b VV".parse().unwrap();
        assert_eq!(p.to_string(), "B1 0B vv");
        assert_eq!(p.capture(&[0xB1, 0x0B, 0x40]), Some(Some(0x40)));
        assert_eq!(p.capture(&[0xB1, 0x0C, 0x40]), None);
        assert_eq!(p.capture(&[0xB1, 0x0B]), None);
        assert_eq!(p.render(0x7F), vec![0xB1, 0x0B, 0x7F]);
    }

    #[test]
    fn pattern_rejects_bad_input() {
        for bad in ["", "vv 01", "B0 vv vv", "B0 7", "ZZ"] {
            assert!(bad.parse::<Pattern>().is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn value_binding_scales_both_ways() {
        let v = ValueBinding { pattern: "B0 07 vv".parse().unwrap(), min: 0, max: 100 };
        assert_eq!(v.normalise(100), 127);
        assert_eq!(v.normalise(0), 0);
        assert_eq!(v.normalise(50), 64);
        assert_eq!(v.denormalise(127), 100);
        assert_eq!(v.denormalise(0), 0);
    }

    #[test]
    fn footage_parses_fractions() {
        assert_eq!(footage_feet("8"), Some(8.0));
        assert!((footage_feet("2 2/3").unwrap() - 2.6667).abs() < 1e-3);
        assert!((footage_feet("1 3/5").unwrap() - 1.6).abs() < 1e-6);
        assert_eq!(footage_feet(""), None);
        assert_eq!(footage_feet("1/0"), None);
    }

    #[test]
    fn builtin_organs_are_valid() {
        let organs = builtin_organs();
        assert!(!organs.is_empty());
        for o in organs {
            o.validate().unwrap();
        }
    }

    #[test]
    fn overlapping_bindings_are_rejected() {
        let mut organ = builtin_organs().remove(0);
        // Same as the stop with index 7's "on" message.
        organ.master_volume = Some(ValueBinding { pattern: "BF 07 vv".parse().unwrap(), min: 0, max: 127 });
        let err = organ.validate().unwrap_err().to_string();
        assert!(err.contains("master volume"), "{err}");

        let mut organ = builtin_organs().remove(0);
        organ.stops[0].midi = vec![StopBinding::Toggle { msg: Pattern::fixed(&[0x90, 0x24, 0x7F]) }];
        let err = organ.validate().unwrap_err().to_string();
        assert!(err.contains("note channel"), "{err}");
    }

    #[test]
    fn duplicate_equivalents_are_rejected() {
        let mut organ = builtin_organs().remove(0);
        let mut dup = organ.stops[0].clone();
        dup.id = "dup".into();
        organ.stops.push(dup);
        assert!(organ.validate().is_err());
    }
}
