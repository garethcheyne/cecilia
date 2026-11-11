//! Signing in to the Cecilia Community and sharing with other churches.
//!
//! Cecilia talks to the community service on the browser's behalf, so the
//! sign-in token stays on this machine.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};
use crate::settings::CommunityAccount;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/community/account", get(account))
        .route("/community/account/start", post(start))
        .route("/community/account/verify", post(verify))
        .route("/community/account/sign-out", post(sign_out))
        .route("/community/account/profile", put(profile))
        .route("/community/shared", get(shared))
        .route("/community/share/organ/{id}", post(share_organ))
        .route("/community/share/music/{id}", post(share_music))
}

fn error((code, msg): (StatusCode, String)) -> Response {
    (code, Json(ApiErrorBody { error: msg })).into_response()
}

fn token(s: &AppState) -> Result<String, Response> {
    s.settings
        .get()
        .community
        .map(|c| c.token)
        .ok_or_else(|| error((StatusCode::UNAUTHORIZED, "Sign in to the Cecilia Community first (Settings).".into())))
}

/// Forget the sign-in if the service no longer accepts it.
fn forget_if_refused(s: &AppState, e: &(StatusCode, String)) {
    if e.0 == StatusCode::UNAUTHORIZED {
        let _ = s.settings.update(|st| st.community = None);
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommunityAccountStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub name: Option<String>,
    pub church: Option<String>,
    /// member, trusted or moderator.
    pub role: Option<String>,
}

/// Whether this Cecilia is signed in to the community, and as whom.
#[utoipa::path(get, path = "/api/v1/community/account", tag = "community",
    responses((status = 200, body = CommunityAccountStatus)))]
pub async fn account(State(s): State<AppState>) -> Result<Json<CommunityAccountStatus>, Response> {
    let Some(acc) = s.settings.get().community else {
        return Ok(Json(CommunityAccountStatus { signed_in: false, email: None, name: None, church: None, role: None }));
    };
    match s.community.service(Method::GET, "/me", Some(&acc.token), None).await {
        Ok(me) => {
            let u = &me["user"];
            let text = |k: &str| u[k].as_str().map(str::to_string);
            Ok(Json(CommunityAccountStatus {
                signed_in: true,
                email: text("email"),
                name: text("name"),
                church: text("church"),
                role: text("role"),
            }))
        }
        Err(e) => {
            forget_if_refused(&s, &e);
            if e.0 == StatusCode::UNAUTHORIZED {
                Ok(Json(CommunityAccountStatus { signed_in: false, email: None, name: None, church: None, role: None }))
            } else {
                // Offline: still signed in, details unknown.
                Ok(Json(CommunityAccountStatus { signed_in: true, email: Some(acc.email), name: None, church: None, role: None }))
            }
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignInStart {
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SignInStarted {
    /// How long the emailed code lasts.
    pub minutes: u32,
    /// Only from a development service, which shows codes instead of emailing them.
    pub code: Option<String>,
}

/// Email a sign-in code.
#[utoipa::path(post, path = "/api/v1/community/account/start", tag = "community", request_body = SignInStart,
    responses((status = 200, body = SignInStarted), (status = 400, body = ApiErrorBody)))]
pub async fn start(State(s): State<AppState>, Json(b): Json<SignInStart>) -> Result<Json<SignInStarted>, Response> {
    let r = s.community.service(Method::POST, "/auth/start", None, Some(json!({ "email": b.email }))).await.map_err(error)?;
    Ok(Json(SignInStarted {
        minutes: r["minutes"].as_u64().unwrap_or(15) as u32,
        code: r["code"].as_str().map(str::to_string),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignInVerify {
    pub email: String,
    pub code: String,
}

/// Sign in with the emailed code.
#[utoipa::path(post, path = "/api/v1/community/account/verify", tag = "community", request_body = SignInVerify,
    responses((status = 200, body = CommunityAccountStatus), (status = 401, body = ApiErrorBody)))]
pub async fn verify(State(s): State<AppState>, Json(b): Json<SignInVerify>) -> Result<Json<CommunityAccountStatus>, Response> {
    let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "this computer".into());
    let r = s
        .community
        .service(
            Method::POST,
            "/auth/verify",
            None,
            Some(json!({ "email": b.email, "code": b.code, "label": format!("Cecilia on {host}") })),
        )
        .await
        .map_err(error)?;
    let token = r["token"].as_str().unwrap_or_default().to_string();
    let email = r["user"]["email"].as_str().unwrap_or(&b.email).to_string();
    s.settings
        .update(|st| st.community = Some(CommunityAccount { token, email }))
        .map_err(|e| error((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())))?;
    account(State(s)).await
}

/// Sign out of the community on this Cecilia.
#[utoipa::path(post, path = "/api/v1/community/account/sign-out", tag = "community", responses((status = 204)))]
pub async fn sign_out(State(s): State<AppState>) -> StatusCode {
    if let Some(acc) = s.settings.get().community {
        let _ = s.community.service(Method::POST, "/auth/sign-out", Some(&acc.token), None).await;
    }
    let _ = s.settings.update(|st| st.community = None);
    StatusCode::NO_CONTENT
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CommunityProfile {
    pub name: String,
    pub church: String,
}

/// Your name and church, as other churches see them.
#[utoipa::path(put, path = "/api/v1/community/account/profile", tag = "community", request_body = CommunityProfile,
    responses((status = 200, body = CommunityAccountStatus), (status = 401, body = ApiErrorBody)))]
pub async fn profile(State(s): State<AppState>, Json(b): Json<CommunityProfile>) -> Result<Json<CommunityAccountStatus>, Response> {
    let t = token(&s)?;
    s.community
        .service(Method::PUT, "/me", Some(&t), Some(json!({ "name": b.name, "church": b.church })))
        .await
        .map_err(|e| {
            forget_if_refused(&s, &e);
            error(e)
        })?;
    account(State(s)).await
}

/// What this account has shared, and where each has got to.
#[utoipa::path(get, path = "/api/v1/community/shared", tag = "community",
    responses((status = 200, body = Value), (status = 401, body = ApiErrorBody)))]
pub async fn shared(State(s): State<AppState>) -> Result<Json<Value>, Response> {
    let t = token(&s)?;
    let r = s.community.service(Method::GET, "/submissions", Some(&t), None).await.map_err(|e| {
        forget_if_refused(&s, &e);
        error(e)
    })?;
    Ok(Json(r))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ShareOrgan {
    pub maker: String,
    pub model: String,
    #[serde(default)]
    pub series: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub years: String,
    /// Things to do on the organ first ("set MIDI to mode B").
    #[serde(default)]
    pub notes: Vec<String>,
    /// A note to the moderator.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareResult {
    pub id: String,
    /// pending (waiting for review) or approved (published).
    pub status: String,
    pub checks: Vec<String>,
}

fn share_result(r: &Value) -> ShareResult {
    ShareResult {
        id: r["id"].as_str().unwrap_or_default().to_string(),
        status: r["status"].as_str().unwrap_or("pending").to_string(),
        checks: r["checks"].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()).unwrap_or_default(),
    }
}

/// Share one of this machine's organ setups with other churches.
#[utoipa::path(post, path = "/api/v1/community/share/organ/{id}", tag = "community", request_body = ShareOrgan,
    params(("id" = String, Path, description = "Organ id")),
    responses((status = 200, body = ShareResult), (status = 401, body = ApiErrorBody), (status = 404, body = ApiErrorBody), (status = 422, body = ApiErrorBody)))]
pub async fn share_organ(State(s): State<AppState>, Path(id): Path<String>, Json(b): Json<ShareOrgan>) -> Result<Json<ShareResult>, Response> {
    let t = token(&s)?;
    let organ = s.organs.get(&id).ok_or_else(|| error((StatusCode::NOT_FOUND, "No such organ.".into())))?;
    let learned = organ.stops.iter().filter(|st| !st.midi.is_empty()).count();
    let confidence = if learned == organ.stops.len() && !organ.stops.is_empty() { "learned" } else { "template" };
    let meta = json!({
        "maker": b.maker, "model": b.model, "series": b.series, "variant": b.variant, "years": b.years,
        "confidence": confidence, "notes": b.notes,
    });
    let r = s
        .community
        .service(Method::POST, "/submissions/organ", Some(&t), Some(json!({ "organ": organ, "meta": meta, "note": b.note })))
        .await
        .map_err(|e| {
            forget_if_refused(&s, &e);
            error(e)
        })?;
    s.community.forget();
    Ok(Json(share_result(&r)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ShareMusic {
    /// public-domain, CC0-1.0, CC-BY-4.0 or CC-BY-SA-4.0: the music itself
    /// must be free to share.
    pub licence: String,
    #[serde(default)]
    pub note: String,
}

/// Share a recording with other churches. The organist is named only if
/// their profile says they've agreed.
#[utoipa::path(post, path = "/api/v1/community/share/music/{id}", tag = "community", request_body = ShareMusic,
    params(("id" = String, Path, description = "Piece id")),
    responses((status = 200, body = ShareResult), (status = 401, body = ApiErrorBody), (status = 404, body = ApiErrorBody), (status = 422, body = ApiErrorBody)))]
pub async fn share_music(State(s): State<AppState>, Path(id): Path<String>, Json(b): Json<ShareMusic>) -> Result<Json<ShareResult>, Response> {
    let t = token(&s)?;
    let track = s.library.get(&id).ok_or_else(|| error((StatusCode::NOT_FOUND, "No such piece.".into())))?;
    let st = s.settings.get();
    let credit = st
        .organists
        .iter()
        .find(|o| !track.info.performer.is_empty() && o.name.eq_ignore_ascii_case(track.info.performer.trim()))
        .map(|o| o.public_credit())
        .unwrap_or_else(|| "A Cecilia organist".into());
    let r = s
        .community
        .service(
            Method::POST,
            "/submissions/music",
            Some(&t),
            Some(json!({ "track": track, "licence": b.licence, "credit": credit, "note": b.note })),
        )
        .await
        .map_err(|e| {
            forget_if_refused(&s, &e);
            error(e)
        })?;
    s.community.forget();
    Ok(Json(share_result(&r)))
}
