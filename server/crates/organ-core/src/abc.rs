//! ABC notation (abcnotation.com) to Cecilia scores.
//!
//! Written for the hymns of the [Open Hymnal Project](https://openhymnal.org):
//! several voices interleaved line by line (`[V: S1V1] …`), lyrics on `w:`
//! lines (only counted, to know how many verses there are), and credits in
//! `C:`, `S:` and `%OHCOMPOSER`-style comment fields. The music is written
//! once; Cecilia's score repeats it for each verse.
//!
//! Supported: notes, accidentals (key signature and bar-local), octave marks,
//! lengths, rests, chords, ties, tuplets, broken rhythm, repeats with first
//! and second endings, and inline `[K:]`, `[L:]`, `[M:]`, `[Q:]` fields.
//! Decorations, slurs, grace notes, annotations and chord symbols are
//! skipped. Voice overlay (`&`) is not supported.

use std::collections::{BTreeMap, BTreeSet};

use crate::organ::DivisionRole;
use crate::sample::{Part, Score};

/// One parsed tune.
#[derive(Debug, Clone, Default)]
pub struct AbcTune {
    pub title: String,
    /// `C:` lines (composer, credits, copyright).
    pub credits: Vec<String>,
    /// `S:` lines (source).
    pub source: Vec<String>,
    /// Comment fields such as `%OHCOMPOSER Monk, William Henry (1823-1889)`,
    /// keyed by name (`OHCOMPOSER`).
    pub fields: BTreeMap<String, String>,
    /// Beats and beat unit; `None` for free rhythm (`M: none`).
    pub meter: Option<(u32, u32)>,
    /// True when the meter changes part way through.
    pub meter_changes: bool,
    /// Key as a Cecilia score key ("Eb", "F#m").
    pub key: String,
    /// Crotchets per minute, if the tune gives a tempo.
    pub bpm: Option<f64>,
    /// Highest verse number on the lyric lines (at least 1).
    pub verses: u32,
    pub voices: Vec<Voice>,
}

#[derive(Debug, Clone, Default)]
pub struct Voice {
    pub id: String,
    pub bass_clef: bool,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Note { keys: Vec<u8>, beats: f64, tie: bool },
    Rest(f64),
    Bar,
    RepeatStart,
    RepeatEnd,
    Ending(u8),
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

const MAJOR: [&str; 15] = ["Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#"];
const MINOR: [&str; 15] = ["Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#"];

/// Fifths and whether the key is minor, from an ABC key ("Eb", "Dm", "G mix").
fn parse_key(text: &str) -> Result<(i8, bool), String> {
    let text = text.split('%').next().unwrap_or("").trim();
    let first = text.split_whitespace().next().unwrap_or("");
    if first.is_empty() || first.eq_ignore_ascii_case("none") || first.contains('=') {
        return Ok((0, false));
    }
    let mut chars = first.chars();
    let letter = chars.next().unwrap().to_ascii_uppercase();
    if !('A'..='G').contains(&letter) {
        return Err(format!("unknown key {text:?}"));
    }
    let rest: String = chars.collect();
    let (acc, mode_inline) = match rest.chars().next() {
        Some('#') => ("#", rest[1..].to_string()),
        Some('b') => ("b", rest[1..].to_string()),
        _ => ("", rest.clone()),
    };
    let tonic = format!("{letter}{acc}");
    let mode_word =
        if mode_inline.is_empty() { text.split_whitespace().nth(1).unwrap_or("").to_string() } else { mode_inline };
    let mode = mode_word.to_ascii_lowercase();
    let m3: String = mode.chars().take(3).collect();
    let offset = match m3.as_str() {
        "" | "maj" | "ion" | "exp" => 0,
        "m" | "min" | "aeo" => -3,
        "mix" => -1,
        "dor" => -2,
        "phr" => -4,
        "lyd" => 1,
        "loc" => -5,
        _ if mode.starts_with('m') => -3,
        _ => 0,
    };
    let base = MAJOR.iter().position(|k| *k == tonic).ok_or_else(|| format!("unknown key {text:?}"))? as i8 - 7;
    let fifths = (base + offset).clamp(-7, 7);
    Ok((fifths, offset == -3))
}

/// Alteration for each letter C D E F G A B under a key signature.
fn signature(fifths: i8) -> [i8; 7] {
    const SHARPS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6]; // F C G D A E B
    const FLATS: [usize; 7] = [6, 2, 5, 1, 4, 0, 3]; // B E A D G C F
    let mut alt = [0i8; 7];
    if fifths > 0 {
        for &i in SHARPS.iter().take(fifths as usize) {
            alt[i] = 1;
        }
    } else {
        for &i in FLATS.iter().take((-fifths) as usize) {
            alt[i] = -1;
        }
    }
    alt
}

fn letter_index(c: char) -> usize {
    match c.to_ascii_uppercase() {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        _ => 6,
    }
}

const SEMITONE: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

fn fraction(s: &str) -> Option<f64> {
    let s = s.trim();
    match s.split_once('/') {
        Some((n, d)) => Some(n.trim().parse::<f64>().ok()? / d.trim().parse::<f64>().ok()?),
        None => s.parse().ok(),
    }
}

/// `Q:` as crotchets per minute; `unit` is the default note length.
fn parse_tempo(text: &str, unit: f64) -> Option<f64> {
    let text: String = {
        // Drop quoted labels such as "Slowly".
        let mut out = String::new();
        let mut quoted = false;
        for c in text.chars() {
            if c == '"' {
                quoted = !quoted;
            } else if !quoted {
                out.push(c);
            }
        }
        out
    };
    let text = text.split('%').next()?.trim();
    match text.split_once('=') {
        Some((len, bpm)) => {
            let len: f64 = len.split_whitespace().filter_map(fraction).sum();
            Some(bpm.trim().parse::<f64>().ok()? * len * 4.0)
        }
        None => Some(text.parse::<f64>().ok()? * unit * 4.0),
    }
    .filter(|b| b.is_finite() && *b > 10.0 && *b < 400.0)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

struct VoiceState {
    fifths: i8,
    unit: f64,
    bar_acc: BTreeMap<(usize, i32), i8>,
    /// Notes left in a tuplet and the factor applied to them.
    tuplet: Option<(u32, f64)>,
    /// Factor for the next note after broken rhythm (`>`/`<`).
    broken: Option<f64>,
}

/// Parse one tune (the first `X:` in the text).
pub fn parse(text: &str) -> Result<AbcTune, String> {
    let mut tune = AbcTune { verses: 1, ..Default::default() };
    let mut fifths: i8 = 0;
    let mut unit: Option<f64> = None;
    let mut in_body = false;
    let mut current: usize = 0;
    let mut states: Vec<VoiceState> = Vec::new();
    let mut max_verse = 0u32;

    let voice_index = |tune: &mut AbcTune, states: &mut Vec<VoiceState>, decl: &str, fifths: i8, unit: f64| -> usize {
        let mut words = decl.split_whitespace();
        let id = words.next().unwrap_or("1").to_string();
        let rest = decl.to_ascii_lowercase();
        let bass = rest.contains("clef=bass") || rest.split_whitespace().any(|w| w == "bass");
        if let Some(i) = tune.voices.iter().position(|v| v.id == id) {
            if bass {
                tune.voices[i].bass_clef = true;
            }
            return i;
        }
        tune.voices.push(Voice { id, bass_clef: bass, items: Vec::new() });
        states.push(VoiceState { fifths, unit, bar_acc: BTreeMap::new(), tuplet: None, broken: None });
        tune.voices.len() - 1
    };

    for raw in text.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix('%') {
            // Comment fields such as %OHCOMPOSER; %% directives are ignored.
            if !rest.starts_with('%') {
                if let Some((k, v)) = rest.split_once(char::is_whitespace) {
                    if k.chars().all(|c| c.is_ascii_uppercase()) && !k.is_empty() {
                        tune.fields.insert(k.to_string(), v.trim().to_string());
                    }
                }
            }
            continue;
        }
        if line.trim().is_empty() {
            if in_body && !tune.voices.iter().all(|v| v.items.is_empty()) {
                // A blank line ends the tune.
                break;
            }
            continue;
        }
        let field = line.len() >= 2 && line.as_bytes()[1] == b':' && line.as_bytes()[0].is_ascii_alphabetic();
        if field && !line.starts_with("[") {
            let (k, v) = (line.as_bytes()[0] as char, line[2..].trim());
            match k {
                'X' if in_body || !tune.title.is_empty() && !tune.voices.is_empty() => break,
                'T' if tune.title.is_empty() => tune.title = v.split('%').next().unwrap_or("").trim().to_string(),
                'C' => tune.credits.push(v.to_string()),
                'S' => tune.source.push(v.to_string()),
                'L' => {
                    let u = fraction(v.split('%').next().unwrap_or("")).ok_or_else(|| format!("bad L: {v:?}"))?;
                    if in_body {
                        if let Some(s) = states.get_mut(current) {
                            s.unit = u;
                        }
                    } else {
                        unit = Some(u);
                    }
                }
                'M' => {
                    let m = v.split('%').next().unwrap_or("").trim();
                    let meter = match m {
                        "C" => Some((4, 4)),
                        "C|" => Some((2, 2)),
                        _ => {
                            m.split_once('/').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
                        }
                    };
                    if in_body {
                        if meter != tune.meter {
                            tune.meter_changes = true;
                        }
                    } else {
                        tune.meter = meter;
                    }
                }
                'Q' => {
                    if tune.bpm.is_none() {
                        tune.bpm = parse_tempo(v, unit.unwrap_or(0.125));
                    }
                }
                'K' => {
                    let (f, minor) = parse_key(v)?;
                    if in_body {
                        if let Some(s) = states.get_mut(current) {
                            s.fifths = f;
                        }
                    } else {
                        fifths = f;
                        tune.key = key_name(f, minor);
                        in_body = true;
                        let u = unit.unwrap_or(default_unit(tune.meter));
                        for s in &mut states {
                            s.fifths = f;
                            s.unit = u;
                        }
                    }
                }
                'V' => {
                    let u = unit.unwrap_or(default_unit(tune.meter));
                    current = voice_index(&mut tune, &mut states, v, fifths, u);
                }
                'w' | 'W' => {
                    let t = v.trim_start_matches(|c: char| c == '~' || c.is_whitespace());
                    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty() && t[digits.len()..].starts_with('.') {
                        max_verse = max_verse.max(digits.parse().unwrap_or(0));
                    }
                }
                _ => {}
            }
            continue;
        }
        if !in_body {
            continue;
        }
        // A music line, perhaps starting with an inline voice field.
        let mut music = line.split('%').next().unwrap_or("").to_string();
        if let Some(rest) = music.trim_start().strip_prefix("[V:") {
            let (decl, after) = rest.split_once(']').ok_or("unclosed [V:")?;
            let u = unit.unwrap_or(default_unit(tune.meter));
            current = voice_index(&mut tune, &mut states, decl, fifths, u);
            music = after.to_string();
        }
        if tune.voices.is_empty() {
            let u = unit.unwrap_or(default_unit(tune.meter));
            current = voice_index(&mut tune, &mut states, "1", fifths, u);
        }
        parse_music(&music, &mut tune, &mut states, current)?;
    }
    if tune.voices.iter().all(|v| v.items.is_empty()) {
        return Err("no music found".into());
    }
    if tune.key.is_empty() {
        tune.key = "C".into();
    }
    tune.verses = max_verse.clamp(1, 12);
    Ok(tune)
}

fn default_unit(meter: Option<(u32, u32)>) -> f64 {
    match meter {
        Some((a, b)) if (a as f64) / (b as f64) < 0.75 => 1.0 / 16.0,
        _ => 1.0 / 8.0,
    }
}

fn key_name(fifths: i8, minor: bool) -> String {
    let i = (fifths + 7) as usize;
    if minor {
        format!("{}m", MINOR[i])
    } else {
        MAJOR[i].to_string()
    }
}

fn parse_music(line: &str, tune: &mut AbcTune, states: &mut [VoiceState], mut v: usize) -> Result<(), String> {
    let ch: Vec<char> = line.chars().collect();
    let n = ch.len();
    let mut i = 0;
    let skip_to =
        |i: usize, end: char| -> usize { ch[i + 1..].iter().position(|c| *c == end).map_or(n, |p| i + 2 + p) };

    while i < n {
        let c = ch[i];
        match c {
            // A dotted tie (".-") ties in some verses only. The tune follows
            // the words, so it plays the note again; lower parts hold on.
            '.' if ch.get(i + 1) == Some(&'-') && v == 0 => i += 2,
            ' ' | '\t' | '`' | '\\' | '$' | 'y' | ')' | '.' | '~' | 'H' | 'L' | 'M' | 'O' | 'P' | 'S' | 'T' | 'u'
            | 'v' => i += 1,
            '"' => i = skip_to(i, '"'),
            '!' => i = skip_to(i, '!'),
            '+' => i = skip_to(i, '+'),
            '{' => i = skip_to(i, '}'),
            '&' => return Err("voice overlay (&) isn't supported".into()),
            '(' => {
                if i + 1 < n && ch[i + 1].is_ascii_digit() {
                    let mut j = i + 1;
                    let mut nums = Vec::new();
                    let mut cur = String::new();
                    while j < n && (ch[j].is_ascii_digit() || ch[j] == ':') {
                        if ch[j] == ':' {
                            nums.push(cur.parse::<u32>().ok());
                            cur.clear();
                        } else {
                            cur.push(ch[j]);
                        }
                        j += 1;
                    }
                    nums.push(cur.parse::<u32>().ok());
                    let p = nums[0].unwrap_or(3);
                    let q = nums.get(1).copied().flatten().unwrap_or(match p {
                        2 | 4 | 8 => 3,
                        _ => 2,
                    });
                    let r = nums.get(2).copied().flatten().unwrap_or(p);
                    states[v].tuplet = Some((r, q as f64 / p as f64));
                    i = j;
                } else {
                    // A slur, perhaps with a direction mark: (' or (,
                    i += 1;
                    while i < n && (ch[i] == '\'' || ch[i] == ',') {
                        i += 1;
                    }
                }
            }
            '-' => {
                if let Some(Item::Note { tie, .. }) = tune.voices[v].items.last_mut() {
                    *tie = true;
                }
                i += 1;
            }
            '>' | '<' => {
                let mut k = 0;
                while i < n && ch[i] == c {
                    k += 1;
                    i += 1;
                }
                let short = 0.5f64.powi(k);
                let (prev, next) = if c == '>' { (2.0 - short, short) } else { (short, 2.0 - short) };
                match tune.voices[v].items.last_mut() {
                    Some(Item::Note { beats, .. }) | Some(Item::Rest(beats)) => *beats *= prev,
                    _ => {}
                }
                states[v].broken = Some(next);
            }
            '[' => {
                let next = ch.get(i + 1).copied().unwrap_or(' ');
                if next.is_ascii_digit() {
                    let d = next.to_digit(10).unwrap() as u8;
                    tune.voices[v].items.push(Item::Ending(d));
                    i += 2;
                } else if next == '|' {
                    push_bar(&mut tune.voices[v].items, &mut states[v]);
                    i += 2;
                } else if next.is_ascii_alphabetic() && ch.get(i + 2) == Some(&':') {
                    let end = ch[i..].iter().position(|c| *c == ']').map_or(n, |p| i + p);
                    let body: String = ch[i + 3..end.min(n)].iter().collect();
                    inline_field(next, &body, tune, states, &mut v)?;
                    i = end + 1;
                } else {
                    // Chord.
                    let end = ch[i..].iter().position(|c| *c == ']').map(|p| i + p).ok_or("unclosed chord")?;
                    let mut keys = Vec::new();
                    let mut first_len: Option<f64> = None;
                    let mut tie = false;
                    let mut j = i + 1;
                    while j < end {
                        let cj = ch[j];
                        if is_note_start(cj) {
                            let (key, len, nj) = read_note(&ch, j, &mut states[v])?;
                            keys.push(key);
                            first_len.get_or_insert(len);
                            j = nj;
                        } else {
                            if cj == '-' {
                                tie = true;
                            }
                            j += 1;
                        }
                    }
                    let (mult, nj) = read_length(&ch, end + 1);
                    i = nj;
                    if i < n && ch[i] == '-' {
                        tie = true;
                        i += 1;
                    }
                    if keys.is_empty() {
                        continue;
                    }
                    let beats = finish_len(first_len.unwrap_or(1.0) * mult, &mut states[v]);
                    tune.voices[v].items.push(Item::Note { keys, beats, tie });
                }
            }
            '|' | ':' => {
                let mut j = i;
                while j < n && (ch[j] == '|' || ch[j] == ':' || (ch[j] == ']' && j > i && ch[j - 1] == '|')) {
                    j += 1;
                }
                let tok: String = ch[i..j].iter().collect();
                let items = &mut tune.voices[v].items;
                if tok.starts_with(':') {
                    push_bar(items, &mut states[v]);
                    items.push(Item::RepeatEnd);
                } else {
                    push_bar(items, &mut states[v]);
                }
                if tok.len() > 1 && tok.ends_with(':') {
                    items.push(Item::RepeatStart);
                }
                if j < n && ch[j].is_ascii_digit() && tok.ends_with('|') {
                    items.push(Item::Ending(ch[j].to_digit(10).unwrap() as u8));
                    j += 1;
                }
                i = j;
            }
            'z' | 'x' => {
                let (mult, nj) = read_length(&ch, i + 1);
                let beats = finish_len(states[v].unit * 4.0 * mult, &mut states[v]);
                tune.voices[v].items.push(Item::Rest(beats));
                i = nj;
            }
            'Z' => {
                let (mult, nj) = read_length(&ch, i + 1);
                let bar = tune.meter.map_or(4.0, |(a, b)| a as f64 * 4.0 / b as f64);
                tune.voices[v].items.push(Item::Rest(bar * mult));
                i = nj;
            }
            _ if is_note_start(c) => {
                let (key, len, nj) = read_note(&ch, i, &mut states[v])?;
                let beats = finish_len(len, &mut states[v]);
                tune.voices[v].items.push(Item::Note { keys: vec![key], beats, tie: false });
                i = nj;
            }
            _ => return Err(format!("unexpected {c:?} in {line:?}")),
        }
    }
    Ok(())
}

fn push_bar(items: &mut Vec<Item>, state: &mut VoiceState) {
    state.bar_acc.clear();
    if !matches!(items.last(), Some(Item::Bar) | None) {
        items.push(Item::Bar);
    }
}

fn inline_field(
    k: char,
    body: &str,
    tune: &mut AbcTune,
    states: &mut [VoiceState],
    v: &mut usize,
) -> Result<(), String> {
    match k {
        'K' => states[*v].fifths = parse_key(body)?.0,
        'L' => {
            if let Some(u) = fraction(body) {
                states[*v].unit = u;
            }
        }
        'M' => {
            let m = body.trim();
            let meter = m.split_once('/').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)));
            if meter != tune.meter {
                tune.meter_changes = true;
            }
        }
        'Q' => {
            if tune.bpm.is_none() {
                tune.bpm = parse_tempo(body, states[*v].unit);
            }
        }
        'V' => {
            let id = body.split_whitespace().next().unwrap_or("");
            if let Some(i) = tune.voices.iter().position(|x| x.id == id) {
                *v = i;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_note_start(c: char) -> bool {
    matches!(c, '^' | '_' | '=' | 'A'..='G' | 'a'..='g')
}

/// A note's MIDI key and length in crotchets (before tuplets), and the index after it.
fn read_note(ch: &[char], mut i: usize, st: &mut VoiceState) -> Result<(u8, f64, usize), String> {
    let mut acc: Option<i8> = None;
    while i < ch.len() && matches!(ch[i], '^' | '_' | '=') {
        let a = match ch[i] {
            '^' => 1,
            '_' => -1,
            _ => 0,
        };
        acc = Some(if ch[i] == '=' { 0 } else { acc.unwrap_or(0) + a });
        i += 1;
    }
    let letter = *ch.get(i).ok_or("accidental without a note")?;
    if !letter.is_ascii_alphabetic() || !matches!(letter.to_ascii_uppercase(), 'A'..='G') {
        return Err(format!("bad note {letter:?}"));
    }
    i += 1;
    let mut octave: i32 = if letter.is_ascii_lowercase() { 5 } else { 4 };
    while i < ch.len() && (ch[i] == '\'' || ch[i] == ',') {
        octave += if ch[i] == '\'' { 1 } else { -1 };
        i += 1;
    }
    let li = letter_index(letter);
    let alter = match acc {
        Some(a) => {
            st.bar_acc.insert((li, octave), a);
            a
        }
        None => st.bar_acc.get(&(li, octave)).copied().unwrap_or(signature(st.fifths)[li]),
    };
    let key = 12 * (octave + 1) + SEMITONE[li] + alter as i32;
    let key = u8::try_from(key).ok().filter(|k| *k <= 127).ok_or("note out of range")?;
    let (mult, i) = read_length(ch, i);
    Ok((key, st.unit * 4.0 * mult, i))
}

/// A length multiplier such as `2`, `3/2`, `/`, `//`, `/4`.
fn read_length(ch: &[char], mut i: usize) -> (f64, usize) {
    let mut num = String::new();
    while i < ch.len() && ch[i].is_ascii_digit() {
        num.push(ch[i]);
        i += 1;
    }
    let mut value: f64 = num.parse().unwrap_or(1.0);
    let mut slashes = 0;
    let mut den = String::new();
    while i < ch.len() && ch[i] == '/' {
        slashes += 1;
        i += 1;
        while i < ch.len() && ch[i].is_ascii_digit() {
            den.push(ch[i]);
            i += 1;
        }
    }
    if slashes > 0 {
        value /= match den.parse::<f64>() {
            Ok(d) if slashes == 1 => d,
            _ => 2f64.powi(slashes),
        };
    }
    (value, i)
}

fn finish_len(mut beats: f64, st: &mut VoiceState) -> f64 {
    if let Some(f) = st.broken.take() {
        beats *= f;
    }
    if let Some((left, f)) = st.tuplet {
        beats *= f;
        st.tuplet = if left > 1 { Some((left - 1, f)) } else { None };
    }
    beats
}

/// Play repeats out: `|: A :|` becomes A A, with first and second endings.
pub fn expand(items: &[Item]) -> Vec<Item> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut taken = BTreeSet::new();
    let mut second = false;
    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            Item::RepeatStart => {
                start = i + 1;
                second = false;
            }
            Item::Ending(1) if second => {
                // Skip the first ending: on to the next ending marker.
                match items[i + 1..].iter().position(|x| matches!(x, Item::Ending(n) if *n >= 2)) {
                    Some(p) => i += p + 1,
                    None => i = items.len(),
                }
                continue;
            }
            Item::Ending(_) => {}
            Item::RepeatEnd => {
                if taken.insert(i) {
                    i = start;
                    second = true;
                    continue;
                }
                start = i + 1;
                second = false;
            }
            other => out.push(other.clone()),
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// To a Cecilia score
// ---------------------------------------------------------------------------

fn fmt_beats(b: f64) -> String {
    for d in [1.0, 2.0, 4.0, 8.0, 16.0] {
        if ((b * d).round() - b * d).abs() < 1e-9 {
            let s = format!("{:.4}", b);
            return s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
    }
    for d in [3.0, 6.0, 12.0, 24.0, 48.0, 5.0, 10.0, 7.0] {
        if ((b * d).round() - b * d).abs() < 1e-6 {
            return format!("{}/{}", (b * d).round() as i64, d as i64);
        }
    }
    format!("{b}")
}

fn pitch_name(key: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
    format!("{}{}", NAMES[(key % 12) as usize], key as i32 / 12 - 1)
}

/// One voice in score notation, with bar lines.
fn voice_notes(items: &[Item]) -> String {
    let mut out: Vec<String> = Vec::new();
    for it in items {
        match it {
            Item::Note { keys, beats, tie } => {
                let tie = if *tie { "~" } else { "" };
                if keys.len() == 1 {
                    out.push(format!("{}:{}{tie}", pitch_name(keys[0]), fmt_beats(*beats)));
                } else {
                    let ps: Vec<String> = keys.iter().map(|k| pitch_name(*k)).collect();
                    out.push(format!("[{}]:{}{tie}", ps.join(" "), fmt_beats(*beats)));
                }
            }
            Item::Rest(b) => out.push(format!("r:{}", fmt_beats(*b))),
            Item::Bar if out.last().is_some_and(|l| l != "|") => out.push("|".into()),
            _ => {}
        }
    }
    while out.last().is_some_and(|l| l == "|") {
        out.pop();
    }
    out.join(" ")
}

fn length(items: &[Item]) -> f64 {
    items
        .iter()
        .map(|i| match i {
            Item::Note { beats, .. } | Item::Rest(beats) => *beats,
            _ => 0.0,
        })
        .sum()
}

/// Beats before the first bar line, if it's a short (pickup) bar.
fn pickup(items: &[Item], bar: f64) -> f64 {
    let mut t = 0.0;
    for it in items {
        match it {
            Item::Note { beats, .. } | Item::Rest(beats) => t += beats,
            Item::Bar => break,
            _ => {}
        }
    }
    if t > 1e-6 && t < bar - 1e-6 {
        t
    } else {
        0.0
    }
}

fn lowest(notes: &str) -> Option<u8> {
    notes
        .split(|c: char| c.is_whitespace() || c == '[' || c == ']')
        .filter_map(|t| t.split(':').next())
        .filter(|p| !p.is_empty() && *p != "r" && *p != "|")
        .filter_map(|p| crate::sample::parse_pitch(p).ok())
        .min()
}

fn transpose(notes: &str, by: i32) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let (p, rest) = match word.split_once(':') {
            Some((p, r)) => (p.to_string(), format!(":{r}")),
            None => (word.clone(), String::new()),
        };
        match crate::sample::parse_pitch(&p) {
            Ok(k) if p != "r" => out.push_str(&format!("{}{rest}", pitch_name((k as i32 + by).clamp(0, 127) as u8))),
            _ => out.push_str(word),
        }
        word.clear();
    };
    for c in notes.chars() {
        if c.is_whitespace() || c == '[' || c == ']' {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A plain hymn registration that the virtual organist maps onto any organ.
pub fn hymn_registration() -> Vec<String> {
    [
        "great principal 8",
        "great flute 8",
        "great principal 4",
        "swell flute 8",
        "swell principal 4",
        "swell to great",
        "pedal flute 16",
        "pedal principal 8",
        "great to pedal",
    ]
    .map(String::from)
    .to_vec()
}

/// Turn a parsed tune into a score. The lowest voice goes to the pedals when
/// there are three or more voices and it never has chords; the rest are
/// played on the Great.
pub fn to_score(tune: &AbcTune, id: &str) -> Result<Score, String> {
    let voices: Vec<Vec<Item>> = tune.voices.iter().map(|v| expand(&v.items)).filter(|v| length(v) > 0.0).collect();
    if voices.is_empty() {
        return Err("no notes".into());
    }
    let (beats, unit) = tune.meter.unwrap_or((4, 4));
    let bar = beats as f64 * 4.0 / unit as f64;
    let pedal_ok = voices.len() >= 3
        && voices.last().is_some_and(|v| v.iter().all(|i| !matches!(i, Item::Note { keys, .. } if keys.len() > 1)));
    let parts: Vec<Part> = voices
        .iter()
        .enumerate()
        .map(|(i, items)| Part {
            division: if pedal_ok && i == voices.len() - 1 { DivisionRole::Pedal } else { DivisionRole::Great },
            notes: voice_notes(items),
        })
        .collect();

    // Manual parts below the keyboards' bottom C go up an octave.
    let mut parts = parts;
    for p in &mut parts {
        if p.division == DivisionRole::Great && lowest(&p.notes).is_some_and(|k| k < 36) {
            p.notes = transpose(&p.notes, 12);
        }
    }
    let verses = tune.verses;
    let mut verse_registration = BTreeMap::new();
    if verses >= 3 {
        verse_registration.insert(verses, vec!["+great principal 2".to_string(), "+swell reed 8".to_string()]);
    }
    let mut score = Score {
        id: id.to_string(),
        title: tune.title.clone(),
        composer: String::new(),
        melody: String::new(),
        copyright: String::new(),
        publication: String::new(),
        licence: String::new(),
        source: String::new(),
        source_url: String::new(),
        tags: Vec::new(),
        other_titles: Vec::new(),
        bpm: tune.bpm.unwrap_or(90.0).round(),
        time: format!("{beats}/{unit}"),
        key: tune.key.clone(),
        pickup: if tune.meter.is_some() { pickup(&voices[0], bar) } else { 0.0 },
        verses,
        gap: 2.0,
        registration: hymn_registration(),
        verse_registration,
        expression: BTreeMap::from([(DivisionRole::Swell, 90)]),
        parts,
    };
    // Keep the bar lines when they check out; free-rhythm or irregular
    // music plays the same without them.
    if tune.meter.is_none() || tune.meter_changes || score.notes().is_err() {
        for p in &mut score.parts {
            p.notes = p.notes.split_whitespace().filter(|t| *t != "|").collect::<Vec<_>>().join(" ");
        }
        score.pickup = 0.0;
    }
    score.notes()?;
    Ok(score)
}

// ---------------------------------------------------------------------------
// Open Hymnal credits and copyright
// ---------------------------------------------------------------------------

pub const OPEN_HYMNAL: &str = "Open Hymnal Project";
pub const OPEN_HYMNAL_URL: &str = "https://openhymnal.org";

/// "Monk, William Henry (1823-1889)" → "William Henry Monk (1823–1889)".
pub fn person(field: &str) -> String {
    let field = field.trim();
    let (name, dates) = match field.find('(') {
        Some(p) => (field[..p].trim(), field[p..].replace('-', "–")),
        None => (field, String::new()),
    };
    let name = match name.split_once(", ") {
        Some((last, first)) if !name.starts_with("from ") => format!("{first} {last}"),
        _ => name.trim_start_matches("from ").to_string(),
    };
    format!("{name} {dates}").trim().to_string()
}

/// Everyone named in a credit field ("A (dates);B (dates)"), readably:
/// "A (dates) and B (dates)". Traditional sources read "Traditional German";
/// "unknown", "none" and "composite" add nothing.
fn people(field: &str) -> String {
    let names: Vec<String> = field
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let low = p.to_ascii_lowercase();
            if ["unknown", "none", "composite"].contains(&low.as_str()) {
                None
            } else if is_traditional(p) {
                let word = p.trim_start_matches(|c: char| !c.is_alphabetic());
                Some(if low.starts_with("traditional") { "Traditional".into() } else { format!("Traditional {word}") })
            } else {
                Some(person(p))
            }
        })
        .collect();
    names.join(" and ")
}

fn is_traditional(field: &str) -> bool {
    let f = field.trim().to_ascii_lowercase();
    let words = [
        "traditional",
        "unknown",
        "none",
        "english",
        "german",
        "french",
        "latin",
        "welsh",
        "dutch",
        "silesian",
        "yoruba",
        "irish",
        "scottish",
        "swedish",
        "czech",
        "polish",
        "american",
        "plainsong",
        "gaelic",
        "italian",
        "spanish",
        "danish",
        "norwegian",
        "finnish",
        "hebrew",
        "russian",
        "bohemian",
        "basque",
        "catalan",
        "african-american",
        "negro spiritual",
        "spiritual",
        "shaker",
        "folk",
        "genevan",
        "medieval",
        "plainchant",
        "gregorian",
    ];
    f.is_empty() || words.iter().any(|w| f == *w || f.starts_with(&format!("{w} ")))
}

/// Why a hymn's music can't be shared worldwide (life + 70 years), or `None`.
/// `cutoff` is the latest year of death that's out of copyright everywhere
/// (the current year minus 71).
pub fn open_hymnal_problem(tune: &AbcTune, cutoff: i32) -> Option<String> {
    let credits = tune.credits.join(" ").to_ascii_lowercase();
    if !credits.contains("copyright: public domain") {
        return Some("not marked public domain".into());
    }
    for key in ["OHCOMPOSER", "OHARRANGER"] {
        let who = if key == "OHCOMPOSER" { "tune" } else { "harmony" };
        let whole = tune.fields.get(key).map(String::as_str).unwrap_or("");
        for field in whole.split(';').map(str::trim) {
            if let Some(problem) = credit_problem(who, field, cutoff) {
                return Some(problem);
            }
        }
    }
    None
}

/// People named without dates in the Open Hymnal, and when they died.
const KNOWN_DEATHS: [(&str, i32); 2] = [("Erythraeus", 1617), ("Von Tucher", 1877)];

fn credit_problem(who: &str, field: &str, cutoff: i32) -> Option<String> {
    {
        if is_traditional(field) || field.eq_ignore_ascii_case("composite") {
            return None;
        }
        if let Some((_, died)) = KNOWN_DEATHS.iter().find(|(n, _)| field.eq_ignore_ascii_case(n)) {
            return (*died > cutoff).then(|| format!("{who}: {field} died in {died}"));
        }
        // Only parentheses with years in them: "(or Edwin)" is part of a name.
        let groups: Vec<&str> = field
            .split('(')
            .skip(1)
            .filter_map(|g| g.split(')').next())
            .filter(|g| g.chars().filter(char::is_ascii_digit).count() >= 4)
            .collect();
        if groups.is_empty() {
            // "from Klug's Geistliche Lieder, Wittenberg, 1533": a book's year.
            let years: Vec<i32> = field
                .split(|c: char| !c.is_ascii_digit())
                .filter(|w| w.len() == 4)
                .filter_map(|w| w.parse().ok())
                .collect();
            if field.starts_with("from ") && !years.is_empty() && years.iter().all(|y| *y <= cutoff) {
                return None;
            }
            return Some(format!("{who}: no dates for {field:?}"));
        }
        for g in groups {
            let parts: Vec<&str> = g.split(['-', '–']).collect();
            let died = parts.last().and_then(|p| p.split(|c: char| !c.is_ascii_digit()).rfind(|w| w.len() == 4));
            match died.and_then(|d| d.parse::<i32>().ok()) {
                Some(y) if parts.len() > 1 || g.contains("d.") => {
                    if y > cutoff {
                        return Some(format!(
                            "{who}: {} died in {y}, in copyright in some countries until {}",
                            person(field),
                            y + 71
                        ));
                    }
                }
                _ => {
                    // Born long enough ago that they can't have died since the cutoff.
                    let born =
                        parts.first().and_then(|p| p.split(|c: char| !c.is_ascii_digit()).find(|w| w.len() == 4));
                    match born.and_then(|b| b.parse::<i32>().ok()) {
                        Some(b) if b + 105 <= cutoff => {}
                        _ => return Some(format!("{who}: can't tell when {} died", person(field))),
                    }
                }
            }
        }
    }
    None
}

/// Hymn-book occasions to the Cecilia library's tags.
fn tags_for(category: &str) -> Vec<String> {
    let c = category.to_ascii_uppercase();
    let map = [
        ("CHRISTMAS", "christmas"),
        ("ADVENT", "advent"),
        ("EPIPHANY", "epiphany"),
        ("LENT", "lent"),
        ("PALM SUNDAY", "lent"),
        ("GOOD FRIDAY", "lent"),
        ("GOOD THURSDAY", "lent"),
        ("EASTER", "easter"),
        ("ASCENSION", "ascension"),
        ("PENTECOST", "pentecost"),
        ("TRINITY", "trinity"),
        ("MORNING", "morning"),
        ("EVENING", "evening"),
        ("FUNERAL", "funeral"),
        ("HEAVEN", "funeral"),
        ("THANKSGIVING", "harvest"),
        ("MISSION", "mission"),
        ("PRAISE", "praise"),
        ("LORD'S SUPPER", "communion"),
        ("BAPTISM", "baptism"),
        ("CHRISTIAN HOME", "wedding"),
    ];
    let mut tags = vec!["hymn".to_string()];
    for (k, t) in map {
        if c.contains(k) && !tags.iter().any(|x| x == t) {
            tags.push(t.to_string());
        }
    }
    tags
}

/// The tune's name, e.g. "EVENTIDE", from `C: … Music: 'Eventide' …`.
pub fn tune_name(tune: &AbcTune, file_stem: &str) -> String {
    let credits = tune.credits.join(" ").replace('`', "'");
    if let Some(p) = credits.find("Music") {
        let after = &credits[p..];
        if let Some(q) = after.find('\'') {
            if let Some(end) = after[q + 1..].find('\'') {
                let name = &after[q + 1..q + 1 + end];
                if !name.trim().is_empty() && name.len() < 60 && !name.trim().eq_ignore_ascii_case("untitled") {
                    return name.trim().to_uppercase();
                }
            }
        }
    }
    let from_file = file_stem.split_once('-').map_or(file_stem, |(_, t)| t).replace(['_', '-'], " ");
    if from_file.trim().eq_ignore_ascii_case("untitled") {
        return tune.title.to_uppercase();
    }
    from_file.to_uppercase()
}

/// A score from one Open Hymnal file, with its credits. Fails with the
/// reason when it can't be shared worldwide or can't be converted.
pub fn from_open_hymnal(text: &str, file_stem: &str, cutoff: i32) -> Result<Score, String> {
    let tune = parse(text)?;
    if let Some(problem) = open_hymnal_problem(&tune, cutoff) {
        return Err(problem);
    }
    let melody = tune_name(&tune, file_stem);
    let id = slug(&melody);
    let mut score = to_score(&tune, &id)?;
    score.melody = melody;
    let composer = people(tune.fields.get("OHCOMPOSER").map(String::as_str).unwrap_or(""));
    let arranger = people(tune.fields.get("OHARRANGER").map(String::as_str).unwrap_or(""));
    score.composer = match (composer.as_str(), arranger.as_str()) {
        ("", "") => "Traditional".into(),
        (c, a) if a.is_empty() || a == c => c.to_string(),
        ("", a) => format!("harmony by {a}"),
        (c, a) => format!("{c}; harmony by {a}"),
    };
    score.copyright = "Public domain".into();
    score.licence = "public-domain".into();
    score.publication =
        tune.source.first().map(|s| s.trim_start_matches("Music source:").trim().to_string()).unwrap_or_default();
    score.source = OPEN_HYMNAL.into();
    score.source_url = OPEN_HYMNAL_URL.into();
    score.tags = tags_for(tune.fields.get("OHCATEGORY").map(String::as_str).unwrap_or(""));
    Ok(score)
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        // German and other accented letters: "Wasserflüssen" → "wasserfluessen".
        let plain = match c {
            'ä' | 'æ' => "ae",
            'ö' | 'ø' | 'œ' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            'à' | 'á' | 'â' | 'ã' | 'å' => "a",
            'è' | 'é' | 'ê' | 'ë' => "e",
            'ì' | 'í' | 'î' | 'ï' => "i",
            'ò' | 'ó' | 'ô' | 'õ' => "o",
            'ù' | 'ú' | 'û' => "u",
            'ç' => "c",
            'ñ' => "n",
            'ý' | 'ÿ' => "y",
            _ => "",
        };
        if !plain.is_empty() {
            out.push_str(plain);
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTIDE: &str = r#"%OHCATEGORY CROSS AND COMFORT
%OHCOMPOSER Monk, William Henry (1823-1889)
%OHARRANGER Monk, William Henry (1823-1889)
X: 1
T: Abide With Me
C: Words: Henry F. Lyte, 1847.  Music: 'Eventide' William H. Monk, 1861.
C: copyright: public domain.  This score is a part of the Open Hymnal Project.
S: Music source: Hymns Ancient and Modern, 1869 hymn 14.
M: 4/4 % time signature
L: 1/4 % default length
V: S1V1 clef=treble
V: S1V2
V: S2V1 clef=bass
V: S2V2
K: Eb % key signature
[V: S1V1] [Q:1/4=100] G2 G F | E2 B2 |
w: 1.~A- bide with me; fast falls the
w: 2.~Swift to its close ebbs out life's
[V: S1V2]  E2 D D | E2 E2 |
[V: S2V1]  B,2 B, A, | G,2 E,2 |
[V: S2V2]  E,2 B,, B,, | C,2 G,,2 |]
W: 3.I need Thy presence every passing hour.
"#;

    #[test]
    fn reads_an_open_hymnal_hymn() {
        let t = parse(EVENTIDE).unwrap();
        assert_eq!(t.title, "Abide With Me");
        assert_eq!(t.key, "Eb");
        assert_eq!(t.voices.len(), 4);
        assert_eq!(t.verses, 3);
        assert_eq!(t.bpm, Some(100.0));
        let s = to_score(&t, "eventide").unwrap();
        assert_eq!(s.parts[0].notes, "G4:2 G4:1 F4:1 | Eb4:2 Bb4:2");
        assert_eq!(s.parts[3].division, DivisionRole::Pedal);
        assert_eq!(s.parts[3].notes, "Eb3:2 Bb2:1 Bb2:1 | C3:2 G2:2");
        s.compile().unwrap();
    }

    #[test]
    fn credits_and_copyright() {
        let s = from_open_hymnal(EVENTIDE, "Abide_With_Me-Eventide", 1955).unwrap();
        assert_eq!(s.id, "eventide");
        assert_eq!(s.melody, "EVENTIDE");
        assert_eq!(s.composer, "William Henry Monk (1823–1889)");
        assert_eq!(s.publication, "Hymns Ancient and Modern, 1869 hymn 14.");
        assert_eq!(s.source, "Open Hymnal Project");

        let vw = EVENTIDE
            .replace("%OHARRANGER Monk, William Henry (1823-1889)", "%OHARRANGER Vaughan Williams, Ralph (1872-1958)");
        let err = from_open_hymnal(&vw, "x-y", 1955).unwrap_err();
        assert!(err.contains("1958"), "{err}");
        let book = EVENTIDE.replace(
            "Monk, William Henry (1823-1889)\n%OHARRANGER",
            "from Klug's Geistliche Lieder, Wittenberg, 1533\n%OHARRANGER",
        );
        assert!(from_open_hymnal(&book, "x-y", 1955).is_ok());
        let living =
            EVENTIDE.replace("%OHARRANGER Monk, William Henry (1823-1889)", "%OHARRANGER Dumont, Brian James (1971-)");
        assert!(from_open_hymnal(&living, "x-y", 1955).is_err());
    }

    #[test]
    fn credits_with_several_people() {
        assert_eq!(
            people("Kocher, Conrad (1786-1872);Monk, William Henry (1823-1889)"),
            "Conrad Kocher (1786–1872) and William Henry Monk (1823–1889)"
        );
        assert_eq!(people("German"), "Traditional German");
        assert_eq!(people("unknown"), "");
        assert_eq!(slug("An Wasserflüssen Babylon"), "an-wasserfluessen-babylon");
    }

    #[test]
    fn accidentals_last_the_bar() {
        let t = parse("X:1\nL:1/4\nM:4/4\nK:C\n^F F =F F | F4 |\n").unwrap();
        let keys: Vec<u8> = t.voices[0]
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Note { keys, .. } => Some(keys[0]),
                _ => None,
            })
            .collect();
        assert_eq!(keys, vec![66, 66, 65, 65, 65]);
    }

    #[test]
    fn repeats_endings_tuplets_and_broken_rhythm() {
        let t = parse("X:1\nL:1/4\nM:2/4\nK:G\n|: G A |1 B2 :|2 c2 |] (3d/e/f/ g | a>b |\n").unwrap();
        let s = to_score(&t, "x").unwrap();
        assert_eq!(
            s.parts[0].notes,
            "G4:1 A4:1 | B4:2 | G4:1 A4:1 | C5:2 | D5:1/3 E5:1/3 F#5:1/3 G5:1 | A5:1.5 B5:0.5"
        );
    }

    #[test]
    fn chords_ties_and_modes() {
        let t = parse("X:1\nT:Test\nL:1/8\nM:3/4\nK:D mix\n[CEG]2- [CEG]2 z2 | c6 |\n").unwrap();
        assert_eq!(t.key, "G");
        let s = to_score(&t, "x").unwrap();
        assert_eq!(s.parts[0].notes, "[C4 E4 G4]:1~ [C4 E4 G4]:1 r:1 | C5:3");
        s.compile().unwrap();
    }
}
