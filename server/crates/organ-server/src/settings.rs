//! Persistent server settings (a JSON file in the data directory).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use organ_core::MidiConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub midi: MidiConfig,
    /// Organ the simulator plays (defaults to the built-in demo).
    #[serde(default)]
    pub simulator_organ: Option<String>,
    /// What the assistant (MCP / voice control) is called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_name: Option<String>,
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Load settings from `dir/settings.json`, falling back to defaults if the
    /// file is missing or unreadable (a bad file is logged, never fatal).
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("settings.json");
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                tracing::warn!(path = %path.display(), %err, "ignoring unreadable settings file");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        Ok(Self {
            path,
            current: Mutex::new(current),
        })
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> anyhow::Result<Settings> {
        let mut guard = self.current.lock().unwrap();
        f(&mut guard);
        // Write to a temp file then rename, so a crash never leaves a torn file.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&*guard)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(guard.clone())
    }
}
