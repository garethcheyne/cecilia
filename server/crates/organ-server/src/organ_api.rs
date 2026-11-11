//! Organ definitions: copy, edit and delete user organs.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use axum::{Json, Router};
use organ_core::organ::OrganDefinition;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/organs/copy", post(copy_organ))
        .route("/organs/{id}/save", put(save_organ))
        .route("/organs/{id}/delete", post(delete_organ))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyOrgan {
    /// Organ to start from.
    pub from: String,
    pub name: String,
}

/// Make a new organ as a copy of another (the way to start a definition for
/// your own instrument, e.g. from the Makin template).
#[utoipa::path(post, path = "/api/v1/organs/copy", tag = "organs", request_body = CopyOrgan,
    responses((status = 200, body = OrganDefinition), (status = 404, body = ApiErrorBody)))]
pub async fn copy_organ(State(s): State<AppState>, Json(req): Json<CopyOrgan>) -> Result<Json<OrganDefinition>, Response> {
    let mut organ = s
        .organs
        .get(&req.from)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, format!("no organ called {:?}", req.from)))?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "an organ needs a name"));
    }
    organ.name = name.to_string();
    organ.id = s.organs.unique_id(name);
    s.organs.save(&organ).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(organ))
}

/// Save changes to a user organ. If it's the organ in use, it's reloaded
/// everywhere straight away.
#[utoipa::path(put, path = "/api/v1/organs/{id}/save", tag = "organs", request_body = OrganDefinition,
    params(("id" = String, Path, description = "Organ id")),
    responses((status = 200, body = OrganDefinition), (status = 400, body = ApiErrorBody), (status = 409, body = ApiErrorBody)))]
pub async fn save_organ(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(organ): Json<OrganDefinition>,
) -> Result<Json<OrganDefinition>, Response> {
    if organ.id != id {
        return Err(error(StatusCode::BAD_REQUEST, "the organ id doesn't match the address"));
    }
    if s.organs.is_builtin(&id) {
        return Err(error(StatusCode::CONFLICT, "Built-in organs can't be changed. Make a copy and edit that."));
    }
    s.organs.save(&organ).map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    if s.live.organ().id == id {
        s.set_organ(organ.clone()).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(Json(organ))
}

/// Delete a user organ. If it was in use, the demo organ takes over.
#[utoipa::path(post, path = "/api/v1/organs/{id}/delete", tag = "organs",
    params(("id" = String, Path, description = "Organ id")),
    responses((status = 204), (status = 404, body = ApiErrorBody), (status = 409, body = ApiErrorBody)))]
pub async fn delete_organ(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    if s.organs.is_builtin(&id) {
        return error(StatusCode::CONFLICT, "Built-in organs can't be deleted.");
    }
    match s.organs.delete(&id) {
        Ok(true) => {
            if s.live.organ().id == id {
                if let Some(demo) = s.organs.get("demo") {
                    let _ = s.set_organ(demo);
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, format!("no organ called {id:?}")),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
