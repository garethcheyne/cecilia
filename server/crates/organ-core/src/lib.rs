//! Platform-independent organ MIDI engine.
//!
//! * [`midi`]: the engine thread that owns every MIDI connection. Callers use
//!   the cloneable [`MidiHub`] handle and observe [`EngineEvent`]s.
//! * [`organ`]: organ definitions (divisions, stops, MIDI bindings) as JSON.
//! * [`driver`]: MIDI ⇄ organ events for one definition.
//! * [`console`]: stop/key/pedal state of a console.
//! * [`live`]: the connected organ's current state, tracked from MIDI traffic.
//! * [`simulator`] + [`synth`]: the built-in virtual organ and its sound.

pub mod console;
pub mod driver;
pub mod live;
pub mod midi;
pub mod organ;
pub mod simulator;
pub mod synth;

pub use midi::{
    Direction, EngineEvent, MidiConfig, MidiError, MidiHub, MidiStatus, PortList, TestState,
    TestStatus, SIMULATOR_PORT,
};
