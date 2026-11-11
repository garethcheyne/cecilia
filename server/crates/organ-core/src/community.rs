//! The shared Cecilia library (the `cecilia-library` repository): checking
//! contributions and building the catalogue that Cecilia installs browse.
//!
//! Layout:
//!
//! ```text
//! music/**/<id>.score.json     scores in the sample format (licence required)
//! music/**/<id>.track.json     recordings (info.copyright holds the licence)
//! organs/**/<id>.organ.json    organ definitions, each with
//! organs/**/<id>.meta.json     a description: maker, model, how far it's tested
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::organ::{builtin_organs, OrganDefinition};
use crate::organist::Organist;
use crate::sample::Score;
use crate::track::{Track, TrackEvent};

pub const CATALOGUE_SCHEMA: u32 = 1;

/// How far a shared organ definition can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Worked out from a manual or guessed; stop numbers may be wrong.
    Template,
    /// Every stop captured with MIDI learn on a real organ.
    Learned,
    /// Learned, and a recording has been played back on the organ.
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganMeta {
    pub maker: String,
    /// The model as the maker names it, e.g. "Studio 170" or "Westmorland 2".
    pub model: String,
    /// The family the model belongs to, e.g. "Studio" or "Opus", for grouping.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub series: String,
    /// What sets this setup apart from others of the same model, e.g.
    /// "with optional Solo division" or "built 2010 onwards (MIDI channel 16)".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub variant: String,
    /// Other names or numbers the same organ is sold under, for searching.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_known_as: Vec<String>,
    /// e.g. "1995–2005".
    #[serde(default)]
    pub years: String,
    pub confidence: Confidence,
    /// Anything to do on the organ first ("set MIDI to mode B").
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub contributor: String,
    /// Always CC0-1.0 in the shared library.
    pub licence: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MusicKind {
    Score,
    Recording,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicEntry {
    pub id: String,
    /// Path within the library, with forward slashes.
    pub path: String,
    pub kind: MusicKind,
    pub title: String,
    pub composer: String,
    pub melody: String,
    pub licence: String,
    pub tags: Vec<String>,
    pub verses: usize,
    pub duration_ms: u32,
    /// Where it came from, for credit (e.g. "Open Hymnal Project").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_url: String,
    /// Other hymns sung to the same tune.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganEntry {
    pub id: String,
    pub path: String,
    pub name: String,
    pub meta: OrganMeta,
    pub divisions: usize,
    pub stops: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Catalogue {
    pub schema: u32,
    pub music: Vec<MusicEntry>,
    pub organs: Vec<OrganEntry>,
}

/// Every note fits a keyboard and the piece makes a sound on each built-in
/// organ (so it'll play on any reasonable instrument).
pub fn check_playable(t: &Track) -> Result<(), String> {
    if t.note_count() == 0 {
        return Err("it has no notes".into());
    }
    for o in builtin_organs() {
        let vo = Organist::new(o.clone());
        for e in &t.events {
            if let TrackEvent::Note { division, key, .. } = e.event {
                let Some(d) = vo.division(division).and_then(|r| o.division(r)) else { continue };
                if !(d.low_key..=d.high_key).contains(&key) {
                    return Err(format!("a note ({key}) is off the {:?} keyboard on {}", division, o.name));
                }
            }
        }
    }
    Ok(())
}

fn files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for p in entries.flatten().map(|e| e.path()) {
            if p.is_dir() {
                out.extend(files(&p, suffix));
            } else if p.to_string_lossy().ends_with(suffix) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn stem(p: &Path, suffix: &str) -> String {
    p.file_name().unwrap_or_default().to_string_lossy().trim_end_matches(suffix).to_string()
}

const LICENCES: [&str; 4] = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "public-domain"];

/// Check every file under `root` and build the catalogue. Problems are
/// returned as readable sentences; the catalogue lists only good files.
pub fn build_catalogue(root: &Path) -> (Catalogue, Vec<String>) {
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    let mut music = Vec::new();
    let mut organs = Vec::new();

    for p in files(&root.join("music"), ".score.json") {
        let path = rel(root, &p);
        let id = stem(&p, ".score.json");
        let parsed = std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| Score::from_json(&t));
        let score = match parsed {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!("{path}: {e}"));
                continue;
            }
        };
        if score.id != id {
            problems.push(format!("{path}: \"id\" should be \"{id}\", the same as the file name"));
            continue;
        }
        if !LICENCES.contains(&score.licence.as_str()) {
            problems.push(format!("{path}: \"licence\" must be one of {}", LICENCES.join(", ")));
            continue;
        }
        let track = match score.compile().and_then(|t| check_playable(&t).map(|_| t)) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{path}: {e}"));
                continue;
            }
        };
        if !ids.insert(format!("music:{id}")) {
            problems.push(format!("{path}: another piece is already called \"{id}\""));
            continue;
        }
        music.push(MusicEntry {
            id,
            path,
            kind: MusicKind::Score,
            title: score.title.clone(),
            composer: score.composer.clone(),
            melody: score.melody.clone(),
            licence: score.licence.clone(),
            tags: score.tags.clone(),
            verses: track.verses.len() + 1,
            duration_ms: track.duration_ms,
            source: score.source.clone(),
            source_url: score.source_url.clone(),
            other_titles: score.other_titles.clone(),
        });
    }

    for p in files(&root.join("music"), ".track.json") {
        let path = rel(root, &p);
        let id = stem(&p, ".track.json");
        let parsed: Result<Track, String> = std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
            .and_then(|t: Track| t.validate().map(|_| t))
            .and_then(|t| check_playable(&t).map(|_| t));
        let t = match parsed {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{path}: {e}"));
                continue;
            }
        };
        if !LICENCES.contains(&t.info.copyright.as_str()) {
            problems.push(format!("{path}: info.copyright must be a licence: one of {}", LICENCES.join(", ")));
            continue;
        }
        if !ids.insert(format!("music:{id}")) {
            problems.push(format!("{path}: another piece is already called \"{id}\""));
            continue;
        }
        music.push(MusicEntry {
            id,
            path,
            kind: MusicKind::Recording,
            title: t.info.title.clone(),
            composer: t.info.composer.clone(),
            melody: t.info.melody.clone(),
            licence: t.info.copyright.clone(),
            tags: Vec::new(),
            verses: t.verses.len() + 1,
            duration_ms: t.duration_ms,
            source: String::new(),
            source_url: String::new(),
            other_titles: Vec::new(),
        });
    }

    for p in files(&root.join("organs"), ".organ.json") {
        let path = rel(root, &p);
        let id = stem(&p, ".organ.json");
        let organ = match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| OrganDefinition::from_json(&t).map_err(|e| e.to_string()))
        {
            Ok(o) => o,
            Err(e) => {
                problems.push(format!("{path}: {e}"));
                continue;
            }
        };
        if organ.id != id {
            problems.push(format!("{path}: \"id\" should be \"{id}\", the same as the file name"));
            continue;
        }
        let meta_path = p.with_file_name(format!("{id}.meta.json"));
        let meta: OrganMeta = match std::fs::read_to_string(&meta_path)
            .map_err(|_| format!("needs a {id}.meta.json beside it (maker, model, confidence, licence)"))
            .and_then(|t| serde_json::from_str(&t).map_err(|e| format!("{id}.meta.json: {e}")))
        {
            Ok(m) => m,
            Err(e) => {
                problems.push(format!("{path}: {e}"));
                continue;
            }
        };
        if meta.licence != "CC0-1.0" {
            problems.push(format!("{path}: organ definitions are shared as CC0-1.0"));
            continue;
        }
        let toggles = organ.stops.iter().any(|s| matches!(s.midi.first(), Some(crate::organ::StopBinding::Toggle { .. })));
        if toggles && organ.all_stops_off.is_none() {
            problems.push(format!("{path}: its stops toggle, so it needs all_stops_off (the general cancel message)"));
            continue;
        }
        if !ids.insert(format!("organ:{id}")) {
            problems.push(format!("{path}: another organ is already called \"{id}\""));
            continue;
        }
        organs.push(OrganEntry {
            id,
            path,
            name: organ.name.clone(),
            meta,
            divisions: organ.divisions.len(),
            stops: organ.stops.len(),
        });
    }

    music.sort_by(|a, b| a.title.cmp(&b.title));
    organs.sort_by(|a, b| (&a.meta.maker, &a.meta.model).cmp(&(&b.meta.maker, &b.meta.model)));
    (Catalogue { schema: CATALOGUE_SCHEMA, music, organs }, problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::SAMPLES;

    #[test]
    fn a_small_library_checks_and_catalogues() {
        let dir = tempfile_dir();
        let music = dir.join("music/hymns");
        let organs = dir.join("organs/makin");
        std::fs::create_dir_all(&music).unwrap();
        std::fs::create_dir_all(&organs).unwrap();
        // A good score (with a licence), and one without.
        let (_, text) = SAMPLES.iter().find(|(n, _)| *n == "crimond").unwrap();
        let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
        v["licence"] = "public-domain".into();
        v["tags"] = serde_json::json!(["hymn", "psalm"]);
        std::fs::write(music.join("crimond.score.json"), v.to_string()).unwrap();
        v["id"] = "no-licence".into();
        v["licence"] = "".into();
        std::fs::write(music.join("no-licence.score.json"), v.to_string()).unwrap();
        // An organ with and without its description.
        let makin = builtin_organs().into_iter().find(|o| o.id == "makin-2m").unwrap();
        std::fs::write(organs.join("makin-2m.organ.json"), serde_json::to_string(&makin).unwrap()).unwrap();
        std::fs::write(
            organs.join("makin-2m.meta.json"),
            r#"{"maker":"Makin","model":"Two-manual (template)","confidence":"template","licence":"CC0-1.0"}"#,
        )
        .unwrap();
        let mut other = makin.clone();
        other.id = "lonely".into();
        std::fs::write(organs.join("lonely.organ.json"), serde_json::to_string(&other).unwrap()).unwrap();

        let (cat, problems) = build_catalogue(&dir);
        assert_eq!(cat.music.len(), 1);
        assert_eq!(cat.music[0].path, "music/hymns/crimond.score.json");
        assert_eq!(cat.music[0].tags, ["hymn", "psalm"]);
        assert_eq!(cat.organs.len(), 1);
        assert_eq!(cat.organs[0].meta.confidence, Confidence::Template);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("licence")));
        assert!(problems.iter().any(|p| p.contains("lonely.meta.json")));
        let _ = std::fs::remove_dir_all(dir);
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cecilia-community-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
