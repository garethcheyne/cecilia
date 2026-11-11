//! What the organ at the end of the MIDI cables is doing right now.
//!
//! Every message the engine receives from the organ, and every message it
//! sends to it, is decoded with the organ's driver and applied to a
//! [`ConsoleState`]. Remote controls (the MCP server, later voice control and
//! playback) use it to know which stops are drawn: essential for organs whose
//! stops only *toggle*, where sending a stop that's already on would push it in.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::console::ConsoleState;
use crate::driver::{Driver, OrganEvent};
use crate::midi::{EngineEvent, MidiHub};
use crate::organ::OrganDefinition;

struct Inner {
    driver: Driver,
    console: ConsoleState,
}

#[derive(Clone)]
pub struct LiveOrgan {
    inner: Arc<Mutex<Inner>>,
    changes: broadcast::Sender<ConsoleState>,
}

impl LiveOrgan {
    pub fn start(hub: &MidiHub, organ: OrganDefinition) -> Self {
        let live = Self {
            inner: Arc::new(Mutex::new(Inner { console: ConsoleState::new(&organ), driver: Driver::new(organ) })),
            changes: broadcast::channel(256).0,
        };
        let mut rx = hub.subscribe();
        let worker = live.clone();
        std::thread::Builder::new()
            .name("live-organ".into())
            .spawn(move || loop {
                match rx.blocking_recv() {
                    Ok(EngineEvent::Midi { bytes, .. }) => worker.observe(&bytes),
                    Ok(_) => {}
                    // Missed traffic: the state may be stale until the next
                    // general cancel, which is the best we can do.
                    Err(RecvError::Lagged(n)) => tracing::warn!(skipped = n, "live organ state lagged"),
                    Err(RecvError::Closed) => break,
                }
            })
            .expect("failed to spawn live-organ thread");
        live
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn observe(&self, msg: &[u8]) {
        let mut inner = self.lock();
        let mut changed = false;
        for e in inner.driver.decode(msg) {
            changed |= inner.console.apply(&e);
        }
        if changed {
            let _ = self.changes.send(inner.console.clone());
        }
    }

    /// The console state after every change.
    pub fn subscribe(&self) -> broadcast::Receiver<ConsoleState> {
        self.changes.subscribe()
    }

    /// Switch to another organ definition; its state starts fresh.
    pub fn load_organ(&self, organ: OrganDefinition) {
        let mut inner = self.lock();
        inner.console = ConsoleState::new(&organ);
        inner.driver = Driver::new(organ);
        let _ = self.changes.send(inner.console.clone());
    }

    pub fn organ(&self) -> OrganDefinition {
        self.lock().driver.organ().clone()
    }

    pub fn console(&self) -> ConsoleState {
        self.lock().console.clone()
    }

    /// The MIDI that makes the organ do `event`, given its current state.
    pub fn encode(&self, event: &OrganEvent) -> Vec<Vec<u8>> {
        let inner = self.lock();
        inner.driver.encode(event, |id| inner.console.is_on(id))
    }
}
