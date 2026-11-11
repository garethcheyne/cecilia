//! Cecilia server library: builds the HTTP/WebSocket app around the organ
//! engine. `main.rs` runs it; integration tests start it on a spare port.

pub mod api;
pub mod community_api;
pub mod files_api;
pub mod library;
pub mod library_cli;
pub mod mcp;
pub mod music;
pub mod organ_api;
pub mod organs;
pub mod playlist_api;
pub mod playlists;
pub mod services;
pub mod settings;
pub mod system_api;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use axum::http::{HeaderValue, Method};
use axum::routing::post;
use axum::Router;
use organ_core::live::LiveOrgan;
use organ_core::simulator::Simulator;
use organ_core::MidiHub;
use tower_http::cors::{AllowHeaders, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::AppState;
use crate::library::LibraryStore;
use crate::organs::OrganStore;
use crate::services::{install_samples, player_settings, recorder_config, start_player, Recorder};
use crate::settings::SettingsStore;

/// Everything the server needs to start.
#[derive(Debug, Clone)]
pub struct Config {
    /// Settings, organs and the music library live here.
    pub data_dir: PathBuf,
    /// Static web UI to serve, if any.
    pub web_dir: Option<PathBuf>,
    /// Browser origins allowed to call the API (besides same-origin).
    pub allowed_origins: Vec<HeaderValue>,
    /// Play the simulator through the sound card.
    pub audio: bool,
    /// Bearer token required by `/mcp`.
    pub mcp_token: Option<String>,
    /// OMR command template with `{input}` and `{output}` placeholders.
    pub omr_command: Option<String>,
    /// The shared library: a web address (ending in /) or a local folder.
    pub library_url: String,
}

/// Where the shared Cecilia Library is published.
pub const DEFAULT_LIBRARY_URL: &str = "https://garethcheyne.github.io/cecilia-library/";

impl Config {
    /// Read the `CECILIA_*` environment variables (see `main.rs`).
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = match env("DATA_DIR") {
            Some(d) => PathBuf::from(d),
            None => default_data_dir().context("could not determine a data directory; set CECILIA_DATA_DIR")?,
        };
        Ok(Self {
            data_dir,
            web_dir: web_dir(),
            allowed_origins: parse_origins(
                &env("CORS").unwrap_or_else(|| "http://localhost:3000,http://127.0.0.1:3000".into()),
            ),
            audio: env("AUDIO").as_deref() != Some("0"),
            mcp_token: env("MCP_TOKEN").filter(|t| !t.is_empty()),
            omr_command: env("OMR").filter(|c| !c.trim().is_empty()).or_else(find_audiveris),
            library_url: env("LIBRARY_URL").unwrap_or_else(|| DEFAULT_LIBRARY_URL.into()),
        })
    }

    /// A silent, API-only server on `data_dir`, for tests.
    pub fn for_tests(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            web_dir: None,
            allowed_origins: Vec::new(),
            audio: false,
            mcp_token: None,
            omr_command: None,
            library_url: DEFAULT_LIBRARY_URL.into(),
        }
    }
}

pub struct App {
    pub router: Router,
    pub state: AppState,
}

/// Start the engine and build the app. Nothing listens until the caller
/// serves `router`.
pub async fn build(config: Config) -> anyhow::Result<App> {
    let settings = Arc::new(SettingsStore::load(&config.data_dir)?);
    tracing::info!(dir = %config.data_dir.display(), "data directory");
    let organs = Arc::new(OrganStore::load(&config.data_dir)?);

    // The simulator is a virtual MIDI device: the engine forwards anything
    // sent to its port down this channel.
    let (to_simulator, from_engine) = std::sync::mpsc::channel();
    let hub = MidiHub::start(Some(to_simulator));
    let synth = if config.audio { start_audio() } else { None };
    let sim_organ = settings
        .get()
        .simulator_organ
        .and_then(|id| organs.get(&id))
        .or_else(|| organs.get("demo"))
        .context("no organ definition available for the simulator")?;
    let live = LiveOrgan::start(&hub, sim_organ.clone());
    let simulator = Simulator::start(from_engine, hub.clone(), sim_organ.clone(), synth);

    let library = Arc::new(LibraryStore::load(&config.data_dir)?);
    if let Err(err) = install_samples(&library, &settings) {
        tracing::warn!(%err, "could not add the sample music");
    }
    let (app_events, _) = tokio::sync::broadcast::channel(256);
    let current = settings.get();
    let recorder = Recorder::start(
        hub.clone(),
        sim_organ.clone(),
        recorder_config(&current.recording),
        Arc::clone(&library),
        app_events.clone(),
    );
    let player = start_player(hub.clone(), live.clone(), sim_organ, player_settings(&current.playback));
    let playlists = Arc::new(crate::playlists::PlaylistStore::load(&config.data_dir)?);
    let service = crate::playlists::Service::start(
        player.clone(),
        Arc::clone(&library),
        hub.clone(),
        live.clone(),
        app_events.clone(),
    );

    let status = hub.configure(settings.get().midi).await?;
    if !status.missing.is_empty() {
        tracing::warn!(missing = ?status.missing, "some configured MIDI ports are unavailable");
    }
    reconnect_when_plugged_in(hub.clone(), Arc::clone(&settings));

    let state = AppState {
        hub,
        settings,
        organs,
        simulator,
        live,
        allowed_origins: Arc::new(config.allowed_origins.clone()),
        mcp_token: config.mcp_token.map(Into::into),
        library,
        recorder,
        player,
        app_events,
        playlists,
        service,
        data_dir: config.data_dir.clone(),
        sessions: Default::default(),
        omr_command: config.omr_command.clone(),
        community: crate::community_api::Community::new(config.library_url.clone()),
    };
    let mut router = Router::new()
        .nest("/api/v1", api::router().merge(music::router()).merge(playlist_api::router())
                .merge(organ_api::router())
                .merge(community_api::router())
                .merge(system_api::router().layer(axum::extract::DefaultBodyLimit::max(200 * 1024 * 1024)))
                .merge(files_api::router().layer(axum::extract::DefaultBodyLimit::max(files_api::MAX_IMPORT_BYTES))))
        .route("/mcp", post(mcp::post).get(mcp::get))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(state.clone(), system_api::require_session))
        .layer(cors_layer(config.allowed_origins))
        .layer(TraceLayer::new_for_http());

    if let Some(web_dir) = config.web_dir {
        tracing::info!(dir = %web_dir.display(), "serving web UI");
        let not_found = ServeFile::new(web_dir.join("404.html"));
        router = router.fallback_service(ServeDir::new(&web_dir).not_found_service(not_found));
    } else {
        tracing::info!("no web UI build found; API only");
    }
    Ok(App { router, state })
}

fn start_audio() -> Option<organ_core::synth::SynthHandle> {
    match organ_core::synth::start_audio() {
        Ok(s) => {
            tracing::info!(device = %s.device, "simulator audio ready");
            Some(s)
        }
        Err(err) => {
            tracing::warn!(%err, "simulator audio unavailable; running silent");
            None
        }
    }
}

/// Only listed origins may call the API from a browser, so an arbitrary web
/// page can't drive the organ through a user's local server.
pub fn parse_origins(list: &str) -> Vec<HeaderValue> {
    list.split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect()
}

fn cors_layer(origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers(AllowHeaders::mirror_request())
        // So the sign-in cookie works for the dev UI on another port.
        .allow_credentials(true)
}

/// Ports that were missing (the organ or the MIDI lead switched on or
/// plugged in after Cecilia started) are connected as soon as they appear.
/// Working connections are only touched once a missing port is really back.
fn reconnect_when_plugged_in(hub: MidiHub, settings: Arc<SettingsStore>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tick.tick().await;
            let Ok(status) = hub.status().await else { continue };
            if status.missing.is_empty() {
                continue;
            }
            let Ok(ports) = hub.ports().await else { continue };
            let available: Vec<&str> =
                ports.inputs.iter().chain(&ports.outputs).map(|n| organ_core::midi::port_key(n)).collect();
            let back = status.missing.iter().any(|m| available.contains(&organ_core::midi::port_key(m)));
            if back {
                if let Ok(s) = hub.configure(settings.get().midi).await {
                    tracing::info!(inputs = ?s.inputs, output = ?s.output, still_missing = ?s.missing, "reconnected MIDI");
                }
            }
        }
    });
}

/// A setting from `CECILIA_<name>`, or the older `REGISTRA_<name>` (the
/// project's working name), so existing setups keep working.
pub fn env(name: &str) -> Option<String> {
    std::env::var(format!("CECILIA_{name}"))
        .or_else(|_| std::env::var(format!("REGISTRA_{name}")))
        .ok()
}

/// `<OS data dir>/Cecilia`. A folder left by an older build under the
/// working name "Registra" is moved across the first time.
fn default_data_dir() -> Option<PathBuf> {
    let base = dirs::data_dir()?;
    let dir = base.join("Cecilia");
    let old = base.join("Registra");
    if !dir.exists() && old.is_dir() {
        match std::fs::rename(&old, &dir) {
            Ok(()) => tracing::info!(from = %old.display(), to = %dir.display(), "moved the data folder"),
            Err(err) => {
                tracing::warn!(%err, "could not move the old data folder; using it where it is");
                return Some(old);
            }
        }
    }
    Some(dir)
}

/// Audiveris on the PATH, if installed.
fn find_audiveris() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let names = if cfg!(windows) { ["Audiveris.exe", "audiveris.bat"] } else { ["audiveris", "Audiveris"] };
    std::env::split_paths(&path)
        .flat_map(|d| names.map(|n| d.join(n)))
        .find(|p| p.is_file())
        .map(|p| format!("{} -batch -export -output {{output}} {{input}}", p.display()))
}

fn web_dir() -> Option<PathBuf> {
    if let Some(dir) = env("WEB_DIR") {
        return Some(PathBuf::from(dir));
    }
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("web")));
    [beside_exe, Some(PathBuf::from("../web/out"))]
        .into_iter()
        .flatten()
        .find(|d| d.join("index.html").is_file())
}
