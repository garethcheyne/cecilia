//! The music library: one `<id>.track.json` per track in `<data>/library`,
//! with an in-memory index of summaries for fast listing.
//!
//! Every save goes to a temporary file first and is then renamed into
//! place, so a power cut never leaves a half-written track. The previous
//! version of a track is kept in `<data>/history/<id>/` on each save.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{bail, Context};
use organ_core::recorder::unix_ms;
use organ_core::track::{valid_id, Track, TrackSummary};

/// How many old versions of each track to keep.
const HISTORY_KEEP: usize = 20;

pub struct LibraryStore {
    dir: PathBuf,
    history: PathBuf,
    index: RwLock<BTreeMap<String, TrackSummary>>,
}

impl LibraryStore {
    pub fn load(data_dir: &Path) -> anyhow::Result<Self> {
        let dir = data_dir.join("library");
        let history = data_dir.join("history");
        std::fs::create_dir_all(&dir)?;
        let store = Self { dir, history, index: RwLock::new(BTreeMap::new()) };
        store.reload();
        Ok(store)
    }

    /// Re-scan the library folder.
    pub fn reload(&self) {
        let mut index = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for path in entries.flatten().map(|e| e.path()) {
                if !path.to_string_lossy().ends_with(".track.json") {
                    continue;
                }
                match read_track(&path) {
                    Ok(t) => {
                        index.insert(t.id.clone(), t.summary());
                    }
                    Err(err) => tracing::warn!(path = %path.display(), %err, "skipping track file"),
                }
            }
        }
        *self.index.write().unwrap() = index;
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.track.json"))
    }

    pub fn list(&self) -> Vec<TrackSummary> {
        let mut v: Vec<_> = self.index.read().unwrap().values().cloned().collect();
        // Newest first; samples (created 0) last, alphabetically.
        v.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.info.title.cmp(&b.info.title)));
        v
    }

    pub fn contains(&self, id: &str) -> bool {
        self.index.read().unwrap().contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<Track> {
        if !valid_id(id) || !self.contains(id) {
            return None;
        }
        read_track(&self.path(id)).ok()
    }

    /// Write a track (new or replacing an existing one).
    pub fn save(&self, track: &Track) -> anyhow::Result<TrackSummary> {
        track.validate().map_err(anyhow::Error::msg)?;
        let path = self.path(&track.id);
        if path.exists() {
            self.keep_history(&track.id, &path);
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(track)?)?;
        std::fs::rename(&tmp, &path)?;
        let summary = track.summary();
        self.index.write().unwrap().insert(track.id.clone(), summary.clone());
        Ok(summary)
    }

    fn keep_history(&self, id: &str, path: &Path) {
        let dir = self.history.join(id);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = std::fs::copy(path, dir.join(format!("{}.track.json", unix_ms())));
        // Keep only the newest few.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            let excess = files.len().saturating_sub(HISTORY_KEEP);
            for old in files.into_iter().take(excess) {
                let _ = std::fs::remove_file(old);
            }
        }
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<bool> {
        if !valid_id(id) || !self.contains(id) {
            return Ok(false);
        }
        let path = self.path(id);
        self.keep_history(id, &path);
        std::fs::remove_file(&path).context("could not delete the track file")?;
        self.index.write().unwrap().remove(id);
        Ok(true)
    }

    /// A fresh id based on `base` that isn't taken.
    pub fn unique_id(&self, base: &str) -> String {
        let base: String = base
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() { "track".to_string() } else { base.chars().take(60).collect() };
        if !self.contains(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|id| !self.contains(id)).unwrap()
    }

    /// Copy a track under a new id and title.
    pub fn duplicate(&self, id: &str) -> anyhow::Result<TrackSummary> {
        let Some(mut t) = self.get(id) else { bail!("no track called {id:?}") };
        t.id = self.unique_id(&format!("{}-copy", t.id));
        t.info.title = format!("{} (copy)", t.info.title);
        t.created = unix_ms();
        t.modified = t.created;
        self.save(&t)
    }
}

fn read_track(path: &Path) -> anyhow::Result<Track> {
    let text = std::fs::read_to_string(path)?;
    let t: Track = serde_json::from_str(&text)?;
    t.validate().map_err(anyhow::Error::msg)?;
    Ok(t)
}
