//! The built-in virtual organ.
//!
//! It behaves like real hardware on the [`SIMULATOR_PORT`]: MIDI sent to it
//! changes its stops, keys and pedals (and sounds through the synth), and
//! anything done at its console is encoded with the organ's own bindings and
//! fed back in as MIDI input. So recording, playback and translation exercise
//! exactly the same paths as with a real instrument.
//!
//! [`SIMULATOR_PORT`]: crate::midi::SIMULATOR_PORT

use std::sync::{mpsc, Arc, Mutex, MutexGuard};

use serde::Serialize;
use tokio::sync::broadcast;
use utoipa::ToSchema;

use crate::console::ConsoleState;
use crate::driver::{Driver, OrganEvent};
use crate::midi::MidiHub;
use crate::organ::OrganDefinition;
use crate::synth::{pipes_for, SynthHandle};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SimSnapshot {
    pub organ_id: String,
    /// Name of the audio device, if sound is available.
    pub audio: Option<String>,
    pub console: ConsoleState,
}

struct Inner {
    driver: Driver,
    console: ConsoleState,
}

#[derive(Clone)]
pub struct Simulator {
    inner: Arc<Mutex<Inner>>,
    events: broadcast::Sender<SimSnapshot>,
    hub: MidiHub,
    synth: Option<SynthHandle>,
}

impl Simulator {
    /// `from_engine` receives whatever the engine sends to the simulator port.
    pub fn start(
        from_engine: mpsc::Receiver<Vec<u8>>,
        hub: MidiHub,
        organ: OrganDefinition,
        synth: Option<SynthHandle>,
    ) -> Self {
        let console = ConsoleState::new(&organ);
        let (events, _) = broadcast::channel(1024);
        let sim = Self {
            inner: Arc::new(Mutex::new(Inner { driver: Driver::new(organ), console })),
            events,
            hub,
            synth,
        };
        let worker = sim.clone();
        std::thread::Builder::new()
            .name("simulator".into())
            .spawn(move || {
                for msg in from_engine {
                    worker.receive_midi(&msg);
                }
            })
            .expect("failed to spawn simulator thread");
        sim
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SimSnapshot> {
        self.events.subscribe()
    }

    pub fn snapshot(&self) -> SimSnapshot {
        let inner = self.lock();
        self.snapshot_of(&inner)
    }

    fn snapshot_of(&self, inner: &Inner) -> SimSnapshot {
        SimSnapshot {
            organ_id: inner.driver.organ().id.clone(),
            audio: self.synth.as_ref().map(|s| s.device.clone()),
            console: inner.console.clone(),
        }
    }

    /// Re-render sound and notify subscribers after a change.
    fn changed(&self, inner: &Inner) {
        if let Some(synth) = &self.synth {
            synth.set_pipes(pipes_for(inner.driver.organ(), &inner.console));
        }
        let _ = self.events.send(self.snapshot_of(inner));
    }

    /// MIDI arriving at the simulated organ (from playback etc.).
    fn receive_midi(&self, msg: &[u8]) {
        let mut inner = self.lock();
        let events = inner.driver.decode(msg);
        let mut changed = false;
        for e in &events {
            changed |= inner.console.apply(e);
        }
        if changed {
            self.changed(&inner);
        }
    }

    /// The organist did something at the simulated console: sound it, show
    /// it, and send the MIDI the real organ would send.
    pub fn act(&self, event: OrganEvent) {
        let mut inner = self.lock();
        let msgs = {
            let console = &inner.console;
            inner.driver.encode(&event, |id| console.is_on(id))
        };
        if inner.console.apply(&event) {
            self.changed(&inner);
        }
        drop(inner);
        for m in msgs {
            self.hub.inject(m);
        }
    }

    /// Lift every key held at the simulated console.
    pub fn release_keys(&self) {
        let keys: Vec<_> = {
            let inner = self.lock();
            inner
                .console
                .notes
                .iter()
                .flat_map(|(d, ks)| ks.iter().map(move |k| (*d, *k)))
                .collect()
        };
        for (division, key) in keys {
            self.act(OrganEvent::Note { division, key, velocity: 0 });
        }
    }

    /// Release all keys and push in every stop, sending what the organ would
    /// (its general-cancel message, or one "off" per drawn stop).
    pub fn reset(&self) {
        self.release_keys();
        self.act(OrganEvent::AllStopsOff);
    }

    /// Swap the simulated instrument. The console starts fresh.
    pub fn load_organ(&self, organ: OrganDefinition) {
        let mut inner = self.lock();
        inner.console = ConsoleState::new(&organ);
        inner.driver = Driver::new(organ);
        self.changed(&inner);
    }
}
