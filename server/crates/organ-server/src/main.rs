//! Registra server: runs the organ MIDI engine headless and exposes it over
//! HTTP/WebSocket. Optionally serves the static web UI from the same port.
//!
//! Environment:
//!   REGISTRA_ADDR      listen address            (default 127.0.0.1:7070)
//!   REGISTRA_DATA_DIR  settings / library folder (default <OS data dir>/Registra)
//!   REGISTRA_WEB_DIR   static web UI to serve    (default ./web next to the binary, or ../web/out)
//!   REGISTRA_CORS      allowed browser origins   (default http://localhost:3000,http://127.0.0.1:3000)
//!   REGISTRA_AUDIO     set to 0 to keep the simulator silent
//!   REGISTRA_MCP_TOKEN if set, `/mcp` requires `Authorization: Bearer <token>`
//!   RUST_LOG           log filter                (default info)
//!
//! `organ-server openapi` prints the OpenAPI document and exits.

mod api;
mod mcp;
mod organs;
mod settings;

use std::net::SocketAddr;
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
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;

use crate::api::{ApiDoc, AppState};
use crate::organs::OrganStore;
use crate::settings::SettingsStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        println!("{}", ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let data_dir = std::env::var_os("REGISTRA_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|d| d.join("Registra")))
        .context("could not determine a data directory; set REGISTRA_DATA_DIR")?;
    let settings = Arc::new(SettingsStore::load(&data_dir)?);
    tracing::info!(dir = %data_dir.display(), "data directory");

    let organs = Arc::new(OrganStore::load(&data_dir)?);

    // The simulator is a virtual MIDI device: the engine forwards anything
    // sent to its port down this channel.
    let (to_simulator, from_engine) = std::sync::mpsc::channel();
    let hub = MidiHub::start(Some(to_simulator));
    let synth = if std::env::var("REGISTRA_AUDIO").as_deref() == Ok("0") {
        None
    } else {
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
    };
    let sim_organ = settings
        .get()
        .simulator_organ
        .and_then(|id| organs.get(&id))
        .or_else(|| organs.get("demo"))
        .context("no organ definition available for the simulator")?;
    let live = LiveOrgan::start(&hub, sim_organ.clone());
    let simulator = Simulator::start(from_engine, hub.clone(), sim_organ, synth);

    let status = hub.configure(settings.get().midi).await?;
    if !status.missing.is_empty() {
        tracing::warn!(missing = ?status.missing, "some configured MIDI ports are unavailable");
    }

    let origins = allowed_origins();
    let state = AppState {
        hub,
        settings,
        organs,
        simulator,
        live,
        allowed_origins: Arc::new(origins.clone()),
        mcp_token: std::env::var("REGISTRA_MCP_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())
            .map(Into::into),
    };
    let mcp_open = state.mcp_token.is_none();
    let mut app = Router::new()
        .nest("/api/v1", api::router())
        .route("/mcp", post(mcp::post).get(mcp::get))
        .with_state(state)
        .layer(cors_layer(origins))
        .layer(TraceLayer::new_for_http());

    if let Some(web_dir) = web_dir() {
        tracing::info!(dir = %web_dir.display(), "serving web UI");
        let not_found = ServeFile::new(web_dir.join("404.html"));
        app = app.fallback_service(ServeDir::new(&web_dir).not_found_service(not_found));
    } else {
        tracing::info!("no web UI build found; API only");
    }

    let addr: SocketAddr = std::env::var("REGISTRA_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7070".into())
        .parse()
        .context("REGISTRA_ADDR must look like 127.0.0.1:7070")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("could not listen on {addr}"))?;
    tracing::info!("listening on http://{addr} (MCP at /mcp)");
    if mcp_open && !addr.ip().is_loopback() {
        tracing::warn!(
            "the organ can be controlled by anyone on the network through /mcp;              set REGISTRA_MCP_TOKEN to require a token"
        );
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// Only listed origins may call the API from a browser, so an arbitrary web
/// page can't drive the organ through a user's local server.
fn allowed_origins() -> Vec<HeaderValue> {
    std::env::var("REGISTRA_CORS")
        .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".into())
        .split(',')
        .filter_map(|o| HeaderValue::from_str(o.trim()).ok())
        .collect()
}

fn cors_layer(origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(AllowHeaders::mirror_request())
}

fn web_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("REGISTRA_WEB_DIR") {
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
