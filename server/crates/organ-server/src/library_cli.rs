//! `cecilia library import openhymnal <abc zip or folder> <library>`: brings
//! Open Hymnal Project hymns into a copy of the shared Cecilia Library.
//!
//! Each hymn is converted to a Cecilia score with its credits, checked to be
//! out of copyright worldwide (life + 70 years for the tune and the harmony)
//! and playable on the built-in organs, then written to `music/hymns/`. A
//! tune that's already in the library is left alone; further hymns sung to a
//! tune are listed as its other titles.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use organ_core::abc;
use organ_core::community::check_playable;
use organ_core::sample::Score;

/// Hymns whose shape a Cecilia score can't show faithfully: repeats that
/// mean "for each verse", Alleluias sung only at the start and end, or parts
/// that disagree with the Open Hymnal's own recording. Checked by comparing
/// every conversion with the project's MIDI files.
const NOT_FAITHFUL: [&str; 8] = [
    "The_Strife_Is_Oer_The_Battle_Done-Victory-Palestrina",
    "Did_You_Think_To_Pray-Stockbridge",
    "Dear_Christians_One_And_All_Rejoice-Nun-Freut-Euch",
    "If_God_Himself_Be_For_Me-Woodbird-Es_Flog_Ein_Kleins_Waldvogelein",
    "Come_Your_Heart_and_Voices_Raising-Quem_Pastores",
    "Were_You_There-Were_You_There",
    "Lord_God_Thy_Praise_We_Sing-Herr_Gott_dich_Loben_Wir",
    "We_All_Believe_In_One_True_God-Wir_Glauben_All_An_Einen_Gott_Schoepfer",
];

/// The title a tune is best known by, where the Open Hymnal's first hymn
/// for it is a less familiar one. The others stay as its other titles.
const BEST_KNOWN: [(&str, &str); 10] = [
    ("aberystwyth-parry", "Jesu, Lover of My Soul"),
    ("aurelia", "The Church's One Foundation"),
    ("darwall", "Rejoice, the Lord Is King"),
    ("ebenezer", "O the Deep, Deep Love of Jesus"),
    ("franconia", "Blest Are the Pure in Heart"),
    ("italian-hymn", "Come, Thou Almighty King"),
    ("melita", "Eternal Father, Strong to Save"),
    ("ode-to-joy", "Joyful, Joyful, We Adore Thee"),
    ("st-agnes", "Jesus, the Very Thought of Thee"),
    ("tempus-adest-floridum", "Good King Wenceslas"),
];

/// The latest year of death that's out of copyright everywhere we know of.
fn cutoff_year() -> i32 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    1970 + (secs / 31_556_952) as i32 - 71
}

/// ABC files as (file stem, text), from a zip or a folder.
fn read_sources(source: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let decode = |bytes: Vec<u8>| match String::from_utf8(bytes) {
        Ok(s) => s,
        // Older files are Latin-1.
        Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
    };
    let mut out = Vec::new();
    if source.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let file = std::fs::File::open(source).with_context(|| format!("can't open {}", source.display()))?;
        let mut zip = zip::ZipArchive::new(file)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            let name = f.name().rsplit('/').next().unwrap_or("").to_string();
            if let Some(stem) = name.strip_suffix(".abc") {
                let mut bytes = Vec::new();
                f.read_to_end(&mut bytes)?;
                out.push((stem.to_string(), decode(bytes)));
            }
        }
    } else {
        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for p in rd.flatten().map(|e| e.path()) {
                    if p.is_dir() {
                        walk(&p, out);
                    } else if p.extension().is_some_and(|e| e == "abc") {
                        out.push(p);
                    }
                }
            }
        }
        let mut paths = Vec::new();
        walk(source, &mut paths);
        for p in paths {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            out.push((stem, decode(std::fs::read(&p)?)));
        }
    }
    out.sort();
    Ok(out)
}

/// Ids and tune names already in the library.
fn existing(library: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut ids = BTreeSet::new();
    let mut tunes = BTreeSet::new();
    fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for p in rd.flatten().map(|e| e.path()) {
                if p.is_dir() {
                    walk(&p, f);
                } else {
                    f(&p);
                }
            }
        }
    }
    walk(&library.join("music"), &mut |p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        for suffix in [".score.json", ".track.json"] {
            if let Some(id) = name.strip_suffix(suffix) {
                ids.insert(id.to_string());
                if let Ok(s) = std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| Score::from_json(&t))
                {
                    tunes.insert(abc::slug(&s.melody));
                }
            }
        }
    });
    (ids, tunes)
}

pub fn import_open_hymnal(source: &Path, library: &Path, write: bool) -> anyhow::Result<()> {
    let cutoff = cutoff_year();
    let files = read_sources(source)?;
    let (ids, tunes) = existing(library);
    eprintln!("{} hymns to look at; music by people who died in {cutoff} or before is free everywhere", files.len());

    let mut accepted: BTreeMap<String, Score> = BTreeMap::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut already = 0;
    for (stem, text) in &files {
        if NOT_FAITHFUL.contains(&stem.as_str()) {
            skipped.push((stem.clone(), "its repeats or parts can't be shown faithfully".into()));
            continue;
        }
        let score = match abc::from_open_hymnal(text, stem, cutoff) {
            Ok(s) => s,
            Err(e) => {
                skipped.push((stem.clone(), e));
                continue;
            }
        };
        if ids.contains(&score.id) || tunes.contains(&abc::slug(&score.melody)) {
            already += 1;
            continue;
        }
        if let Some(first) = accepted.get_mut(&score.id) {
            // Another hymn to the same tune.
            if !first.title.eq_ignore_ascii_case(&score.title) && !first.other_titles.contains(&score.title) {
                first.other_titles.push(score.title.clone());
            }
            continue;
        }
        let checked = score.compile().and_then(|t| check_playable(&t));
        match checked {
            Ok(()) => {
                accepted.insert(score.id.clone(), score);
            }
            Err(e) => skipped.push((stem.clone(), e)),
        }
    }

    for (id, best) in BEST_KNOWN {
        if let Some(s) = accepted.get_mut(id) {
            if s.title != best {
                let old = std::mem::replace(&mut s.title, best.to_string());
                s.other_titles.retain(|t| {
                    !t.eq_ignore_ascii_case(best) && !t.to_ascii_lowercase().ends_with(&best.to_ascii_lowercase())
                });
                s.other_titles.insert(0, old);
            }
        }
    }

    let dir = library.join("music").join("hymns");
    if write {
        std::fs::create_dir_all(&dir)?;
        for s in accepted.values() {
            let path = dir.join(format!("{}.score.json", s.id));
            std::fs::write(&path, serde_json::to_string_pretty(s)? + "\n")?;
        }
    }
    for (stem, why) in &skipped {
        eprintln!("skipped {stem}: {why}");
    }
    eprintln!(
        "{} {} new tunes; {} already in the library; {} skipped",
        if write { "wrote" } else { "would write" },
        accepted.len(),
        already,
        skipped.len()
    );
    Ok(())
}
