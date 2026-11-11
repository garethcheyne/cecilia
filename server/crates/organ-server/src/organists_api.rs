//! The people who play here: who's playing now (recordings are credited to
//! them) and whether each may be named when a recording is shared.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};
use crate::settings::Organist;

pub fn router() -> Router<AppState> {
    Router::new().route("/organists", get(get_organists).put(put_organists))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganistsBody {
    pub organists: Vec<Organist>,
    /// Who's playing now (an id from the list), if anyone.
    pub current: Option<String>,
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

/// The organists, and who's playing now.
#[utoipa::path(get, path = "/api/v1/organists", tag = "organists", responses((status = 200, body = OrganistsBody)))]
pub async fn get_organists(State(s): State<AppState>) -> Json<OrganistsBody> {
    let st = s.settings.get();
    Json(OrganistsBody { organists: st.organists, current: st.current_organist })
}

/// Replace the list of organists (new ones may have an empty id).
#[utoipa::path(put, path = "/api/v1/organists", tag = "organists", request_body = OrganistsBody,
    responses((status = 200, body = OrganistsBody), (status = 400, body = ApiErrorBody)))]
pub async fn put_organists(
    State(s): State<AppState>,
    Json(mut b): Json<OrganistsBody>,
) -> Result<Json<OrganistsBody>, Response> {
    if b.organists.len() > 100 {
        return Err(error(StatusCode::BAD_REQUEST, "That's more organists than Cecilia keeps."));
    }
    let mut seen = std::collections::HashSet::new();
    for o in &mut b.organists {
        o.name = o.name.trim().chars().take(80).collect();
        o.credit_as = o.credit_as.trim().chars().take(80).collect();
        if o.name.is_empty() {
            return Err(error(StatusCode::BAD_REQUEST, "Every organist needs a name."));
        }
        if o.id.trim().is_empty() {
            o.id = uuid_like();
        }
        if !seen.insert(o.id.clone()) {
            return Err(error(StatusCode::BAD_REQUEST, "Two organists have the same id."));
        }
    }
    if let Some(c) = &b.current {
        if !b.organists.iter().any(|o| &o.id == c) {
            b.current = None;
        }
    }
    let saved = s
        .settings
        .update(|st| {
            st.organists = b.organists.clone();
            st.current_organist = b.current.clone();
        })
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(OrganistsBody { organists: saved.organists, current: saved.current_organist }))
}

/// A random id (128 bits, hex).
fn uuid_like() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("random");
    b.iter().map(|x| format!("{x:02x}")).collect()
}
