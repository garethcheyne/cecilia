//! Cecilia server: runs the organ MIDI engine headless and exposes it over
//! HTTP/WebSocket. Optionally serves the static web UI from the same port.
//!
//! Environment:
//!   CECILIA_ADDR      listen address            (default 127.0.0.1:7070)
//!   CECILIA_DATA_DIR  settings / library folder (default <OS data dir>/Cecilia)
//!   CECILIA_WEB_DIR   static web UI to serve    (default ./web next to the binary, or ../web/out)
//!   CECILIA_CORS      allowed browser origins   (default http://localhost:3000,http://127.0.0.1:3000)
//!   CECILIA_AUDIO     set to 0 to keep the simulator silent
//!   CECILIA_MCP_TOKEN if set, `/mcp` requires `Authorization: Bearer <token>`
//!   RUST_LOG           log filter                (default info)
//!
//! Older `REGISTRA_*` names still work.
//!
//! `cecilia openapi` prints the OpenAPI document and exits.
//! `cecilia library check <dir>` checks a copy of the shared library;
//! `cecilia library index <dir>` also prints its catalogue (index.json).

use std::net::SocketAddr;

use anyhow::Context;
use organ_server::{api::ApiDoc, build, Config};
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["openapi"] => {
            println!("{}", ApiDoc::openapi().to_pretty_json()?);
            return Ok(());
        }
        // For the shared library repository's checks.
        ["library", "check", dir] | ["library", "index", dir] => {
            let (catalogue, problems) = organ_core::community::build_catalogue(std::path::Path::new(dir));
            for p in &problems {
                eprintln!("problem: {p}");
            }
            eprintln!(
                "{} pieces and {} organs are fine; {} problem(s)",
                catalogue.music.len(),
                catalogue.organs.len(),
                problems.len()
            );
            if args[1] == "index" {
                println!("{}", serde_json::to_string_pretty(&catalogue)?);
            }
            std::process::exit(if problems.is_empty() { 0 } else { 1 });
        }
        _ => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    let mcp_open = config.mcp_token.is_none();
    let app = build(config).await?;

    let addr: SocketAddr = organ_server::env("ADDR")
        .unwrap_or_else(|| "127.0.0.1:7070".into())
        .parse()
        .context("CECILIA_ADDR must look like 127.0.0.1:7070")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("could not listen on {addr}"))?;
    tracing::info!("listening on http://{addr} (MCP at /mcp)");
    if mcp_open && !addr.ip().is_loopback() {
        tracing::warn!(
            "the organ can be controlled by anyone on the network through /mcp;              set CECILIA_MCP_TOKEN to require a token"
        );
    }

    axum::serve(listener, app.router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
