//! The state of an organ console: which stops are drawn, which keys are
//! held, where the expression pedals are.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use utoipa::ToSchema;

use crate::driver::OrganEvent;
use crate::organ::{DivisionRole, Equivalent, OrganDefinition};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct ConsoleState {
    /// Ids of stops that are on.
    pub stops: BTreeSet<String>,
    /// Held keys per division.
    pub notes: BTreeMap<DivisionRole, BTreeSet<u8>>,
    /// Expression per division, 0-127 (only divisions that have a pedal).
    pub expression: BTreeMap<DivisionRole, u8>,
    pub master_volume: u8,
}

impl ConsoleState {
    /// Fresh console for `organ`: all stops in, keys up, pedals fully open.
    pub fn new(organ: &OrganDefinition) -> Self {
        Self {
            stops: BTreeSet::new(),
            notes: BTreeMap::new(),
            expression: organ
                .divisions
                .iter()
                .filter(|d| d.expression.is_some())
                .map(|d| (d.role, 127))
                .collect(),
            master_volume: 127,
        }
    }

    pub fn is_on(&self, stop: &str) -> bool {
        self.stops.contains(stop)
    }

    /// Apply an event; returns true if anything changed.
    pub fn apply(&mut self, event: &OrganEvent) -> bool {
        match event {
            OrganEvent::Note { division, key, velocity } => {
                let held = self.notes.entry(*division).or_default();
                let changed = if *velocity > 0 { held.insert(*key) } else { held.remove(key) };
                if held.is_empty() {
                    self.notes.remove(division);
                }
                changed
            }
            OrganEvent::Stop { stop, on } => {
                if *on {
                    self.stops.insert(stop.clone())
                } else {
                    self.stops.remove(stop)
                }
            }
            OrganEvent::StopToggle { stop } => {
                if !self.stops.remove(stop) {
                    self.stops.insert(stop.clone());
                }
                true
            }
            OrganEvent::Expression { division, value } => {
                self.expression.insert(*division, *value) != Some(*value)
            }
            OrganEvent::MasterVolume { value } => {
                std::mem::replace(&mut self.master_volume, *value) != *value
            }
            OrganEvent::AllStopsOff => {
                let changed = !self.stops.is_empty();
                self.stops.clear();
                changed
            }
        }
    }

    /// Release every key (stops untouched).
    pub fn release_all(&mut self) -> bool {
        let changed = !self.notes.is_empty();
        self.notes.clear();
        changed
    }

    /// Every (sounding-stop id, MIDI note) pair: held keys through drawn
    /// stops, plus one level of couplers.
    pub fn sounding(&self, organ: &OrganDefinition) -> Vec<(String, u8)> {
        let mut out = BTreeSet::new();
        let unison_off: BTreeSet<DivisionRole> = organ
            .stops
            .iter()
            .filter(|s| self.is_on(&s.id) && s.equivalent == Equivalent::UnisonOff)
            .map(|s| s.division)
            .collect();

        for (keyboard, keys) in &self.notes {
            // (division whose pipes speak, semitone offset)
            let mut sources = Vec::new();
            if !unison_off.contains(keyboard) {
                sources.push((*keyboard, 0i16));
            }
            for s in &organ.stops {
                if s.division == *keyboard && self.is_on(&s.id) {
                    if let Equivalent::Coupler { source, pitch } = &s.equivalent {
                        sources.push((*source, pitch.semitones() as i16));
                    }
                }
            }
            for (division, offset) in sources {
                for s in &organ.stops {
                    if s.division != division
                        || !self.is_on(&s.id)
                        || !matches!(s.equivalent, Equivalent::Voice { .. })
                    {
                        continue;
                    }
                    for &key in keys {
                        let note = key as i16 + offset;
                        if (0..=127).contains(&note) {
                            out.insert((s.id.clone(), note as u8));
                        }
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::builtin_organs;

    #[test]
    fn couplers_add_source_division_pipes() {
        let organ = builtin_organs().remove(0);
        let mut c = ConsoleState::new(&organ);
        for id in ["gt-opendiap", "sw-oboe", "gt-sw"] {
            c.apply(&OrganEvent::Stop { stop: id.into(), on: true });
        }
        c.apply(&OrganEvent::Note { division: DivisionRole::Great, key: 60, velocity: 90 });
        let sounding = c.sounding(&organ);
        assert!(sounding.contains(&("gt-opendiap".into(), 60)));
        assert!(sounding.contains(&("sw-oboe".into(), 60)), "Swell to Great");
        assert_eq!(sounding.len(), 2);

        c.apply(&OrganEvent::Note { division: DivisionRole::Great, key: 60, velocity: 0 });
        assert!(c.sounding(&organ).is_empty());
        assert!(c.notes.is_empty());
    }

    #[test]
    fn toggle_flips_and_expression_defaults_open() {
        let organ = builtin_organs().remove(0);
        let mut c = ConsoleState::new(&organ);
        assert_eq!(c.expression.get(&DivisionRole::Swell), Some(&127));
        assert!(!c.expression.contains_key(&DivisionRole::Great));
        c.apply(&OrganEvent::StopToggle { stop: "sw-oboe".into() });
        assert!(c.is_on("sw-oboe"));
        c.apply(&OrganEvent::StopToggle { stop: "sw-oboe".into() });
        assert!(!c.is_on("sw-oboe"));
    }
}
