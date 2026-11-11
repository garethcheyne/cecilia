//! Standard MIDI Files: import and export.
//!
//! * **Organ format** is MIDI exactly as the current organ speaks it (a
//!   general cancel, the ceciliation, then the performance). It is what
//!   OrganAssist's "Backup Entire Library as MIDI" produces, so it is also
//!   how a library comes across from the original program.
//! * **General MIDI** puts each division's notes on its own channel (Great 1,
//!   Swell 2, Pedal 3… by default) with the Church Organ sound, for other
//!   software and for MuseScore.
//!
//! Imports turn a file into ordinary tracks, optionally split into pieces at
//! long silences.

use std::collections::{BTreeMap, BTreeSet};

use midly::{Format, Header, MetaMessage, Smf, Timing, TrackEvent, TrackEventKind};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::console::ConsoleState;
use crate::driver::{Driver, OrganEvent};
use crate::organ::{DivisionRole, Equivalent, Family, OrganDefinition};
use crate::organist::Organist;
use crate::recorder::{AutoRecorder, RecorderConfig};
use crate::track::{Ceciliation, StopRef, Track, TrackEvent as TEvent, TrackSource};

const PPQ: u16 = 480;
/// 120 crotchets a minute: one tick is 500000/480 µs, so 1 ms = 0.96 ticks.
const TEMPO_US: u32 = 500_000;
/// General MIDI "Church Organ" (program 20, zero-based 19).
const GM_CHURCH_ORGAN: u8 = 19;

#[derive(Debug, thiserror::Error)]
pub enum SmfError {
    #[error("this isn't a MIDI file that can be read: {0}")]
    Parse(String),
    #[error("SMPTE-timed MIDI files aren't supported")]
    Timecode,
    #[error("the file has no notes")]
    Empty,
}

/// Default General MIDI channel (1-16) for each division.
pub fn default_channels() -> BTreeMap<DivisionRole, u8> {
    use DivisionRole::*;
    [(Great, 1), (Swell, 2), (Pedal, 3), (Choir, 4), (Solo, 5), (Echo, 6)].into_iter().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// The current organ's own MIDI, ready to play into it.
    Organ,
    /// One channel per division, Church Organ sound.
    GeneralMidi,
}

fn ms_to_ticks(ms: u32) -> u64 {
    (ms as u64 * 1000 * PPQ as u64) / TEMPO_US as u64
}

/// Write `track` as a Standard MIDI File.
pub fn export(track: &Track, format: ExportFormat, organ: &OrganDefinition, channels: &BTreeMap<DivisionRole, u8>) -> Vec<u8> {
    let mut timed: Vec<(u32, Vec<u8>)> = Vec::new();
    match format {
        ExportFormat::Organ => organ_messages(track, organ, &mut timed),
        ExportFormat::GeneralMidi => gm_messages(track, channels, &mut timed),
    }
    timed.sort_by_key(|(t, _)| *t);

    // Raw bytes -> midly events. Keep the byte buffers alive for the arena.
    let arena = midly::Arena::new();
    let mut events: Vec<TrackEvent> = vec![
        TrackEvent { delta: 0.into(), kind: TrackEventKind::Meta(MetaMessage::Tempo(TEMPO_US.into())) },
        TrackEvent {
            delta: 0.into(),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(arena.add(track.info.title.as_bytes()))),
        },
    ];
    let mut last = 0u64;
    for (t, bytes) in &timed {
        let Ok(live) = midly::live::LiveEvent::parse(bytes) else { continue };
        let tick = ms_to_ticks(*t);
        let delta = (tick - last).min(0x0FFF_FFFF) as u32;
        last = tick;
        events.push(TrackEvent { delta: delta.into(), kind: live.as_track_event(&arena) });
    }
    let end = ms_to_ticks(track.duration_ms);
    events.push(TrackEvent {
        delta: (end.saturating_sub(last).min(0x0FFF_FFFF) as u32).into(),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    let mut smf = Smf::new(Header::new(Format::SingleTrack, Timing::Metrical(PPQ.into())));
    smf.tracks.push(events);
    let mut out = Vec::new();
    smf.write_std(&mut out).expect("writing to memory can't fail");
    out
}

fn organ_messages(track: &Track, organ: &OrganDefinition, out: &mut Vec<(u32, Vec<u8>)>) {
    let organist = Organist::new(organ.clone());
    let driver = Driver::new(organ.clone());
    let mut console = ConsoleState::new(organ);
    // Start from a known state: every stop pushed in. A toggle organ needs
    // its cancel message; others get one "off" per stop.
    let mut push = |t: u32, e: OrganEvent, console: &mut ConsoleState, out: &mut Vec<(u32, Vec<u8>)>| {
        for m in driver.encode(&e, |id| console.is_on(id)) {
            out.push((t, m));
        }
        console.apply(&e);
    };
    if organ.all_stops_off.is_some() {
        push(0, OrganEvent::AllStopsOff, &mut console, out);
    } else {
        for s in &organ.stops {
            for m in driver.encode(&OrganEvent::Stop { stop: s.id.clone(), on: false }, |_| true) {
                out.push((0, m));
            }
        }
    }
    let mut reg = track.initial.clone();
    let set_reg = |t: u32, reg: &Ceciliation, console: &mut ConsoleState, out: &mut Vec<(u32, Vec<u8>)>, push: &mut dyn FnMut(u32, OrganEvent, &mut ConsoleState, &mut Vec<(u32, Vec<u8>)>)| {
        let target = organist.stops_for(reg);
        let drawn: BTreeSet<String> = console.stops.clone();
        for id in drawn.difference(&target) {
            push(t, OrganEvent::Stop { stop: id.clone(), on: false }, console, out);
        }
        for id in target.difference(&drawn) {
            push(t, OrganEvent::Stop { stop: id.clone(), on: true }, console, out);
        }
    };
    // Ceciliation a moment after the cancel, so a toggle organ settles.
    let start = if organ.all_stops_off.is_some() { 1 } else { 0 };
    set_reg(start, &reg, &mut console, out, &mut push);
    for (d, v) in &track.initial.expression {
        if let Some(to) = organist.expression_division(*d) {
            push(start, OrganEvent::Expression { division: to, value: *v }, &mut console, out);
        }
    }
    let volume = |v: u8| (v as u32 * track.info.volume as u32 / 100) as u8;
    if organ.master_volume.is_some() {
        push(start, OrganEvent::MasterVolume { value: volume(track.initial.master_volume.unwrap_or(127)) }, &mut console, out);
    }
    let mut i = 0;
    while i < track.events.len() {
        let t = track.events[i].t.max(start);
        let mut reg_changed = false;
        // Everything at this instant.
        while i < track.events.len() && track.events[i].t.max(start) == t {
            match &track.events[i].event {
                TEvent::Note { division, key, velocity } => {
                    if reg_changed {
                        set_reg(t, &reg, &mut console, out, &mut push);
                        reg_changed = false;
                    }
                    if let Some(d) = organist.division(*division) {
                        push(t, OrganEvent::Note { division: d, key: *key, velocity: *velocity }, &mut console, out);
                    }
                }
                TEvent::Stop { stop, on } => {
                    if *on {
                        reg.stops.insert(stop.clone());
                    } else {
                        reg.stops.remove(stop);
                    }
                    reg_changed = true;
                }
                TEvent::Expression { division, value } => {
                    if let Some(to) = organist.expression_division(*division) {
                        push(t, OrganEvent::Expression { division: to, value: *value }, &mut console, out);
                    }
                }
                TEvent::MasterVolume { value } => {
                    if organ.master_volume.is_some() {
                        push(t, OrganEvent::MasterVolume { value: volume(*value) }, &mut console, out);
                    }
                }
            }
            i += 1;
        }
        if reg_changed {
            set_reg(t, &reg, &mut console, out, &mut push);
        }
    }
}

fn gm_messages(track: &Track, channels: &BTreeMap<DivisionRole, u8>, out: &mut Vec<(u32, Vec<u8>)>) {
    let ch = |d: &DivisionRole| channels.get(d).copied().filter(|c| (1..=16).contains(c)).map(|c| c - 1);
    let used: BTreeSet<u8> = track
        .events
        .iter()
        .filter_map(|e| match &e.event {
            TEvent::Note { division, .. } => ch(division),
            _ => None,
        })
        .collect();
    for c in &used {
        out.push((0, vec![0xC0 | c, GM_CHURCH_ORGAN]));
        out.push((0, vec![0xB0 | c, 7, (track.info.volume as u32 * 127 / 100) as u8]));
    }
    for (d, v) in &track.initial.expression {
        if let Some(c) = ch(d) {
            out.push((0, vec![0xB0 | c, 11, *v]));
        }
    }
    for e in &track.events {
        match &e.event {
            TEvent::Note { division, key, velocity } => {
                if let Some(c) = ch(division) {
                    out.push((e.t, if *velocity > 0 { vec![0x90 | c, *key, *velocity] } else { vec![0x80 | c, *key, 0] }));
                }
            }
            TEvent::Expression { division, value } => {
                if let Some(c) = ch(division) {
                    out.push((e.t, vec![0xB0 | c, 11, *value]));
                }
            }
            _ => {}
        }
    }
}

/// Every track as an organ-format MIDI file in one zip: the backup the
/// original program calls "Backup Entire Library as MIDI".
pub fn library_zip(tracks: &[Track], organ: &OrganDefinition) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut used = BTreeSet::new();
        for t in tracks {
            let mut name = file_name(&t.info.title);
            let mut n = 2;
            while !used.insert(name.clone()) {
                name = format!("{} ({n})", file_name(&t.info.title));
                n += 1;
            }
            if zip.start_file(format!("{name}.mid"), opts).is_ok() {
                let _ = zip.write_all(&export(t, ExportFormat::Organ, organ, &default_channels()));
            }
        }
        let _ = zip.finish();
    }
    buf.into_inner()
}

/// A title made safe for a file name on any system.
pub fn file_name(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.,()'".contains(c) { c } else { '_' })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "track".into() } else { s.chars().take(80).collect() }
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Every channel/SysEx message in the file with its time in µs, all tracks
/// merged and the tempo map applied.
pub fn read_messages(bytes: &[u8]) -> Result<(Vec<(u64, Vec<u8>)>, BTreeMap<u8, String>), SmfError> {
    let smf = Smf::parse(bytes).map_err(|e| SmfError::Parse(e.to_string()))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int().max(1) as u64,
        Timing::Timecode(..) => return Err(SmfError::Timecode),
    };
    // (absolute tick, track index, event) for all tracks, then a tempo walk.
    let mut all: Vec<(u64, usize, &TrackEventKind)> = Vec::new();
    let mut names = BTreeMap::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0u64;
        let mut channel_of_track = None;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Midi { channel, .. } = ev.kind {
                channel_of_track.get_or_insert(channel.as_int() + 1);
            }
            all.push((tick, ti, &ev.kind));
        }
        // A named track's first channel gets the name (handy for mapping).
        if let Some(name) = track.iter().find_map(|e| match e.kind {
            TrackEventKind::Meta(MetaMessage::TrackName(n)) => Some(String::from_utf8_lossy(n).trim().to_string()),
            _ => None,
        }) {
            if let Some(c) = channel_of_track {
                if !name.is_empty() {
                    names.entry(c).or_insert(name);
                }
            }
        }
    }
    all.sort_by_key(|(tick, ti, _)| (*tick, *ti));
    let mut out = Vec::new();
    let (mut last_tick, mut us, mut tempo) = (0u64, 0u64, TEMPO_US as u64);
    for (tick, _, kind) in all {
        us += (tick - last_tick) * tempo / ppq;
        last_tick = tick;
        match kind {
            TrackEventKind::Meta(MetaMessage::Tempo(t)) => tempo = t.as_int() as u64,
            TrackEventKind::Midi { .. } | TrackEventKind::SysEx(_) => {
                if let Some(live) = kind.as_live_event() {
                    let mut raw = Vec::new();
                    if live.write_std(&mut raw).is_ok() {
                        out.push((us, raw));
                    }
                }
            }
            _ => {}
        }
    }
    Ok((out, names))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChannelInfo {
    /// 1-16.
    pub channel: u8,
    pub name: Option<String>,
    pub notes: usize,
    pub low: u8,
    pub high: u8,
    /// Average number of notes sounding together while any sound.
    pub polyphony: f32,
    /// Same notes at the same times as an earlier channel (a doubled part).
    pub duplicate_of: Option<u8>,
    pub suggested: Option<DivisionRole>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MidiAnalysis {
    pub channels: Vec<ChannelInfo>,
    pub duration_ms: u32,
    /// Share (0-1) of the file's non-note messages that the current organ
    /// understands. High means it was recorded on (or for) this organ.
    pub organ_match: f32,
    /// Silences of 4 s or more (ms from the start), where the file could be
    /// split into separate pieces.
    pub gaps: Vec<u32>,
}

/// Look at a MIDI file before importing it.
pub fn analyse(bytes: &[u8], organ: &OrganDefinition) -> Result<MidiAnalysis, SmfError> {
    let (msgs, names) = read_messages(bytes)?;
    let driver = Driver::new(organ.clone());
    let mut per: BTreeMap<u8, Vec<(u64, u8, bool)>> = BTreeMap::new();
    let (mut other, mut understood) = (0usize, 0usize);
    for (t, m) in &msgs {
        let status = m[0];
        if matches!(status & 0xF0, 0x80 | 0x90) && m.len() == 3 {
            per.entry((status & 0x0F) + 1).or_default().push((*t, m[1], status & 0xF0 == 0x90 && m[2] > 0));
        } else if !(status & 0xF0 == 0xB0 && matches!(m.get(1), Some(7 | 10 | 91 | 93 | 121 | 0 | 32))) && status & 0xF0 != 0xC0 {
            other += 1;
            if !driver.decode(m).is_empty() {
                understood += 1;
            }
        }
    }
    if per.values().all(|v| v.iter().all(|(_, _, on)| !on)) {
        return Err(SmfError::Empty);
    }
    let duration_ms = msgs.last().map_or(0, |(t, _)| (t / 1000) as u32);
    let mut infos = Vec::new();
    let mut signatures: Vec<(u8, Vec<(u64, u8)>)> = Vec::new();
    for (ch, evs) in &per {
        let ons: Vec<(u64, u8)> = evs.iter().filter(|e| e.2).map(|e| (e.0 / 10_000, e.1)).collect();
        if ons.is_empty() {
            continue;
        }
        let low = ons.iter().map(|e| e.1).min().unwrap();
        let high = ons.iter().map(|e| e.1).max().unwrap();
        // Polyphony: average held count over time while > 0.
        let (mut held, mut last, mut area, mut sounding) = (0i32, 0u64, 0u64, 0u64);
        for (t, _, on) in evs {
            if held > 0 {
                area += (t - last) * held as u64;
                sounding += t - last;
            }
            held = (held + if *on { 1 } else { -1 }).max(0);
            last = *t;
        }
        let duplicate_of = signatures.iter().find(|(_, s)| *s == ons).map(|(c, _)| *c);
        signatures.push((*ch, ons.clone()));
        infos.push(ChannelInfo {
            channel: *ch,
            name: names.get(ch).cloned(),
            notes: ons.len(),
            low,
            high,
            polyphony: if sounding > 0 { area as f32 / sounding as f32 } else { 0.0 },
            duplicate_of,
            suggested: None,
        });
    }
    suggest_divisions(&mut infos, organ);
    Ok(MidiAnalysis {
        channels: infos,
        duration_ms,
        organ_match: if other == 0 { 0.0 } else { understood as f32 / other as f32 },
        gaps: find_gaps(&msgs, 4_000_000),
    })
}

/// Guess which division each channel belongs to: a channel matching one of
/// the organ's own note channels keeps it; otherwise the lowest-pitched
/// channel is the pedal and the rest go to the manuals.
fn suggest_divisions(infos: &mut [ChannelInfo], organ: &OrganDefinition) {
    for info in infos.iter_mut() {
        info.suggested = organ.divisions.iter().find(|d| d.channel == info.channel).map(|d| d.role);
        if let Some(name) = info.name.as_deref().map(str::to_lowercase) {
            for (word, role) in [
                ("ped", DivisionRole::Pedal),
                ("bass", DivisionRole::Pedal),
                ("great", DivisionRole::Great),
                ("swell", DivisionRole::Swell),
                ("choir", DivisionRole::Choir),
                ("solo", DivisionRole::Solo),
            ] {
                if name.contains(word) && organ.division(role).is_some() {
                    info.suggested = Some(role);
                }
            }
        }
    }
    let unassigned: Vec<usize> = (0..infos.len()).filter(|&i| infos[i].suggested.is_none() && infos[i].duplicate_of.is_none()).collect();
    if unassigned.is_empty() {
        return;
    }
    let lowest = *unassigned.iter().min_by_key(|&&i| infos[i].low as u32 + infos[i].high as u32).unwrap();
    let has_pedal = organ.division(DivisionRole::Pedal).is_some();
    for &i in &unassigned {
        infos[i].suggested = Some(if i == lowest && has_pedal && unassigned.len() > 1 && infos[i].high <= 67 {
            DivisionRole::Pedal
        } else if organ.division(DivisionRole::Great).is_some() {
            DivisionRole::Great
        } else {
            organ.divisions.iter().map(|d| d.role).find(|r| *r != DivisionRole::Pedal).unwrap_or(DivisionRole::Great)
        });
    }
}

fn find_gaps(msgs: &[(u64, Vec<u8>)], min_us: u64) -> Vec<u32> {
    let mut gaps = Vec::new();
    let mut held: BTreeSet<(u8, u8)> = BTreeSet::new();
    let mut silent_since: Option<u64> = None;
    for (t, m) in msgs {
        let s = m[0] & 0xF0;
        if !(matches!(s, 0x80 | 0x90) && m.len() == 3) {
            continue;
        }
        let key = (m[0] & 0x0F, m[1]);
        if s == 0x90 && m[2] > 0 {
            if let Some(since) = silent_since.take() {
                if t - since >= min_us {
                    gaps.push(((since + (t - since) / 2) / 1000) as u32);
                }
            }
            held.insert(key);
        } else {
            held.remove(&key);
            if held.is_empty() {
                silent_since = Some(*t);
            }
        }
    }
    gaps
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ImportOptions {
    /// Read the file as the current organ's own MIDI (stops and all).
    /// Otherwise notes are mapped by channel.
    #[serde(default)]
    pub organ_format: bool,
    /// Channel (1-16) to division, for General MIDI files. Channels left out
    /// are ignored. Default: the analysis's suggestions.
    #[serde(default)]
    pub channels: BTreeMap<u8, DivisionRole>,
    /// Pedal notes an octave up (MuseScore writes pedal parts at 16′ pitch).
    #[serde(default)]
    pub pedal_octave_up: bool,
    /// Split into separate pieces at silences this long (ms). None = one piece.
    #[serde(default)]
    pub split_ms: Option<u32>,
    /// Title for the piece (pieces get " (2)", " (3)"…). Default from the file.
    #[serde(default)]
    pub title: Option<String>,
}

/// Turn a MIDI file into tracks for `organ`.
pub fn import(bytes: &[u8], organ: &OrganDefinition, options: &ImportOptions, title: &str) -> Result<Vec<Track>, SmfError> {
    let (msgs, _) = read_messages(bytes)?;
    let config = RecorderConfig {
        silence_ms: options.split_ms.unwrap_or(u32::MAX / 2).max(1000),
        margin_start_ms: 1000,
        margin_end_ms: 2000,
    };
    let mut rec = AutoRecorder::new(organ.clone(), config);
    rec.arm();
    let mut tracks = Vec::new();
    let mut last_us = 0u64;
    if options.organ_format {
        for (t, m) in &msgs {
            if let Some(done) = rec.tick(*t) {
                tracks.push(done);
            }
            rec.on_midi(*t, m);
            last_us = *t;
        }
    } else {
        let channels = if options.channels.is_empty() {
            analyse(bytes, organ)?
                .channels
                .into_iter()
                .filter(|c| c.duplicate_of.is_none())
                .filter_map(|c| c.suggested.map(|d| (c.channel, d)))
                .collect()
        } else {
            options.channels.clone()
        };
        // A plain ceciliation to start from: these files carry no stops.
        for e in default_ceciliation(organ, channels.values().copied().collect()) {
            rec.on_event(0, &e);
        }
        for (t, m) in &msgs {
            if let Some(done) = rec.tick(*t) {
                tracks.push(done);
            }
            let s = m[0] & 0xF0;
            if !(matches!(s, 0x80 | 0x90) && m.len() == 3) {
                continue;
            }
            let Some(&division) = channels.get(&((m[0] & 0x0F) + 1)) else { continue };
            let mut key = m[1] as i16;
            if division == DivisionRole::Pedal && options.pedal_octave_up {
                key += 12;
            }
            let velocity = if s == 0x90 { m[2] } else { 0 };
            if let Ok(key) = u8::try_from(key) {
                rec.on_event(*t, &OrganEvent::Note { division, key: key.min(127), velocity });
            }
            last_us = *t;
        }
    }
    if let Some(done) = rec.disarm(last_us + 1) {
        tracks.push(done);
    }
    if tracks.is_empty() {
        return Err(SmfError::Empty);
    }
    let base = options.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| title.to_string());
    let n = tracks.len();
    for (i, t) in tracks.iter_mut().enumerate() {
        t.source = TrackSource::Imported;
        t.info.title = if n == 1 { base.clone() } else { format!("{base} ({})", i + 1) };
        t.id = format!("imp-{}-{}", t.created, i + 1);
    }
    Ok(tracks)
}

/// Principal 8′ and 4′ on the manuals, 16′ and 8′ on the pedal, Great to
/// Pedal: a sensible start when a file has no ceciliation of its own.
fn default_ceciliation(organ: &OrganDefinition, divisions: BTreeSet<DivisionRole>) -> Vec<OrganEvent> {
    let voice = |division, family, footage: &str| StopRef {
        division,
        equivalent: Equivalent::Voice { family, footage: footage.into(), ranks: None, tag: String::new() },
    };
    let mut reg = Ceciliation::default();
    for d in &divisions {
        if *d == DivisionRole::Pedal {
            reg.stops.insert(voice(*d, Family::Flute, "16"));
            reg.stops.insert(voice(*d, Family::Principal, "8"));
            reg.stops.insert(StopRef {
                division: *d,
                equivalent: Equivalent::Coupler { source: DivisionRole::Great, pitch: crate::organ::CouplerPitch::Unison },
            });
        } else {
            reg.stops.insert(voice(*d, Family::Principal, "8"));
            reg.stops.insert(voice(*d, Family::Principal, "4"));
        }
    }
    Organist::new(organ.clone())
        .stops_for(&reg)
        .into_iter()
        .map(|stop| OrganEvent::Stop { stop, on: true })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organ::builtin_organs;
    use crate::sample::sample_tracks;

    fn organ(id: &str) -> OrganDefinition {
        builtin_organs().into_iter().find(|o| o.id == id).unwrap()
    }

    #[test]
    fn organ_format_round_trips_ceciliation_and_notes() {
        let demo = organ("demo");
        let hymn = sample_tracks().into_iter().next().unwrap();
        let bytes = export(&hymn, ExportFormat::Organ, &demo, &default_channels());
        assert_eq!(&bytes[..4], b"MThd");

        let a = analyse(&bytes, &demo).unwrap();
        assert!(a.organ_match > 0.9, "it's this organ's MIDI: {}", a.organ_match);
        let back = import(&bytes, &demo, &ImportOptions { organ_format: true, ..Default::default() }, "Hymn").unwrap();
        assert_eq!(back.len(), 1);
        let t = &back[0];
        assert_eq!(t.note_count(), hymn.note_count());
        let want = Organist::new(demo.clone()).stops_for(&hymn.initial);
        let got = Organist::new(demo.clone()).stops_for(&t.initial);
        assert_eq!(got, want, "opening ceciliation survives");
        // Verse 4 adds stops: those changes come back as stop events.
        assert!(t.events.iter().any(|e| matches!(e.event, TEvent::Stop { on: true, .. })));
        // Timing is kept to the millisecond-ish.
        let first = |t: &Track| t.events.iter().find(|e| matches!(e.event, TEvent::Note { .. })).unwrap().t;
        let last = |t: &Track| t.events.iter().rev().find(|e| matches!(e.event, TEvent::Note { .. })).unwrap().t;
        let span = |t: &Track| last(t) - first(t);
        assert!(span(t).abs_diff(span(&hymn)) <= 3, "{} vs {}", span(t), span(&hymn));
    }

    #[test]
    fn toggle_organ_exports_start_with_a_general_cancel() {
        let makin = organ("makin-2m");
        let hymn = sample_tracks().into_iter().next().unwrap();
        let bytes = export(&hymn, ExportFormat::Organ, &makin, &default_channels());
        let (msgs, _) = read_messages(&bytes).unwrap();
        assert_eq!(msgs[0].1, vec![0xF0, 0x00, 0x4A, 0x4F, 0x48, 0x41, 0x53, 0x7F, 0xF7]);
        let back = import(&bytes, &makin, &ImportOptions { organ_format: true, ..Default::default() }, "x").unwrap();
        assert_eq!(back[0].note_count(), hymn.note_count());
    }

    #[test]
    fn general_midi_maps_channels_back_to_divisions() {
        let demo = organ("demo");
        let hymn = sample_tracks().into_iter().next().unwrap();
        let bytes = export(&hymn, ExportFormat::GeneralMidi, &demo, &default_channels());
        let a = analyse(&bytes, &demo).unwrap();
        let ch: Vec<u8> = a.channels.iter().map(|c| c.channel).collect();
        assert_eq!(ch, [1, 3], "Great on 1, Pedal on 3");
        assert_eq!(a.channels[0].suggested, Some(DivisionRole::Great));
        assert_eq!(a.channels[1].suggested, Some(DivisionRole::Pedal));
        assert!(a.channels[0].polyphony > 2.0, "three parts on the Great");

        let back = import(&bytes, &demo, &ImportOptions::default(), "Old 100th").unwrap();
        assert_eq!(back[0].info.title, "Old 100th");
        assert_eq!(back[0].source, TrackSource::Imported);
        assert_eq!(back[0].note_count(), hymn.note_count());
        assert!(!back[0].initial.stops.is_empty(), "a starting ceciliation is supplied");
    }

    #[test]
    fn long_silences_split_a_file_into_pieces() {
        let demo = organ("demo");
        let mut hymn = sample_tracks().into_iter().next().unwrap();
        // Make the gap between verses 8 s.
        let marker = hymn.verses[0];
        for e in hymn.events.iter_mut().filter(|e| e.t > marker) {
            e.t += 8000;
        }
        hymn.duration_ms += 8000;
        let bytes = export(&hymn, ExportFormat::GeneralMidi, &demo, &default_channels());
        let a = analyse(&bytes, &demo).unwrap();
        assert_eq!(a.gaps.len(), 1);
        let pieces = import(&bytes, &demo, &ImportOptions { split_ms: Some(5000), ..Default::default() }, "Set").unwrap();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[1].info.title, "Set (2)");
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(analyse(b"not midi", &organ("demo")), Err(SmfError::Parse(_))));
    }
}
