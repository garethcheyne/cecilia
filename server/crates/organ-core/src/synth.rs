//! A small wavetable pipe-organ synthesiser for the simulator.
//!
//! The DSP (`Voices`) is plain Rust with no audio dependency, so it can be
//! tested and, later, compiled to WebAssembly. The `audio` feature adds a
//! cpal output stream (WASAPI / CoreAudio / ALSA) so the server, including a
//! Raspberry Pi, can play through its own speakers.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::Arc;

use crate::console::ConsoleState;
use crate::organ::{footage_feet, Equivalent, Family, OrganDefinition};

const TABLE_LEN: usize = 2048;
const ATTACK_S: f32 = 0.04;
const RELEASE_S: f32 = 0.12;
const MASTER_GAIN: f32 = 0.08;
const TREMULANT_HZ: f32 = 5.6;

/// One pipe that should be sounding.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipe {
    /// Stable identity so a held pipe keeps its phase across updates.
    pub key: (u16, u8, u8),
    pub freq: f32,
    pub family: Family,
    /// 0..1, including expression and master volume.
    pub gain: f32,
    pub tremulant: bool,
}

/// Work out every pipe that should sound for this console state.
pub fn pipes_for(organ: &OrganDefinition, console: &ConsoleState) -> Vec<Pipe> {
    let master = console.master_volume as f32 / 127.0;
    let index: HashMap<&str, usize> = organ.stops.iter().enumerate().map(|(i, s)| (s.id.as_str(), i)).collect();

    let mut pipes = Vec::new();
    for (stop_id, note) in console.sounding(organ) {
        let Some(&i) = index.get(stop_id.as_str()) else { continue };
        let stop = &organ.stops[i];
        let Equivalent::Voice { family, footage, ranks, .. } = &stop.equivalent else {
            continue;
        };
        let Some(feet) = footage_feet(footage) else { continue };
        let expression = console.expression.get(&stop.division).copied().unwrap_or(127) as f32 / 127.0;
        // Pedals don't open fully silent; keep a little sound when closed.
        let gain = master * (0.15 + 0.85 * expression);
        let tremulant = organ
            .stops
            .iter()
            .any(|t| t.division == stop.division && t.equivalent == Equivalent::Tremulant && console.is_on(&t.id));
        let base = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0) * (8.0 / feet);

        let rank_ratios: &[f32] = match family {
            // Fifth/octave chain, breaking back an octave in the treble.
            Family::Mixture => &[1.0, 4.0 / 3.0, 2.0, 8.0 / 3.0, 4.0, 16.0 / 3.0],
            _ => &[1.0],
        };
        let count = match family {
            Family::Mixture => ranks.unwrap_or(3).clamp(1, rank_ratios.len() as u8) as usize,
            _ => 1,
        };
        for (r, ratio) in rank_ratios.iter().take(count).enumerate() {
            let mut freq = base * ratio;
            while freq > 6000.0 {
                freq /= 2.0;
            }
            if *family == Family::Celeste {
                freq *= 2f32.powf(6.0 / 1200.0); // tuned sharp to beat
            }
            pipes.push(Pipe { key: (i as u16, note, r as u8), freq, family: *family, gain, tremulant });
        }
    }
    pipes
}

// ---------------------------------------------------------------------------
// Wavetables
// ---------------------------------------------------------------------------

/// Three band-limited versions per family: rich, medium, nearly sine.
struct Tables {
    by_family: HashMap<Family, [Vec<f32>; 3]>,
}

const HARMONIC_LIMITS: [usize; 3] = [32, 8, 2];

fn spectrum(family: Family, n: usize) -> f32 {
    let n = n as f32;
    match family {
        Family::Principal => {
            [1.0, 0.55, 0.35, 0.22, 0.14, 0.1, 0.07, 0.05].get(n as usize - 1).copied().unwrap_or(0.4 / (n * n))
        }
        Family::Flute | Family::Mutation => [1.0, 0.12, 0.06, 0.02].get(n as usize - 1).copied().unwrap_or(0.0),
        Family::String | Family::Celeste => 0.7 / n.powf(0.75),
        Family::Reed => {
            if n == 1.0 {
                0.6
            } else {
                0.9 / n.powf(0.55)
            }
        }
        Family::Mixture => [1.0, 0.3, 0.12].get(n as usize - 1).copied().unwrap_or(0.0),
        Family::Percussion | Family::Other => 1.0 / (n * n),
    }
}

fn family_level(family: Family) -> f32 {
    match family {
        Family::Principal => 0.8,
        Family::Flute => 0.7,
        Family::String => 0.45,
        Family::Celeste => 0.4,
        Family::Reed => 0.6,
        Family::Mutation => 0.4,
        Family::Mixture => 0.3,
        Family::Percussion | Family::Other => 0.5,
    }
}

impl Tables {
    fn build() -> Self {
        let families = [
            Family::Principal,
            Family::Flute,
            Family::String,
            Family::Celeste,
            Family::Reed,
            Family::Mutation,
            Family::Mixture,
            Family::Percussion,
            Family::Other,
        ];
        let by_family = families
            .into_iter()
            .map(|f| {
                let make = |limit: usize| {
                    let mut t = vec![0f32; TABLE_LEN + 1]; // +1 guard for interpolation
                    for h in 1..=limit {
                        let a = spectrum(f, h);
                        if a == 0.0 {
                            continue;
                        }
                        for (i, s) in t.iter_mut().enumerate() {
                            *s += a * (TAU * h as f32 * i as f32 / TABLE_LEN as f32).sin();
                        }
                    }
                    let peak = t.iter().fold(0f32, |m, s| m.max(s.abs())).max(1e-6);
                    t.iter_mut().for_each(|s| *s /= peak);
                    t
                };
                (f, HARMONIC_LIMITS.map(make))
            })
            .collect();
        Self { by_family }
    }

    fn for_pipe(&self, family: Family, freq: f32, sample_rate: f32) -> &[f32] {
        let tables = &self.by_family[&family];
        let max_harmonic = (sample_rate * 0.45 / freq) as usize;
        let idx = HARMONIC_LIMITS.iter().position(|&l| l <= max_harmonic).unwrap_or(2);
        &tables[idx]
    }
}

// ---------------------------------------------------------------------------
// Voices (the DSP)
// ---------------------------------------------------------------------------

struct Voice {
    pipe: Pipe,
    phase: f32,
    env: f32,
    releasing: bool,
}

pub struct Voices {
    sample_rate: f32,
    tables: Arc<Tables>,
    voices: HashMap<(u16, u8, u8), Voice>,
    lfo_phase: f32,
}

impl Voices {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, tables: Arc::new(Tables::build()), voices: HashMap::new(), lfo_phase: 0.0 }
    }

    /// Replace the set of pipes that should be sounding.
    pub fn set_pipes(&mut self, pipes: Vec<Pipe>) {
        let wanted: HashMap<_, _> = pipes.into_iter().map(|p| (p.key, p)).collect();
        for (key, v) in self.voices.iter_mut() {
            if !wanted.contains_key(key) {
                v.releasing = true;
            }
        }
        for (key, pipe) in wanted {
            match self.voices.get_mut(&key) {
                Some(v) => {
                    v.pipe = pipe;
                    v.releasing = false;
                }
                None => {
                    self.voices.insert(key, Voice { pipe, phase: 0.0, env: 0.0, releasing: false });
                }
            }
        }
    }

    pub fn active(&self) -> usize {
        self.voices.len()
    }

    /// Render mono samples into `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate;
        let attack = 1.0 / (ATTACK_S * sr);
        let release = 1.0 / (RELEASE_S * sr);
        let lfo_step = TREMULANT_HZ / sr;
        out.iter_mut().for_each(|s| *s = 0.0);

        let tables = Arc::clone(&self.tables);
        for v in self.voices.values_mut() {
            let table = tables.for_pipe(v.pipe.family, v.pipe.freq, sr);
            let level = family_level(v.pipe.family) * v.pipe.gain;
            let mut lfo = self.lfo_phase;
            for s in out.iter_mut() {
                let trem = if v.pipe.tremulant { (TAU * lfo).sin() } else { 0.0 };
                lfo = (lfo + lfo_step).fract();
                let step = v.pipe.freq * (1.0 + 0.003 * trem) * TABLE_LEN as f32 / sr;
                let pos = v.phase;
                let i = pos as usize;
                let frac = pos - i as f32;
                let sample = table[i] + (table[i + 1] - table[i]) * frac;
                v.phase = (pos + step) % TABLE_LEN as f32;

                v.env = if v.releasing { (v.env - release).max(0.0) } else { (v.env + attack).min(1.0) };
                *s += sample * v.env * level * (1.0 + 0.12 * trem);
            }
        }
        self.lfo_phase = (self.lfo_phase + lfo_step * out.len() as f32).fract();
        self.voices.retain(|_, v| !(v.releasing && v.env <= 0.0));

        for s in out.iter_mut() {
            let x = *s * MASTER_GAIN;
            *s = x / (1.0 + x.abs()); // soft clip
        }
    }
}

// ---------------------------------------------------------------------------
// Audio output
// ---------------------------------------------------------------------------

/// Sends pipe sets to the audio thread. Cheap to clone.
#[derive(Clone)]
pub struct SynthHandle {
    tx: std::sync::mpsc::Sender<Vec<Pipe>>,
    pub device: String,
}

impl SynthHandle {
    pub fn set_pipes(&self, pipes: Vec<Pipe>) {
        let _ = self.tx.send(pipes);
    }
}

#[cfg(feature = "audio")]
pub fn start_audio() -> Result<SynthHandle, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample};

    fn build<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        rx: std::sync::mpsc::Receiver<Vec<Pipe>>,
    ) -> Result<cpal::Stream, String> {
        let channels = config.channels as usize;
        let mut voices = Voices::new(config.sample_rate.0 as f32);
        let mut mono = Vec::new();
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _| {
                    while let Ok(pipes) = rx.try_recv() {
                        voices.set_pipes(pipes);
                    }
                    let frames = data.len() / channels;
                    mono.resize(frames, 0.0);
                    voices.render(&mut mono);
                    for (frame, s) in data.chunks_mut(channels).zip(&mono) {
                        frame.iter_mut().for_each(|x| *x = T::from_sample(*s));
                    }
                },
                |err| tracing::warn!(%err, "audio stream error"),
                None,
            )
            .map_err(|e| e.to_string())
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    // Some backends' streams aren't Send, so the stream lives on its own thread.
    std::thread::Builder::new()
        .name("synth-audio".into())
        .spawn(move || {
            let result = (|| {
                let host = cpal::default_host();
                let device = host.default_output_device().ok_or("no audio output device")?;
                let name = device.name().unwrap_or_else(|_| "audio output".into());
                let supported = device.default_output_config().map_err(|e| e.to_string())?;
                let config = supported.config();
                let stream = match supported.sample_format() {
                    SampleFormat::F32 => build::<f32>(&device, &config, rx),
                    SampleFormat::I16 => build::<i16>(&device, &config, rx),
                    SampleFormat::U16 => build::<u16>(&device, &config, rx),
                    SampleFormat::I32 => build::<i32>(&device, &config, rx),
                    other => Err(format!("unsupported sample format {other:?}")),
                }?;
                stream.play().map_err(|e| e.to_string())?;
                Ok::<_, String>((stream, name))
            })();
            match result {
                Ok((_stream, name)) => {
                    let _ = ready_tx.send(Ok(name));
                    loop {
                        std::thread::park(); // keep the stream alive
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let device = ready_rx.recv().map_err(|_| "audio thread died".to_string())??;
    Ok(SynthHandle { tx, device })
}

#[cfg(not(feature = "audio"))]
pub fn start_audio() -> Result<SynthHandle, String> {
    Err("built without the `audio` feature".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::OrganEvent;
    use crate::organ::{builtin_organs, DivisionRole};

    #[test]
    fn renders_sound_and_releases_to_silence() {
        let organ = builtin_organs().remove(0);
        let mut console = ConsoleState::new(&organ);
        console.apply(&OrganEvent::Stop { stop: "gt-mixture".into(), on: true });
        console.apply(&OrganEvent::Stop { stop: "gt-opendiap".into(), on: true });
        console.apply(&OrganEvent::Note { division: DivisionRole::Great, key: 60, velocity: 100 });

        let pipes = pipes_for(&organ, &console);
        assert_eq!(pipes.len(), 1 + 4, "diapason + 4 mixture ranks");

        let mut v = Voices::new(48_000.0);
        v.set_pipes(pipes);
        let mut buf = vec![0.0; 4800];
        v.render(&mut buf);
        let peak = buf.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01 && peak < 1.0, "audible and not clipping: {peak}");

        v.set_pipes(vec![]);
        for _ in 0..20 {
            v.render(&mut buf);
        }
        assert_eq!(v.active(), 0, "voices removed after release");
        assert!(buf.iter().all(|s| s.abs() < 1e-6));
    }
}
