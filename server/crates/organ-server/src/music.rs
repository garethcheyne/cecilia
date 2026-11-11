//! Music API: the library, the auto-recorder and the player.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use organ_core::organist::{Organist, StopMapping};
use organ_core::player::{PlayOptions, PlayerStatus};
use organ_core::recorder::{unix_ms, RecorderStatus};
use organ_core::track::{Track, TrackEvent, TrackSummary};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};
use crate::services::AppEvent;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/library", get(list_tracks))
        .route("/library/{id}", get(get_track).put(put_track).patch(patch_track).delete(delete_track))
        .route("/library/{id}/duplicate", post(duplicate_track))
        .route("/library/{id}/mapping", get(track_mapping))
        .route("/recorder", get(recorder_status))
        .route("/recorder/start", post(recorder_start))
        .route("/recorder/stop", post(recorder_stop))
        .route("/player", get(player_status))
        .route("/player/load", post(player_load))
        .route("/player/play", post(player_play))
        .route("/player/pause", post(player_pause))
        .route("/player/stop", post(player_stop))
        .route("/player/seek", post(player_seek))
        .route("/player/tempo", post(player_tempo))
        .route("/player/verses", post(player_verses))
        .route("/player/fade", post(player_fade))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

fn no_track(id: &str) -> Response {
    error(StatusCode::NOT_FOUND, format!("no track called {id:?}"))
}

fn changed(s: &AppState, saved: Option<TrackSummary>) {
    let _ = s.app_events.send(AppEvent::Library { saved });
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

/// Every track in the library, newest first.
#[utoipa::path(get, path = "/api/v1/library", tag = "library",
    responses((status = 200, body = Vec<TrackSummary>)))]
pub async fn list_tracks(State(s): State<AppState>) -> Json<Vec<TrackSummary>> {
    Json(s.library.list())
}

/// A whole track, with its events.
#[utoipa::path(get, path = "/api/v1/library/{id}", tag = "library",
    params(("id" = String, Path, description = "Track id")),
    responses((status = 200, body = Track), (status = 404, body = ApiErrorBody)))]
pub async fn get_track(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Track>, Response> {
    s.library.get(&id).map(Json).ok_or_else(|| no_track(&id))
}

/// Save a whole track (from the editor or an import). Creates or replaces.
#[utoipa::path(put, path = "/api/v1/library/{id}", tag = "library", request_body = Track,
    params(("id" = String, Path, description = "Track id")),
    responses((status = 200, body = TrackSummary), (status = 400, body = ApiErrorBody)))]
pub async fn put_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(mut track): Json<Track>,
) -> Result<Json<TrackSummary>, Response> {
    if track.id != id {
        return Err(error(StatusCode::BAD_REQUEST, "the track id doesn't match the address"));
    }
    track.sort();
    track.modified = unix_ms();
    if track.created == 0 && track.source != organ_core::track::TrackSource::Sample {
        track.created = track.modified;
    }
    let summary = s.library.save(&track).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    changed(&s, None);
    Ok(Json(summary))
}

/// Fields to change; anything left out stays as it is.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TrackPatch {
    pub title: Option<String>,
    pub composer: Option<String>,
    pub performer: Option<String>,
    pub publication: Option<String>,
    pub copyright: Option<String>,
    pub melody: Option<String>,
    /// 1-100.
    pub volume: Option<u8>,
    /// Usual playback speed, 25-400 percent.
    pub tempo: Option<u16>,
    /// Verse boundaries in ms.
    pub verses: Option<Vec<u32>>,
}

/// Edit a track's details (title, composer, volume, verse markers…).
#[utoipa::path(patch, path = "/api/v1/library/{id}", tag = "library", request_body = TrackPatch,
    params(("id" = String, Path, description = "Track id")),
    responses((status = 200, body = TrackSummary), (status = 400, body = ApiErrorBody), (status = 404, body = ApiErrorBody)))]
pub async fn patch_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<TrackPatch>,
) -> Result<Json<TrackSummary>, Response> {
    let mut t = s.library.get(&id).ok_or_else(|| no_track(&id))?;
    let set = |field: &mut String, v: Option<String>| {
        if let Some(v) = v {
            *field = v.trim().to_string();
        }
    };
    set(&mut t.info.title, p.title);
    set(&mut t.info.composer, p.composer);
    set(&mut t.info.performer, p.performer);
    set(&mut t.info.publication, p.publication);
    set(&mut t.info.copyright, p.copyright);
    set(&mut t.info.melody, p.melody);
    if let Some(v) = p.volume {
        t.info.volume = v;
    }
    if let Some(v) = p.tempo {
        t.info.tempo = v;
    }
    if let Some(mut v) = p.verses {
        v.sort_unstable();
        v.dedup();
        v.retain(|m| *m > 0 && *m < t.duration_ms);
        t.verses = v;
    }
    t.modified = unix_ms();
    let summary = s.library.save(&t).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    changed(&s, None);
    Ok(Json(summary))
}

/// Delete a track (a copy is kept in the history folder).
#[utoipa::path(delete, path = "/api/v1/library/{id}", tag = "library",
    params(("id" = String, Path, description = "Track id")),
    responses((status = 204), (status = 404, body = ApiErrorBody)))]
pub async fn delete_track(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    match s.library.delete(&id) {
        Ok(true) => {
            changed(&s, None);
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => no_track(&id),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// Copy a track under a new name.
#[utoipa::path(post, path = "/api/v1/library/{id}/duplicate", tag = "library",
    params(("id" = String, Path, description = "Track id")),
    responses((status = 200, body = TrackSummary), (status = 404, body = ApiErrorBody)))]
pub async fn duplicate_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TrackSummary>, Response> {
    if !s.library.contains(&id) {
        return Err(no_track(&id));
    }
    let summary = s.library.duplicate(&id).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    changed(&s, None);
    Ok(Json(summary))
}

/// How the track's stops map onto the current organ (the virtual organist's plan).
#[utoipa::path(get, path = "/api/v1/library/{id}/mapping", tag = "library",
    params(("id" = String, Path, description = "Track id")),
    responses((status = 200, body = Vec<StopMapping>), (status = 404, body = ApiErrorBody)))]
pub async fn track_mapping(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<StopMapping>>, Response> {
    let t = s.library.get(&id).ok_or_else(|| no_track(&id))?;
    let organist = Organist::new(s.live.organ());
    let used: Vec<_> = t
        .initial
        .stops
        .iter()
        .chain(t.events.iter().filter_map(|e| match &e.event {
            TrackEvent::Stop { stop, .. } => Some(stop),
            _ => None,
        }))
        .cloned()
        .collect();
    Ok(Json(organist.report(used.iter())))
}

// ---------------------------------------------------------------------------
// Recorder
// ---------------------------------------------------------------------------

/// Auto-recorder state.
#[utoipa::path(get, path = "/api/v1/recorder", tag = "recorder",
    responses((status = 200, body = RecorderStatus)))]
pub async fn recorder_status(State(s): State<AppState>) -> Json<RecorderStatus> {
    Json(s.recorder.status())
}

/// Arm the auto-recorder: each piece played from now on is saved to the library.
#[utoipa::path(post, path = "/api/v1/recorder/start", tag = "recorder",
    responses((status = 200, body = RecorderStatus), (status = 409, body = ApiErrorBody)))]
pub async fn recorder_start(State(s): State<AppState>) -> Result<Json<RecorderStatus>, Response> {
    let status = s.hub.status().await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    if status.inputs.is_empty() {
        return Err(error(StatusCode::CONFLICT, "Choose an Organ in on MIDI Setup first."));
    }
    Ok(Json(s.recorder.arm()))
}

/// Stop recording; a piece in progress is saved.
#[utoipa::path(post, path = "/api/v1/recorder/stop", tag = "recorder",
    responses((status = 200, body = RecorderStatus)))]
pub async fn recorder_stop(State(s): State<AppState>) -> Json<RecorderStatus> {
    Json(s.recorder.disarm())
}

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

/// Transport state.
#[utoipa::path(get, path = "/api/v1/player", tag = "player",
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_status(State(s): State<AppState>) -> Json<PlayerStatus> {
    Json(s.player.status())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoadTrack {
    pub track_id: String,
    #[serde(default)]
    pub options: PlayOptions,
    /// Start playing straight away (default true).
    #[serde(default = "yes")]
    pub play: bool,
}

fn yes() -> bool {
    true
}

async fn wait_for_player(s: &AppState, pred: impl Fn(&PlayerStatus) -> bool) -> PlayerStatus {
    // Commands are asynchronous; give the player thread a moment to act so
    // the response shows the new state.
    for _ in 0..40 {
        let st = s.player.status();
        if pred(&st) {
            return st;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    s.player.status()
}

/// Load a track from the library, and by default start playing it.
#[utoipa::path(post, path = "/api/v1/player/load", tag = "player", request_body = LoadTrack,
    responses((status = 200, body = PlayerStatus), (status = 404, body = ApiErrorBody), (status = 409, body = ApiErrorBody)))]
pub async fn player_load(
    State(s): State<AppState>,
    Json(req): Json<LoadTrack>,
) -> Result<Json<PlayerStatus>, Response> {
    let track = s.library.get(&req.track_id).ok_or_else(|| no_track(&req.track_id))?;
    let status = s.hub.status().await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    if status.output.is_none() {
        return Err(error(StatusCode::CONFLICT, "Choose an Organ out on MIDI Setup first."));
    }
    let id = track.id.clone();
    let tempo = req.options.tempo.unwrap_or(track.info.tempo).clamp(25, 400);
    // Wait for this load to take effect (the same piece may already be loaded).
    let before = s.player.status();
    s.player.load(track, req.options, req.play);
    let status = wait_for_player(&s, |st| {
        st.track_id.as_deref() == Some(id.as_str())
            && st.tempo == tempo
            && (before.track_id.as_deref() != Some(id.as_str())
                || st.state != before.state
                || st.tempo != before.tempo
                || !req.play)
    })
    .await;
    Ok(Json(status))
}

/// Play, or resume from pause.
#[utoipa::path(post, path = "/api/v1/player/play", tag = "player",
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_play(State(s): State<AppState>) -> Json<PlayerStatus> {
    s.player.play();
    Json(wait_for_player(&s, |st| st.state != organ_core::player::PlayerState::Paused).await)
}

/// Pause (press again to continue).
#[utoipa::path(post, path = "/api/v1/player/pause", tag = "player",
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_pause(State(s): State<AppState>) -> Json<PlayerStatus> {
    let before = s.player.status().state;
    s.player.pause();
    Json(wait_for_player(&s, |st| st.state != before).await)
}

/// Stop playback.
#[utoipa::path(post, path = "/api/v1/player/stop", tag = "player",
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_stop(State(s): State<AppState>) -> Json<PlayerStatus> {
    s.player.stop();
    Json(wait_for_player(&s, |st| st.state == organ_core::player::PlayerState::Stopped).await)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Seek {
    /// Track position in ms.
    pub ms: u32,
}

/// Jump to a position; the registration there is re-created.
#[utoipa::path(post, path = "/api/v1/player/seek", tag = "player", request_body = Seek,
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_seek(State(s): State<AppState>, Json(req): Json<Seek>) -> Json<PlayerStatus> {
    s.player.seek(req.ms);
    Json(wait_for_player(&s, |st| st.position_ms.abs_diff(req.ms) < 500).await)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Tempo {
    /// 25-400 percent.
    pub percent: u16,
}

/// Change the playback speed.
#[utoipa::path(post, path = "/api/v1/player/tempo", tag = "player", request_body = Tempo,
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_tempo(State(s): State<AppState>, Json(req): Json<Tempo>) -> Json<PlayerStatus> {
    let p = req.percent.clamp(25, 400);
    s.player.set_tempo(p);
    Json(wait_for_player(&s, |st| st.tempo == p).await)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerseControl {
    /// Stop when the verse being played ends.
    pub stop_after_verse: Option<bool>,
    /// When this verse ends, go straight to the last one.
    pub skip_to_last: Option<bool>,
}

/// Verse control for hymns.
#[utoipa::path(post, path = "/api/v1/player/verses", tag = "player", request_body = VerseControl,
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_verses(State(s): State<AppState>, Json(req): Json<VerseControl>) -> Json<PlayerStatus> {
    if let Some(on) = req.stop_after_verse {
        s.player.stop_after_verse(on);
    }
    if let Some(on) = req.skip_to_last {
        s.player.skip_to_last_verse(on);
    }
    Json(
        wait_for_player(&s, |st| {
            req.stop_after_verse.is_none_or(|v| st.stop_after_verse == v)
                && req.skip_to_last.is_none_or(|v| st.skip_to_last == v)
        })
        .await,
    )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FadeRequest {
    /// true starts a fade-out, false cancels it.
    pub on: bool,
}

/// Fade out and stop (or cancel a fade).
#[utoipa::path(post, path = "/api/v1/player/fade", tag = "player", request_body = FadeRequest,
    responses((status = 200, body = PlayerStatus)))]
pub async fn player_fade(State(s): State<AppState>, Json(req): Json<FadeRequest>) -> Json<PlayerStatus> {
    s.player.fade(req.on);
    Json(wait_for_player(&s, |st| st.fading == req.on || st.state == organ_core::player::PlayerState::Stopped).await)
}
