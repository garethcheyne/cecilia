//! Playlists API and the service runner.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use organ_core::player::VersePlan;
use organ_core::playlist::{parse_verses, Playlist, PlaylistRow, PlaylistSummary, PLAYLIST_SCHEMA};
use organ_core::recorder::unix_ms;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};
use crate::playlists::ServiceStatus;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/playlists", get(list_playlists).post(create_playlist))
        .route("/playlists/{id}", get(get_playlist).put(put_playlist).delete(delete_playlist))
        .route("/playlists/{id}/duplicate", post(duplicate_playlist))
        .route("/verses/parse", post(verses_parse))
        .route("/service", get(service_status))
        .route("/service/start", post(service_start))
        .route("/service/next", post(service_next))
        .route("/service/previous", post(service_previous))
        .route("/service/select", post(service_select))
        .route("/service/end", post(service_end))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

fn no_playlist(id: &str) -> Response {
    error(StatusCode::NOT_FOUND, format!("no playlist called {id:?}"))
}

/// All playlists, most recently changed first.
#[utoipa::path(get, path = "/api/v1/playlists", tag = "playlists",
    responses((status = 200, body = Vec<PlaylistSummary>)))]
pub async fn list_playlists(State(s): State<AppState>) -> Json<Vec<PlaylistSummary>> {
    Json(s.playlists.list())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewPlaylist {
    pub name: String,
}

/// Create an empty playlist.
#[utoipa::path(post, path = "/api/v1/playlists", tag = "playlists", request_body = NewPlaylist,
    responses((status = 200, body = Playlist), (status = 400, body = ApiErrorBody)))]
pub async fn create_playlist(
    State(s): State<AppState>,
    Json(req): Json<NewPlaylist>,
) -> Result<Json<Playlist>, Response> {
    let name = req.name.trim().to_string();
    let now = unix_ms();
    let p = Playlist {
        schema: PLAYLIST_SCHEMA,
        id: s.playlists.unique_id(&name),
        name,
        rows: Vec::new(),
        repeat: false,
        shuffle: false,
        silent: false,
        created: now,
        modified: now,
    };
    s.playlists.save(&p).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(p))
}

/// A playlist with all its rows.
#[utoipa::path(get, path = "/api/v1/playlists/{id}", tag = "playlists",
    params(("id" = String, Path, description = "Playlist id")),
    responses((status = 200, body = Playlist), (status = 404, body = ApiErrorBody)))]
pub async fn get_playlist(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Playlist>, Response> {
    s.playlists.get(&id).map(Json).ok_or_else(|| no_playlist(&id))
}

/// Save a playlist. Saving also marks every track as checked, clearing
/// "edited since it was added" warnings.
#[utoipa::path(put, path = "/api/v1/playlists/{id}", tag = "playlists", request_body = Playlist,
    params(("id" = String, Path, description = "Playlist id")),
    responses((status = 200, body = Playlist), (status = 400, body = ApiErrorBody)))]
pub async fn put_playlist(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(mut p): Json<Playlist>,
) -> Result<Json<Playlist>, Response> {
    if p.id != id {
        return Err(error(StatusCode::BAD_REQUEST, "the playlist id doesn't match the address"));
    }
    p.name = p.name.trim().to_string();
    p.modified = unix_ms();
    if p.created == 0 {
        p.created = p.modified;
    }
    for row in &mut p.rows {
        if let PlaylistRow::Track { track_id, track_modified, .. } = row {
            let summary = s.library.list().into_iter().find(|t| &t.id == track_id);
            *track_modified = summary.map_or(0, |t| t.modified);
        }
    }
    s.playlists.save(&p).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    s.service.playlist_saved(&p);
    Ok(Json(p))
}

/// Delete a playlist (the tracks stay in the library).
#[utoipa::path(delete, path = "/api/v1/playlists/{id}", tag = "playlists",
    params(("id" = String, Path, description = "Playlist id")),
    responses((status = 204), (status = 404, body = ApiErrorBody)))]
pub async fn delete_playlist(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    if s.service.status().playlist_id.as_deref() == Some(id.as_str()) {
        s.service.end();
    }
    match s.playlists.delete(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => no_playlist(&id),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// Copy a playlist (e.g. last Sunday's as a starting point).
#[utoipa::path(post, path = "/api/v1/playlists/{id}/duplicate", tag = "playlists",
    params(("id" = String, Path, description = "Playlist id")),
    responses((status = 200, body = Playlist), (status = 404, body = ApiErrorBody)))]
pub async fn duplicate_playlist(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Playlist>, Response> {
    let mut p = s.playlists.get(&id).ok_or_else(|| no_playlist(&id))?;
    p.name = format!("{} (copy)", p.name);
    p.id = s.playlists.unique_id(&p.name);
    p.created = unix_ms();
    p.modified = p.created;
    s.playlists.save(&p).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(p))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerseText {
    /// What the organist typed, e.g. "4" or "12357".
    pub text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ParsedVerses {
    pub verses: Option<VersePlan>,
}

/// Check a verse entry ("4" = four verses, "12357" = exactly those).
#[utoipa::path(post, path = "/api/v1/verses/parse", tag = "playlists", request_body = VerseText,
    responses((status = 200, body = ParsedVerses), (status = 400, body = ApiErrorBody)))]
pub async fn verses_parse(Json(req): Json<VerseText>) -> Result<Json<ParsedVerses>, Response> {
    parse_verses(&req.text).map(|verses| Json(ParsedVerses { verses })).map_err(|e| error(StatusCode::BAD_REQUEST, e))
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Which playlist is running and where it's up to.
#[utoipa::path(get, path = "/api/v1/service", tag = "service",
    responses((status = 200, body = ServiceStatus)))]
pub async fn service_status(State(s): State<AppState>) -> Json<ServiceStatus> {
    Json(s.service.status())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartService {
    pub playlist_id: String,
    /// Row to start from (default: the first track).
    pub row: Option<usize>,
    /// Start playing at once (default false: load and wait for Play).
    #[serde(default)]
    pub play: bool,
}

/// Run a playlist.
#[utoipa::path(post, path = "/api/v1/service/start", tag = "service", request_body = StartService,
    responses((status = 200, body = ServiceStatus), (status = 404, body = ApiErrorBody), (status = 409, body = ApiErrorBody)))]
pub async fn service_start(
    State(s): State<AppState>,
    Json(req): Json<StartService>,
) -> Result<Json<ServiceStatus>, Response> {
    let p = s.playlists.get(&req.playlist_id).ok_or_else(|| no_playlist(&req.playlist_id))?;
    let status = s.hub.status().await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    if status.output.is_none() {
        return Err(error(StatusCode::CONFLICT, "Choose an Organ out on MIDI Setup first."));
    }
    s.service.begin(p, req.row, req.play).map(Json).map_err(|e| error(StatusCode::CONFLICT, e))
}

/// Skip to the next track and play it.
#[utoipa::path(post, path = "/api/v1/service/next", tag = "service",
    responses((status = 200, body = ServiceStatus), (status = 409, body = ApiErrorBody)))]
pub async fn service_next(State(s): State<AppState>) -> Result<Json<ServiceStatus>, Response> {
    s.service.next().map(Json).map_err(|e| error(StatusCode::CONFLICT, e))
}

/// Go back to the previous track and play it.
#[utoipa::path(post, path = "/api/v1/service/previous", tag = "service",
    responses((status = 200, body = ServiceStatus), (status = 409, body = ApiErrorBody)))]
pub async fn service_previous(State(s): State<AppState>) -> Result<Json<ServiceStatus>, Response> {
    s.service.previous().map(Json).map_err(|e| error(StatusCode::CONFLICT, e))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SelectRow {
    pub row: usize,
}

/// Get a row ready to play.
#[utoipa::path(post, path = "/api/v1/service/select", tag = "service", request_body = SelectRow,
    responses((status = 200, body = ServiceStatus), (status = 409, body = ApiErrorBody)))]
pub async fn service_select(
    State(s): State<AppState>,
    Json(req): Json<SelectRow>,
) -> Result<Json<ServiceStatus>, Response> {
    s.service.select(req.row).map(Json).map_err(|e| error(StatusCode::CONFLICT, e))
}

/// Stop running the playlist.
#[utoipa::path(post, path = "/api/v1/service/end", tag = "service",
    responses((status = 200, body = ServiceStatus)))]
pub async fn service_end(State(s): State<AppState>) -> Json<ServiceStatus> {
    Json(s.service.end())
}
