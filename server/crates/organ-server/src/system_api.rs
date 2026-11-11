//! Settings, backup and restore, the optional password, and scanning
//! printed music (OMR).

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path as FsPath;
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};

use crate::api::{ApiErrorBody, AppState};
use crate::services::{player_settings, recorder_config, AppEvent};
use crate::settings::{PlaybackSettings, RecordingSettings};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/export/backup.zip", get(backup))
        .route("/import/backup", post(restore))
        .route("/auth", get(auth_status))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/password", put(set_password))
        .route("/omr", post(omr))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SettingsBody {
    pub recording: RecordingSettings,
    pub playback: PlaybackSettings,
}

/// Recording and playback settings.
#[utoipa::path(get, path = "/api/v1/settings", tag = "system", responses((status = 200, body = SettingsBody)))]
pub async fn get_settings(State(s): State<AppState>) -> Json<SettingsBody> {
    let st = s.settings.get();
    Json(SettingsBody { recording: st.recording, playback: st.playback })
}

/// Change recording and playback settings (they apply straight away).
#[utoipa::path(put, path = "/api/v1/settings", tag = "system", request_body = SettingsBody,
    responses((status = 200, body = SettingsBody), (status = 400, body = ApiErrorBody)))]
pub async fn put_settings(State(s): State<AppState>, Json(mut b): Json<SettingsBody>) -> Result<Json<SettingsBody>, Response> {
    // Keep values in sensible ranges rather than refusing them.
    b.recording.silence_ms = b.recording.silence_ms.clamp(2000, 60_000);
    b.recording.margin_start_ms = b.recording.margin_start_ms.min(15_000);
    b.recording.margin_end_ms = b.recording.margin_end_ms.min(15_000);
    b.playback.fade_ms = b.playback.fade_ms.clamp(1000, 30_000);
    b.playback.settle_ms = b.playback.settle_ms.min(3000);
    s.settings
        .update(|st| {
            st.recording = b.recording;
            st.playback = b.playback;
        })
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    s.recorder.set_config(recorder_config(&b.recording));
    s.player.set_settings(player_settings(&b.playback));
    Ok(Json(b))
}

// ---------------------------------------------------------------------------
// Backup and restore
// ---------------------------------------------------------------------------

const BACKUP_DIRS: [&str; 3] = ["library", "organs", "playlists"];

/// Everything that matters (library, organs, playlists, settings) in one zip.
#[utoipa::path(get, path = "/api/v1/export/backup.zip", tag = "system",
    responses((status = 200, content_type = "application/zip")))]
pub async fn backup(State(s): State<AppState>) -> Response {
    let dir = s.data_dir.clone();
    let bytes = tokio::task::spawn_blocking(move || make_backup(&dir)).await;
    match bytes {
        Ok(Ok(b)) => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
                (header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"cecilia-backup.zip\"")),
            ],
            b,
        )
            .into_response(),
        Ok(Err(e)) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

fn make_backup(dir: &FsPath) -> anyhow::Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let settings = dir.join("settings.json");
        if settings.exists() {
            zip.start_file("settings.json", opts)?;
            zip.write_all(&std::fs::read(settings)?)?;
        }
        for sub in BACKUP_DIRS {
            let Ok(entries) = std::fs::read_dir(dir.join(sub)) else { continue };
            for path in entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")) {
                let name = format!("{sub}/{}", path.file_name().unwrap().to_string_lossy());
                zip.start_file(name, opts)?;
                zip.write_all(&std::fs::read(&path)?)?;
            }
        }
        zip.finish()?;
    }
    Ok(buf.into_inner())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreResult {
    pub tracks: usize,
    pub organs: usize,
    pub playlists: usize,
}

/// Restore a backup made with `export/backup.zip`. Items with the same name
/// are replaced; everything else is kept. This machine's MIDI ports and
/// password stay as they are.
#[utoipa::path(post, path = "/api/v1/import/backup", tag = "system",
    request_body(content = Vec<u8>, content_type = "application/zip"),
    responses((status = 200, body = RestoreResult), (status = 400, body = ApiErrorBody)))]
pub async fn restore(State(s): State<AppState>, body: Bytes) -> Result<Json<RestoreResult>, Response> {
    let dir = s.data_dir.clone();
    let result = tokio::task::spawn_blocking(move || apply_backup(&dir, &body))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;
    s.library.reload();
    s.organs.reload();
    s.playlists.reload();
    let _ = s.app_events.send(AppEvent::Library { saved: None });
    Ok(Json(result))
}

fn apply_backup(dir: &FsPath, bytes: &[u8]) -> anyhow::Result<RestoreResult> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| anyhow::anyhow!("that isn't a Cecilia backup (.zip)"))?;
    // Read and check everything before writing anything.
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut counts = RestoreResult { tracks: 0, organs: 0, playlists: 0 };
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f.name().replace('\\', "/");
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        let (sub, file) = match name.split_once('/') {
            Some((sub, file)) if BACKUP_DIRS.contains(&sub) && !file.contains('/') && !file.starts_with('.') => (sub, file),
            None if name == "settings.json" => ("", name.as_str()),
            _ => continue, // ignore anything unexpected
        };
        let text = std::str::from_utf8(&data)?;
        match sub {
            "library" => {
                let t: organ_core::track::Track = serde_json::from_str(text)?;
                t.validate().map_err(anyhow::Error::msg)?;
                counts.tracks += 1;
            }
            "organs" => {
                organ_core::organ::OrganDefinition::from_json(text)?;
                counts.organs += 1;
            }
            "playlists" => {
                let p: organ_core::playlist::Playlist = serde_json::from_str(text)?;
                p.validate().map_err(anyhow::Error::msg)?;
                counts.playlists += 1;
            }
            _ => {}
        }
        let path = if sub.is_empty() { file.to_string() } else { format!("{sub}/{file}") };
        files.push((path, data));
    }
    if counts.tracks + counts.organs + counts.playlists == 0 {
        anyhow::bail!("the backup is empty");
    }
    for (path, data) in files {
        if path == "settings.json" {
            // Merge: keep this machine's ports and password.
            let mut incoming: crate::settings::Settings = serde_json::from_slice(&data)?;
            let current: crate::settings::Settings = std::fs::read(dir.join("settings.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            incoming.midi = current.midi;
            incoming.password = current.password;
            incoming.samples_seen = current.samples_seen;
            std::fs::write(dir.join("settings.json"), serde_json::to_vec_pretty(&incoming)?)?;
            continue;
        }
        let target = dir.join(&path);
        std::fs::create_dir_all(target.parent().unwrap())?;
        let tmp = target.with_extension("json.tmp");
        std::fs::write(&tmp, &data)?;
        std::fs::rename(&tmp, &target)?;
    }
    Ok(counts)
}

// ---------------------------------------------------------------------------
// Password
// ---------------------------------------------------------------------------

/// Signed-in sessions (forgotten on restart, which is fine for a home box).
#[derive(Clone, Default)]
pub struct Sessions(Arc<Mutex<HashSet<String>>>);

const COOKIE: &str = "cecilia_session";

fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).expect("system randomness");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hash(salt: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(password.as_bytes());
    // Stretch a little so guessing is slow.
    let mut out = h.finalize();
    for _ in 0..50_000 {
        out = Sha256::digest(out);
    }
    out.iter().map(|x| format!("{x:02x}")).collect()
}

fn check(stored: &str, password: &str) -> bool {
    let Some((salt, want)) = stored.split_once('$') else { return false };
    let got = hash(salt, password);
    // Constant-time compare.
    got.len() == want.len() && got.bytes().zip(want.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn session_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_string())
}

fn signed_in(s: &AppState, headers: &HeaderMap) -> bool {
    s.settings.get().password.is_none()
        || session_of(headers).is_some_and(|t| s.sessions.0.lock().unwrap().contains(&t))
}

/// Refuse API calls without a session when a password is set. Health, the
/// sign-in endpoints and the static UI stay open.
pub async fn require_session(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let open = !path.starts_with("/api/") || path == "/api/v1/health" || path.starts_with("/api/v1/auth");
    if open || signed_in(&s, req.headers()) {
        next.run(req).await
    } else {
        error(StatusCode::UNAUTHORIZED, "Sign in to use Cecilia.")
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStatus {
    /// A password is set.
    pub required: bool,
    pub signed_in: bool,
}

/// Whether a password is needed, and whether this browser is signed in.
#[utoipa::path(get, path = "/api/v1/auth", tag = "auth", responses((status = 200, body = AuthStatus)))]
pub async fn auth_status(State(s): State<AppState>, headers: HeaderMap) -> Json<AuthStatus> {
    Json(AuthStatus { required: s.settings.get().password.is_some(), signed_in: signed_in(&s, &headers) })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Login {
    pub password: String,
}

fn session_cookie(token: &str, max_age: i64) -> HeaderValue {
    HeaderValue::from_str(&format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}")).unwrap()
}

/// Sign in.
#[utoipa::path(post, path = "/api/v1/auth/login", tag = "auth", request_body = Login,
    responses((status = 204), (status = 401, body = ApiErrorBody)))]
pub async fn login(State(s): State<AppState>, Json(req): Json<Login>) -> Response {
    let Some(stored) = s.settings.get().password else { return StatusCode::NO_CONTENT.into_response() };
    let ok = tokio::task::spawn_blocking(move || check(&stored, &req.password)).await.unwrap_or(false);
    if !ok {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        return error(StatusCode::UNAUTHORIZED, "That password isn't right.");
    }
    let token = random_hex(24);
    s.sessions.0.lock().unwrap().insert(token.clone());
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, session_cookie(&token, 60 * 60 * 24 * 365))]).into_response()
}

/// Sign out.
#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth", responses((status = 204)))]
pub async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(t) = session_of(&headers) {
        s.sessions.0.lock().unwrap().remove(&t);
    }
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, session_cookie("", 0))]).into_response()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewPassword {
    /// The new password, or null to remove the password.
    pub password: Option<String>,
}

/// Set, change or remove the password (needs a session if one is set).
/// Other signed-in browsers are signed out.
#[utoipa::path(put, path = "/api/v1/auth/password", tag = "auth", request_body = NewPassword,
    responses((status = 204), (status = 400, body = ApiErrorBody), (status = 401, body = ApiErrorBody)))]
pub async fn set_password(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<NewPassword>) -> Response {
    if !signed_in(&s, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Sign in first.");
    }
    let stored = match req.password {
        Some(p) if p.chars().count() < 6 => return error(StatusCode::BAD_REQUEST, "Use at least 6 characters."),
        Some(p) => {
            let salt = random_hex(16);
            let h = tokio::task::spawn_blocking({
                let salt = salt.clone();
                move || hash(&salt, &p)
            })
            .await
            .unwrap_or_default();
            Some(format!("{salt}${h}"))
        }
        None => None,
    };
    let setting = stored.is_some();
    if let Err(e) = s.settings.update(|st| st.password = stored) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    let mut sessions = s.sessions.0.lock().unwrap();
    sessions.clear();
    if setting {
        // Keep this browser signed in.
        let token = random_hex(24);
        sessions.insert(token.clone());
        return (StatusCode::NO_CONTENT, [(header::SET_COOKIE, session_cookie(&token, 60 * 60 * 24 * 365))]).into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

// ---------------------------------------------------------------------------
// Scanned music
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, IntoParams)]
pub struct OmrQuery {
    /// The scan's file name (PNG, JPEG, TIFF or PDF).
    pub name: String,
}

/// Read printed music from a scan or photo with the OMR engine and return
/// MusicXML, ready for the usual import.
#[utoipa::path(post, path = "/api/v1/omr", tag = "files", params(OmrQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, content_type = "application/vnd.recordare.musicxml"), (status = 422, body = ApiErrorBody), (status = 501, body = ApiErrorBody)))]
pub async fn omr(State(s): State<AppState>, Query(q): Query<OmrQuery>, body: Bytes) -> Response {
    let Some(command) = s.omr_command.clone() else {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "Reading scanned music needs Audiveris (free) installed on the Cecilia computer. See docs/OMR.md.",
        );
    };
    let ext = q.name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    if !["png", "jpg", "jpeg", "tif", "tiff", "pdf"].contains(&ext.as_str()) {
        return error(StatusCode::BAD_REQUEST, "Use a PNG, JPEG, TIFF or PDF of the music.");
    }
    let result = tokio::task::spawn_blocking(move || run_omr(&command, &ext, &body)).await;
    match result {
        Ok(Ok(xml)) => (
            [(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.recordare.musicxml"))],
            Body::from(xml),
        )
            .into_response(),
        Ok(Err(e)) => error(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// Run the engine: `command` is a template whose `{input}` and `{output}`
/// are replaced with the scan's path and an empty folder for the result.
fn run_omr(command: &str, ext: &str, scan: &[u8]) -> anyhow::Result<Vec<u8>> {
    let work = std::env::temp_dir().join(format!("cecilia-omr-{}", random_hex(8)));
    let out = work.join("out");
    std::fs::create_dir_all(&out)?;
    let input = work.join(format!("scan.{ext}"));
    std::fs::write(&input, scan)?;
    let result = (|| {
        let parts: Vec<String> = command
            .split_whitespace()
            .map(|p| p.replace("{input}", &input.to_string_lossy()).replace("{output}", &out.to_string_lossy()))
            .collect();
        let (program, args) = parts.split_first().ok_or_else(|| anyhow::anyhow!("no OMR command set"))?;
        let status = std::process::Command::new(program).args(args).output()?;
        // Whatever it wrote: the first MusicXML file, compressed or not.
        let found = walk(&out).into_iter().find(|p| {
            let n = p.to_string_lossy().to_lowercase();
            n.ends_with(".mxl") || n.ends_with(".musicxml") || n.ends_with(".xml")
        });
        match found {
            Some(p) => Ok(std::fs::read(p)?),
            None => anyhow::bail!(
                "No music could be read from that image. Try a straighter, sharper scan of printed music.{}",
                if status.status.success() { String::new() } else { format!(" ({})", String::from_utf8_lossy(&status.stderr).lines().last().unwrap_or("")) }
            ),
        }
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn walk(dir: &FsPath) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for p in entries.flatten().map(|e| e.path()) {
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}
