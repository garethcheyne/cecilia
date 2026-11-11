//! MusicXML import (from MuseScore, Sibelius, Finale, Dorico, or scanned
//! music via OMR), as exact, notated tracks.
//!
//! Each voice of each staff is a "stream" that can be sent to any division.
//! By default the lowest voice goes to the pedal (in a hymn's short score
//! that's the bass line) and everything else to the Great.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::organ::{CouplerPitch, DivisionRole, Equivalent, Family};
use crate::track::{
    Meter, Mode, Registration, StopRef, TimedEvent, Track, TrackEvent, TrackInfo, TrackSource, TRACK_SCHEMA,
};

const START_MARGIN_MS: f64 = 1000.0;
const END_MARGIN_MS: u32 = 2500;
const RELEASE_MS: f64 = 35.0;
const VELOCITY: u8 = 80;

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("this file isn't MusicXML: {0}")]
    Parse(String),
    #[error("compressed MusicXML (.mxl) could not be opened: {0}")]
    Zip(String),
    #[error("only part-wise MusicXML is supported (most programs write it)")]
    Timewise,
    #[error("the score has no notes")]
    Empty,
}

/// The XML text of a `.musicxml`/`.xml` file or a compressed `.mxl`.
pub fn read_text(bytes: &[u8]) -> Result<String, XmlError> {
    if bytes.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| XmlError::Zip(e.to_string()))?;
        // META-INF/container.xml names the score; otherwise take the first XML file.
        let mut root = None;
        if let Ok(mut f) = zip.by_name("META-INF/container.xml") {
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            if let Ok(doc) = roxmltree::Document::parse(&s) {
                root = doc
                    .descendants()
                    .find(|n| n.has_tag_name("rootfile"))
                    .and_then(|n| n.attribute("full-path"))
                    .map(str::to_string);
            }
        }
        let name = match root {
            Some(r) => r,
            None => (0..zip.len())
                .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
                .find(|n| !n.starts_with("META-INF") && (n.ends_with(".xml") || n.ends_with(".musicxml")))
                .ok_or_else(|| XmlError::Zip("no score inside".into()))?,
        };
        let mut f = zip.by_name(&name).map_err(|e| XmlError::Zip(e.to_string()))?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| XmlError::Zip(e.to_string()))?;
        Ok(s)
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| XmlError::Parse("not UTF-8 text".into()))
    }
}

/// A note in crotchets from the start of the piece.
#[derive(Debug, Clone, PartialEq)]
struct XNote {
    stream: String,
    key: u8,
    start: f64,
    beats: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Stream {
    /// "part:staff:voice", used to choose its division.
    pub id: String,
    /// e.g. "Organ, lower staff, voice 2".
    pub label: String,
    pub notes: usize,
    pub low: u8,
    pub high: u8,
    pub suggested: DivisionRole,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XmlAnalysis {
    pub title: String,
    pub composer: String,
    pub streams: Vec<Stream>,
    /// Crotchets per minute (from the score, or 80 if it doesn't say).
    pub bpm: f32,
    pub beats: u8,
    pub beat_unit: u8,
    pub fifths: i8,
    pub measures: usize,
}

struct Parsed {
    title: String,
    composer: String,
    notes: Vec<XNote>,
    labels: BTreeMap<String, String>,
    bpm: Option<f64>,
    beats: u8,
    beat_unit: u8,
    fifths: i8,
    mode: Mode,
    /// Length of an opening pick-up bar, in crotchets.
    pickup: f64,
    length: f64,
    measures: usize,
}

fn child<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<roxmltree::Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(tag))
}

fn text_of(n: roxmltree::Node, tag: &str) -> Option<String> {
    child(n, tag).and_then(|c| c.text()).map(|t| t.trim().to_string())
}

fn num<T: std::str::FromStr>(n: roxmltree::Node, tag: &str) -> Option<T> {
    text_of(n, tag)?.parse().ok()
}

fn parse(xml: &str) -> Result<Parsed, XmlError> {
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(xml, opts).map_err(|e| XmlError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.has_tag_name("score-timewise") {
        return Err(XmlError::Timewise);
    }
    if !root.has_tag_name("score-partwise") {
        return Err(XmlError::Parse("no <score-partwise> element".into()));
    }
    let title = child(root, "work")
        .and_then(|w| text_of(w, "work-title"))
        .or_else(|| text_of(root, "movement-title"))
        .unwrap_or_default();
    let composer = root
        .descendants()
        .find(|n| n.has_tag_name("creator") && n.attribute("type") == Some("composer"))
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    let part_names: BTreeMap<String, String> = root
        .descendants()
        .filter(|n| n.has_tag_name("score-part"))
        .filter_map(|n| Some((n.attribute("id")?.to_string(), text_of(n, "part-name").unwrap_or_default())))
        .collect();

    let mut p = Parsed {
        title,
        composer,
        notes: Vec::new(),
        labels: BTreeMap::new(),
        bpm: None,
        beats: 4,
        beat_unit: 4,
        fifths: 0,
        mode: Mode::Major,
        pickup: 0.0,
        length: 0.0,
        measures: 0,
    };
    let mut meter_set = false;
    for (pi, part) in root.children().filter(|n| n.has_tag_name("part")).enumerate() {
        let pid = part.attribute("id").unwrap_or("P").to_string();
        let pname =
            part_names.get(&pid).cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Part {}", pi + 1));
        let mut divisions = 1.0f64;
        let mut staves = 1u8;
        let mut measure_start = 0.0f64;
        let mut open_ties: BTreeMap<(String, u8), usize> = BTreeMap::new();
        let mut last_start = 0.0f64;
        for (mi, measure) in part.children().filter(|n| n.has_tag_name("measure")).enumerate() {
            let mut t = 0.0f64; // crotchets into the measure
            let mut furthest = 0.0f64;
            for el in measure.children().filter(|n| n.is_element()) {
                match el.tag_name().name() {
                    "attributes" => {
                        if let Some(d) = num::<f64>(el, "divisions") {
                            divisions = d.max(1.0);
                        }
                        if let Some(s) = num::<u8>(el, "staves") {
                            staves = s;
                        }
                        if !meter_set {
                            if let Some(k) = child(el, "key") {
                                p.fifths = num(k, "fifths").unwrap_or(0);
                                if text_of(k, "mode").as_deref() == Some("minor") {
                                    p.mode = Mode::Minor;
                                }
                            }
                            if let Some(tm) = child(el, "time") {
                                p.beats = num(tm, "beats").unwrap_or(4);
                                p.beat_unit = num(tm, "beat-type").unwrap_or(4);
                                meter_set = true;
                            }
                        }
                    }
                    "direction" | "sound" => {
                        let sound = if el.has_tag_name("sound") { Some(el) } else { child(el, "sound") };
                        let tempo = sound
                            .and_then(|s| s.attribute("tempo"))
                            .and_then(|v| v.parse::<f64>().ok())
                            .or_else(|| {
                                el.descendants().find(|n| n.has_tag_name("metronome")).and_then(|m| {
                                    let per = num::<f64>(m, "per-minute")?;
                                    let unit = text_of(m, "beat-unit").unwrap_or_else(|| "quarter".into());
                                    let dotted = child(m, "beat-unit-dot").is_some();
                                    let q = match unit.as_str() {
                                        "half" => 2.0,
                                        "eighth" => 0.5,
                                        "whole" => 4.0,
                                        _ => 1.0,
                                    } * if dotted { 1.5 } else { 1.0 };
                                    Some(per * q)
                                })
                            });
                        if p.bpm.is_none() {
                            p.bpm = tempo.filter(|b| *b > 10.0 && *b < 400.0);
                        }
                    }
                    "backup" => t -= num::<f64>(el, "duration").unwrap_or(0.0) / divisions,
                    "forward" => {
                        t += num::<f64>(el, "duration").unwrap_or(0.0) / divisions;
                        furthest = furthest.max(t);
                    }
                    "note" => {
                        if child(el, "grace").is_some() || child(el, "cue").is_some() {
                            continue;
                        }
                        let dur = num::<f64>(el, "duration").unwrap_or(0.0) / divisions;
                        let chord = child(el, "chord").is_some();
                        let start = if chord { last_start } else { t };
                        if !chord {
                            last_start = t;
                            t += dur;
                            furthest = furthest.max(t);
                        }
                        let Some(pitch) = child(el, "pitch") else { continue }; // a rest
                        let step = text_of(pitch, "step").unwrap_or_default();
                        let alter: f64 = num(pitch, "alter").unwrap_or(0.0);
                        let octave: i32 = num(pitch, "octave").unwrap_or(4);
                        let base = match step.as_str() {
                            "C" => 0,
                            "D" => 2,
                            "E" => 4,
                            "F" => 5,
                            "G" => 7,
                            "A" => 9,
                            "B" => 11,
                            _ => continue,
                        };
                        let Ok(key) = u8::try_from((octave + 1) * 12 + base + alter.round() as i32) else { continue };
                        let staff: u8 = num(el, "staff").unwrap_or(1);
                        let voice: String = text_of(el, "voice").unwrap_or_else(|| "1".into());
                        let stream = format!("{pid}:{staff}:{voice}");
                        p.labels.entry(stream.clone()).or_insert_with(|| {
                            let staff_name = match (staves, staff) {
                                (1, _) => String::new(),
                                (2, 1) | (3, 1) => ", upper staff".into(),
                                (2, 2) => ", lower staff".into(),
                                (3, 2) => ", left hand".into(),
                                (3, 3) => ", pedal staff".into(),
                                (_, s) => format!(", staff {s}"),
                            };
                            format!("{pname}{staff_name}, voice {voice}")
                        });
                        let ties: Vec<&str> = el
                            .children()
                            .filter(|c| c.has_tag_name("tie"))
                            .filter_map(|c| c.attribute("type"))
                            .collect();
                        let abs = measure_start + start;
                        let tie_key = (stream.clone(), key);
                        let idx = match (ties.contains(&"stop"), open_ties.remove(&tie_key)) {
                            (true, Some(i)) => {
                                p.notes[i].beats = abs + dur - p.notes[i].start;
                                i
                            }
                            _ => {
                                p.notes.push(XNote { stream, key, start: abs, beats: dur });
                                p.notes.len() - 1
                            }
                        };
                        if ties.contains(&"start") {
                            open_ties.insert(tie_key, idx);
                        }
                    }
                    _ => {}
                }
            }
            if mi == 0 && pi == 0 {
                let bar = p.beats as f64 * 4.0 / p.beat_unit as f64;
                if measure.attribute("implicit") == Some("yes") || (furthest > 0.0 && furthest < bar - 1e-6) {
                    p.pickup = furthest;
                }
            }
            measure_start += furthest;
            p.measures = p.measures.max(mi + 1);
        }
        p.length = p.length.max(measure_start);
    }
    if p.notes.is_empty() {
        return Err(XmlError::Empty);
    }
    Ok(p)
}

fn suggestions(p: &Parsed) -> BTreeMap<String, DivisionRole> {
    // Streams on a third ("pedal") staff go to the pedal; otherwise the
    // lowest stream does, if it's within pedal compass.
    let mut out = BTreeMap::new();
    let streams: BTreeSet<&String> = p.notes.iter().map(|n| &n.stream).collect();
    let mut any_pedal = false;
    for s in &streams {
        let staff = s.split(':').nth(1).unwrap_or("1");
        let label = &p.labels[*s];
        let pedal = staff == "3" || label.to_lowercase().contains("ped");
        any_pedal |= pedal;
        out.insert((*s).clone(), if pedal { DivisionRole::Pedal } else { DivisionRole::Great });
    }
    if !any_pedal && streams.len() > 1 {
        let avg = |s: &String| {
            let keys: Vec<f64> = p.notes.iter().filter(|n| &n.stream == s).map(|n| n.key as f64).collect();
            keys.iter().sum::<f64>() / keys.len() as f64
        };
        if let Some(lowest) = streams.iter().min_by(|a, b| avg(a).total_cmp(&avg(b))) {
            let high = p.notes.iter().filter(|n| &n.stream == *lowest).map(|n| n.key).max().unwrap_or(0);
            if high <= 67 {
                out.insert((*lowest).clone(), DivisionRole::Pedal);
            }
        }
    }
    out
}

pub fn analyse(xml: &str) -> Result<XmlAnalysis, XmlError> {
    let p = parse(xml)?;
    let suggested = suggestions(&p);
    let mut streams: Vec<Stream> = suggested
        .iter()
        .map(|(id, d)| {
            let keys: Vec<u8> = p.notes.iter().filter(|n| &n.stream == id).map(|n| n.key).collect();
            Stream {
                id: id.clone(),
                label: p.labels[id].clone(),
                notes: keys.len(),
                low: *keys.iter().min().unwrap(),
                high: *keys.iter().max().unwrap(),
                suggested: *d,
            }
        })
        .collect();
    streams.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(XmlAnalysis {
        title: p.title,
        composer: p.composer,
        streams,
        bpm: p.bpm.unwrap_or(80.0) as f32,
        beats: p.beats,
        beat_unit: p.beat_unit,
        fifths: p.fifths,
        measures: p.measures,
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct XmlImportOptions {
    /// Stream id to division; streams left out use the suggestion. Map a
    /// stream to nothing by leaving it out and listing it in `skip`.
    #[serde(default)]
    pub streams: BTreeMap<String, DivisionRole>,
    #[serde(default)]
    pub skip: Vec<String>,
    /// Crotchets per minute (default: the score's tempo, or 80).
    #[serde(default)]
    pub bpm: Option<f32>,
    /// Play it this many times as hymn verses (default 1).
    #[serde(default)]
    pub verses: Option<u32>,
    #[serde(default)]
    pub title: Option<String>,
}

/// A plain hymn registration: Principal 8′ and 4′, Flute 8′ on the Great,
/// Swell to Great, 16′ and 8′ on the Pedal with Great to Pedal.
pub fn hymn_registration() -> Registration {
    let voice = |division, family, footage: &str| StopRef {
        division,
        equivalent: Equivalent::Voice { family, footage: footage.into(), ranks: None, tag: String::new() },
    };
    let coupler = |division, source| StopRef {
        division,
        equivalent: Equivalent::Coupler { source, pitch: CouplerPitch::Unison },
    };
    use DivisionRole::*;
    let mut r = Registration::default();
    r.stops.extend([
        voice(Great, Family::Principal, "8"),
        voice(Great, Family::Flute, "8"),
        voice(Great, Family::Principal, "4"),
        voice(Swell, Family::Flute, "8"),
        voice(Swell, Family::Principal, "4"),
        coupler(Great, Swell),
        voice(Pedal, Family::Flute, "16"),
        voice(Pedal, Family::Principal, "8"),
        coupler(Pedal, Great),
    ]);
    r
}

pub fn import(xml: &str, options: &XmlImportOptions, fallback_title: &str) -> Result<Track, XmlError> {
    let p = parse(xml)?;
    let mut map = suggestions(&p);
    map.extend(options.streams.clone());
    for s in &options.skip {
        map.remove(s);
    }
    let bpm = options.bpm.map(f64::from).or(p.bpm).unwrap_or(80.0).clamp(20.0, 300.0);
    let verses = options.verses.unwrap_or(1).clamp(1, 12);
    let ms_per_beat = 60_000.0 / bpm;
    let gap = 2.0;
    let ms = |beat: f64| (START_MARGIN_MS + beat * ms_per_beat).round() as u32;
    let mut events = Vec::new();
    let mut markers = Vec::new();
    for v in 0..verses {
        let offset = v as f64 * (p.length + gap);
        if v > 0 {
            markers.push(ms(offset - gap / 2.0));
        }
        for n in &p.notes {
            let Some(&division) = map.get(&n.stream) else { continue };
            let on = ms(offset + n.start);
            let off = (START_MARGIN_MS + (offset + n.start + n.beats) * ms_per_beat - RELEASE_MS)
                .round()
                .max(on as f64 + 1.0) as u32;
            events.push(TimedEvent { t: on, event: TrackEvent::Note { division, key: n.key, velocity: VELOCITY } });
            events.push(TimedEvent { t: off, event: TrackEvent::Note { division, key: n.key, velocity: 0 } });
        }
    }
    if events.is_empty() {
        return Err(XmlError::Empty);
    }
    let end = ms(verses as f64 * p.length + (verses - 1) as f64 * gap);
    let created = crate::recorder::unix_ms();
    let title = options
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| Some(p.title.clone()).filter(|t| !t.is_empty()))
        .unwrap_or_else(|| fallback_title.to_string());
    let mut track = Track {
        schema: TRACK_SCHEMA,
        id: format!("xml-{created}"),
        info: TrackInfo { composer: p.composer.clone(), ..TrackInfo::titled(title) },
        source: TrackSource::Imported,
        organ_id: String::new(),
        organ_name: "MusicXML".into(),
        created,
        modified: created,
        initial: hymn_registration(),
        duration_ms: end + END_MARGIN_MS,
        verses: markers,
        meter: Some(Meter {
            bpm: bpm as f32,
            beats: p.beats,
            beat_unit: p.beat_unit,
            fifths: p.fifths,
            mode: p.mode,
            offset_ms: ms(p.pickup),
        }),
        events,
    };
    track.sort();
    Ok(track)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two bars of a hymn in short score: S+A on the upper staff, T+B on the
    /// lower, with a pick-up, a tie and a chord.
    const HYMN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <work><work-title>Test Hymn</work-title></work>
  <identification><creator type="composer">Anon.</creator></identification>
  <part-list><score-part id="P1"><part-name>Organ</part-name></score-part></part-list>
  <part id="P1">
    <measure number="0" implicit="yes">
      <attributes><divisions>2</divisions><key><fifths>1</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><staves>2</staves></attributes>
      <direction><sound tempo="96"/></direction>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><staff>1</staff></note>
      <backup><duration>2</duration></backup>
      <note><pitch><step>G</step><octave>2</octave></pitch><duration>2</duration><voice>2</voice><staff>2</staff></note>
    </measure>
    <measure number="1">
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice><staff>1</staff></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><tie type="stop"/><voice>1</voice><staff>1</staff></note>
      <backup><duration>8</duration></backup>
      <note><pitch><step>B</step><octave>3</octave></pitch><duration>8</duration><voice>2</voice><staff>2</staff></note>
      <note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>8</duration><voice>2</voice><staff>2</staff></note>
      <backup><duration>8</duration></backup>
      <note><pitch><step>G</step><octave>2</octave></pitch><duration>8</duration><voice>3</voice><staff>2</staff></note>
    </measure>
  </part>
</score-partwise>"#;

    #[test]
    fn reads_title_meter_and_streams() {
        let a = analyse(HYMN).unwrap();
        assert_eq!(a.title, "Test Hymn");
        assert_eq!(a.composer, "Anon.");
        assert_eq!((a.bpm, a.beats, a.beat_unit, a.fifths), (96.0, 4, 4, 1));
        let ids: Vec<&str> = a.streams.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["P1:1:1", "P1:2:2", "P1:2:3"]);
        assert_eq!(a.streams[0].label, "Organ, upper staff, voice 1");
        // The lowest voice (the bass) goes to the pedal.
        let pedal: Vec<&str> =
            a.streams.iter().filter(|s| s.suggested == DivisionRole::Pedal).map(|s| s.id.as_str()).collect();
        assert!(pedal == ["P1:2:2"] || pedal == ["P1:2:3"], "{pedal:?}");
    }

    #[test]
    fn ties_merge_and_chords_share_a_start() {
        let p = parse(HYMN).unwrap();
        assert_eq!(p.pickup, 1.0);
        let g4: Vec<&XNote> = p.notes.iter().filter(|n| n.key == 67).collect();
        assert_eq!(g4.len(), 1, "tied notes are one note");
        assert_eq!((g4[0].start, g4[0].beats), (1.0, 4.0));
        let b3 = p.notes.iter().find(|n| n.key == 59).unwrap();
        let d4 = p.notes.iter().find(|n| n.key == 62).unwrap();
        assert_eq!(b3.start, d4.start);
        assert_eq!(p.length, 5.0);
    }

    #[test]
    fn imports_as_an_exact_track() {
        let t = import(HYMN, &XmlImportOptions { verses: Some(2), ..Default::default() }, "x").unwrap();
        assert_eq!(t.info.title, "Test Hymn");
        assert_eq!(t.verses.len(), 1);
        let m = t.meter.as_ref().unwrap();
        assert_eq!(m.bpm, 96.0);
        assert_eq!(m.offset_ms, 1000 + 625, "first downbeat after a crotchet pick-up at 96 bpm");
        assert_eq!(t.note_count(), 2 * 6);
        t.validate().unwrap();
    }

    #[test]
    fn rejects_other_xml() {
        assert!(matches!(analyse("<html/>"), Err(XmlError::Parse(_))));
        assert!(matches!(analyse("<score-timewise/>"), Err(XmlError::Timewise)));
    }
}
