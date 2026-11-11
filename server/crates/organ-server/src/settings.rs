//! Persistent server settings (a JSON file in the data directory).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use organ_core::MidiConfig;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How the auto-recorder splits a service into pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct RecordingSettings {
    /// Silence (after the last key is released) that ends a piece. 2-60 s.
    pub silence_ms: u32,
    /// Silence kept before the first note and after the last. 0-15 s.
    pub margin_start_ms: u32,
    pub margin_end_ms: u32,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        Self { silence_ms: 8000, margin_start_ms: 2000, margin_end_ms: 2000 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct PlaybackSettings {
    /// Length of a fade-out. 1-30 s.
    pub fade_ms: u32,
    /// Wait after a general cancel on organs whose stops toggle.
    pub settle_ms: u32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self { fade_ms: 5000, settle_ms: 400 }
    }
}

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
    #[serde(default)]
    pub recording: RecordingSettings,
    #[serde(default)]
    pub playback: PlaybackSettings,
    /// Bundled samples already put in the library (deleted ones stay gone).
    #[serde(default)]
    pub samples_seen: Vec<String>,
    /// "salt$sha256" of the password, if one is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
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
