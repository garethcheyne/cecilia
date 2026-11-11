//! The virtual organist: plays a recording on whatever organ is connected.
//!
//! Tracks name stops by what they are ([`StopRef`]), so on the organ they
//! were recorded on every stop maps exactly. On another organ each stop is
//! matched to the closest thing it has, by division, pitch and tonal family,
//! with one firm rule: **never substitute something louder** (no Tuba for a
//! missing Salicional). Missing keyboards are redirected, ultimately to the
//! Great. And it works on whole ceciliations: if a division would fall
//! silent because none of its stops exist here, its quietest 8′ flute or
//! principal is drawn instead.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use utoipa::ToSchema;

use crate::organ::{footage_feet, DivisionRole, Equivalent, Family, OrganDefinition};
use crate::track::{Ceciliation, StopRef};

/// Where each recorded division's notes, stops and pedal go on this organ.
pub fn division_map(organ: &OrganDefinition) -> BTreeMap<DivisionRole, DivisionRole> {
    use DivisionRole::*;
    let has = |d: DivisionRole| organ.division(d).is_some();
    let fallbacks: [(DivisionRole, &[DivisionRole]); 6] = [
        (Pedal, &[Pedal, Great]),
        (Great, &[Great, Swell, Choir]),
        (Swell, &[Swell, Choir, Great]),
        (Choir, &[Choir, Swell, Great]),
        (Solo, &[Solo, Swell, Choir, Great]),
        (Echo, &[Echo, Solo, Swell, Choir, Great]),
    ];
    let first_manual = organ.divisions.iter().map(|d| d.role).find(|r| *r != Pedal);
    fallbacks
        .into_iter()
        .filter_map(|(from, order)| {
            order
                .iter()
                .copied()
                .find(|d| has(*d))
                .or(first_manual)
                .map(|to| (from, to))
        })
        .collect()
}

/// Rough loudness of a family, for the never-louder rule.
fn loudness(f: Family) -> u8 {
    match f {
        Family::String | Family::Celeste => 1,
        Family::Flute | Family::Mutation | Family::Percussion | Family::Other => 2,
        Family::Principal => 3,
        Family::Mixture => 4,
        Family::Reed => 5,
    }
}

/// How well `candidate` stands in for `wanted` (higher is better), or None
/// if it may not.
fn voice_score(wanted: (Family, f32), candidate: (Family, f32)) -> Option<u32> {
    let (wf, wfeet) = wanted;
    let (cf, cfeet) = candidate;
    // Mixtures are compound stops: any mixture stands in for any other.
    if wf == Family::Mixture && cf == Family::Mixture {
        return Some(100);
    }
    if (wfeet - cfeet).abs() > 0.01 || loudness(cf) > loudness(wf) {
        return None;
    }
    Some(if cf == wf {
        100
    } else {
        // Same pitch, quieter family: prefer the nearest in character.
        50 - 10 * (loudness(wf) - loudness(cf)) as u32
    })
}

pub struct Organist {
    organ: OrganDefinition,
    divisions: BTreeMap<DivisionRole, DivisionRole>,
    cache: std::cell::RefCell<BTreeMap<StopRef, Option<String>>>,
}

/// One line of "how will this track sound here".
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StopMapping {
    pub recorded: StopRef,
    /// Stop id on this organ, or None if it is left out.
    pub stop: Option<String>,
    pub exact: bool,
}

impl Organist {
    pub fn new(organ: OrganDefinition) -> Self {
        Self { divisions: division_map(&organ), organ, cache: Default::default() }
    }

    pub fn organ(&self) -> &OrganDefinition {
        &self.organ
    }

    /// The division on this organ that plays what was recorded on `d`.
    pub fn division(&self, d: DivisionRole) -> Option<DivisionRole> {
        self.divisions.get(&d).copied()
    }

    /// This organ's stand-in for a recorded stop.
    pub fn stop(&self, r: &StopRef) -> Option<String> {
        if let Some(hit) = self.cache.borrow().get(r) {
            return hit.clone();
        }
        let found = self.find(r);
        self.cache.borrow_mut().insert(r.clone(), found.clone());
        found
    }

    fn find(&self, r: &StopRef) -> Option<String> {
        if let Some(id) = r.find_in(&self.organ) {
            return Some(id.to_string());
        }
        let division = self.division(r.division)?;
        let mut here = self.organ.stops.iter().filter(|s| s.division == division);
        match &r.equivalent {
            Equivalent::Voice { family, footage, .. } => {
                let wanted = (*family, footage_feet(footage)?);
                here.filter_map(|s| match &s.equivalent {
                    Equivalent::Voice { family, footage, .. } => {
                        let score = voice_score(wanted, (*family, footage_feet(footage)?))?;
                        Some((score, &s.id))
                    }
                    _ => None,
                })
                // Highest score; ties go to the first in console order.
                .fold(None::<(u32, &String)>, |best, c| match best {
                    Some(b) if b.0 >= c.0 => Some(b),
                    _ => Some(c),
                })
                .map(|(_, id)| id.clone())
            }
            Equivalent::Coupler { source, pitch } => {
                let source = self.division(*source)?;
                if source == division {
                    return None; // a keyboard coupled to itself is just itself
                }
                here.find(|s| matches!(&s.equivalent, Equivalent::Coupler { source: s2, pitch: p2 } if *s2 == source && p2 == pitch))
                    .map(|s| s.id.clone())
            }
            Equivalent::Tremulant | Equivalent::UnisonOff => {
                here.find(|s| s.equivalent == r.equivalent).map(|s| s.id.clone())
            }
        }
    }

    /// The stops to draw on this organ for a whole recorded ceciliation.
    pub fn stops_for(&self, reg: &Ceciliation) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = reg.stops.iter().filter_map(|r| self.stop(r)).collect();
        // Any division that had speaking stops but would now be silent gets
        // its quietest 8' flute or principal.
        let wanted_voices: BTreeSet<DivisionRole> = reg
            .stops
            .iter()
            .filter(|r| matches!(r.equivalent, Equivalent::Voice { .. }))
            .filter_map(|r| self.division(r.division))
            .collect();
        for d in wanted_voices {
            let speaks = out.iter().any(|id| {
                self.organ
                    .stop(id)
                    .is_some_and(|s| s.division == d && matches!(s.equivalent, Equivalent::Voice { .. }))
            });
            if !speaks {
                if let Some(s) = self.quiet_foundation(d) {
                    out.insert(s);
                }
            }
        }
        out
    }

    fn quiet_foundation(&self, d: DivisionRole) -> Option<String> {
        let unison = if d == DivisionRole::Pedal { 16.0 } else { 8.0 };
        self.organ
            .stops
            .iter()
            .filter(|s| s.division == d)
            .filter_map(|s| match &s.equivalent {
                Equivalent::Voice { family, footage, .. }
                    if matches!(family, Family::Flute | Family::Principal | Family::String)
                        && footage_feet(footage).is_some_and(|f| (f - unison).abs() < 0.01) =>
                {
                    Some((loudness(*family), &s.id))
                }
                _ => None,
            })
            .min_by_key(|(l, _)| *l)
            .map(|(_, id)| id.clone())
    }

    /// Expression for a recorded division, if this organ can show it.
    pub fn expression_division(&self, d: DivisionRole) -> Option<DivisionRole> {
        let to = self.division(d)?;
        self.organ.division(to)?.expression.as_ref().map(|_| to)
    }

    /// How every stop used in a set of ceciliations maps onto this organ.
    pub fn report<'a>(&self, stops: impl IntoIterator<Item = &'a StopRef>) -> Vec<StopMapping> {
        let unique: BTreeSet<&StopRef> = stops.into_iter().collect();
        unique
            .into_iter()
            .map(|r| {
                let stop = self.stop(r);
                let exact = stop.as_deref().is_some_and(|id| r.find_in(&self.organ) == Some(id));
                StopMapping { recorded: r.clone(), stop, exact }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::{builtin_organs, CouplerPitch};

    fn organ(id: &str) -> OrganDefinition {
        builtin_organs().into_iter().find(|o| o.id == id).unwrap()
    }

    fn voice(division: DivisionRole, family: Family, footage: &str, tag: &str) -> StopRef {
        StopRef {
            division,
            equivalent: Equivalent::Voice { family, footage: footage.into(), ranks: None, tag: tag.into() },
        }
    }

    #[test]
    fn same_organ_maps_every_stop_exactly() {
        let o = organ("demo");
        let vo = Organist::new(o.clone());
        for s in &o.stops {
            let r = StopRef::of(&o, &s.id).unwrap();
            assert_eq!(vo.stop(&r).as_deref(), Some(s.id.as_str()), "{}", s.id);
        }
    }

    #[test]
    fn similar_stops_substitute_but_never_louder() {
        let vo = Organist::new(organ("demo"));
        // A Great 8' principal with a tag the demo doesn't have: still the Open Diapason.
        let r = voice(DivisionRole::Great, Family::Principal, "8", "Open Diapason II");
        assert_eq!(vo.stop(&r).as_deref(), Some("gt-opendiap"));
        // A Swell 8' string the demo lacks? It has a Salicional (string): exact family.
        let r = voice(DivisionRole::Swell, Family::String, "8", "Viola");
        assert_eq!(vo.stop(&r).as_deref(), Some("sw-salicional"));
        // A Swell 8' principal is missing: a quieter 8' (Gedackt/Salicional) stands in, never the Oboe.
        let r = voice(DivisionRole::Swell, Family::Principal, "8", "");
        let got = vo.stop(&r).unwrap();
        assert!(got == "sw-gedackt" || got == "sw-salicional", "{got}");
        // A Salicional where only reeds exist at that pitch would get nothing.
        let r = voice(DivisionRole::Great, Family::String, "8", "");
        assert_ne!(vo.stop(&r).as_deref(), Some("gt-trumpet"));
    }

    #[test]
    fn missing_manuals_go_to_the_great() {
        let vo = Organist::new(organ("demo"));
        assert_eq!(vo.division(DivisionRole::Choir), Some(DivisionRole::Swell));
        assert_eq!(vo.division(DivisionRole::Solo), Some(DivisionRole::Swell));
        let coupler = StopRef {
            division: DivisionRole::Great,
            equivalent: Equivalent::Coupler { source: DivisionRole::Choir, pitch: CouplerPitch::Unison },
        };
        // Choir to Great becomes Swell to Great, as Choir plays on the Swell here.
        assert_eq!(vo.stop(&coupler).as_deref(), Some("gt-sw"));
    }

    #[test]
    fn a_division_never_falls_silent() {
        let vo = Organist::new(organ("demo"));
        let mut reg = Ceciliation::default();
        // A Great 32' reed nobody has.
        reg.stops.insert(voice(DivisionRole::Great, Family::Reed, "32", ""));
        let stops = vo.stops_for(&reg);
        assert_eq!(stops.len(), 1);
        let s = organ("demo").stop(stops.iter().next().unwrap()).unwrap().clone();
        assert_eq!(s.division, DivisionRole::Great);
    }

    #[test]
    fn demo_tracks_play_on_the_makin() {
        let demo = organ("demo");
        let vo = Organist::new(organ("makin-2m"));
        let report = vo.report(
            demo.stops.iter().map(|s| StopRef::of(&demo, &s.id).unwrap()).collect::<Vec<_>>().iter(),
        );
        let mapped = report.iter().filter(|m| m.stop.is_some()).count();
        assert!(mapped * 10 >= report.len() * 8, "most stops have a stand-in: {mapped}/{}", report.len());
    }
}
