//! Translates between raw MIDI and organ-level events for one organ
//! definition: the "printer driver" layer.
//!
//! This is the generic, table-driven driver. Manufacturer-specific drivers
//! (Rodgers' stateful SysEx, NRPN sequences, etc.) will plug in behind the
//! same interface.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::organ::{DivisionRole, OrganDefinition, StopBinding};

/// Something that happened on (or should happen to) an organ console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrganEvent {
    Note {
        division: DivisionRole,
        key: u8,
        /// 0 = note off.
        velocity: u8,
    },
    Stop {
        stop: String,
        on: bool,
    },
    /// A toggle message: the receiver flips the stop's current state.
    StopToggle {
        stop: String,
    },
    Expression {
        division: DivisionRole,
        /// Normalised 0-127.
        value: u8,
    },
    MasterVolume {
        value: u8,
    },
    /// General cancel: every stop pushed in.
    AllStopsOff,
}

pub struct Driver {
    organ: OrganDefinition,
}

impl Driver {
    pub fn new(organ: OrganDefinition) -> Self {
        Self { organ }
    }

    pub fn organ(&self) -> &OrganDefinition {
        &self.organ
    }

    /// Interpret one incoming MIDI message. Unrecognised messages yield
    /// nothing; one message can in principle affect several stops.
    pub fn decode(&self, msg: &[u8]) -> Vec<OrganEvent> {
        let mut out = Vec::new();
        let Some(&status) = msg.first() else {
            return out;
        };

        // Notes
        if matches!(status & 0xF0, 0x80 | 0x90) && msg.len() == 3 {
            let channel = (status & 0x0F) + 1;
            if let Some(div) = self.organ.divisions.iter().find(|d| d.channel == channel) {
                let velocity = if status & 0xF0 == 0x80 { 0 } else { msg[2] };
                out.push(OrganEvent::Note { division: div.role, key: msg[1], velocity });
                return out;
            }
        }

        if self.organ.all_stops_off.as_ref().is_some_and(|p| p.capture(msg).is_some()) {
            out.push(OrganEvent::AllStopsOff);
            return out;
        }

        // Expression
        for div in &self.organ.divisions {
            if let Some(binding) = &div.expression {
                if let Some(Some(raw)) = binding.pattern.capture(msg) {
                    out.push(OrganEvent::Expression {
                        division: div.role,
                        value: binding.normalise(raw),
                    });
                }
            }
        }
        if let Some(binding) = &self.organ.master_volume {
            if let Some(Some(raw)) = binding.pattern.capture(msg) {
                out.push(OrganEvent::MasterVolume { value: binding.normalise(raw) });
            }
        }

        // Stops
        for stop in &self.organ.stops {
            for binding in &stop.midi {
                let event = match binding {
                    StopBinding::OnOff { on, off } => {
                        if on.capture(msg).is_some() {
                            Some(OrganEvent::Stop { stop: stop.id.clone(), on: true })
                        } else if off.capture(msg).is_some() {
                            Some(OrganEvent::Stop { stop: stop.id.clone(), on: false })
                        } else {
                            None
                        }
                    }
                    StopBinding::Toggle { msg: m } => m
                        .capture(msg)
                        .map(|_| OrganEvent::StopToggle { stop: stop.id.clone() }),
                    StopBinding::Switch { pattern, on, off } => match pattern.capture(msg) {
                        Some(Some(v)) if v == *on => {
                            Some(OrganEvent::Stop { stop: stop.id.clone(), on: true })
                        }
                        Some(Some(v)) if v == *off => {
                            Some(OrganEvent::Stop { stop: stop.id.clone(), on: false })
                        }
                        _ => None,
                    },
                };
                if let Some(event) = event {
                    out.push(event);
                    break; // first matching binding wins for this stop
                }
            }
        }
        out
    }

    /// MIDI to send to make the organ do `event`. `currently_on` is needed for
    /// toggle-only stops, which must not be sent when already in the wanted
    /// state.
    pub fn encode(&self, event: &OrganEvent, currently_on: impl Fn(&str) -> bool) -> Vec<Vec<u8>> {
        self.encode_dyn(event, &currently_on)
    }

    fn encode_dyn(&self, event: &OrganEvent, currently_on: &dyn Fn(&str) -> bool) -> Vec<Vec<u8>> {
        match event {
            OrganEvent::Note { division, key, velocity } => {
                let Some(div) = self.organ.division(*division) else {
                    return vec![];
                };
                let ch = div.channel - 1;
                if *velocity == 0 {
                    vec![vec![0x80 | ch, *key, 0]]
                } else {
                    vec![vec![0x90 | ch, *key, (*velocity).min(127)]]
                }
            }
            OrganEvent::Stop { stop, on } => {
                let Some(def) = self.organ.stop(stop) else {
                    return vec![];
                };
                // Always use the primary binding for output.
                match def.midi.first() {
                    Some(StopBinding::OnOff { on: on_msg, off }) => {
                        vec![if *on { on_msg.render(0) } else { off.render(0) }]
                    }
                    Some(StopBinding::Toggle { msg }) => {
                        if currently_on(stop) == *on {
                            vec![]
                        } else {
                            vec![msg.render(0)]
                        }
                    }
                    Some(StopBinding::Switch { pattern, on: v_on, off: v_off }) => {
                        vec![pattern.render(if *on { *v_on } else { *v_off })]
                    }
                    None => vec![],
                }
            }
            OrganEvent::StopToggle { stop } => {
                let on = !currently_on(stop);
                self.encode_dyn(&OrganEvent::Stop { stop: stop.clone(), on }, currently_on)
            }
            OrganEvent::Expression { division, value } => self
                .organ
                .division(*division)
                .and_then(|d| d.expression.as_ref())
                .map(|b| vec![b.pattern.render(b.denormalise(*value))])
                .unwrap_or_default(),
            OrganEvent::MasterVolume { value } => self
                .organ
                .master_volume
                .as_ref()
                .map(|b| vec![b.pattern.render(b.denormalise(*value))])
                .unwrap_or_default(),
            OrganEvent::AllStopsOff => match &self.organ.all_stops_off {
                Some(p) => vec![p.render(0)],
                // No cancel message: push each drawn stop in individually.
                None => self
                    .organ
                    .stops
                    .iter()
                    .filter(|s| currently_on(&s.id))
                    .flat_map(|s| {
                        self.encode_dyn(&OrganEvent::Stop { stop: s.id.clone(), on: false }, currently_on)
                    })
                    .collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::builtin_organs;

    fn demo() -> Driver {
        Driver::new(builtin_organs().remove(0))
    }

    #[test]
    fn notes_route_by_channel() {
        let d = demo();
        assert_eq!(
            d.decode(&[0x91, 60, 100]),
            vec![OrganEvent::Note { division: DivisionRole::Swell, key: 60, velocity: 100 }]
        );
        // velocity-0 note-on and 0x8n are both note-off
        assert_eq!(
            d.decode(&[0x90, 60, 0]),
            vec![OrganEvent::Note { division: DivisionRole::Great, key: 60, velocity: 0 }]
        );
        assert_eq!(
            d.decode(&[0x82, 40, 64]),
            vec![OrganEvent::Note { division: DivisionRole::Pedal, key: 40, velocity: 0 }]
        );
        assert!(d.decode(&[0x9A, 60, 100]).is_empty(), "unmapped channel");
    }

    #[test]
    fn stops_round_trip() {
        let d = demo();
        let on = OrganEvent::Stop { stop: "gt-trumpet".into(), on: true };
        let msgs = d.encode(&on, |_| false);
        assert_eq!(msgs.len(), 1);
        assert_eq!(d.decode(&msgs[0]), vec![on]);

        let off = OrganEvent::Stop { stop: "gt-trumpet".into(), on: false };
        let msgs = d.encode(&off, |_| true);
        assert_eq!(d.decode(&msgs[0]), vec![off]);
    }

    fn makin() -> Driver {
        Driver::new(builtin_organs().into_iter().find(|o| o.id == "makin-2m").unwrap())
    }

    #[test]
    fn makin_stops_toggle_by_program_change() {
        let d = makin();
        // Program Change 0x12 on channel 12 = the 19th stop in console order.
        assert_eq!(
            d.decode(&[0xCB, 0x12]),
            vec![OrganEvent::StopToggle { stop: "gt-opendiap".into() }]
        );
        // Asking for a stop that's already on sends nothing (it would flip it off).
        let on = OrganEvent::Stop { stop: "gt-opendiap".into(), on: true };
        assert_eq!(d.encode(&on, |_| false), vec![vec![0xCB, 0x12]]);
        assert!(d.encode(&on, |_| true).is_empty());
    }

    #[test]
    fn makin_general_cancel_round_trips() {
        let d = makin();
        let cancel = [0xF0, 0x00, 0x4A, 0x4F, 0x48, 0x41, 0x53, 0x7F, 0xF7];
        assert_eq!(d.decode(&cancel), vec![OrganEvent::AllStopsOff]);
        assert_eq!(d.encode(&OrganEvent::AllStopsOff, |_| true), vec![cancel.to_vec()]);
        // Other Johannus SysEx (e.g. piston values) is not a cancel.
        assert!(d.decode(&[0xF0, 0x00, 0x4A, 0x4F, 0x48, 0x41, 0x53, 0x01, 0xF7]).is_empty());
        // The swell pedal never goes below 55 on a Makin: 55 reads as closed.
        assert_eq!(
            d.decode(&[0xB1, 0x0B, 55]),
            vec![OrganEvent::Expression { division: DivisionRole::Swell, value: 0 }]
        );
    }

    #[test]
    fn cancel_without_a_cancel_message_sends_each_drawn_stop_off() {
        let d = demo();
        let drawn = ["gt-trumpet", "sw-oboe"];
        let msgs = d.encode(&OrganEvent::AllStopsOff, |id| drawn.contains(&id));
        assert_eq!(msgs.len(), 2);
        for m in &msgs {
            assert!(matches!(d.decode(m)[..], [OrganEvent::Stop { on: false, .. }]));
        }
    }

    #[test]
    fn expression_round_trip() {
        let d = demo();
        let e = OrganEvent::Expression { division: DivisionRole::Swell, value: 90 };
        let msgs = d.encode(&e, |_| false);
        assert_eq!(msgs, vec![vec![0xB1, 0x0B, 90]]);
        assert_eq!(d.decode(&msgs[0]), vec![e]);
        // Great has no expression pedal
        assert!(d
            .encode(&OrganEvent::Expression { division: DivisionRole::Great, value: 1 }, |_| false)
            .is_empty());
    }
}
