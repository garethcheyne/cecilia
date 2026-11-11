//! Organ definitions available to the server: built-ins plus user files in
//! `<data>/organs/*.organ.json`. A user file with a built-in's id overrides it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use organ_core::organ::{builtin_organs, OrganDefinition, OrganSummary};

pub struct OrganStore {
    dir: PathBuf,
    organs: RwLock<BTreeMap<String, (OrganDefinition, bool)>>,
}

impl OrganStore {
    pub fn load(data_dir: &Path) -> anyhow::Result<Self> {
        let dir = data_dir.join("organs");
        std::fs::create_dir_all(&dir)?;
        let store = Self { dir, organs: RwLock::new(BTreeMap::new()) };
        store.reload();
        Ok(store)
    }

    /// Re-scan the organs folder.
    pub fn reload(&self) {
        let mut organs: BTreeMap<_, _> = builtin_organs()
            .into_iter()
            .map(|o| (o.id.clone(), (o, true)))
            .collect();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for path in entries.flatten().map(|e| e.path()) {
                if !path.to_string_lossy().ends_with(".organ.json") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|t| Ok(OrganDefinition::from_json(&t)?))
                {
                    Ok(o) => {
                        organs.insert(o.id.clone(), (o, false));
                    }
                    Err(err) => tracing::warn!(path = %path.display(), %err, "skipping organ file"),
                }
            }
        }
        *self.organs.write().unwrap() = organs;
    }

    pub fn list(&self) -> Vec<OrganSummary> {
        self.organs
            .read()
            .unwrap()
            .values()
            .map(|(o, builtin)| o.summary(*builtin))
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<OrganDefinition> {
        self.organs.read().unwrap().get(id).map(|(o, _)| o.clone())
    }
}
