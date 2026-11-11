//! HTTP + WebSocket API. Everything the web UI can do goes through here, so
//! any other client (a tablet app, a script, a hardware button box) can too.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use organ_core::console::ConsoleState;
use organ_core::driver::OrganEvent;
use organ_core::live::LiveOrgan;
use organ_core::organ::{OrganDefinition, OrganSummary};
use organ_core::simulator::{SimSnapshot, Simulator};
use organ_core::{
    Direction, EngineEvent, MidiConfig, MidiError, MidiHub, MidiStatus, PortList, TestState,
    TestStatus,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use utoipa::{OpenApi, ToSchema};

use crate::library::LibraryStore;
use crate::organs::OrganStore;
use crate::services::{AppEvent, Recorder};
use crate::settings::SettingsStore;
use organ_core::player::{Player, PlayerEvent};

#[derive(Clone)]
pub struct AppState {
    pub hub: MidiHub,
    pub settings: Arc<SettingsStore>,
    pub organs: Arc<OrganStore>,
    pub simulator: Simulator,
    /// State of the organ on the MIDI output, for remote control.
    pub live: LiveOrgan,
    /// Browser origins allowed to call the API (besides same-origin).
    pub allowed_origins: Arc<Vec<HeaderValue>>,
    /// Bearer token required by `/mcp`, if set.
    pub mcp_token: Option<Arc<str>>,
    pub library: Arc<LibraryStore>,
    pub recorder: Recorder,
    pub player: Player,
    pub app_events: tokio::sync::broadcast::Sender<AppEvent>,
    pub playlists: Arc<crate::playlists::PlaylistStore>,
    pub service: crate::playlists::Service,
}

impl AppState {
    /// Make `organ` the current organ: the simulator plays it, and the live
    /// tracker, recorder and player all speak its MIDI. Saved.
    pub fn set_organ(&self, organ: OrganDefinition) -> anyhow::Result<()> {
        self.settings.update(|st| st.simulator_organ = Some(organ.id.clone()))?;
        self.simulator.load_organ(organ.clone());
        self.live.load_organ(organ.clone());
        self.recorder.load_organ(organ.clone());
        self.player.set_organ(organ);
        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi))
        .route("/events", get(events))
        .route("/midi/ports", get(ports))
        .route("/midi/config", get(get_config).put(put_config))
        .route("/midi/status", get(status))
        .route("/midi/panic", post(panic))
        .route("/midi/test/record", post(test_record))
        .route("/midi/test/stop", post(test_stop))
        .route("/midi/test/play", post(test_play))
        .route("/organs", get(list_organs))
        .route("/organs/{id}", get(get_organ))
        .route("/simulator", get(sim_state))
        .route("/simulator/organ", put(sim_load_organ))
        .route("/simulator/act", post(sim_act))
        .route("/simulator/reset", post(sim_reset))
        .route("/assistant", get(get_assistant).put(put_assistant))
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Registra API",
        description = "Headless organ MIDI engine. Live events are streamed over a WebSocket at \
                       `/api/v1/events` as JSON `EngineEvent` messages."
    ),
    paths(
        health, ports, get_config, put_config, status, panic, test_record, test_stop, test_play,
        list_organs, get_organ, sim_state, sim_load_organ, sim_act, sim_reset, get_assistant,
        put_assistant,
        crate::music::list_tracks, crate::music::get_track, crate::music::put_track,
        crate::music::patch_track, crate::music::delete_track, crate::music::duplicate_track,
        crate::music::track_mapping, crate::music::recorder_status, crate::music::recorder_start,
        crate::music::recorder_stop, crate::music::player_status, crate::music::player_load,
        crate::music::player_play, crate::music::player_pause, crate::music::player_stop,
        crate::music::player_seek, crate::music::player_tempo, crate::music::player_verses,
        crate::music::player_fade,
        crate::playlist_api::list_playlists, crate::playlist_api::create_playlist,
        crate::playlist_api::get_playlist, crate::playlist_api::put_playlist,
        crate::playlist_api::delete_playlist, crate::playlist_api::duplicate_playlist,
        crate::playlist_api::verses_parse, crate::playlist_api::service_status,
        crate::playlist_api::service_start, crate::playlist_api::service_next,
        crate::playlist_api::service_previous, crate::playlist_api::service_select,
        crate::playlist_api::service_end,
        crate::files_api::analyse, crate::files_api::import, crate::files_api::export_midi,
        crate::files_api::export_csv, crate::files_api::export_zip
    ),
    components(schemas(
        Health, ApiErrorBody, PortList, MidiConfig, MidiStatus, TestStatus, TestState,
        EngineEvent, Direction, OrganSummary, OrganDefinition, OrganEvent, SimSnapshot,
        ConsoleState, LoadOrgan, Assistant,
        organ_core::track::Track, organ_core::track::TrackSummary, organ_core::track::TrackInfo,
        organ_core::track::TrackEvent, organ_core::track::TimedEvent, organ_core::track::StopRef,
        organ_core::track::Registration, organ_core::track::Meter, organ_core::track::Mode,
        organ_core::track::TrackSource, organ_core::organist::StopMapping,
        organ_core::recorder::RecorderStatus, organ_core::recorder::RecorderState,
        organ_core::player::PlayerStatus, organ_core::player::PlayerState,
        organ_core::player::PlayOptions, organ_core::player::VersePlan,
        organ_core::player::PlayerEvent, crate::services::AppEvent,
        crate::music::LoadTrack, crate::music::TrackPatch, crate::music::Seek, crate::music::Tempo,
        crate::music::VerseControl, crate::music::FadeRequest,
        organ_core::playlist::Playlist, organ_core::playlist::PlaylistRow,
        organ_core::playlist::PlaylistSummary, organ_core::playlist::Delay,
        crate::playlists::ServiceStatus, crate::playlists::ServiceState,
        crate::playlist_api::NewPlaylist, crate::playlist_api::StartService,
        crate::playlist_api::SelectRow, crate::playlist_api::VerseText, crate::playlist_api::ParsedVerses,
        crate::files_api::FileKind, crate::files_api::ImportAnalysis,
        organ_core::smf::MidiAnalysis, organ_core::smf::ChannelInfo, organ_core::smf::ImportOptions,
        organ_core::smf::ExportFormat, organ_core::musicxml::XmlAnalysis, organ_core::musicxml::Stream,
        organ_core::musicxml::XmlImportOptions
    ))
)]
pub struct ApiDoc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct ApiErrorBody {
    pub error: String,
}

pub enum ApiError {
    Midi(MidiError),
    Internal(anyhow::Error),
}

impl From<MidiError> for ApiError {
    fn from(e: MidiError) -> Self {
        Self::Midi(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Midi(e) if e.is_conflict() => (StatusCode::CONFLICT, e.to_string()),
            Self::Midi(e) => (StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
            Self::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        };
        (status, Json(ApiErrorBody { error: message })).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub name: &'static str,
    pub version: &'static str,
}

/// Server identity and version.
#[utoipa::path(get, path = "/api/v1/health", tag = "system",
    responses((status = 200, body = Health)))]
async fn health() -> Json<Health> {
    Json(Health {
        name: "registra",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// MIDI ports currently available on this machine.
#[utoipa::path(get, path = "/api/v1/midi/ports", tag = "midi",
    responses((status = 200, body = PortList), (status = 503, body = ApiErrorBody)))]
async fn ports(State(s): State<AppState>) -> ApiResult<PortList> {
    Ok(Json(s.hub.ports().await?))
}

/// The saved port selection.
#[utoipa::path(get, path = "/api/v1/midi/config", tag = "midi",
    responses((status = 200, body = MidiConfig)))]
async fn get_config(State(s): State<AppState>) -> Json<MidiConfig> {
    Json(s.settings.get().midi)
}

/// Save a new port selection and reconnect. Ports that could not be opened
/// are listed in `missing` in the response.
#[utoipa::path(put, path = "/api/v1/midi/config", tag = "midi", request_body = MidiConfig,
    responses((status = 200, body = MidiStatus)))]
async fn put_config(State(s): State<AppState>, Json(cfg): Json<MidiConfig>) -> ApiResult<MidiStatus> {
    s.settings.update(|st| st.midi = cfg.clone())?;
    Ok(Json(s.hub.configure(cfg).await?))
}

/// Open connections and test recorder state.
#[utoipa::path(get, path = "/api/v1/midi/status", tag = "midi",
    responses((status = 200, body = MidiStatus)))]
async fn status(State(s): State<AppState>) -> ApiResult<MidiStatus> {
    Ok(Json(s.hub.status().await?))
}

/// Send All Notes Off and All Sound Off on every channel.
#[utoipa::path(post, path = "/api/v1/midi/panic", tag = "midi",
    responses((status = 204), (status = 409, body = ApiErrorBody)))]
async fn panic(State(s): State<AppState>) -> Result<StatusCode, ApiError> {
    s.hub.panic().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Start the raw test recorder (discards the previous test recording).
#[utoipa::path(post, path = "/api/v1/midi/test/record", tag = "test",
    responses((status = 200, body = MidiStatus), (status = 409, body = ApiErrorBody)))]
async fn test_record(State(s): State<AppState>) -> ApiResult<MidiStatus> {
    Ok(Json(s.hub.test_record().await?))
}

/// Stop test recording or playback.
#[utoipa::path(post, path = "/api/v1/midi/test/stop", tag = "test",
    responses((status = 200, body = MidiStatus)))]
async fn test_stop(State(s): State<AppState>) -> ApiResult<MidiStatus> {
    Ok(Json(s.hub.test_stop().await?))
}

/// Play the test recording back, byte-for-byte, to the output port.
#[utoipa::path(post, path = "/api/v1/midi/test/play", tag = "test",
    responses((status = 200, body = MidiStatus), (status = 409, body = ApiErrorBody)))]
async fn test_play(State(s): State<AppState>) -> ApiResult<MidiStatus> {
    Ok(Json(s.hub.test_play().await?))
}

/// Organ definitions available on this server.
#[utoipa::path(get, path = "/api/v1/organs", tag = "organs",
    responses((status = 200, body = Vec<OrganSummary>)))]
async fn list_organs(State(s): State<AppState>) -> Json<Vec<OrganSummary>> {
    Json(s.organs.list())
}

/// A full organ definition.
#[utoipa::path(get, path = "/api/v1/organs/{id}", tag = "organs",
    params(("id" = String, Path, description = "Organ id")),
    responses((status = 200, body = OrganDefinition), (status = 404, body = ApiErrorBody)))]
async fn get_organ(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<OrganDefinition>, Response> {
    s.organs.get(&id).map(Json).ok_or_else(|| not_found(&id))
}

fn not_found(id: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(ApiErrorBody { error: format!("no organ called {id:?}") })).into_response()
}

/// The simulator's organ, console state and audio device.
#[utoipa::path(get, path = "/api/v1/simulator", tag = "simulator",
    responses((status = 200, body = SimSnapshot)))]
async fn sim_state(State(s): State<AppState>) -> Json<SimSnapshot> {
    Json(s.simulator.snapshot())
}

#[derive(Deserialize, ToSchema)]
pub struct LoadOrgan {
    pub organ_id: String,
}

/// Choose which organ the simulator plays (saved).
#[utoipa::path(put, path = "/api/v1/simulator/organ", tag = "simulator", request_body = LoadOrgan,
    responses((status = 200, body = SimSnapshot), (status = 404, body = ApiErrorBody)))]
async fn sim_load_organ(State(s): State<AppState>, Json(req): Json<LoadOrgan>) -> Result<Json<SimSnapshot>, Response> {
    let organ = s.organs.get(&req.organ_id).ok_or_else(|| not_found(&req.organ_id))?;
    s.set_organ(organ).map_err(|e| ApiError::Internal(e).into_response())?;
    Ok(Json(s.simulator.snapshot()))
}

/// Do something at the simulated console: press a key, draw a stop, move a
/// pedal. It sounds, and the organ's MIDI is sent to the engine as input.
#[utoipa::path(post, path = "/api/v1/simulator/act", tag = "simulator", request_body = OrganEvent,
    responses((status = 204)))]
async fn sim_act(State(s): State<AppState>, Json(event): Json<OrganEvent>) -> StatusCode {
    s.simulator.act(event);
    StatusCode::NO_CONTENT
}

/// Release all keys and push in every stop.
#[utoipa::path(post, path = "/api/v1/simulator/reset", tag = "simulator",
    responses((status = 204)))]
async fn sim_reset(State(s): State<AppState>) -> StatusCode {
    s.simulator.reset();
    StatusCode::NO_CONTENT
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Assistant {
    /// What the assistant is called (and, with voice control, its wake word).
    pub name: String,
}

/// The assistant's name, used by the MCP server (`/mcp`).
#[utoipa::path(get, path = "/api/v1/assistant", tag = "assistant",
    responses((status = 200, body = Assistant)))]
async fn get_assistant(State(s): State<AppState>) -> Json<Assistant> {
    Json(Assistant { name: crate::mcp::assistant_name(&s) })
}

/// Rename the assistant (1-40 characters).
#[utoipa::path(put, path = "/api/v1/assistant", tag = "assistant", request_body = Assistant,
    responses((status = 200, body = Assistant), (status = 400, body = ApiErrorBody)))]
async fn put_assistant(State(s): State<AppState>, Json(req): Json<Assistant>) -> Result<Json<Assistant>, Response> {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorBody { error: "the name must be 1-40 characters".into() }),
        )
            .into_response());
    }
    s.settings
        .update(|st| st.assistant_name = Some(name.clone()))
        .map_err(|e| ApiError::Internal(e).into_response())?;
    Ok(Json(Assistant { name }))
}

// ---------------------------------------------------------------------------
// WebSocket event stream
// ---------------------------------------------------------------------------

/// Adds a `"type"` tag to a payload for the event stream.
#[derive(Serialize)]
struct Tagged<'a, T> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    data: &'a T,
}

async fn events(ws: WebSocketUpgrade, headers: HeaderMap, State(s): State<AppState>) -> Response {
    // Browsers don't apply CORS to WebSockets, so check the origin ourselves.
    if !origin_allowed(&headers, &s.allowed_origins) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| stream_events(socket, s))
}

/// True for requests from non-browser clients (no `Origin`), from the page
/// this server itself serves, or from an allowed origin.
pub fn origin_allowed(headers: &HeaderMap, allowed: &[HeaderValue]) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let same_origin = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|host| {
            origin.to_str().ok().is_some_and(|o| {
                o.strip_prefix("http://").or_else(|| o.strip_prefix("https://")) == Some(host)
            })
        });
    same_origin || allowed.contains(origin)
}

async fn stream_events(mut socket: WebSocket, state: AppState) {
    let hub = state.hub.clone();
    let simulator = state.simulator.clone();
    let mut rx = hub.subscribe();
    let mut sim_rx = simulator.subscribe();
    let mut player_rx = state.player.subscribe();
    let mut app_rx = state.app_events.subscribe();

    // Start every client with full snapshots.
    if let Ok(status) = hub.status().await {
        if send_json(&mut socket, &EngineEvent::Status(status)).await.is_err() {
            return;
        }
    }
    let snapshot = simulator.snapshot();
    if send_json(&mut socket, &Tagged { kind: "simulator", data: &snapshot }).await.is_err() {
        return;
    }
    let player = state.player.status();
    if send_json(&mut socket, &Tagged { kind: "player", data: &player }).await.is_err() {
        return;
    }
    let recorder = AppEvent::Recorder(state.recorder.status());
    if send_json(&mut socket, &recorder).await.is_err() {
        return;
    }
    let service = AppEvent::Service(state.service.status());
    if send_json(&mut socket, &service).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            snap = sim_rx.recv() => match snap {
                Ok(snap) => {
                    if send_json(&mut socket, &Tagged { kind: "simulator", data: &snap }).await.is_err() {
                        break;
                    }
                }
                // Snapshots are full state, so skipping some is harmless.
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            event = rx.recv() => match event {
                Ok(event) => {
                    if send_json(&mut socket, &event).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    let msg = serde_json::json!({ "type": "lagged", "skipped": skipped });
                    if send_json(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            ev = player_rx.recv() => match ev {
                Ok(PlayerEvent::Status(st)) => {
                    if send_json(&mut socket, &Tagged { kind: "player", data: &st }).await.is_err() {
                        break;
                    }
                }
                Ok(PlayerEvent::Finished { track_id }) => {
                    let msg = serde_json::json!({ "type": "player_finished", "track_id": track_id });
                    if send_json(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            ev = app_rx.recv() => match ev {
                Ok(ev) => {
                    if send_json(&mut socket, &ev).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {} // clients don't send anything yet
            },
        }
    }
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, value: &T) -> Result<(), axum::Error> {
    let text = serde_json::to_string(value).expect("event serialises");
    socket.send(Message::Text(text.into())).await
}
