//! The shared Cecilia Library, from inside Cecilia: browse its catalogue
//! and add pieces and organ setups to this machine.
//!
//! The library is normally the published website; a local folder (a clone of
//! `cecilia-library`) works too, for testing and offline use.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use organ_core::community::Catalogue;
use organ_core::organ::{OrganDefinition, OrganSummary};
use organ_core::recorder::unix_ms;
use organ_core::sample::Score;
use organ_core::track::{Track, TrackSource, TrackSummary};

use crate::api::{ApiErrorBody, AppState};
use crate::services::AppEvent;

/// How long a downloaded catalogue is reused before checking again.
const FRESH: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
pub struct Community {
    base: String,
    http: reqwest::Client,
    cache: Arc<Mutex<Option<(Instant, Catalogue)>>>,
}

impl Community {
    pub fn new(base: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("Cecilia/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Self { base, http, cache: Arc::new(Mutex::new(None)) }
    }

    fn is_web(&self) -> bool {
        self.base.starts_with("http://") || self.base.starts_with("https://")
    }

    /// Call the Cecilia Community service's API (sharing and accounts).
    /// Errors come back as (status, a message to show).
    pub(crate) async fn service(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, (StatusCode, String)> {
        if !self.is_web() {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Sharing needs the Cecilia Community service; this Cecilia reads the library from a folder.".into(),
            ));
        }
        let url = format!("{}/api{}", self.base.trim_end_matches('/'), path);
        let mut req = self.http.request(method, &url);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await.map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, offline()))?;
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let value: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
        if status.is_success() {
            Ok(value)
        } else {
            let msg = value["error"].as_str().unwrap_or("The community service refused that.").to_string();
            Err((status, msg))
        }
    }

    /// Forget the cached catalogue (after sharing something, say).
    pub(crate) fn forget(&self) {
        *self.cache.lock().unwrap() = None;
    }

    /// A file from the library, by its path in the catalogue.
    async fn fetch(&self, path: &str) -> Result<String, String> {
        if path.contains("..") || path.starts_with('/') {
            return Err("bad path in the catalogue".into());
        }
        if self.is_web() {
            let url = format!("{}/{}", self.base.trim_end_matches('/'), path);
            let res = self.http.get(&url).send().await.map_err(|_| offline())?;
            if !res.status().is_success() {
                return Err(format!("the library couldn't provide {path} ({})", res.status()));
            }
            res.text().await.map_err(|e| e.to_string())
        } else {
            tokio::fs::read_to_string(std::path::Path::new(&self.base).join(path))
                .await
                .map_err(|e| format!("couldn't read {path}: {e}"))
        }
    }

    pub async fn catalogue(&self, refresh: bool) -> Result<Catalogue, String> {
        if !refresh {
            if let Some((at, c)) = self.cache.lock().unwrap().as_ref() {
                if at.elapsed() < FRESH {
                    return Ok(c.clone());
                }
            }
        }
        let cat = if self.is_web() {
            let text = self.fetch("index.json").await?;
            serde_json::from_str::<Catalogue>(&text).map_err(|e| format!("the library's catalogue is unreadable: {e}"))?
        } else {
            // A local clone: build the catalogue from its files.
            let base = self.base.clone();
            tokio::task::spawn_blocking(move || organ_core::community::build_catalogue(std::path::Path::new(&base)).0)
                .await
                .map_err(|e| e.to_string())?
        };
        *self.cache.lock().unwrap() = Some((Instant::now(), cat.clone()));
        Ok(cat)
    }
}

fn offline() -> String {
    "Can't reach the shared library. Is this computer connected to the internet?".into()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/community", get(catalogue))
        .route("/community/refresh", post(refresh))
        .route("/community/music/{id}/add", post(add_music))
        .route("/community/organs/{id}", get(get_organ))
        .route("/community/organs/{id}/add", post(add_organ))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

/// What's in the shared library.
#[utoipa::path(get, path = "/api/v1/community", tag = "community",
    responses((status = 200, body = Catalogue), (status = 503, body = ApiErrorBody)))]
pub async fn catalogue(State(s): State<AppState>) -> Result<Json<Catalogue>, Response> {
    s.community.catalogue(false).await.map(Json).map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))
}

/// Check the shared library again now.
#[utoipa::path(post, path = "/api/v1/community/refresh", tag = "community",
    responses((status = 200, body = Catalogue), (status = 503, body = ApiErrorBody)))]
pub async fn refresh(State(s): State<AppState>) -> Result<Json<Catalogue>, Response> {
    s.community.catalogue(true).await.map(Json).map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))
}

/// Add a piece from the shared library to this Cecilia's library.
#[utoipa::path(post, path = "/api/v1/community/music/{id}/add", tag = "community",
    params(("id" = String, Path, description = "Piece id in the shared library")),
    responses((status = 200, body = TrackSummary), (status = 404, body = ApiErrorBody), (status = 503, body = ApiErrorBody)))]
pub async fn add_music(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<TrackSummary>, Response> {
    let cat = s.community.catalogue(false).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let entry = cat
        .music
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("the shared library has no piece called {id:?}")))?;
    let text = s.community.fetch(&entry.path).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let bad = |e: String| error(StatusCode::UNPROCESSABLE_ENTITY, e);
    let mut track: Track = if entry.path.ends_with(".score.json") {
        Score::from_json(&text).and_then(|sc| sc.compile()).map_err(bad)?
    } else {
        let t: Track = serde_json::from_str(&text).map_err(|e| bad(e.to_string()))?;
        t.validate().map_err(bad)?;
        t
    };
    // Already added? Return what's there rather than making a copy.
    let lib_id = format!("lib-{id}");
    if let Some(existing) = s.library.list().into_iter().find(|t| t.id == lib_id) {
        return Ok(Json(existing));
    }
    track.id = lib_id;
    track.source = TrackSource::Imported;
    track.organ_name = "Cecilia Library".into();
    track.created = unix_ms();
    track.modified = track.created;
    let summary = s.library.save(&track).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = s.app_events.send(AppEvent::Library { saved: None });
    Ok(Json(summary))
}

/// A shared organ setup in full, to look at before adding it.
#[utoipa::path(get, path = "/api/v1/community/organs/{id}", tag = "community",
    params(("id" = String, Path, description = "Organ id in the shared library")),
    responses((status = 200, body = OrganDefinition), (status = 404, body = ApiErrorBody), (status = 503, body = ApiErrorBody)))]
pub async fn get_organ(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<OrganDefinition>, Response> {
    let cat = s.community.catalogue(false).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let entry = cat
        .organs
        .iter()
        .find(|o| o.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("the shared library has no organ called {id:?}")))?;
    let text = s.community.fetch(&entry.path).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    OrganDefinition::from_json(&text).map(Json).map_err(|e| error(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))
}

/// Add an organ setup from the shared library (as one you can edit).
#[utoipa::path(post, path = "/api/v1/community/organs/{id}/add", tag = "community",
    params(("id" = String, Path, description = "Organ id in the shared library")),
    responses((status = 200, body = OrganSummary), (status = 404, body = ApiErrorBody), (status = 503, body = ApiErrorBody)))]
pub async fn add_organ(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<OrganSummary>, Response> {
    let cat = s.community.catalogue(false).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let entry = cat
        .organs
        .iter()
        .find(|o| o.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("the shared library has no organ called {id:?}")))?;
    let text = s.community.fetch(&entry.path).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let mut organ = OrganDefinition::from_json(&text).map_err(|e| error(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    organ.id = s.organs.unique_id(&format!("{} {}", entry.meta.maker, entry.meta.model));
    organ.name = format!("{} {}", entry.meta.maker, entry.meta.model);
    let mut notes = organ.connection.clone().unwrap_or_default();
    for n in &entry.meta.notes {
        if !notes.notes.contains(n) {
            notes.notes.push(n.clone());
        }
    }
    organ.connection = Some(notes);
    s.organs.save(&organ).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(organ.summary(false)))
}
