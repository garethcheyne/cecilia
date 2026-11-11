//! MIDI device engine: port discovery, connections, live monitoring and a raw
//! "test" recorder/player that bypasses all organ logic (used to prove the
//! hardware and cabling work before anything else is configured).

use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};
use utoipa::ToSchema;

const CLIENT_NAME: &str = "Cecilia";

/// Name of the built-in virtual organ's port pair (input and output).
pub const SIMULATOR_PORT: &str = "Cecilia Simulator";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Which MIDI ports to use. Ports are identified by name, because OS-level port
/// ids are not stable across restarts or replugging.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MidiConfig {
    /// Input ports to listen to (the organ console, or Hauptwerk's output).
    #[serde(default)]
    pub inputs: Vec<String>,
    /// The single output port to drive (the organ, or Hauptwerk's input).
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PortList {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestState {
    Idle,
    Recording,
    Playing,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestStatus {
    pub state: TestState,
    /// Number of MIDI messages captured by the test recorder.
    pub events: usize,
    /// Length of the test recording (grows live while recording).
    pub duration_ms: u64,
    /// Playback position while playing, otherwise 0.
    pub position_ms: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MidiStatus {
    /// Input ports that are currently open.
    pub inputs: Vec<String>,
    /// Output port that is currently open.
    pub output: Option<String>,
    /// Configured ports that could not be opened (unplugged, in use, renamed).
    pub missing: Vec<String>,
    pub test: TestStatus,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
}

/// Pushed to every subscriber (e.g. WebSocket clients).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    /// A raw MIDI message was received or sent.
    Midi {
        port: String,
        direction: Direction,
        /// Microseconds since the engine started.
        t_us: u64,
        bytes: Vec<u8>,
    },
    /// Connections or test recorder state changed.
    Status(MidiStatus),
}

#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    #[error("MIDI subsystem unavailable: {0}")]
    Init(String),
    #[error("no MIDI output is connected")]
    NoOutput,
    #[error("the test recording is empty")]
    Empty,
    #[error("cannot do that while {0}")]
    Busy(&'static str),
    #[error("failed to send MIDI: {0}")]
    Send(String),
    #[error("MIDI engine has stopped")]
    EngineGone,
}

impl MidiError {
    /// True when the request conflicts with the current state rather than
    /// being a fault (maps to HTTP 409).
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::NoOutput | Self::Empty | Self::Busy(_))
    }
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

type Reply<T> = oneshot::Sender<T>;

enum Cmd {
    Ports(Reply<Result<PortList, MidiError>>),
    Configure(MidiConfig, Reply<MidiStatus>),
    Status(Reply<MidiStatus>),
    Send(Vec<u8>, Reply<Result<(), MidiError>>),
    /// Send without waiting for the result (used by the player thread).
    SendQuiet(Vec<u8>),
    Panic(Reply<Result<(), MidiError>>),
    TestRecord(Reply<Result<MidiStatus, MidiError>>),
    TestStop(Reply<MidiStatus>),
    TestPlay(Reply<Result<MidiStatus, MidiError>>),
    /// MIDI arriving from the simulator, treated like any other input.
    Inject(Vec<u8>),
}

/// Cheap, cloneable handle to the MIDI engine thread.
#[derive(Clone)]
pub struct MidiHub {
    tx: mpsc::Sender<Cmd>,
    events: broadcast::Sender<EngineEvent>,
    epoch: Instant,
}

impl MidiHub {
    /// Spawn the engine thread. If `simulator` is given, a virtual
    /// [`SIMULATOR_PORT`] appears in the port lists: output sent to it is
    /// forwarded to that channel, and [`MidiHub::inject`] feeds its input.
    pub fn start(simulator: Option<mpsc::Sender<Vec<u8>>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let (events, _) = broadcast::channel(4096);
        let engine_events = events.clone();
        let epoch = Instant::now();
        std::thread::Builder::new()
            .name("midi-engine".into())
            .spawn(move || Engine::new(engine_events, simulator, epoch).run(rx))
            .expect("failed to spawn MIDI engine thread");
        Self { tx, events, epoch }
    }

    /// Microseconds since the engine started: the clock of
    /// [`EngineEvent::Midi::t_us`].
    pub fn now_us(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }

    /// Deliver MIDI from the simulator's console as if it came from a port.
    /// Ignored unless the simulator port is a selected input.
    pub fn inject(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(Cmd::Inject(bytes));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    async fn call<R>(&self, make: impl FnOnce(Reply<R>) -> Cmd) -> Result<R, MidiError> {
        let (reply, rx) = oneshot::channel();
        self.tx.send(make(reply)).map_err(|_| MidiError::EngineGone)?;
        rx.await.map_err(|_| MidiError::EngineGone)
    }

    pub async fn ports(&self) -> Result<PortList, MidiError> {
        self.call(Cmd::Ports).await?
    }

    /// Close all current connections and open the given ports. Ports that
    /// cannot be opened are reported in [`MidiStatus::missing`].
    pub async fn configure(&self, config: MidiConfig) -> Result<MidiStatus, MidiError> {
        self.call(|r| Cmd::Configure(config, r)).await
    }

    pub async fn status(&self) -> Result<MidiStatus, MidiError> {
        self.call(Cmd::Status).await
    }

    pub async fn send(&self, bytes: Vec<u8>) -> Result<(), MidiError> {
        self.call(|r| Cmd::Send(bytes, r)).await?
    }

    /// Queue a message for the output without waiting (callable from any
    /// thread). Failures, such as having no output, are dropped.
    pub fn send_quiet(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(Cmd::SendQuiet(bytes));
    }

    /// All Notes Off + All Sound Off on every channel.
    pub async fn panic(&self) -> Result<(), MidiError> {
        self.call(Cmd::Panic).await?
    }

    pub async fn test_record(&self) -> Result<MidiStatus, MidiError> {
        self.call(Cmd::TestRecord).await?
    }

    pub async fn test_stop(&self) -> Result<MidiStatus, MidiError> {
        self.call(Cmd::TestStop).await
    }

    pub async fn test_play(&self) -> Result<MidiStatus, MidiError> {
        self.call(Cmd::TestPlay).await?
    }
}

// ---------------------------------------------------------------------------
// Engine (runs on its own thread)
// ---------------------------------------------------------------------------

/// Written by MIDI input callbacks (which run on driver threads).
struct Recorder {
    recording: bool,
    start: Instant,
    events: Vec<(u64, Vec<u8>)>,
}

struct Playback {
    start: Instant,
    next: usize,
    events: Vec<(u64, Vec<u8>)>,
}

enum Output {
    Device(MidiOutputConnection),
    Simulator,
}

struct Engine {
    epoch: Instant,
    events: broadcast::Sender<EngineEvent>,
    inputs: Vec<(String, MidiInputConnection<()>)>,
    output: Option<(String, Output)>,
    missing: Vec<String>,
    recorder: Arc<Mutex<Recorder>>,
    playback: Option<Playback>,
    /// Where output to the simulator port goes (None = no simulator).
    simulator: Option<mpsc::Sender<Vec<u8>>>,
    /// The simulator port is a selected input.
    simulator_input: bool,
}

fn lock(m: &Mutex<Recorder>) -> MutexGuard<'_, Recorder> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Shared by real input callbacks and injected simulator input.
fn handle_input(
    recorder: &Mutex<Recorder>,
    events: &broadcast::Sender<EngineEvent>,
    epoch: Instant,
    port: &str,
    bytes: &[u8],
) {
    let now = Instant::now();
    {
        let mut rec = lock(recorder);
        if rec.recording {
            let t = now.duration_since(rec.start).as_micros() as u64;
            rec.events.push((t, bytes.to_vec()));
        }
    }
    let _ = events.send(EngineEvent::Midi {
        port: port.to_string(),
        direction: Direction::In,
        t_us: now.duration_since(epoch).as_micros() as u64,
        bytes: bytes.to_vec(),
    });
}

impl Engine {
    fn new(events: broadcast::Sender<EngineEvent>, simulator: Option<mpsc::Sender<Vec<u8>>>, epoch: Instant) -> Self {
        raise_timer_resolution();
        let now = Instant::now();
        Self {
            epoch,
            events,
            inputs: Vec::new(),
            output: None,
            missing: Vec::new(),
            simulator,
            simulator_input: false,
            recorder: Arc::new(Mutex::new(Recorder {
                recording: false,
                start: now,
                events: Vec::new(),
            })),
            playback: None,
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Cmd>) {
        loop {
            let cmd = match self.time_to_next_event() {
                None => match rx.recv() {
                    Ok(cmd) => Some(cmd),
                    Err(_) => break,
                },
                Some(wait) => match rx.recv_timeout(wait) {
                    Ok(cmd) => Some(cmd),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                },
            };
            if let Some(cmd) = cmd {
                self.handle(cmd);
            }
            self.pump_playback();
        }
        tracing::info!("MIDI engine stopped");
    }

    fn handle(&mut self, cmd: Cmd) {
        // A dropped reply just means the caller went away; ignore it.
        match cmd {
            Cmd::Ports(r) => {
                let ports = match list_ports() {
                    Ok(p) => Ok(p),
                    // No MIDI system (e.g. Linux without the ALSA sequencer):
                    // the simulator still works, so offer it on its own.
                    Err(err) if self.simulator.is_some() => {
                        tracing::warn!(%err, "no MIDI system; only the simulator is available");
                        Ok(PortList { inputs: Vec::new(), outputs: Vec::new() })
                    }
                    Err(err) => Err(err),
                };
                let ports = ports.map(|mut p| {
                    if self.simulator.is_some() {
                        p.inputs.push(SIMULATOR_PORT.into());
                        p.outputs.push(SIMULATOR_PORT.into());
                    }
                    p
                });
                let _ = r.send(ports);
            }
            Cmd::Inject(bytes) => {
                if self.simulator_input {
                    handle_input(&self.recorder, &self.events, self.epoch, SIMULATOR_PORT, &bytes);
                }
            }
            Cmd::Configure(cfg, r) => {
                let status = self.configure(cfg);
                let _ = r.send(status);
            }
            Cmd::Status(r) => {
                let _ = r.send(self.status());
            }
            Cmd::Send(bytes, r) => {
                let _ = r.send(self.send(&bytes));
            }
            Cmd::SendQuiet(bytes) => {
                if let Err(err) = self.send(&bytes) {
                    tracing::trace!(%err, "dropped MIDI message");
                }
            }
            Cmd::Panic(r) => {
                let _ = r.send(self.panic());
            }
            Cmd::TestRecord(r) => {
                let _ = r.send(self.test_record());
            }
            Cmd::TestStop(r) => {
                let _ = r.send(self.test_stop());
            }
            Cmd::TestPlay(r) => {
                let _ = r.send(self.test_play());
            }
        }
    }

    // -- connections --------------------------------------------------------

    fn configure(&mut self, cfg: MidiConfig) -> MidiStatus {
        self.stop_playback();
        lock(&self.recorder).recording = false;
        for (_, conn) in self.inputs.drain(..) {
            conn.close();
        }
        if let Some((_, Output::Device(conn))) = self.output.take() {
            conn.close();
        }
        self.missing.clear();
        self.simulator_input = false;

        for name in cfg.inputs {
            if name == SIMULATOR_PORT && self.simulator.is_some() {
                self.simulator_input = true;
                continue;
            }
            if self.inputs.iter().any(|(n, _)| *n == name) {
                continue;
            }
            match self.open_input(&name) {
                Ok(conn) => {
                    tracing::info!(port = %name, "opened MIDI input");
                    self.inputs.push((name, conn));
                }
                Err(err) => {
                    tracing::warn!(port = %name, %err, "could not open MIDI input");
                    self.missing.push(name);
                }
            }
        }
        if let Some(name) = cfg.output {
            if name == SIMULATOR_PORT && self.simulator.is_some() {
                self.output = Some((name, Output::Simulator));
                return self.broadcast_status();
            }
            match open_output(&name) {
                Ok(conn) => {
                    tracing::info!(port = %name, "opened MIDI output");
                    self.output = Some((name, Output::Device(conn)));
                }
                Err(err) => {
                    tracing::warn!(port = %name, %err, "could not open MIDI output");
                    self.missing.push(name);
                }
            }
        }
        self.broadcast_status()
    }

    fn open_input(&self, name: &str) -> Result<MidiInputConnection<()>, String> {
        let mut midi_in = MidiInput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
        // Clock and active-sensing would flood the monitor and the recorder.
        midi_in.ignore(Ignore::TimeAndActiveSense);
        let ports = midi_in.ports();
        let names: Vec<String> = ports.iter().map(|p| midi_in.port_name(p).unwrap_or_default()).collect();
        let port = find_port(&names, name)
            .map(|i| ports[i].clone())
            .ok_or_else(|| "port not found".to_string())?;

        let recorder = Arc::clone(&self.recorder);
        let events = self.events.clone();
        let epoch = self.epoch;
        let port_name = name.to_string();
        midi_in
            .connect(
                &port,
                "cecilia-in",
                move |_driver_ts, bytes, _| {
                    handle_input(&recorder, &events, epoch, &port_name, bytes);
                },
                (),
            )
            .map_err(|e| e.to_string())
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), MidiError> {
        let (name, out) = self.output.as_mut().ok_or(MidiError::NoOutput)?;
        match out {
            Output::Device(conn) => conn.send(bytes).map_err(|e| MidiError::Send(e.to_string()))?,
            Output::Simulator => {
                let sim = self.simulator.as_ref().ok_or(MidiError::NoOutput)?;
                sim.send(bytes.to_vec())
                    .map_err(|_| MidiError::Send("simulator stopped".into()))?;
            }
        }
        let _ = self.events.send(EngineEvent::Midi {
            port: name.clone(),
            direction: Direction::Out,
            t_us: self.epoch.elapsed().as_micros() as u64,
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    fn panic(&mut self) -> Result<(), MidiError> {
        for ch in 0u8..16 {
            self.send(&[0xB0 | ch, 123, 0])?; // All Notes Off
            self.send(&[0xB0 | ch, 120, 0])?; // All Sound Off
        }
        Ok(())
    }

    // -- test recorder / player --------------------------------------------

    fn test_state(&self) -> TestState {
        if self.playback.is_some() {
            TestState::Playing
        } else if lock(&self.recorder).recording {
            TestState::Recording
        } else {
            TestState::Idle
        }
    }

    fn test_record(&mut self) -> Result<MidiStatus, MidiError> {
        if self.playback.is_some() {
            return Err(MidiError::Busy("playing"));
        }
        {
            let mut rec = lock(&self.recorder);
            rec.events.clear();
            rec.start = Instant::now();
            rec.recording = true;
        }
        Ok(self.broadcast_status())
    }

    fn test_stop(&mut self) -> MidiStatus {
        lock(&self.recorder).recording = false;
        self.stop_playback();
        self.broadcast_status()
    }

    fn test_play(&mut self) -> Result<MidiStatus, MidiError> {
        match self.test_state() {
            TestState::Recording => return Err(MidiError::Busy("recording")),
            TestState::Playing => return Err(MidiError::Busy("playing")),
            TestState::Idle => {}
        }
        if self.output.is_none() {
            return Err(MidiError::NoOutput);
        }
        let events = lock(&self.recorder).events.clone();
        if events.is_empty() {
            return Err(MidiError::Empty);
        }
        self.playback = Some(Playback {
            start: Instant::now(),
            next: 0,
            events,
        });
        Ok(self.broadcast_status())
    }

    fn stop_playback(&mut self) {
        if self.playback.take().is_some() {
            // Don't leave notes hanging if we stopped mid-chord.
            for ch in 0u8..16 {
                let _ = self.send(&[0xB0 | ch, 123, 0]);
            }
        }
    }

    fn time_to_next_event(&self) -> Option<Duration> {
        let pb = self.playback.as_ref()?;
        let due = pb.events.get(pb.next).map_or(0, |(t, _)| *t);
        let elapsed = pb.start.elapsed().as_micros() as u64;
        Some(Duration::from_micros(due.saturating_sub(elapsed)))
    }

    fn pump_playback(&mut self) {
        let Some(pb) = self.playback.as_mut() else {
            return;
        };
        let elapsed = pb.start.elapsed().as_micros() as u64;
        let mut due = Vec::new();
        while let Some((t, bytes)) = pb.events.get(pb.next) {
            if *t > elapsed {
                break;
            }
            due.push(bytes.clone());
            pb.next += 1;
        }
        let finished = pb.next >= pb.events.len();

        for bytes in due {
            if let Err(err) = self.send(&bytes) {
                tracing::warn!(%err, "test playback send failed");
            }
        }
        if finished {
            self.playback = None;
            self.broadcast_status();
        }
    }

    // -- status ---------------------------------------------------------------

    fn status(&self) -> MidiStatus {
        let state = self.test_state();
        let (events, duration_ms) = {
            let rec = lock(&self.recorder);
            let duration = if rec.recording {
                rec.start.elapsed().as_millis() as u64
            } else {
                rec.events.last().map_or(0, |(t, _)| t / 1000)
            };
            (rec.events.len(), duration)
        };
        let position_ms = self
            .playback
            .as_ref()
            .map_or(0, |pb| pb.start.elapsed().as_millis() as u64);
        let mut inputs: Vec<String> = self.inputs.iter().map(|(n, _)| n.clone()).collect();
        if self.simulator_input {
            inputs.push(SIMULATOR_PORT.into());
        }
        MidiStatus {
            inputs,
            output: self.output.as_ref().map(|(n, _)| n.clone()),
            missing: self.missing.clone(),
            test: TestStatus {
                state,
                events,
                duration_ms,
                position_ms,
            },
        }
    }

    fn broadcast_status(&self) -> MidiStatus {
        let status = self.status();
        let _ = self.events.send(EngineEvent::Status(status.clone()));
        status
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn list_ports() -> Result<PortList, MidiError> {
    let midi_in = MidiInput::new(CLIENT_NAME).map_err(|e| MidiError::Init(e.to_string()))?;
    let midi_out = MidiOutput::new(CLIENT_NAME).map_err(|e| MidiError::Init(e.to_string()))?;
    let inputs = midi_in
        .ports()
        .iter()
        .filter_map(|p| midi_in.port_name(p).ok())
        .filter(|n| useful_port(n))
        .collect();
    let outputs = midi_out
        .ports()
        .iter()
        .filter_map(|p| midi_out.port_name(p).ok())
        .filter(|n| useful_port(n))
        .collect();
    Ok(PortList { inputs, outputs })
}

/// A port name without the ALSA client:port numbers Linux appends
/// ("UM-ONE:UM-ONE MIDI 1 20:0" becomes "UM-ONE:UM-ONE MIDI 1"). Those
/// numbers can change after a reboot or replugging, so they're ignored when
/// finding a saved port.
pub fn port_key(name: &str) -> &str {
    match name.rsplit_once(' ') {
        Some((head, tail))
            if tail.split_once(':').is_some_and(|(a, b)| {
                !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
            }) =>
        {
            head
        }
        _ => name,
    }
}

/// The saved port: exactly, or else the same device with different numbers.
fn find_port(names: &[String], wanted: &str) -> Option<usize> {
    names
        .iter()
        .position(|n| n == wanted)
        .or_else(|| names.iter().position(|n| port_key(n) == port_key(wanted)))
}

/// Hide Linux's "Midi Through" loopback, which is never what an organist wants.
fn useful_port(name: &str) -> bool {
    !name.starts_with("Midi Through")
}

fn open_output(name: &str) -> Result<MidiOutputConnection, String> {
    let midi_out = MidiOutput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
    let ports = midi_out.ports();
    let names: Vec<String> = ports.iter().map(|p| midi_out.port_name(p).unwrap_or_default()).collect();
    let port = find_port(&names, name)
        .map(|i| ports[i].clone())
        .ok_or_else(|| "port not found".to_string())?;
    midi_out
        .connect(&port, "cecilia-out")
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_port_numbers_dont_matter() {
        assert_eq!(port_key("UM-ONE:UM-ONE MIDI 1 20:0"), "UM-ONE:UM-ONE MIDI 1");
        assert_eq!(port_key("Microsoft GS Wavetable Synth"), "Microsoft GS Wavetable Synth");
        assert_eq!(port_key("Port 2"), "Port 2", "only client:port pairs are stripped");
        let names = vec!["Midi Through:Midi Through Port-0 14:0".to_string(), "UM-ONE:UM-ONE MIDI 1 24:0".to_string()];
        assert_eq!(find_port(&names, "UM-ONE:UM-ONE MIDI 1 20:0"), Some(1), "renumbered after a reboot");
        assert_eq!(find_port(&names, "Gone 20:0"), None);
        assert!(!useful_port("Midi Through:Midi Through Port-0 14:0"));
    }
}

/// Windows' default scheduler tick is ~15.6 ms, far too coarse for MIDI
/// playback. Ask for 1 ms for the lifetime of the process.
fn raise_timer_resolution() {
    #[cfg(windows)]
    // SAFETY: timeBeginPeriod has no memory-safety preconditions.
    unsafe {
        windows_sys::Win32::Media::timeBeginPeriod(1);
    }
}
