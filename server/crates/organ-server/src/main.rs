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

use std::net::SocketAddr;

use anyhow::Context;
use organ_server::{api::ApiDoc, build, Config};
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        println!("{}", ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    let mcp_open = config.mcp_token.is_none();
    let app = build(config).await?;

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

    axum::serve(listener, app.router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
